use std::path::Path;
use std::time::Duration;

use axum::Router;
use log::{debug, info, error};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use migration::{Migrator, MigratorTrait};

mod logger;
mod api;
mod config;
mod scanner;

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
        .nest("/api/v1", api::create_router(state))
        .layer(CorsLayer::permissive());

    let listener = match TcpListener::bind(&bind_address).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind to address {}: {}", bind_address, e);
            return Err(Box::new(e));
        }
    };

    info!("API server starting on http://{}", bind_address);
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
    info!("");
    info!("📖 API Documentation available at:");
    info!("  http://{}/api/v1/docs - Interactive Swagger UI", bind_address);
    info!("  http://{}/api/v1/openapi.yaml - OpenAPI 3.0 specification", bind_address);

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        return Err(Box::new(e));
    }

    Ok(())
}
