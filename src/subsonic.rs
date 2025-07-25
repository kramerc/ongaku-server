use axum::{
    extract::{Query, State},
    response::Response,
    routing::get,
    Router,
    http::{StatusCode, HeaderMap},
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

    // Return minimal subsonic-response format as per API specification
    let xml_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}"> </subsonic-response>"#,
        query.auth.v  // Use the version from the client request
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
    State(state): State<AppState>,
    Query(query): Query<GetMusicFoldersQuery>,
) -> Response {
    debug!("GetMusicFolders request from client: {}", query.auth.c);

    // Get the music folder name from the music path
    let music_path = std::path::Path::new(&state.music_path);
    let folder_name = music_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Music")
        .to_string();

    // Create the XML response manually to match the expected Subsonic format
    let xml_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}">
<musicFolders>
<musicFolder id="1" name="{}"/>
</musicFolders>
</subsonic-response>"#,
        SUBSONIC_API_VERSION, folder_name
    );

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(xml_content.into())
        .unwrap()
}

async fn get_indexes(
    State(state): State<AppState>,
    Query(query): Query<GetIndexesQuery>,
) -> Response {
    debug!("GetIndexes request from client: {}", query.auth.c);

    // Get all unique artists from database using a custom query
    let artists = match Track::find()
        .select_only()
        .column(track::Column::Artist)
        .distinct()
        .into_tuple::<String>()
        .all(&state.db)
        .await
    {
        Ok(artists) => artists,
        Err(e) => {
            error!("Failed to fetch artists: {}", e);
            // Return error in correct format
            let xml_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="failed" version="{}">
</subsonic-response>"#,
                SUBSONIC_API_VERSION
            );
            return axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/xml; charset=utf-8")
                .body(xml_content.into())
                .unwrap();
        }
    };

    // Group artists by first letter
    let mut indexes_map: HashMap<char, Vec<String>> = HashMap::new();

    for artist_name in artists {
        if artist_name.is_empty() {
            continue;
        }

        let first_char = artist_name.chars().next().unwrap_or('#').to_ascii_uppercase();
        let index_char = if first_char.is_ascii_alphabetic() { first_char } else { '#' };

        indexes_map.entry(index_char).or_insert_with(Vec::new).push(artist_name);
    }

    // Build XML manually to match the expected format
    let mut xml_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}">
