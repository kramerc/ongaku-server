use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use log::error;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use entity::prelude::Track;
use entity::track;

use crate::AppState;

// Subsonic API version
const SUBSONIC_API_VERSION: &str = "1.16.1";

#[derive(Debug, Deserialize)]
pub struct SubsonicAuth {
    pub u: Option<String>,     // username
    pub p: Option<String>,     // password (plain text or hex encoded)
    pub t: Option<String>,     // token (hex encoded)
    pub s: Option<String>,     // salt (random string)
    pub v: Option<String>,     // version
    pub c: Option<String>,     // client identifier
    pub f: Option<String>,     // format (xml or json)
}

#[derive(Debug, Deserialize)]
pub struct PingParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
}

#[derive(Debug, Deserialize)]
pub struct GetMusicFoldersParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
}

#[derive(Debug, Deserialize)]
pub struct GetIndexesParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<String>,
    #[serde(rename = "ifModifiedSince")]
    pub if_modified_since: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GetMusicDirectoryParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetArtistsParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetArtistParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetAlbumParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetSongParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    pub query: Option<String>,
    #[serde(rename = "artistCount")]
    pub artist_count: Option<u32>,
    #[serde(rename = "artistOffset")]
    pub artist_offset: Option<u32>,
    #[serde(rename = "albumCount")]
    pub album_count: Option<u32>,
    #[serde(rename = "albumOffset")]
    pub album_offset: Option<u32>,
    #[serde(rename = "songCount")]
    pub song_count: Option<u32>,
    #[serde(rename = "songOffset")]
    pub song_offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct StreamParams {
    #[serde(flatten)]
    pub auth: SubsonicAuth,
    pub id: String,
    #[serde(rename = "maxBitRate")]
    pub max_bit_rate: Option<u32>,
    pub format: Option<String>,
    #[serde(rename = "timeOffset")]
    pub time_offset: Option<u32>,
    pub size: Option<String>,
    #[serde(rename = "estimateContentLength")]
    pub estimate_content_length: Option<bool>,
    pub converted: Option<bool>,
}

