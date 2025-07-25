use std::path::Path;
use std::time::Duration;

use axum::{Router, extract::Request, middleware::{self, Next}, response::Response, body::Body};
use log::{debug, info, error};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use migration::{Migrator, MigratorTrait};

mod logger;
mod api;
mod config;
mod scanner;
mod lastfm;
mod subsonic;

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    // Load environment variables from .env file if it exists
    dotenv::dotenv().ok();

    logger::init().unwrap();

    let config = config::Config::from_env();

    let mut opt = ConnectOptions::new(&config.database_url);
    opt.max_connections(150)       // Increased from 100 for better concurrency
        .min_connections(10)       // Increased from 5 to maintain ready connections
        .connect_timeout(Duration::from_secs(10))  // Slightly increased
        .acquire_timeout(Duration::from_secs(10))  // Slightly increased
        .idle_timeout(Duration::from_secs(300))    // Increased from 8 seconds
        .max_lifetime(Duration::from_secs(1800))   // Increased from 8 seconds
        .sqlx_logging(false); // Disable SQL logging to clean up progress bar display
    let db: DatabaseConnection = Database::connect(opt).await?;
    Migrator::up(&db, None).await?;

    // Clone database connections for API server and scanner
    let api_db = db.clone();
    let scan_db = db.clone();
    let bind_address = config.bind_address();
    let music_path_str = config.music_path.clone();

    // Start initial music library scan in background
    let _scan_handle = tokio::spawn(async move {
        info!("Starting initial music library scan...");
        debug!("Path: {:?}", music_path_str);
        debug!("Path exists: {}", Path::new(&music_path_str).exists());

        let scan_config = scanner::ScanConfig {
            music_path: music_path_str,
            show_progress: true,
            batch_size: 100,         // Smaller batches for consistency
            path_batch_size: 2500,   // Balanced query efficiency
            use_optimized_scanning: true,
        };

        match scanner::scan_music_library(&scan_db, scan_config).await {
            Ok(result) => {
                info!("Initial scan completed: {} files scanned, {} tracks processed",
                      result.files_scanned, result.tracks_processed);
            }
            Err(e) => {
                error!("Error during initial scan: {}", e);
            }
        }
    });

    // Start API server (this will run indefinitely)
    let api_handle = tokio::spawn(async move {
        if let Err(e) = start_api_server(api_db, bind_address).await {
            error!("API server failed to start: {}", e);
        }
    });

    // Wait for API server (it runs indefinitely)
    // The scan runs in the background and doesn't block the API
    if let Err(e) = api_handle.await {
        error!("API server task failed: {}", e);
        return Err(DbErr::Custom("API server failed".to_string()));
    }

    Ok(())
}

async fn start_api_server(db: DatabaseConnection, bind_address: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = config::Config::from_env();
    let state = api::AppState {
        db,
        music_path: config.music_path,
    };

    let app = Router::new()
        .nest("/api/v1", api::create_router(state.clone()))
        .nest("/rest", subsonic::create_subsonic_router(state))
        .layer(middleware::from_fn(request_logging_middleware))
        .layer(CorsLayer::permissive());

    let listener = match TcpListener::bind(&bind_address).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind to address {}: {}", bind_address, e);
            return Err(Box::new(e));
        }
    };

    const PUBLIC_ADDRESS: &str = "ongaku-dev.m3r.dev";

    info!("API server starting on https://{}", PUBLIC_ADDRESS);
    info!("API endpoints available at:");
    info!("  GET /api/v1/tracks - List tracks with pagination");
    info!("  GET /api/v1/tracks/:id - Get track by ID");
    info!("  GET /api/v1/tracks/:id/play - Stream audio file");
    info!("  GET /api/v1/tracks/search?q=query - Search tracks");
    info!("  GET /api/v1/stats - Get database statistics");
    info!("  GET /api/v1/artists - Get list of artists");
    info!("  GET /api/v1/albums - Get list of albums");
    info!("  GET /api/v1/genres - Get list of genres");
    info!("  POST /api/v1/rescan - Trigger music library rescan");
    info!("  GET /api/v1/lastfm/auth - Get Last.fm authentication URL");
    info!("  POST /api/v1/lastfm/session - Create Last.fm session");
    info!("  POST /api/v1/tracks/:id/scrobble - Scrobble track to Last.fm");
    info!("  POST /api/v1/tracks/:id/now-playing - Update Last.fm now playing");
    info!("");
    info!("🎵 Subsonic API endpoints available at:");
    info!("  /rest/ping - Test server connectivity");
    info!("  /rest/getMusicFolders - Get music folders");
    info!("  /rest/getIndexes - Get artist index");
    info!("  /rest/getArtists - Get all artists (ID3)");
    info!("  /rest/getArtist?id=<id> - Get artist details");
    info!("  /rest/getAlbum?id=<id> - Get album details");
    info!("  /rest/getMusicDirectory?id=<id> - Get directory contents");
    info!("  /rest/search3?query=<q> - Search artists, albums, songs");
    info!("  /rest/stream?id=<id> - Stream audio file");
    info!("  /rest/getGenres - Get list of genres");
    info!("");
    info!("📖 API Documentation available at:");
    info!("  https://{}/api/v1/docs - Interactive Swagger UI", PUBLIC_ADDRESS);
    info!("  https://{}/api/v1/openapi.yaml - OpenAPI 3.0 specification", PUBLIC_ADDRESS);

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        return Err(Box::new(e));
    }

    Ok(())
}

async fn request_logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let user_agent = request
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    let client_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
        })
        .unwrap_or("unknown");

    let start = std::time::Instant::now();

    info!("Request: {} {} from {} - {}", method, uri, client_ip, user_agent);

    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status();

    // Extract and log response body
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return Response::from_parts(parts, Body::from("Internal Server Error"));
        }
    };

    // Log response with body (truncate if too long, skip binary content)
    let body_preview = if uri.path().contains("/stream") {
        format!("(binary content, {} bytes)", bytes.len())
    } else {
        let body_str = String::from_utf8_lossy(&bytes);
        if body_str.len() > 500 {
            // Find a safe character boundary near 500 characters
            let mut truncate_pos = 500.min(body_str.len());
            while truncate_pos > 0 && !body_str.is_char_boundary(truncate_pos) {
                truncate_pos -= 1;
            }
            format!("{}... (truncated, {} bytes total)", &body_str[..truncate_pos], bytes.len())
        } else {
            body_str.to_string()
        }
    };

    info!("Response: {} {} - {} in {:.2}ms | Body: {}",
          method, uri, status, duration.as_millis(), body_preview);

    // Reconstruct response with the body
    Response::from_parts(parts, Body::from(bytes))
}