<indexes lastModified="{}" ignoredArticles="The El La Los Las Le Les">"#,
        SUBSONIC_API_VERSION,
        Utc::now().timestamp_millis()
    );

    // Sort index keys
    let mut sorted_keys: Vec<char> = indexes_map.keys().cloned().collect();
    sorted_keys.sort();

    for index_char in sorted_keys {
        if let Some(mut artists) = indexes_map.remove(&index_char) {
            artists.sort();
            xml_content.push_str(&format!(r#"
<index name="{}">"#, index_char));

            for artist_name in artists {
                let artist_id = format!("artist-{}", urlencoding::encode(&artist_name));
                xml_content.push_str(&format!(
                    r#"
<artist id="{}" name="{}"/>"#,
                    artist_id,
                    artist_name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
                ));
            }

            xml_content.push_str(r#"
</index>"#);
        }
    }

    xml_content.push_str(r#"
</indexes>
</subsonic-response>"#);

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(xml_content.into())
        .unwrap()
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
            .into_tuple::<String>()
            .all(&state.db)
            .await
        {
            Ok(albums) => albums,
            Err(e) => {
                error!("Failed to fetch albums for artist: {}", e);
                let xml_content = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="failed" version="{}">
</subsonic-response>"#,
                    SUBSONIC_API_VERSION
                );
                return axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/xml; charset=utf-8")
                    .body(xml_content.into())
                    .unwrap();
            }
        };

        // Build XML manually to match the expected format
        let mut xml_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}">
<directory id="{}" name="{}">"#,
            SUBSONIC_API_VERSION,
            query.id.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
            artist_name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
        );

        for album_name in albums {
            let album_id = format!("album-{}-{}",
                urlencoding::encode(&artist_name),
                urlencoding::encode(&album_name)
            );
            xml_content.push_str(&format!(
                r#"
<child id="{}" parent="{}" title="{}" artist="{}" isDir="true"/>"#,
                album_id,
                query.id.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                album_name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                artist_name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
            ));
        }

        xml_content.push_str(r#"
</directory>
</subsonic-response>"#);

        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(xml_content.into())
            .unwrap()

    } else if query.id.starts_with("album-") {
        // Return tracks for this album
        // Format: album-{encoded_artist}-{encoded_album}
        // Handle special case where artist starts with hyphen by looking for double hyphens
        let id_without_prefix = &query.id[6..]; // Remove "album-" prefix

        let (artist_encoded, album_encoded) = if let Some(double_hyphen_pos) = id_without_prefix.find("--") {
            // Special case: artist name starts with hyphen, look for double hyphen
            let artist_part = &id_without_prefix[..double_hyphen_pos + 1]; // Include the first hyphen
            let album_part = &id_without_prefix[double_hyphen_pos + 2..]; // Skip both hyphens
            (artist_part, album_part)
        } else {
            // Normal case: split on first hyphen
            if let Some(first_hyphen_pos) = id_without_prefix.find('-') {
                let artist_part = &id_without_prefix[..first_hyphen_pos];
                let album_part = &id_without_prefix[first_hyphen_pos + 1..];
                (artist_part, album_part)
            } else {
                ("", "")
            }
        };

        if artist_encoded.is_empty() || album_encoded.is_empty() {
            let xml_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="failed" version="{}">
</subsonic-response>"#,
                SUBSONIC_API_VERSION
            );
            return axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/xml; charset=utf-8")
                .body(xml_content.into())
                .unwrap();
        }

        let artist_name = urlencoding::decode(artist_encoded).unwrap_or_default();
        let album_name = urlencoding::decode(album_encoded).unwrap_or_default();

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
                let xml_content = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="failed" version="{}">
</subsonic-response>"#,
                    SUBSONIC_API_VERSION
                );
                return axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/xml; charset=utf-8")
                    .body(xml_content.into())
                    .unwrap();
            }
        };

        // Build XML manually to match the expected format
        let parent_id = format!("artist-{}", artist_encoded);
        let mut xml_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="ok" version="{}">
