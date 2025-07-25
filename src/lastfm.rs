use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::{Json, Html},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use log::{debug, error, warn};
use rustfm_scrobble_proxy::{Scrobbler, Scrobble};
use md5;

use entity::prelude::Track;
use entity::track;
use sea_orm::EntityTrait;

use crate::api::AppState;

const LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
const LASTFM_AUTH_URL: &str = "https://www.last.fm/api/auth";

// Session storage
fn get_session_file_path() -> Result<PathBuf, String> {
    let mut path = dirs::config_dir()
        .ok_or("Could not determine config directory")?;
    path.push("ongaku-server");
    if !path.exists() {
        fs::create_dir_all(&path)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    path.push("lastfm_session");
    Ok(path)
}

#[derive(Deserialize)]
struct LastfmTokenResponse {
    token: Option<String>,
    error: Option<i32>,
    message: Option<String>,
}

#[derive(Serialize)]
pub struct LastfmAuthResponse {
    pub auth_url: String,
    pub token: String,
}

// Empty struct since callback URL is now pre-configured in Last.fm app settings
#[derive(Deserialize)]
pub struct AuthUrlQuery {}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub token: String,
}

#[derive(Deserialize)]
pub struct LastfmSessionRequest {
    pub token: String,
}

#[derive(Serialize)]
pub struct LastfmSessionResponse {
    pub session_key: String,
    pub username: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct ScrobbleRequest {
    pub session_key: String,
    pub timestamp: i64,
    pub album_artist: Option<String>,
}

#[derive(Deserialize)]
pub struct NowPlayingRequest {
    pub session_key: String,
}

#[derive(Serialize)]
pub struct ScrobbleResponse {
    pub success: bool,
    pub message: String,
    pub scrobble_id: Option<String>,
}

#[derive(Serialize)]
pub struct NowPlayingResponse {
    pub success: bool,
    pub message: String,
}

pub struct LastfmClient {
    client: Client,
    api_key: String,
    api_secret: String,
}

impl LastfmClient {
    pub fn new() -> Result<Self, String> {
        let api_key = env::var("LASTFM_API_KEY")
            .map_err(|_| "LASTFM_API_KEY environment variable not set")?;
        let api_secret = env::var("LASTFM_SHARED_SECRET")
            .map_err(|_| "LASTFM_SHARED_SECRET environment variable not set")?;

        Ok(Self {
            client: Client::new(),
            api_key,
            api_secret,
        })
    }

