use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use log::error;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tower_http::services::ServeFile;

use entity::prelude::Track;
use entity::track;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub music_path: String,
}

#[derive(Deserialize)]
pub struct TrackQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub album_artist: Option<String>,
}

#[derive(Serialize)]
pub struct TrackResponse {
    pub id: i32,
    pub path: String,
    pub extension: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub disc_number: Option<i32>,
    pub track_number: Option<i32>,
    pub year: Option<i32>,
    pub genre: String,
    pub album_artist: String,
    pub publisher: String,
    pub catalog_number: String,
    pub duration_seconds: i32,
    pub audio_bitrate: i32,
    pub overall_bitrate: i32,
    pub sample_rate: i32,
    pub bit_depth: i32,
    pub channels: i32,
    pub tags: Value,
    pub created: chrono::DateTime<chrono::Utc>,
    pub modified: chrono::DateTime<chrono::Utc>,
}

impl From<track::Model> for TrackResponse {
    fn from(model: track::Model) -> Self {
        let tags = model.tags;

        Self {
            id: model.id,
            path: model.path,
            extension: model.extension,
            title: model.title,
            artist: model.artist,
            album: model.album,
            disc_number: model.disc_number,
            track_number: model.track_number,
            year: model.year,
            genre: model.genre,
            album_artist: model.album_artist,
            publisher: model.publisher,
            catalog_number: model.catalog_number,
            duration_seconds: model.duration_seconds,
            audio_bitrate: model.audio_bitrate,
            overall_bitrate: model.overall_bitrate,
            sample_rate: model.sample_rate,
            bit_depth: model.bit_depth,
            channels: model.channels,
            tags,
            created: model.created,
            modified: model.modified,
        }
    }
}

#[derive(Serialize)]
pub struct TrackListResponse {
    pub tracks: Vec<TrackResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

#[derive(Serialize)]
pub struct TrackStatsResponse {
    pub total_tracks: u64,
    pub total_duration_seconds: i64,
    pub unique_artists: u64,
    pub unique_albums: u64,
    pub unique_genres: u64,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/tracks", get(get_tracks))
        .route("/tracks/:id", get(get_track_by_id))
        .route("/tracks/:id/play", get(play_track))
        .route("/tracks/search", get(search_tracks))
        .route("/stats", get(get_stats))
        .route("/artists", get(get_artists))
        .route("/albums", get(get_albums))
        .route("/genres", get(get_genres))
        .route("/rescan", post(rescan_library))
        // Documentation routes
        .route_service("/docs", ServeFile::new("api-docs.html"))
        .route_service("/openapi.yaml", ServeFile::new("openapi.yaml"))
        .with_state(state)
}

// GET /tracks - List tracks with pagination and optional filters
async fn get_tracks(
    State(state): State<AppState>,
    Query(params): Query<TrackQuery>,
) -> Result<Json<TrackListResponse>, StatusCode> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20).min(100); // Max 100 per page

    let mut query = Track::find();

    // Apply filters
    let mut condition = Condition::all();
    if let Some(title) = params.title {
        condition = condition.add(track::Column::Title.contains(&title));
    }
    if let Some(artist) = params.artist {
        condition = condition.add(track::Column::Artist.contains(&artist));
    }
    if let Some(album) = params.album {
        condition = condition.add(track::Column::Album.contains(&album));
    }
    if let Some(genre) = params.genre {
        condition = condition.add(track::Column::Genre.contains(&genre));
    }
    if let Some(album_artist) = params.album_artist {
        condition = condition.add(track::Column::AlbumArtist.contains(&album_artist));
    }

    query = query.filter(condition);

    let total = query.clone().count(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_pages = (total + per_page - 1) / per_page;

    let tracks = query
        .order_by_asc(track::Column::Artist)
        .order_by_asc(track::Column::Album)
        .order_by_asc(track::Column::Title)
        .paginate(&state.db, per_page)
        .fetch_page(page - 1)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(TrackResponse::from)
        .collect();

    Ok(Json(TrackListResponse {
        tracks,
        total,
        page,
        per_page,
        total_pages,
    }))
}