<directory id="{}" parent="{}" name="{}">"#,
            SUBSONIC_API_VERSION,
            query.id.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
            parent_id.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
            album_name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
        );

        for track in tracks {
            xml_content.push_str(&format!(
                r#"
<child id="{}" parent="{}" title="{}" isDir="false" album="{}" artist="{}" track="{}" year="{}" genre="{}" contentType="audio/{}" suffix="{}" duration="{}" bitRate="{}" path="{}"/>"#,
                track.id,
                query.id.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                track.title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                track.album.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                track.artist.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                track.track_number.unwrap_or(0),
                track.year.unwrap_or(0),
                if track.genre.is_empty() { "Unknown" } else { &track.genre }.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;"),
                track.extension,
                track.extension,
                track.duration_seconds,
                track.audio_bitrate,
                track.path.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
            ));
        }

        xml_content.push_str(r#"
</directory>
</subsonic-response>"#);

        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(xml_content.into())
            .unwrap()

    } else {
        let xml_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<subsonic-response status="failed" version="{}">
</subsonic-response>"#,
            SUBSONIC_API_VERSION
        );
        axum::response::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(xml_content.into())
            .unwrap()
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
        .into_tuple::<String>()
        .all(&state.db)
        .await;

    let genres = match genres_result {
        Ok(genre_names) => {
            let mut genres: Vec<Genre> = genre_names
                .into_iter()
                .filter(|g| !g.is_empty())
                .map(|g| Genre {
                    value: g.clone(),
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
        .into_tuple::<String>()
        .all(&state.db)
        .await
        .unwrap_or_default();

    let artist_results: Vec<Artist> = artists
        .into_iter()
        .map(|artist_name| Artist {
            id: format!("artist-{}", urlencoding::encode(&artist_name)),
            name: artist_name,
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
        .into_tuple::<(String, String, Option<i32>)>()
        .all(&state.db)
        .await
        .unwrap_or_default();

    let album_results: Vec<Album> = albums
        .into_iter()
        .map(|(artist, album, year)| Album {
            id: format!("album-{}-{}",
                urlencoding::encode(&artist),
                urlencoding::encode(&album)
            ),
            name: album,
            artist: Some(artist.clone()),
            artist_id: Some(format!("artist-{}", urlencoding::encode(&artist))),
            cover_art: None,
            song_count: 0,
            duration: 0,
            created: Utc::now(),
            year,
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
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    debug!("Stream request for ID: {}", query.id);
    debug!("Query parameters: estimateContentLength={:?}, maxBitRate={:?}, timeOffset={:?}",
           query.estimate_content_length, query.max_bit_rate, query.time_offset);

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

    // Get file metadata
    let file_metadata = std::fs::metadata(&file_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let file_size = file_metadata.len();

    debug!("File info: path={:?}, size={} bytes, duration={}s",
           file_path, file_size, track.duration_seconds);

    // Determine MIME type based on file extension
    let mime_type = match track.extension.to_lowercase().as_str() {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        _ => "audio/mpeg", // default fallback
    };

    // Check for Range header to support seeking
    if let Some(range_header) = headers.get("range") {
        if let Ok(range_str) = range_header.to_str() {
            debug!("Range request: {}", range_str);
            if range_str.starts_with("bytes=") {
                let range_part = &range_str[6..]; // Remove "bytes="

                // Parse range (e.g., "0-1023" or "1024-" or "-500")
                if let Some((start_str, end_str)) = range_part.split_once('-') {
                    let start = if start_str.is_empty() {
                        // Suffix range like "-500" (last 500 bytes)
                        if let Ok(suffix_len) = end_str.parse::<u64>() {
                            file_size.saturating_sub(suffix_len)
                        } else {
                            0
                        }
                    } else if let Ok(start_pos) = start_str.parse::<u64>() {
                        start_pos
                    } else {
                        0
                    };

                    let end = if end_str.is_empty() {
                        // Range like "1024-" (from position to end)
                        file_size - 1
                    } else if let Ok(end_pos) = end_str.parse::<u64>() {
                        std::cmp::min(end_pos, file_size - 1)
                    } else {
                        file_size - 1
                    };

                    if start <= end && start < file_size {
                        let content_length = end - start + 1;
                        debug!("Serving range: {}-{}/{} ({} bytes)", start, end, file_size, content_length);

                        // Use tower-http's ServeFile for range requests
                        let service = tower_http::services::ServeFile::new(&file_path);
                        let mut request = axum::http::Request::builder()
                            .uri("/")
                            .header("range", format!("bytes={}-{}", start, end))
                            .body(axum::body::Body::empty())
                            .unwrap();

                        // Copy the range header to the inner request
                        request.headers_mut().insert("range", range_header.clone());

                        match service.oneshot(request).await {
                            Ok(mut response) => {
                                let headers = response.headers_mut();

                                // Set proper headers for range response
                                headers.insert("content-type", mime_type.parse().unwrap());
                                headers.insert("accept-ranges", "bytes".parse().unwrap());
                                headers.insert("content-length", content_length.to_string().parse().unwrap());
                                headers.insert("content-range",
                                    format!("bytes {}-{}/{}", start, end, file_size).parse().unwrap());
                                headers.insert("cache-control", "public, max-age=3600".parse().unwrap());

                                // Remove any transfer-encoding that might conflict with content-length
                                headers.remove("transfer-encoding");

                                // Remove content-encoding to prevent compression confusion
                                headers.remove("content-encoding");

                                // Add mobile-friendly headers for range requests
                                headers.insert("connection", "keep-alive".parse().unwrap());

                                // Set status to 206 Partial Content for range requests
                                *response.status_mut() = StatusCode::PARTIAL_CONTENT;

                                return Ok(response.map(axum::body::Body::new));
                            },
                            Err(e) => {
                                error!("Range request failed: {:?}", e);
                                return Err(StatusCode::INTERNAL_SERVER_ERROR);
                            }
                        }
                    } else {
                        debug!("Invalid range: start={}, end={}, file_size={}", start, end, file_size);
                    }
                }
            }
        }
    }

    // For mobile clients with large files, we might want to encourage range requests
    // by providing hints in the response
    let is_mobile_client = headers.get("user-agent")
        .and_then(|ua| ua.to_str().ok())
        .map(|ua| ua.contains("iSub") || ua.contains("Mobile"))
        .unwrap_or(false);

    debug!("Client info: mobile={}, user-agent={:?}",
           is_mobile_client,
           headers.get("user-agent").and_then(|ua| ua.to_str().ok()));

    // For large files on mobile clients, force a range request to prevent timeouts
    if is_mobile_client && file_size > 20_000_000 { // 20MB threshold for mobile
        debug!("Large file on mobile client, forcing range request to prevent timeout");

        // Serve first chunk (first 5MB) to start playback quickly
        let chunk_size = std::cmp::min(5_242_880, file_size); // 5MB or file size if smaller
        let service = tower_http::services::ServeFile::new(&file_path);

        let request = axum::http::Request::builder()
            .uri("/")
            .header("range", format!("bytes=0-{}", chunk_size - 1))
            .body(axum::body::Body::empty())
            .unwrap();

        match service.oneshot(request).await {
            Ok(mut response) => {
                let headers = response.headers_mut();

                // Set proper headers for partial content
                headers.insert("content-type", mime_type.parse().unwrap());
                headers.insert("accept-ranges", "bytes".parse().unwrap());
                headers.insert("content-length", chunk_size.to_string().parse().unwrap());
                headers.insert("content-range",
                    format!("bytes 0-{}/{}", chunk_size - 1, file_size).parse().unwrap());
                headers.insert("cache-control", "public, max-age=3600".parse().unwrap());
                headers.insert("connection", "keep-alive".parse().unwrap());

                // Add mobile optimization headers
                headers.insert("x-content-duration", track.duration_seconds.to_string().parse().unwrap());
                headers.insert("x-content-bitrate", track.audio_bitrate.to_string().parse().unwrap());
                headers.insert("x-stream-mode", "chunked".parse().unwrap());

                // Remove any conflicting headers
                headers.remove("transfer-encoding");
                headers.remove("content-encoding");

                *response.status_mut() = StatusCode::PARTIAL_CONTENT;
                return Ok(response.map(axum::body::Body::new));
            },
            Err(e) => {
                error!("Mobile chunk request failed: {:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // Normal full file serving (no range request)
    let service = tower_http::services::ServeFile::new(&file_path);    let request = axum::http::Request::builder()
        .uri("/")
        .body(axum::body::Body::empty())
        .unwrap();

    match service.oneshot(request).await {
        Ok(mut response) => {
            // Add streaming-friendly headers
            let headers = response.headers_mut();

            // Set proper content type
            headers.insert("content-type", mime_type.parse().unwrap());

            // Enable range requests for seeking
            headers.insert("accept-ranges", "bytes".parse().unwrap());

            // Set content length
            headers.insert("content-length", file_size.to_string().parse().unwrap());

            // Cache control for better streaming performance
            headers.insert("cache-control", "public, max-age=3600".parse().unwrap());

            // Remove any transfer-encoding that might conflict with content-length
            headers.remove("transfer-encoding");

            // Remove content-encoding to prevent compression confusion
            headers.remove("content-encoding");

            // Add mobile-friendly headers for better streaming
            headers.insert("connection", "keep-alive".parse().unwrap());

            // For large files, add headers to suggest streaming behavior
            if file_size > 50_000_000 { // 50MB threshold
                debug!("Large file detected ({}MB), adding streaming optimization headers", file_size / 1_000_000);
                headers.insert("x-content-duration", track.duration_seconds.to_string().parse().unwrap());
                headers.insert("x-content-bitrate", track.audio_bitrate.to_string().parse().unwrap());
            }

            Ok(response.map(axum::body::Body::new))
        },
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
