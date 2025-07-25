use axum::{
    extract::{Query, State},
    response::Response,
    routing::get,
    Router,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use std::collections::HashMap;
use log::{debug, error};
use chrono::{DateTime, Utc};
use urlencoding;
use tower::util::ServiceExt;
use quick_xml::se::to_string as to_xml_string;

use entity::prelude::Track;
use entity::track;
use crate::api::AppState;

const SUBSONIC_API_VERSION: &str = "1.16.1";
const SUBSONIC_TYPE: &str = "ongaku";
const SUBSONIC_VERSION: &str = "0.1.0";

#[derive(Debug, Serialize, Deserialize)]
pub struct SubsonicResponse<T> {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: ResponseWrapper<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseWrapper<T> {
    pub status: String,
    pub version: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(rename = "serverVersion", skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    #[serde(flatten)]
    pub data: T,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EmptyResponse {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: ServerResponseWrapper,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerResponseWrapper {
    pub status: String,
    pub version: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "serverVersion")]
    pub server_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct License {
    pub license: LicenseInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub valid: bool,
    pub email: Option<String>,
    #[serde(rename = "licenseExpires", skip_serializing_if = "Option::is_none")]
    pub license_expires: Option<String>,
    #[serde(rename = "trialExpires", skip_serializing_if = "Option::is_none")]
    pub trial_expires: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicFolders {
    #[serde(rename = "musicFolders")]
    pub music_folders: MusicFolderList,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicFolderList {
    #[serde(rename = "musicFolder")]
    pub music_folder: Vec<MusicFolder>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicFolder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Indexes {
    pub indexes: IndexesList,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct IndexesList {
    #[serde(rename = "lastModified")]
    pub last_modified: i64,
    #[serde(rename = "ignoredArticles")]
    pub ignored_articles: String,
    pub index: Vec<Index>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Index {
    pub name: String,
    pub artist: Vec<Artist>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(rename = "starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Directory {
    pub directory: DirectoryInfo,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DirectoryInfo {
    pub id: String,
    #[serde(rename = "parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub name: String,
    #[serde(rename = "starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "child")]
    pub children: Vec<Child>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Child {
    pub id: String,
    #[serde(rename = "parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "isDir")]
    pub is_dir: bool,
    pub title: String,
    #[serde(rename = "album", skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(rename = "artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "track", skip_serializing_if = "Option::is_none")]
    pub track: Option<i32>,
    #[serde(rename = "year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "size", skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(rename = "contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "suffix", skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(rename = "duration", skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(rename = "bitRate", skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<i32>,
    #[serde(rename = "path", skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "albumId", skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    #[serde(rename = "artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(rename = "starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Genres {
    pub genres: GenresList,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GenresList {
    pub genre: Vec<Genre>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Genre {
    #[serde(rename = "$text")]
    pub value: String,
    #[serde(rename = "songCount")]
    pub song_count: i32,
    #[serde(rename = "albumCount")]
    pub album_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(rename = "searchResult3")]
    pub search_result3: SearchResult3,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult3 {
    pub artist: Vec<Artist>,
    pub album: Vec<Album>,
    pub song: Vec<Child>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    #[serde(rename = "artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "songCount")]
    pub song_count: i32,
    pub duration: i32,
    #[serde(rename = "created")]
    pub created: DateTime<Utc>,
    #[serde(rename = "year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubsonicQuery {
    u: String,          // username
    p: Option<String>,  // password (deprecated)
    t: Option<String>,  // token
    s: Option<String>,  // salt
    v: String,          // version
    c: String,          // client
    f: Option<String>,  // format (xml, json, jsonp)
}

#[derive(Debug, Deserialize)]
pub struct PingQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
}

#[derive(Debug, Deserialize)]
pub struct GetLicenseQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
}

#[derive(Debug, Deserialize)]
pub struct GetMusicFoldersQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
}

#[derive(Debug, Deserialize)]
pub struct GetIndexesQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
    #[serde(rename = "musicFolderId")]
    music_folder_id: Option<String>,
    #[serde(rename = "ifModifiedSince")]
    if_modified_since: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GetMusicDirectoryQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct GetGenresQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
    query: Option<String>,
    #[serde(rename = "artistCount")]
    artist_count: Option<u64>,
    #[serde(rename = "artistOffset")]
    artist_offset: Option<u64>,
    #[serde(rename = "albumCount")]
    album_count: Option<u64>,
    #[serde(rename = "albumOffset")]
    album_offset: Option<u64>,
    #[serde(rename = "songCount")]
    song_count: Option<u64>,
    #[serde(rename = "songOffset")]
    song_offset: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    #[serde(flatten)]
    auth: SubsonicQuery,
    id: String,
    #[serde(rename = "maxBitRate")]
    max_bit_rate: Option<u32>,
    format: Option<String>,
    #[serde(rename = "timeOffset")]
    time_offset: Option<u32>,
    size: Option<String>,
    #[serde(rename = "estimateContentLength")]
    estimate_content_length: Option<bool>,
    converted: Option<bool>,
}

pub fn create_subsonic_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(server_info))
        .route("/ping", get(ping))
        .route("/ping.view", get(ping))
        .route("/getLicense", get(get_license))
        .route("/getLicense.view", get(get_license))
        .route("/getMusicFolders", get(get_music_folders))
        .route("/getMusicFolders.view", get(get_music_folders))
        .route("/getIndexes", get(get_indexes))
        .route("/getIndexes.view", get(get_indexes))
        .route("/getMusicDirectory", get(get_music_directory))
        .route("/getMusicDirectory.view", get(get_music_directory))
        .route("/getGenres", get(get_genres))
        .route("/getGenres.view", get(get_genres))
        .route("/search3", get(search3))
        .route("/search3.view", get(search3))
        .route("/stream", get(stream))
        .route("/stream.view", get(stream))
        .with_state(state)
}

fn create_success_response<T: Serialize>(data: T) -> Response {
    let response_data = SubsonicResponse {
        subsonic_response: ResponseWrapper {
            status: "ok".to_string(),
            version: SUBSONIC_API_VERSION.to_string(),
            type_: Some(SUBSONIC_TYPE.to_string()),
            server_version: Some(SUBSONIC_VERSION.to_string()),
            data,
        },
    };

    match to_xml_string(&response_data) {
        Ok(xml) => {
            let xml_with_header = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/xml; charset=utf-8")
                .body(xml_with_header.into())
                .unwrap()
        }
        Err(_) => {
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to serialize XML".into())
                .unwrap()
        }
    }
}

fn create_error_response<T: Serialize + Default>(_code: i32, _message: &str) -> Response {
    let response_data = SubsonicResponse {
        subsonic_response: ResponseWrapper {
            status: "failed".to_string(),
            version: SUBSONIC_API_VERSION.to_string(),
            type_: Some(SUBSONIC_TYPE.to_string()),
            server_version: Some(SUBSONIC_VERSION.to_string()),
            data: T::default(),
        },
    };

    match to_xml_string(&response_data) {
        Ok(xml) => {
            let xml_with_header = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}", xml);
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/xml; charset=utf-8")
                .body(xml_with_header.into())
                .unwrap()
        }
        Err(_) => {
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Failed to serialize XML".into())
                .unwrap()
        }
    }
}

pub async fn server_info() -> Response {
    // For server identification, just return a basic subsonic-response
    let xml_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}" type="{}" serverVersion="{}"></subsonic-response>"#,
        SUBSONIC_API_VERSION, SUBSONIC_TYPE, SUBSONIC_VERSION
    );

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(xml_content.into())
        .unwrap()
}

async fn ping(Query(query): Query<PingQuery>) -> Response {
    debug!("Ping request from client: {}", query.auth.c);

    // Return simple subsonic-response format like server_info
    let xml_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}"></subsonic-response>"#,
        SUBSONIC_API_VERSION
    );

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(xml_content.into())
        .unwrap()
}

async fn get_license(Query(query): Query<GetLicenseQuery>) -> Response {
    debug!("GetLicense request from client: {}", query.auth.c);

    let license = License {
        license: LicenseInfo {
            valid: true,
            email: Some("admin@ongaku.local".to_string()),
            license_expires: None,
            trial_expires: None,
        },
    };

    create_success_response(license)
}

async fn get_music_folders(
    State(_state): State<AppState>,
    Query(query): Query<GetMusicFoldersQuery>,
) -> Response {
    debug!("GetMusicFolders request from client: {}", query.auth.c);

    let folders = MusicFolders {
        music_folders: MusicFolderList {
            music_folder: vec![MusicFolder {
                id: "1".to_string(),
                name: "Music".to_string(),
            }],
        },
    };

    create_success_response(folders)
}

async fn get_indexes(
    State(state): State<AppState>,
    Query(query): Query<GetIndexesQuery>,
) -> Response {
    debug!("GetIndexes request from client: {}", query.auth.c);

    // Get all unique artists from database
    let artists = match Track::find()
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .all(&state.db)
        .await
    {
        Ok(artists) => artists,
        Err(e) => {
            error!("Failed to fetch artists: {}", e);
            return create_error_response::<Indexes>(0, "Failed to fetch artists");
        }
    };

    // Group artists by first letter
    let mut indexes_map: HashMap<char, Vec<Artist>> = HashMap::new();

    for artist_model in artists {
        let artist_name = artist_model.artist;
        if artist_name.is_empty() {
            continue;
        }

        let first_char = artist_name.chars().next().unwrap_or('#').to_ascii_uppercase();
        let index_char = if first_char.is_ascii_alphabetic() { first_char } else { '#' };

        let artist = Artist {
            id: format!("artist-{}", urlencoding::encode(&artist_name)),
            name: artist_name,
            starred: None,
        };

        indexes_map.entry(index_char).or_insert_with(Vec::new).push(artist);
    }

    // Convert to Index structs and sort
    let mut indexes: Vec<Index> = indexes_map
        .into_iter()
        .map(|(name, mut artists)| {
            artists.sort_by(|a, b| a.name.cmp(&b.name));
            Index {
                name: name.to_string(),
                artist: artists,
            }
        })
        .collect();

    indexes.sort_by(|a, b| a.name.cmp(&b.name));

    let result = Indexes {
        indexes: IndexesList {
            last_modified: Utc::now().timestamp_millis(),
            ignored_articles: "The El La Los Las Le Les".to_string(),
            index: indexes,
        },
    };

    create_success_response(result)
}

async fn get_music_directory(
    State(state): State<AppState>,
    Query(query): Query<GetMusicDirectoryQuery>,
) -> Response {
    debug!("GetMusicDirectory request for ID: {}", query.id);

    // Parse the ID to determine what type of directory this is
    if query.id.starts_with("artist-") {
        // Return albums for this artist
        let artist_name = urlencoding::decode(&query.id[7..]).unwrap_or_default();

        let albums = match Track::find()
            .filter(track::Column::Artist.eq(artist_name.as_ref()))
            .select_only()
            .column(track::Column::Album)
            .distinct()
            .all(&state.db)
            .await
        {
            Ok(albums) => albums,
            Err(e) => {
                error!("Failed to fetch albums for artist: {}", e);
                return create_error_response::<Directory>(0, "Failed to fetch albums");
            }
        };

        let children: Vec<Child> = albums
            .into_iter()
            .map(|album_model| Child {
                id: format!("album-{}-{}",
                    urlencoding::encode(&artist_name),
                    urlencoding::encode(&album_model.album)
                ),
                parent: Some(query.id.clone()),
                is_dir: true,
                title: album_model.album.clone(),
                album: Some(album_model.album),
                artist: Some(artist_name.to_string()),
                track: None,
                year: None,
                genre: None,
                cover_art: None,
                size: None,
                content_type: None,
                suffix: None,
                duration: None,
                bit_rate: None,
                path: None,
                album_id: None,
                artist_id: Some(query.id.clone()),
                type_: None,
                starred: None,
            })
            .collect();

        let directory = Directory {
            directory: DirectoryInfo {
                id: query.id.clone(),
                parent: None,
                name: artist_name.to_string(),
                starred: None,
                children,
            },
        };

        create_success_response(directory)

    } else if query.id.starts_with("album-") {
        // Return tracks for this album
        let parts: Vec<&str> = query.id[6..].split('-').collect();
        if parts.len() < 2 {
            return create_error_response::<Directory>(70, "Invalid album ID");
        }

        let artist_name = urlencoding::decode(parts[0]).unwrap_or_default();
        let album_name = urlencoding::decode(parts[1]).unwrap_or_default();

        let tracks = match Track::find()
            .filter(
                Condition::all()
                    .add(track::Column::Artist.eq(artist_name.as_ref()))
                    .add(track::Column::Album.eq(album_name.as_ref()))
            )
            .order_by_asc(track::Column::TrackNumber)
            .all(&state.db)
            .await
        {
            Ok(tracks) => tracks,
            Err(e) => {
                error!("Failed to fetch tracks for album: {}", e);
                return create_error_response::<Directory>(0, "Failed to fetch tracks");
            }
        };

        let children: Vec<Child> = tracks
            .into_iter()
            .map(|track| track_to_child(&track, &query.id))
            .collect();

        let directory = Directory {
            directory: DirectoryInfo {
                id: query.id.clone(),
                parent: Some(format!("artist-{}", urlencoding::encode(&artist_name))),
                name: album_name.to_string(),
                starred: None,
                children,
            },
        };

        create_success_response(directory)

    } else {
        create_error_response::<Directory>(70, "Invalid directory ID")
    }
}

async fn get_genres(
    State(state): State<AppState>,
    Query(query): Query<GetGenresQuery>,
) -> Response {
    debug!("GetGenres request from client: {}", query.auth.c);

    let genres_result = Track::find()
        .select_only()
        .column(track::Column::Genre)
        .distinct()
        .all(&state.db)
        .await;

    let genres = match genres_result {
        Ok(genre_models) => {
            let mut genres: Vec<Genre> = genre_models
                .into_iter()
                .filter(|g| !g.genre.is_empty())
                .map(|g| Genre {
                    value: g.genre.clone(),
                    song_count: 0, // We'd need a separate query to get counts
                    album_count: 0,
                })
                .collect();

            genres.sort_by(|a, b| a.value.cmp(&b.value));
            genres
        }
        Err(e) => {
            error!("Failed to fetch genres: {}", e);
            return create_error_response::<Genres>(0, "Failed to fetch genres");
        }
    };

    create_success_response(Genres {
        genres: GenresList { genre: genres },
    })
}

async fn search3(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    debug!("Search3 request: {:?}", query.query);

    let search_term = query.query.unwrap_or_default();
    if search_term.is_empty() {
        return create_success_response(SearchResult {
            search_result3: SearchResult3 {
                artist: vec![],
                album: vec![],
                song: vec![],
            },
        });
    }

    let like_pattern = format!("%{}%", search_term);

    // Search artists
    let artists = Track::find()
        .filter(track::Column::Artist.like(&like_pattern))
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .limit(query.artist_count.unwrap_or(20))
        .offset(query.artist_offset.unwrap_or(0))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let artist_results: Vec<Artist> = artists
        .into_iter()
        .map(|artist| Artist {
            id: format!("artist-{}", urlencoding::encode(&artist.artist)),
            name: artist.artist,
            starred: None,
        })
        .collect();

    // Search albums
    let albums = Track::find()
        .filter(track::Column::Album.like(&like_pattern))
        .select_only()
        .columns([track::Column::Artist, track::Column::Album, track::Column::Year])
        .distinct()
        .limit(query.album_count.unwrap_or(20))
        .offset(query.album_offset.unwrap_or(0))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let album_results: Vec<Album> = albums
        .into_iter()
        .map(|album| Album {
            id: format!("album-{}-{}",
                urlencoding::encode(&album.artist),
                urlencoding::encode(&album.album)
            ),
            name: album.album,
            artist: Some(album.artist.clone()),
            artist_id: Some(format!("artist-{}", urlencoding::encode(&album.artist))),
            cover_art: None,
            song_count: 0,
            duration: 0,
            created: Utc::now(),
            year: album.year,
            genre: None,
        })
        .collect();

    // Search songs
    let songs = Track::find()
        .filter(track::Column::Title.like(&like_pattern))
        .limit(query.song_count.unwrap_or(20))
        .offset(query.song_offset.unwrap_or(0))
        .all(&state.db)
        .await
        .unwrap_or_default();

    let song_results: Vec<Child> = songs
        .iter()
        .map(|track| track_to_child(track, ""))
        .collect();

    let result = SearchResult {
        search_result3: SearchResult3 {
            artist: artist_results,
            album: album_results,
            song: song_results,
        },
    };

    create_success_response(result)
}

async fn stream(
    State(state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Result<Response, StatusCode> {
    debug!("Stream request for ID: {}", query.id);

    // Parse track ID
    let track_id: i32 = query.id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let track = Track::find_by_id(track_id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let file_path = std::path::Path::new(&state.music_path).join(&track.path);

    if !file_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Use axum's built-in file serving
    let service = tower_http::services::ServeFile::new(&file_path);
    let request = axum::http::Request::builder()
        .uri("/")
        .body(axum::body::Body::empty())
        .unwrap();

    match service.oneshot(request).await {
        Ok(response) => Ok(response.map(axum::body::Body::new)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn track_to_child(track: &track::Model, parent: &str) -> Child {
    Child {
        id: track.id.to_string(),
        parent: if parent.is_empty() { None } else { Some(parent.to_string()) },
        is_dir: false,
        title: track.title.clone(),
        album: Some(track.album.clone()),
        artist: Some(track.artist.clone()),
        track: track.track_number,
        year: track.year,
        genre: if track.genre.is_empty() { None } else { Some(track.genre.clone()) },
        cover_art: None,
        size: None, // We'd need to get file size
        content_type: Some(format!("audio/{}", track.extension)),
        suffix: Some(track.extension.clone()),
        duration: Some(track.duration_seconds),
        bit_rate: Some(track.audio_bitrate),
        path: Some(track.path.clone()),
        album_id: Some(format!("album-{}-{}",
            urlencoding::encode(&track.artist),
            urlencoding::encode(&track.album)
        )),
        artist_id: Some(format!("artist-{}", urlencoding::encode(&track.artist))),
        type_: Some("music".to_string()),
        starred: None,
    }
}