// GET /tracks/:id - Get a specific track by ID
async fn get_track_by_id(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<TrackResponse>, StatusCode> {
    let track = Track::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match track {
        Some(track) => Ok(Json(TrackResponse::from(track))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// GET /tracks/:id/play - Stream audio file with range support for web browsers
async fn play_track(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    headers: HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    // Find the track in the database
    let track = Track::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let track = match track {
        Some(track) => track,
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Get the file path
    let file_path = PathBuf::from(&track.path);

    // Check if file exists
    if !file_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get file metadata
    let metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_size = metadata.len();

    // Determine MIME type
    let mime_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Parse Range header if present
    let range_header = headers.get(header::RANGE);

    if let Some(range_value) = range_header {
        // Handle range request
        let range_str = range_value.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;

        if !range_str.starts_with("bytes=") {
            return Err(StatusCode::RANGE_NOT_SATISFIABLE);
        }

        let range_part = &range_str[6..]; // Remove "bytes="
        let (start, end) = parse_range(range_part, file_size)?;

        // Open file and seek to start position
        let mut file = File::open(&file_path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Read the requested range
        let content_length = end - start + 1;
        let mut buffer = vec![0u8; content_length as usize];
        file.read_exact(&mut buffer)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Build response with 206 Partial Content
        let response = Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::CONTENT_LENGTH, content_length.to_string())
            .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size))
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range, Content-Range, Content-Length")
            .body(Body::from(buffer))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(response)
    } else {
        // Return full file
        let file_content = tokio::fs::read(&file_path)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::CONTENT_LENGTH, file_size.to_string())
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, "public, max-age=3600")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range, Content-Range, Content-Length")
            .body(Body::from(file_content))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(response)
    }
}

// Helper function to parse Range header
fn parse_range(range_str: &str, file_size: u64) -> Result<(u64, u64), StatusCode> {
    if let Some(dash_pos) = range_str.find('-') {
        let start_str = &range_str[..dash_pos];
        let end_str = &range_str[dash_pos + 1..];

        let start = if start_str.is_empty() {
            // Suffix range like "-500" (last 500 bytes)
            let suffix_length: u64 = end_str.parse().map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
            file_size.saturating_sub(suffix_length)
        } else {
            start_str.parse().map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?
        };

        let end = if end_str.is_empty() {
            // Range like "500-" (from 500 to end)
            file_size - 1
        } else {
            let parsed_end: u64 = end_str.parse().map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;
            std::cmp::min(parsed_end, file_size - 1)
        };

        if start <= end && end < file_size {
            Ok((start, end))
        } else {
            Err(StatusCode::RANGE_NOT_SATISFIABLE)
        }
    } else {
        Err(StatusCode::RANGE_NOT_SATISFIABLE)
    }
}

// GET /tracks/search - Search tracks
async fn search_tracks(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<TrackListResponse>, StatusCode> {
    let search_term = params.get("q").cloned().unwrap_or_default();
    let page = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
    let per_page = params.get("per_page").and_then(|p| p.parse().ok()).unwrap_or(20).min(100);

    if search_term.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let condition = Condition::any()
        .add(track::Column::Title.contains(&search_term))
        .add(track::Column::Artist.contains(&search_term))
        .add(track::Column::Album.contains(&search_term))
        .add(track::Column::Genre.contains(&search_term))
        .add(track::Column::AlbumArtist.contains(&search_term));

    let query = Track::find().filter(condition);

    let total = query.clone().count(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_pages = (total + per_page - 1) / per_page;

    let tracks = query
        .order_by_asc(track::Column::Artist)
        .order_by_asc(track::Column::Album)
        .order_by_asc(track::Column::Title)
        .paginate(&state.db, per_page)
        .fetch_page(page - 1)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .map(TrackResponse::from)
        .collect();

    Ok(Json(TrackListResponse {
        tracks,
        total,
        page,
        per_page,
        total_pages,
    }))
}

// GET /stats - Get database statistics
async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<TrackStatsResponse>, StatusCode> {
    let total_tracks = Track::find()
        .count(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_duration: Option<i64> = Track::find()
        .select_only()
        .column_as(track::Column::DurationSeconds.sum(), "total_duration")
        .into_tuple::<Option<i64>>()
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .flatten();

    let unique_artists = Track::find()
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .count(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let unique_albums = Track::find()
        .select_only()
        .column(track::Column::Album)
        .distinct()
        .count(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let unique_genres = Track::find()
        .select_only()
        .column(track::Column::Genre)
        .distinct()
        .count(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(TrackStatsResponse {
        total_tracks,
        total_duration_seconds: total_duration.unwrap_or(0),
        unique_artists,
        unique_albums,
        unique_genres,
    }))
}

// GET /artists - Get list of unique artists
async fn get_artists(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let artists: Vec<String> = Track::find()
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .filter(track::Column::Artist.ne(""))
        .order_by_asc(track::Column::Artist)
        .into_tuple()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(artists))
}

// GET /albums - Get list of unique albums
async fn get_albums(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let albums: Vec<String> = Track::find()
        .select_only()
        .column(track::Column::Album)
        .distinct()
        .filter(track::Column::Album.ne(""))
        .order_by_asc(track::Column::Album)
        .into_tuple()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(albums))
}

// GET /genres - Get list of unique genres
async fn get_genres(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let genres: Vec<String> = Track::find()
        .select_only()
        .column(track::Column::Genre)
        .distinct()
        .filter(track::Column::Genre.ne(""))
        .order_by_asc(track::Column::Genre)
        .into_tuple()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(genres))
}

#[derive(Serialize)]
pub struct RescanResponse {
    pub message: String,
    pub status: String,
}

// POST /rescan - Trigger a rescan of the music library
async fn rescan_library(
    State(state): State<AppState>,
) -> Result<Json<RescanResponse>, StatusCode> {
    let music_path = state.music_path.clone();
    let db = state.db.clone();

    tokio::spawn(async move {
        let scan_config = crate::scanner::ScanConfig {
            music_path,
            show_progress: true,
            batch_size: 100,
            path_batch_size: 1000,
            use_optimized_scanning: true,
        };

        match crate::scanner::scan_music_library(&db, scan_config).await {
            Ok(_result) => {
                // Scan completion is now logged inside the scanner module
            }
            Err(e) => {
                error!("Error during rescan: {:?}", e);
            }
        }
    });

    Ok(Json(RescanResponse {
        message: "Music library rescan initiated".to_string(),
        status: "success".to_string(),
    }))
}
