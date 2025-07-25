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
mod subsonic;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub music_path: String,
}

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    // Load environment variables from .env file if it exists
    dotenv::dotenv().ok();

    logger::init().unwrap();

    let config = config::Config::from_env();

    let mut opt = ConnectOptions::new(&config.database_url);
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info);
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
            batch_size: 100,
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
        start_api_server(api_db, bind_address).await;
    });

    // Wait for API server (it runs indefinitely)
    // The scan runs in the background and doesn't block the API
    api_handle.await.unwrap();

    Ok(())
}

async fn start_api_server(db: DatabaseConnection, bind_address: String) {
    let config = config::Config::from_env();
    let state = AppState {
        db,
        music_path: config.music_path,
    };

    let app = Router::new()
        .nest("/api/v1", api::create_router(state.clone()))
        .nest("/rest", subsonic::create_router(state))
        .layer(CorsLayer::permissive());

    let listener = TcpListener::bind(&bind_address).await.unwrap();
    info!("API server starting on http://{}", bind_address);
    info!("API endpoints available at:");
    info!("  GET /api/v1/tracks - List tracks with pagination");
    info!("  GET /api/v1/tracks/:id - Get track by ID");
    info!("  GET /api/v1/tracks/search?q=query - Search tracks");
    info!("  GET /api/v1/stats - Get database statistics");
    info!("  GET /api/v1/artists - Get list of artists");
    info!("  GET /api/v1/albums - Get list of albums");
    info!("  GET /api/v1/genres - Get list of genres");
    info!("  POST /api/v1/rescan - Trigger music library rescan");
    info!("");
    info!("🎵 Subsonic API endpoints available at:");
    info!("  GET /rest/ping - Test connectivity");
    info!("  GET /rest/getMusicFolders - Get music folders");
    info!("  GET /rest/getIndexes - Get artist index");
    info!("  GET /rest/getArtists - Get all artists");
    info!("  GET /rest/getArtist?id=... - Get artist details");
    info!("  GET /rest/getAlbum?id=... - Get album details");
    info!("  GET /rest/search3?query=... - Search tracks");
    info!("  GET /rest/stream/:id - Stream audio file");
    info!("");
    info!("📖 API Documentation available at:");
    info!("  http://{}/api/v1/docs - Interactive Swagger UI", bind_address);
    info!("  http://{}/api/v1/openapi.yaml - OpenAPI 3.0 specification", bind_address);

    axum::serve(listener, app).await.unwrap();
}
