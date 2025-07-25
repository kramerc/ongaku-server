use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
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
        let tags = serde_json::from_str(&model.tags)
            .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));

        Self {
            id: model.id,
            path: model.path,
            extension: model.extension,
            title: model.title,
            artist: model.artist,
            album: model.album,
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
        .route("/tracks/search", get(search_tracks))
        .route("/stats", get(get_stats))
        .route("/artists", get(get_artists))
        .route("/albums", get(get_albums))
        .route("/genres", get(get_genres))
        .route("/rescan", post(rescan_library))
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