    pub async fn get_token(&self) -> Result<String, String> {
        let mut params = HashMap::new();
        params.insert("method", "auth.gettoken");
        params.insert("api_key", &self.api_key);
        params.insert("format", "json");

        let signature = self.generate_signature(&params);
        params.insert("api_sig", &signature);

        debug!("Requesting Last.fm token with params: {:?}", params.keys().collect::<Vec<_>>());

        let response = self.client
            .get(LASTFM_API_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let api_response: LastfmTokenResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

        if let Some(error) = api_response.error {
            return Err(format!("Last.fm API error {}: {}", error, api_response.message.unwrap_or_default()));
        }

        api_response.token.ok_or_else(|| "No token in response".to_string())
    }

    pub async fn get_session(&self, token: &str) -> Result<(String, String), String> {
        // Validate token format - tokens should not be empty
        if token.trim().is_empty() {
            return Err("Invalid token: token cannot be empty".to_string());
        }

        debug!("Creating Last.fm session for token: {}", &token[..std::cmp::min(8, token.len())]);

        let mut scrobbler = Scrobbler::new(&self.api_key, &self.api_secret);

        match scrobbler.authenticate_with_token(token) {
            Ok(session_response) => {
                debug!("Successfully created session for user: {}", session_response.name);

                // Save session key to file for future use
                if let Ok(session_path) = get_session_file_path() {
                    if let Err(e) = fs::write(&session_path, &session_response.key) {
                        warn!("Failed to save session key to file: {}", e);
                    }
                }

                Ok((session_response.key, session_response.name))
            },
            Err(e) => Err(format!("Failed to create Last.fm session: {}", e))
        }
    }

    pub async fn scrobble_track(&self, session_key: &str, track: &track::Model, timestamp: i64, _album_artist: Option<&str>) -> Result<Option<String>, String> {
        // Validate required track data
        if track.artist.trim().is_empty() || track.title.trim().is_empty() {
            return Err("Track must have both artist and title".to_string());
        }

        let mut scrobbler = Scrobbler::new(&self.api_key, &self.api_secret);
        scrobbler.authenticate_with_session_key(session_key);

        let album = if track.album.trim().is_empty() { None } else { Some(track.album.as_str()) };
        let mut scrobble = Scrobble::new(&track.artist, &track.title, album);

        // Set timestamp
        scrobble.with_timestamp(timestamp as u64);

        debug!("Scrobbling track: {} - {} (timestamp: {})", track.artist, track.title, timestamp);

        match scrobbler.scrobble(&scrobble) {
            Ok(_response) => {
                // The proxy library doesn't return scrobble IDs, so we return None
                Ok(None)
            },
            Err(e) => Err(format!("Failed to scrobble track: {}", e))
        }
    }

    pub async fn update_now_playing(&self, session_key: &str, track: &track::Model) -> Result<(), String> {
        // Validate required track data
        if track.artist.trim().is_empty() || track.title.trim().is_empty() {
            return Err("Track must have both artist and title".to_string());
        }

        let mut scrobbler = Scrobbler::new(&self.api_key, &self.api_secret);
        scrobbler.authenticate_with_session_key(session_key);

        let album = if track.album.trim().is_empty() { None } else { Some(track.album.as_str()) };
        let scrobble = Scrobble::new(&track.artist, &track.title, album);

        debug!("Updating now playing: {} - {}", track.artist, track.title);

        match scrobbler.now_playing(&scrobble) {
            Ok(_response) => Ok(()),
            Err(e) => Err(format!("Failed to update now playing: {}", e))
        }
    }

    pub fn build_auth_url(&self, token: &str) -> String {
        // Validate inputs as per documentation
        if token.trim().is_empty() {
            panic!("Token cannot be empty when building auth URL");
        }

        // Build the parameter vector for URL encoding
        // No signature needed for user-facing auth URLs
        let url_params = vec![
            ("api_key", self.api_key.as_str()),
            ("token", token),
        ];

        // Build URL with proper encoding as specified in the documentation
        let query_string = url_params.iter()
            .map(|(k, v)| format!("{}={}",
                urlencoding::encode(k),
                urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let auth_url = format!("{}?{}", LASTFM_AUTH_URL, query_string);
        debug!("Generated auth URL: {}", auth_url);

        auth_url
    }

    /// Load existing session from file if available
    pub fn load_existing_session(&self) -> Option<String> {
        get_session_file_path()
            .ok()
            .and_then(|path| fs::read_to_string(path).ok())
            .filter(|s| !s.trim().is_empty())
    }

    fn generate_signature(&self, params: &HashMap<&str, &str>) -> String {
        let mut sorted_params: Vec<_> = params.iter()
            .filter(|(key, _)| **key != "format" && **key != "api_sig")
            .collect();
        sorted_params.sort_by_key(|(key, _)| *key);

        let mut signature_string = String::new();
        for (key, value) in sorted_params {
            signature_string.push_str(key);
            signature_string.push_str(value);
        }
        signature_string.push_str(&self.api_secret);

        debug!("Signature string: {}", signature_string);
        let signature = format!("{:x}", md5::compute(signature_string.as_bytes()));
        debug!("Generated signature: {}", signature);

        signature
    }
}

// API handlers

pub async fn get_auth_url(
    State(_state): State<AppState>,
    Query(_query): Query<AuthUrlQuery>,
) -> Result<Json<LastfmAuthResponse>, StatusCode> {
    let client = match LastfmClient::new() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Last.fm client: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let token = match client.get_token().await {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to get Last.fm token: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let auth_url = client.build_auth_url(&token);

    Ok(Json(LastfmAuthResponse { auth_url, token }))
}

pub async fn create_session(
    State(_state): State<AppState>,
    Json(request): Json<LastfmSessionRequest>,
) -> Result<Json<LastfmSessionResponse>, StatusCode> {
    let client = match LastfmClient::new() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Last.fm client: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let (session_key, username) = match client.get_session(&request.token).await {
        Ok(session) => session,
        Err(e) => {
            warn!("Failed to create Last.fm session: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    Ok(Json(LastfmSessionResponse {
        session_key,
        username,
        message: "Last.fm session created successfully".to_string(),
    }))
}

pub async fn auth_callback(
    State(_state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Html<String>, StatusCode> {
    let client = match LastfmClient::new() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Last.fm client: {}", e);
            return Ok(Html(format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>Last.fm Authorization Failed</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background-color: #f5f5f5; }}
        .container {{ max-width: 600px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .error {{ color: #d32f2f; }}
        h1 {{ color: #333; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎵 Last.fm Authorization Failed</h1>
        <p class="error">Internal server error occurred while setting up Last.fm client.</p>
        <p>Please try again later.</p>
    </div>
</body>
</html>"#
            )));
        }
    };

    // Validate token
    if query.token.trim().is_empty() {
        return Ok(Html(format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Last.fm Authorization Failed</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background-color: #f5f5f5; }}
        .container {{ max-width: 600px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .error {{ color: #d32f2f; }}
        h1 {{ color: #333; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎵 Last.fm Authorization Failed</h1>
        <p class="error">Invalid or missing authorization token.</p>
        <p>Please restart the authorization process.</p>
    </div>
</body>
</html>"#
        )));
    }

    // Create session from the token
    let (session_key, username) = match client.get_session(&query.token).await {
        Ok(session) => session,
        Err(e) => {
            warn!("Failed to create Last.fm session: {}", e);
            return Ok(Html(format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <title>Last.fm Authorization Failed</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background-color: #f5f5f5; }}
        .container {{ max-width: 600px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .error {{ color: #d32f2f; }}
        h1 {{ color: #333; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🎵 Last.fm Authorization Failed</h1>
        <p class="error">Failed to create Last.fm session: {}</p>
        <p>This usually means the token has expired or was not properly authorized.</p>
        <p>Please restart the authorization process.</p>
    </div>
</body>
</html>"#, e
            )));
        }
    };

    // Success! Return HTML with session information
    Ok(Html(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Last.fm Authorization Complete</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; background-color: #f5f5f5; }}
        .container {{ max-width: 600px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .success {{ color: #2e7d32; }}
        .session-info {{ background: #f8f9fa; padding: 20px; border-radius: 5px; margin: 20px 0; border-left: 4px solid #2e7d32; }}
        .session-key {{ font-family: monospace; background: #e3f2fd; padding: 10px; border-radius: 3px; word-break: break-all; margin: 10px 0; }}
        h1 {{ color: #333; }}
        .copy-btn {{ background: #1976d2; color: white; border: none; padding: 8px 15px; border-radius: 3px; cursor: pointer; margin-left: 10px; }}
        .copy-btn:hover {{ background: #1565c0; }}
    </style>
    <script>
        function copyToClipboard(text) {{
            navigator.clipboard.writeText(text).then(function() {{
                alert('Session key copied to clipboard!');
            }}, function(err) {{
                console.error('Could not copy text: ', err);
            }});
        }}
    </script>
</head>
<body>
    <div class="container">
        <h1>🎵 Last.fm Authorization Complete!</h1>
        <p class="success">✅ Successfully connected to your Last.fm account.</p>

        <div class="session-info">
            <h3>Session Information:</h3>
            <p><strong>Username:</strong> {}</p>
            <p><strong>Session Key:</strong></p>
            <div class="session-key">
                {}
                <button class="copy-btn" onclick="copyToClipboard('{}')">Copy</button>
            </div>
        </div>

        <p>Your Last.fm integration is now active! You can now scrobble tracks and update your "now playing" status.</p>
        <p><strong>Important:</strong> Save your session key as you'll need it for API requests. This window can be closed.</p>
    </div>
</body>
</html>"#,
        username,
        session_key,
        session_key
    )))
}

pub async fn scrobble_track(
    State(state): State<AppState>,
    Path(track_id): Path<i32>,
    Json(request): Json<ScrobbleRequest>,
) -> Result<Json<ScrobbleResponse>, StatusCode> {
    // Get track from database
    let track = match Track::find_by_id(track_id).one(&state.db).await {
        Ok(Some(track)) => track,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let client = match LastfmClient::new() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Last.fm client: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let scrobble_id = match client.scrobble_track(
        &request.session_key,
        &track,
        request.timestamp,
        request.album_artist.as_deref(),
    ).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to scrobble track: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(Json(ScrobbleResponse {
        success: true,
        message: "Track scrobbled successfully".to_string(),
        scrobble_id,
    }))
}

pub async fn update_now_playing(
    State(state): State<AppState>,
    Path(track_id): Path<i32>,
    Json(request): Json<NowPlayingRequest>,
) -> Result<Json<NowPlayingResponse>, StatusCode> {
    // Get track from database
    let track = match Track::find_by_id(track_id).one(&state.db).await {
        Ok(Some(track)) => track,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let client = match LastfmClient::new() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create Last.fm client: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match client.update_now_playing(&request.session_key, &track).await {
        Ok(_) => Ok(Json(NowPlayingResponse {
            success: true,
            message: "Now playing status updated successfully".to_string(),
        })),
        Err(e) => {
            error!("Failed to update now playing: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