// Subsonic response structures
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponse<T> {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: SubsonicResponseBody<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponseBody<T> {
    pub status: String,
    pub version: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub server_version: String,
    #[serde(flatten)]
    pub data: Option<T>,
    pub error: Option<SubsonicError>,
}

#[derive(Debug, Serialize)]
pub struct SubsonicError {
    pub code: u32,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFolders {
    pub music_folder: Vec<MusicFolder>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFolder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Indexes {
    pub shortcut: Vec<Artist>,
    pub index: Vec<Index>,
    pub last_modified: i64,
    pub ignored_articles: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub name: String,
    pub artist: Vec<Artist>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub artist_image_url: Option<String>,
    pub starred: Option<String>,
    pub user_rating: Option<u32>,
    pub average_rating: Option<f32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistWithAlbumsID3 {
    pub id: String,
    pub name: String,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub album_count: u32,
    pub starred: Option<String>,
    pub album: Vec<AlbumID3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistsID3 {
    pub ignored_articles: String,
    pub index: Vec<IndexID3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexID3 {
    pub name: String,
    pub artist: Vec<ArtistID3>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistID3 {
    pub id: String,
    pub name: String,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub album_count: u32,
    pub starred: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumID3 {
    pub id: String,
    pub name: String,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub cover_art: Option<String>,
    pub song_count: u32,
    pub duration: u32,
    pub play_count: Option<u64>,
    pub created: String,
    pub starred: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumWithSongsID3 {
    pub id: String,
    pub name: String,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub cover_art: Option<String>,
    pub song_count: u32,
    pub duration: u32,
    pub play_count: Option<u64>,
    pub created: String,
    pub starred: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub song: Vec<Child>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub starred: Option<String>,
    pub user_rating: Option<u32>,
    pub average_rating: Option<f32>,
    pub play_count: Option<u64>,
    pub child: Vec<Child>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Child {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: bool,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub size: Option<u64>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    pub trans_coded_content_type: Option<String>,
    pub trans_coded_suffix: Option<String>,
    pub duration: Option<u32>,
    pub bit_rate: Option<u32>,
    pub path: Option<String>,
    pub is_video: Option<bool>,
    pub user_rating: Option<u32>,
    pub average_rating: Option<f32>,
    pub play_count: Option<u64>,
    pub disc_number: Option<i32>,
    pub created: Option<String>,
    pub starred: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub bookmark_position: Option<u64>,
    pub original_width: Option<u32>,
    pub original_height: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult3 {
    pub artist: Vec<ArtistID3>,
    pub album: Vec<AlbumID3>,
    pub song: Vec<Child>,
}

// Response format enum
#[derive(Debug)]
pub enum ResponseFormat {
    Json,
    Xml,
}

impl fmt::Display for ResponseFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResponseFormat::Json => write!(f, "json"),
            ResponseFormat::Xml => write!(f, "xml"),
        }
    }
}

impl ResponseFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "jsonp" => ResponseFormat::Json,
            _ => ResponseFormat::Xml, // Default to XML for compatibility
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            ResponseFormat::Json => "application/json",
            ResponseFormat::Xml => "application/xml",
        }
    }
}

// Helper to create successful response
fn success_response<T: Serialize>(data: T, format: ResponseFormat) -> Result<Response, StatusCode> {
    let response = SubsonicResponse {
        subsonic_response: SubsonicResponseBody {
            status: "ok".to_string(),
            version: SUBSONIC_API_VERSION.to_string(),
            response_type: "ongaku-server".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            data: Some(data),
            error: None,
        },
    };

    match format {
        ResponseFormat::Json => {
            let json = serde_json::to_string(&response)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok((
                [(header::CONTENT_TYPE, "application/json")],
                json,
            ).into_response())
        }
        ResponseFormat::Xml => {
            // For XML, we'd need to implement XML serialization
            // For now, return JSON with XML content type
            let json = serde_json::to_string(&response)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok((
                [(header::CONTENT_TYPE, "application/xml")],
                json,
            ).into_response())
        }
    }
}

// Helper to create error response
fn error_response(code: u32, message: &str, format: ResponseFormat) -> Result<Response, StatusCode> {
    let response = SubsonicResponse {
        subsonic_response: SubsonicResponseBody {
            status: "failed".to_string(),
            version: SUBSONIC_API_VERSION.to_string(),
            response_type: "ongaku-server".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            data: None::<()>,
            error: Some(SubsonicError {
                code,
                message: message.to_string(),
            }),
        },
    };

    match format {
        ResponseFormat::Json => {
            let json = serde_json::to_string(&response)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok((
                StatusCode::OK, // Subsonic always returns 200, errors are in the response body
                [(header::CONTENT_TYPE, "application/json")],
                json,
            ).into_response())
        }
        ResponseFormat::Xml => {
            let json = serde_json::to_string(&response)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok((
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/xml")],
                json,
            ).into_response())
        }
    }
}

// Authentication validation (simplified - in production you'd want proper validation)
fn validate_auth(auth: &SubsonicAuth) -> bool {
    // For demo purposes, accept any non-empty username
    // In production, implement proper authentication with token/salt validation
    auth.u.as_ref().map_or(false, |u| !u.is_empty())
}

fn get_format(auth: &SubsonicAuth) -> ResponseFormat {
    auth.f
        .as_ref()
        .map(|f| ResponseFormat::from_str(f))
        .unwrap_or(ResponseFormat::Xml)
}

// Create Subsonic router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/ping", get(ping))
        .route("/getMusicFolders", get(get_music_folders))
        .route("/getIndexes", get(get_indexes))
        .route("/getMusicDirectory", get(get_music_directory))
        .route("/getArtists", get(get_artists))
        .route("/getArtist", get(get_artist))
        .route("/getAlbum", get(get_album))
        .route("/getSong", get(get_song))
        .route("/search3", get(search3))
        .route("/stream/:id", get(stream))
        .with_state(state)
}

// Endpoint implementations

async fn ping(Query(params): Query<PingParams>) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    success_response(serde_json::json!({}), format)
}

async fn get_music_folders(
    Query(params): Query<GetMusicFoldersParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    let music_folders = MusicFolders {
        music_folder: vec![MusicFolder {
            id: "1".to_string(),
            name: "Music".to_string(),
        }],
    };

    success_response(music_folders, format)
}

async fn get_indexes(
    State(state): State<AppState>,
    Query(params): Query<GetIndexesParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    // Get all unique artists from the database
    let artists = Track::find()
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .filter(track::Column::Artist.ne(""))
        .order_by_asc(track::Column::Artist)
        .all(&state.db)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Group artists by first letter
    let mut indices: HashMap<String, Vec<Artist>> = HashMap::new();

    for track in artists {
        let artist_name = track.artist;
        let first_char = artist_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();

        let artist = Artist {
            id: format!("artist-{}", base64_encode(&artist_name)),
            name: artist_name,
            artist_image_url: None,
            starred: None,
            user_rating: None,
            average_rating: None,
        };

        indices.entry(first_char).or_insert_with(Vec::new).push(artist);
    }

    let mut index_list: Vec<Index> = indices
        .into_iter()
        .map(|(name, artist)| Index { name, artist })
        .collect();

    index_list.sort_by(|a, b| a.name.cmp(&b.name));

    let indexes = Indexes {
        shortcut: vec![], // Could be populated with popular artists
        index: index_list,
        last_modified: chrono::Utc::now().timestamp() * 1000,
        ignored_articles: "The El La Los Las Le Les".to_string(),
    };

    success_response(indexes, format)
}

async fn get_music_directory(
    State(state): State<AppState>,
    Query(params): Query<GetMusicDirectoryParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    // Parse the ID to determine what we're looking for
    if params.id.starts_with("artist-") {
        // Get albums for this artist
        let artist_name = base64_decode(&params.id[7..])
            .ok_or_else(|| StatusCode::BAD_REQUEST)?;

        let albums = Track::find()
            .select_only()
            .columns([track::Column::Album, track::Column::Artist])
            .distinct()
            .filter(track::Column::Artist.eq(&artist_name))
            .filter(track::Column::Album.ne(""))
            .order_by_asc(track::Column::Album)
            .all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let children: Vec<Child> = albums
            .into_iter()
            .map(|track| Child {
                id: format!("album-{}-{}",
                    base64_encode(&track.artist),
                    base64_encode(&track.album)
                ),
                parent: Some(params.id.clone()),
                is_dir: true,
                title: track.album,
                album: None,
                artist: Some(track.artist),
                track: None,
                year: None,
                genre: None,
                cover_art: None,
                size: None,
                content_type: None,
                suffix: None,
                trans_coded_content_type: None,
                trans_coded_suffix: None,
                duration: None,
                bit_rate: None,
                path: None,
                is_video: None,
                user_rating: None,
                average_rating: None,
                play_count: None,
                disc_number: None,
                created: None,
                starred: None,
                album_id: None,
                artist_id: None,
                media_type: None,
                bookmark_position: None,
                original_width: None,
                original_height: None,
            })
            .collect();

        let directory = Directory {
            id: params.id.clone(),
            parent: Some("1".to_string()),
            name: artist_name,
            starred: None,
            user_rating: None,
            average_rating: None,
            play_count: None,
            child: children,
        };

        success_response(directory, format)
    } else if params.id.starts_with("album-") {
        // Get tracks for this album
        let parts: Vec<&str> = params.id[6..].split('-').collect();
        if parts.len() != 2 {
            return Err(StatusCode::BAD_REQUEST);
        }

        let artist_name = base64_decode(parts[0])
            .ok_or_else(|| StatusCode::BAD_REQUEST)?;
        let album_name = base64_decode(parts[1])
            .ok_or_else(|| StatusCode::BAD_REQUEST)?;

        let tracks = Track::find()
            .filter(track::Column::Artist.eq(&artist_name))
            .filter(track::Column::Album.eq(&album_name))
            .order_by_asc(track::Column::TrackNumber)
            .order_by_asc(track::Column::Title)
            .all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let children: Vec<Child> = tracks
            .into_iter()
            .map(|track| Child {
                id: track.id.to_string(),
                parent: Some(params.id.clone()),
                is_dir: false,
                title: track.title,
                album: Some(track.album),
                artist: Some(track.artist),
                track: track.track_number,
                year: track.year,
                genre: Some(track.genre),
                cover_art: None,
                size: None, // Could calculate from file size
                content_type: Some(format!("audio/{}", track.extension)),
                suffix: Some(track.extension),
                trans_coded_content_type: None,
                trans_coded_suffix: None,
                duration: Some(track.duration_seconds as u32),
                bit_rate: Some(track.audio_bitrate as u32),
                path: Some(track.path),
                is_video: Some(false),
                user_rating: None,
                average_rating: None,
                play_count: None,
                disc_number: track.disc_number,
                created: Some(track.created.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
                starred: None,
                album_id: Some(format!("album-{}-{}",
                    base64_encode(&artist_name),
                    base64_encode(&album_name)
                )),
                artist_id: Some(format!("artist-{}", base64_encode(&artist_name))),
                media_type: Some("music".to_string()),
                bookmark_position: None,
                original_width: None,
                original_height: None,
            })
            .collect();

        let directory = Directory {
            id: params.id.clone(),
            parent: Some(format!("artist-{}", base64_encode(&artist_name))),
            name: album_name,
            starred: None,
            user_rating: None,
            average_rating: None,
            play_count: None,
            child: children,
        };

        success_response(directory, format)
    } else {
        error_response(70, "The requested data was not found", format)
    }
}

async fn get_artists(
    State(state): State<AppState>,
    Query(params): Query<GetArtistsParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    // Get all unique artists with album counts
    // Since we can't easily do this complex query with SeaORM, let's do a simpler approach
    let tracks = Track::find()
        .select_only()
        .columns([track::Column::Artist, track::Column::Album])
        .distinct()
        .filter(track::Column::Artist.ne(""))
        .order_by_asc(track::Column::Artist)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Group by artist and count albums
    let mut artist_albums: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for track in tracks {
        artist_albums
            .entry(track.artist)
            .or_insert_with(std::collections::HashSet::new)
            .insert(track.album);
    }

    // Group artists by first letter
    let mut indices: HashMap<String, Vec<ArtistID3>> = HashMap::new();

    for (artist_name, albums) in artist_albums {
        let first_char = artist_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();

        let artist = ArtistID3 {
            id: format!("artist-{}", base64_encode(&artist_name)),
            name: artist_name,
            cover_art: None,
            artist_image_url: None,
            album_count: albums.len() as u32,
            starred: None,
        };

        indices.entry(first_char).or_insert_with(Vec::new).push(artist);
    }

    let mut index_list: Vec<IndexID3> = indices
        .into_iter()
        .map(|(name, artist)| IndexID3 { name, artist })
        .collect();

    index_list.sort_by(|a, b| a.name.cmp(&b.name));

    let artists = ArtistsID3 {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        index: index_list,
    };

    success_response(artists, format)
}

async fn get_artist(
    State(state): State<AppState>,
    Query(params): Query<GetArtistParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    if !params.id.starts_with("artist-") {
        return error_response(70, "The requested data was not found", format);
    }

    let artist_name = base64_decode(&params.id[7..])
        .ok_or_else(|| StatusCode::BAD_REQUEST)?;

    // Get albums for this artist
    let albums_data = Track::find()
        .select_only()
        .columns([
            track::Column::Album,
            track::Column::Year,
            track::Column::Genre,
        ])
        .distinct()
        .filter(track::Column::Artist.eq(&artist_name))
        .filter(track::Column::Album.ne(""))
        .order_by_asc(track::Column::Album)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut albums = Vec::new();
    for album_track in albums_data {
        // Get song count and duration for this album
        let album_stats = Track::find()
            .filter(track::Column::Artist.eq(&artist_name))
            .filter(track::Column::Album.eq(&album_track.album))
            .all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let song_count = album_stats.len() as u32;
        let duration: u32 = album_stats.iter().map(|t| t.duration_seconds as u32).sum();

        let album = AlbumID3 {
            id: format!("album-{}-{}",
                base64_encode(&artist_name),
                base64_encode(&album_track.album)
            ),
            name: album_track.album,
            artist: Some(artist_name.clone()),
            artist_id: Some(params.id.clone()),
            cover_art: None,
            song_count,
            duration,
            play_count: None,
            created: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            starred: None,
            year: album_track.year,
            genre: Some(album_track.genre),
        };
        albums.push(album);
    }

    let artist = ArtistWithAlbumsID3 {
        id: params.id,
        name: artist_name,
        cover_art: None,
        artist_image_url: None,
        album_count: albums.len() as u32,
        starred: None,
        album: albums,
    };

    success_response(artist, format)
}

async fn get_album(
    State(state): State<AppState>,
    Query(params): Query<GetAlbumParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    if !params.id.starts_with("album-") {
        return error_response(70, "The requested data was not found", format);
    }

    let parts: Vec<&str> = params.id[6..].split('-').collect();
    if parts.len() != 2 {
        return error_response(70, "The requested data was not found", format);
    }

    let artist_name = base64_decode(parts[0])
        .ok_or_else(|| StatusCode::BAD_REQUEST)?;
    let album_name = base64_decode(parts[1])
        .ok_or_else(|| StatusCode::BAD_REQUEST)?;

    let tracks = Track::find()
        .filter(track::Column::Artist.eq(&artist_name))
        .filter(track::Column::Album.eq(&album_name))
        .order_by_asc(track::Column::TrackNumber)
        .order_by_asc(track::Column::Title)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if tracks.is_empty() {
        return error_response(70, "The requested data was not found", format);
    }

    let song_count = tracks.len() as u32;
    let duration: u32 = tracks.iter().map(|t| t.duration_seconds as u32).sum();
    let year = tracks.first().and_then(|t| t.year);
    let genre = tracks.first().map(|t| t.genre.clone());

    let songs: Vec<Child> = tracks
        .into_iter()
        .map(|track| Child {
            id: track.id.to_string(),
            parent: Some(params.id.clone()),
            is_dir: false,
            title: track.title,
            album: Some(track.album),
            artist: Some(track.artist),
            track: track.track_number,
            year: track.year,
            genre: Some(track.genre),
            cover_art: None,
            size: None,
            content_type: Some(format!("audio/{}", track.extension)),
            suffix: Some(track.extension),
            trans_coded_content_type: None,
            trans_coded_suffix: None,
            duration: Some(track.duration_seconds as u32),
            bit_rate: Some(track.audio_bitrate as u32),
            path: Some(track.path),
            is_video: Some(false),
            user_rating: None,
            average_rating: None,
            play_count: None,
            disc_number: track.disc_number,
            created: Some(track.created.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            starred: None,
            album_id: Some(params.id.clone()),
            artist_id: Some(format!("artist-{}", base64_encode(&artist_name))),
            media_type: Some("music".to_string()),
            bookmark_position: None,
            original_width: None,
            original_height: None,
        })
        .collect();

    let album = AlbumWithSongsID3 {
        id: params.id,
        name: album_name,
        artist: Some(artist_name.clone()),
        artist_id: Some(format!("artist-{}", base64_encode(&artist_name))),
        cover_art: None,
        song_count,
        duration,
        play_count: None,
        created: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        starred: None,
        year,
        genre,
        song: songs,
    };

    success_response(album, format)
}

async fn get_song(
    State(state): State<AppState>,
    Query(params): Query<GetSongParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    let track_id: i32 = params.id.parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let track = Track::find_by_id(track_id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match track {
        Some(track) => {
            let song = Child {
                id: track.id.to_string(),
                parent: Some(format!("album-{}-{}",
                    base64_encode(&track.artist),
                    base64_encode(&track.album)
                )),
                is_dir: false,
                title: track.title,
                album: Some(track.album.clone()),
                artist: Some(track.artist.clone()),
                track: track.track_number,
                year: track.year,
                genre: Some(track.genre),
                cover_art: None,
                size: None,
                content_type: Some(format!("audio/{}", track.extension)),
                suffix: Some(track.extension),
                trans_coded_content_type: None,
                trans_coded_suffix: None,
                duration: Some(track.duration_seconds as u32),
                bit_rate: Some(track.audio_bitrate as u32),
                path: Some(track.path),
                is_video: Some(false),
                user_rating: None,
                average_rating: None,
                play_count: None,
                disc_number: track.disc_number,
                created: Some(track.created.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
                starred: None,
                album_id: Some(format!("album-{}-{}",
                    base64_encode(&track.artist),
                    base64_encode(&track.album)
                )),
                artist_id: Some(format!("artist-{}", base64_encode(&track.artist))),
                media_type: Some("music".to_string()),
                bookmark_position: None,
                original_width: None,
                original_height: None,
            };

            success_response(song, format)
        }
        None => error_response(70, "The requested data was not found", format),
    }
}

async fn search3(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    let query = params.query.unwrap_or_default();
    if query.is_empty() {
        return error_response(10, "Required parameter is missing", format);
    }

    let artist_count = params.artist_count.unwrap_or(20) as u64;
    let artist_offset = params.artist_offset.unwrap_or(0) as u64;
    let album_count = params.album_count.unwrap_or(20) as u64;
    let album_offset = params.album_offset.unwrap_or(0) as u64;
    let song_count = params.song_count.unwrap_or(20) as u64;
    let song_offset = params.song_offset.unwrap_or(0) as u64;

    // Search artists
    let artist_tracks = Track::find()
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .filter(track::Column::Artist.contains(&query))
        .filter(track::Column::Artist.ne(""))
        .order_by_asc(track::Column::Artist)
        .paginate(&state.db, artist_count)
        .fetch_page(artist_offset / artist_count)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let artists: Vec<ArtistID3> = artist_tracks
        .into_iter()
        .map(|track| ArtistID3 {
            id: format!("artist-{}", base64_encode(&track.artist)),
            name: track.artist,
            cover_art: None,
            artist_image_url: None,
            album_count: 0, // Would need a separate query to get accurate count
            starred: None,
        })
        .collect();

    // Search albums
    let album_tracks = Track::find()
        .select_only()
        .columns([track::Column::Album, track::Column::Artist, track::Column::Year, track::Column::Genre])
        .distinct()
        .filter(track::Column::Album.contains(&query))
        .filter(track::Column::Album.ne(""))
        .order_by_asc(track::Column::Album)
        .paginate(&state.db, album_count)
        .fetch_page(album_offset / album_count)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let albums: Vec<AlbumID3> = album_tracks
        .into_iter()
        .map(|track| AlbumID3 {
            id: format!("album-{}-{}",
                base64_encode(&track.artist),
                base64_encode(&track.album)
            ),
            name: track.album,
            artist: Some(track.artist.clone()),
            artist_id: Some(format!("artist-{}", base64_encode(&track.artist))),
            cover_art: None,
            song_count: 0, // Would need a separate query to get accurate count
            duration: 0,   // Would need a separate query to get accurate duration
            play_count: None,
            created: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            starred: None,
            year: track.year,
            genre: Some(track.genre),
        })
        .collect();

    // Search songs
    let mut song_condition = Condition::any();
    song_condition = song_condition.add(track::Column::Title.contains(&query));
    song_condition = song_condition.add(track::Column::Artist.contains(&query));
    song_condition = song_condition.add(track::Column::Album.contains(&query));

    let song_tracks = Track::find()
        .filter(song_condition)
        .order_by_asc(track::Column::Artist)
        .order_by_asc(track::Column::Album)
        .order_by_asc(track::Column::Title)
        .paginate(&state.db, song_count)
        .fetch_page(song_offset / song_count)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let songs: Vec<Child> = song_tracks
        .into_iter()
        .map(|track| Child {
            id: track.id.to_string(),
            parent: Some(format!("album-{}-{}",
                base64_encode(&track.artist),
                base64_encode(&track.album)
            )),
            is_dir: false,
            title: track.title,
            album: Some(track.album.clone()),
            artist: Some(track.artist.clone()),
            track: track.track_number,
            year: track.year,
            genre: Some(track.genre),
            cover_art: None,
            size: None,
            content_type: Some(format!("audio/{}", track.extension)),
            suffix: Some(track.extension),
            trans_coded_content_type: None,
            trans_coded_suffix: None,
            duration: Some(track.duration_seconds as u32),
            bit_rate: Some(track.audio_bitrate as u32),
            path: Some(track.path),
            is_video: Some(false),
            user_rating: None,
            average_rating: None,
            play_count: None,
            disc_number: track.disc_number,
            created: Some(track.created.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            starred: None,
            album_id: Some(format!("album-{}-{}",
                base64_encode(&track.artist),
                base64_encode(&track.album)
            )),
            artist_id: Some(format!("artist-{}", base64_encode(&track.artist))),
            media_type: Some("music".to_string()),
            bookmark_position: None,
            original_width: None,
            original_height: None,
        })
        .collect();

    let search_result = SearchResult3 {
        artist: artists,
        album: albums,
        song: songs,
    };

    success_response(search_result, format)
}

async fn stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<StreamParams>,
) -> Result<Response, StatusCode> {
    let format = get_format(&params.auth);

    if !validate_auth(&params.auth) {
        return error_response(40, "Wrong username or password", format);
    }

    let track_id: i32 = id.parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let track = Track::find_by_id(track_id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match track {
        Some(track) => {
            // Serve the actual audio file
            let file_path = std::path::Path::new(&track.path);
            if file_path.exists() {
                let file = tokio::fs::File::open(&track.path).await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                let stream = tokio_util::io::ReaderStream::new(file);
                let body = axum::body::Body::from_stream(stream);

                let content_type = match track.extension.as_str() {
                    "mp3" => "audio/mpeg",
                    "flac" => "audio/flac",
                    "ogg" => "audio/ogg",
                    "m4a" | "mp4" => "audio/mp4",
                    "wav" => "audio/wav",
                    _ => "application/octet-stream",
                };

                Ok((
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::CONTENT_DISPOSITION, &format!("inline; filename=\"{}.{}\"", track.title, track.extension)),
                    ],
                    body,
                ).into_response())
            } else {
                error_response(70, "The requested data was not found", format)
            }
        }
        None => error_response(70, "The requested data was not found", format),
    }
}

// Helper functions for base64 encoding/decoding (simple implementation)
fn base64_encode(input: &str) -> String {
    base64::encode(input)
}

fn base64_decode(input: &str) -> Option<String> {
    base64::decode(input)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

// Simple base64 implementation since we don't want to add another dependency
mod base64 {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(input: &str) -> String {
        let input_bytes = input.as_bytes();
        let mut result = String::new();

        for chunk in input_bytes.chunks(3) {
            let b1 = chunk[0];
            let b2 = chunk.get(1).copied().unwrap_or(0);
            let b3 = chunk.get(2).copied().unwrap_or(0);

            let combined = ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);

            result.push(CHARS[((combined >> 18) & 63) as usize] as char);
            result.push(CHARS[((combined >> 12) & 63) as usize] as char);
            result.push(if chunk.len() > 1 { CHARS[((combined >> 6) & 63) as usize] as char } else { '=' });
            result.push(if chunk.len() > 2 { CHARS[(combined & 63) as usize] as char } else { '=' });
        }

        result
    }

    pub fn decode(input: &str) -> Result<Vec<u8>, ()> {
        let input = input.trim_end_matches('=');
        let mut result = Vec::new();

        for chunk in input.as_bytes().chunks(4) {
            let mut values = [0u8; 4];
            for (i, &byte) in chunk.iter().enumerate() {
                values[i] = CHARS.iter().position(|&c| c == byte).ok_or(())? as u8;
            }

            let combined = ((values[0] as u32) << 18) |
                          ((values[1] as u32) << 12) |
                          ((values[2] as u32) << 6) |
                          (values[3] as u32);

            result.push((combined >> 16) as u8);
            if chunk.len() > 2 {
                result.push((combined >> 8) as u8);
            }
            if chunk.len() > 3 {
                result.push(combined as u8);
            }
        }

        Ok(result)
    }
}
