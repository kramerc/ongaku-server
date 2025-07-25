use std::path::Path;
use std::time::Duration;

use axum::Router;
use log::{info, error};
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use entity::prelude::Track;
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

    // Clone database connection for API server
    let api_db = db.clone();
    let bind_address = config.bind_address();

    // Start API server in background
    let api_handle = tokio::spawn(async move {
        start_api_server(api_db, bind_address).await;
    });

    info!("Starting music library scan...");
    let music_path_str = config.music_path.clone();

    println!("Path: {:?}", music_path_str);
    println!("Path exists: {}", Path::new(&music_path_str).exists());

    let scan_config = scanner::ScanConfig {
        music_path: music_path_str,
        show_progress: true,
        batch_size: 100,
    };

    let scan_result = match scanner::scan_music_library(&db, scan_config).await {
        Ok(result) => result,
        Err(e) => {
            error!("Error during scan: {}", e);
            return Err(DbErr::Custom("Scan failed".to_string()));
        }
    };

    println!("{} tracks are in the database", Track::find().count(&db).await?);
    info!("Scan completed: {} files scanned, {} tracks processed",
          scan_result.files_scanned, scan_result.tracks_processed);

    // Wait for API server (it runs indefinitely)
    api_handle.await.unwrap();

    Ok(())
}

async fn start_api_server(db: DatabaseConnection, bind_address: String) {
    let config = config::Config::from_env();
    let state = api::AppState {
        db,
        music_path: config.music_path,
    };

    let app = Router::new()
        .nest("/api/v1", api::create_router(state))
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

    axum::serve(listener, app).await.unwrap();
}
