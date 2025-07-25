use std::collections::HashMap;
use std::env;
use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::{Json, Html},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use log::{debug, error, warn};
use md5;
use urlencoding;

use entity::prelude::Track;
use entity::track;
use sea_orm::EntityTrait;

use crate::api::AppState;

const LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
const LASTFM_AUTH_URL: &str = "https://www.last.fm/api/auth";

// Last.fm API error codes as per documentation
const LASTFM_ERROR_INVALID_SERVICE: i32 = 2;
const LASTFM_ERROR_INVALID_METHOD: i32 = 3;
const LASTFM_ERROR_AUTH_FAILED: i32 = 4;
const LASTFM_ERROR_INVALID_FORMAT: i32 = 5;
const LASTFM_ERROR_INVALID_PARAMS: i32 = 6;
const LASTFM_ERROR_INVALID_RESOURCE: i32 = 7;
const LASTFM_ERROR_OPERATION_FAILED: i32 = 8;
const LASTFM_ERROR_INVALID_SESSION: i32 = 9;
const LASTFM_ERROR_INVALID_API_KEY: i32 = 10;
const LASTFM_ERROR_SERVICE_OFFLINE: i32 = 11;
const LASTFM_ERROR_INVALID_SIGNATURE: i32 = 13;
const LASTFM_ERROR_UNAUTHORIZED_TOKEN: i32 = 14;
const LASTFM_ERROR_TOKEN_EXPIRED: i32 = 15;
const LASTFM_ERROR_TEMP_ERROR: i32 = 16;
const LASTFM_ERROR_SUSPENDED_API_KEY: i32 = 26;
const LASTFM_ERROR_RATE_LIMIT: i32 = 29;

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

#[derive(Deserialize)]
struct LastfmApiResponse {
    token: Option<String>,
    session: Option<LastfmSession>,
    error: Option<i32>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct LastfmTokenResponse {
    token: Option<String>,
    error: Option<i32>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct LastfmSession {
    name: String,
    key: String,
    #[allow(dead_code)]
    subscriber: Option<String>,
}

#[derive(Deserialize)]
struct LastfmScrobbleResponse {
    scrobbles: Option<LastfmScrobbles>,
    error: Option<i32>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct LastfmScrobbles {
    scrobble: Option<LastfmScrobble>,
}

#[derive(Deserialize)]
struct LastfmScrobble {
    track: Option<LastfmScrobbleTrack>,
}

#[derive(Deserialize)]
struct LastfmScrobbleTrack {
    #[serde(rename = "#text")]
    text: Option<String>,
}

pub struct LastfmClient {
    client: Client,
    api_key: String,
    shared_secret: String,
}

impl LastfmClient {
    pub fn new() -> Result<Self, String> {
        let api_key = env::var("LASTFM_API_KEY")
            .map_err(|_| "LASTFM_API_KEY environment variable not set")?;
        let shared_secret = env::var("LASTFM_SHARED_SECRET")
            .map_err(|_| "LASTFM_SHARED_SECRET environment variable not set")?;

        Ok(Self {
            client: Client::new(),
            api_key,
            shared_secret,
        })
    }

    pub async fn get_token(&self) -> Result<String, String> {
        let mut params = HashMap::new();
        params.insert("method", "auth.gettoken");
        params.insert("api_key", &self.api_key);
        params.insert("format", "json");

        // No longer adding callback URL since it's configured in Last.fm app settings

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
            let error_msg = match error {
                LASTFM_ERROR_INVALID_SERVICE => "Invalid service - This service does not exist",
                LASTFM_ERROR_INVALID_METHOD => "Invalid Method - No method with that name exists in this package",
                LASTFM_ERROR_AUTH_FAILED => "Authentication Failed - You do not have permissions to access the service",
                LASTFM_ERROR_INVALID_FORMAT => "Invalid format - This service doesn't exist in that format",
                LASTFM_ERROR_INVALID_PARAMS => "Invalid parameters - Your request is missing a required parameter",
                LASTFM_ERROR_INVALID_RESOURCE => "Invalid resource specified",
                LASTFM_ERROR_OPERATION_FAILED => "Operation failed - Something else went wrong",
                LASTFM_ERROR_INVALID_SESSION => "Invalid session key - Please re-authenticate",
                LASTFM_ERROR_INVALID_API_KEY => "Invalid API key - You must be granted a valid key by last.fm",
                LASTFM_ERROR_SERVICE_OFFLINE => "Service Offline - This service is temporarily offline. Please try again later",
                LASTFM_ERROR_INVALID_SIGNATURE => "Invalid method signature supplied",
                LASTFM_ERROR_TEMP_ERROR => "There was a temporary error processing your request. Please try again",
                LASTFM_ERROR_SUSPENDED_API_KEY => "Suspended API key - Access for your account has been suspended, please contact Last.fm",
                LASTFM_ERROR_RATE_LIMIT => "Rate limit exceeded - Your IP has made too many requests in a short period",
                _ => "Unknown error",
            };
            return Err(format!("Last.fm API error {}: {} - {}", error, error_msg, api_response.message.unwrap_or_default()));
        }

        api_response.token.ok_or_else(|| "No token in response".to_string())
    }

    pub async fn get_session(&self, token: &str) -> Result<(String, String), String> {
        // Validate token format - tokens should not be empty
        if token.trim().is_empty() {
            return Err("Invalid token: token cannot be empty".to_string());
        }

        let mut params = HashMap::new();
        params.insert("method", "auth.getsession");
        params.insert("api_key", &self.api_key);
        params.insert("token", token);
        params.insert("format", "json");

        let signature = self.generate_signature(&params);
        params.insert("api_sig", &signature);

        debug!("Creating Last.fm session for token: {}", &token[..std::cmp::min(8, token.len())]);

        let response = self.client
            .get(LASTFM_API_URL)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let api_response: LastfmApiResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

        if let Some(error) = api_response.error {
            let error_msg = match error {
                LASTFM_ERROR_AUTH_FAILED => "Authentication Failed - The token has not been authorized by the user",
                LASTFM_ERROR_UNAUTHORIZED_TOKEN => "Unauthorized Token - This token has not been authorized",
                LASTFM_ERROR_TOKEN_EXPIRED => "Token has expired",
                _ => "Unknown authentication error",
            };
            return Err(format!("Last.fm API error {}: {} - {}", error, error_msg, api_response.message.unwrap_or_default()));
        }

        let session = api_response.session.ok_or_else(|| "No session in response".to_string())?;

        debug!("Successfully created session for user: {}", session.name);
        Ok((session.key, session.name))
    }

    pub async fn scrobble_track(&self, session_key: &str, track: &track::Model, timestamp: i64, album_artist: Option<&str>) -> Result<Option<String>, String> {
        // Validate session key format
        if !self.validate_session_key(session_key) {
            return Err("Invalid session key format".to_string());
        }

        // Validate required track data
        if track.artist.trim().is_empty() || track.title.trim().is_empty() {
            return Err("Track must have both artist and title".to_string());
        }

        let timestamp_str = timestamp.to_string();
        let track_number_str = track.track_number.map(|n| n.to_string());
        let duration_str = track.duration_seconds.to_string();

        let mut params = HashMap::new();
        params.insert("method", "track.scrobble");
        params.insert("api_key", &self.api_key);
        params.insert("sk", session_key);
        params.insert("artist", &track.artist);
        params.insert("track", &track.title);
        params.insert("timestamp", &timestamp_str);
        params.insert("album", &track.album);

        if let Some(album_artist) = album_artist {
            params.insert("albumArtist", album_artist);
        } else if !track.album_artist.is_empty() {
            params.insert("albumArtist", &track.album_artist);
        }

        if let Some(ref track_number_str) = track_number_str {
            params.insert("trackNumber", track_number_str);
        }

        if track.duration_seconds > 0 {
            params.insert("duration", &duration_str);
        }

        params.insert("format", "json");

        let signature = self.generate_signature(&params);
        params.insert("api_sig", &signature);

        debug!("Scrobbling track: {} - {} (timestamp: {})", track.artist, track.title, timestamp);

        let response = self.client
            .post(LASTFM_API_URL)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let api_response: LastfmScrobbleResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

        if let Some(error) = api_response.error {
            return Err(format!("Last.fm API error {}: {}", error, api_response.message.unwrap_or_default()));
        }

        // Extract scrobble ID if available
        let scrobble_id = api_response.scrobbles
            .and_then(|s| s.scrobble)
            .and_then(|s| s.track)
            .and_then(|t| t.text);

        Ok(scrobble_id)
    }

    pub async fn update_now_playing(&self, session_key: &str, track: &track::Model) -> Result<(), String> {
        // Validate session key format
        if !self.validate_session_key(session_key) {
            return Err("Invalid session key format".to_string());
        }

        // Validate required track data
        if track.artist.trim().is_empty() || track.title.trim().is_empty() {
            return Err("Track must have both artist and title".to_string());
        }

        let track_number_str = track.track_number.map(|n| n.to_string());
        let duration_str = track.duration_seconds.to_string();

        let mut params = HashMap::new();
        params.insert("method", "track.updateNowPlaying");
        params.insert("api_key", &self.api_key);
        params.insert("sk", session_key);
        params.insert("artist", &track.artist);
        params.insert("track", &track.title);
        params.insert("album", &track.album);

        if !track.album_artist.is_empty() {
            params.insert("albumArtist", &track.album_artist);
        }

        if let Some(ref track_number_str) = track_number_str {
            params.insert("trackNumber", track_number_str);
        }

        if track.duration_seconds > 0 {
            params.insert("duration", &duration_str);
        }

        params.insert("format", "json");

        let signature = self.generate_signature(&params);
        params.insert("api_sig", &signature);

        debug!("Updating now playing: {} - {}", track.artist, track.title);

        let response = self.client
            .post(LASTFM_API_URL)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let api_response: LastfmApiResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

        if let Some(error) = api_response.error {
            return Err(format!("Last.fm API error {}: {}", error, api_response.message.unwrap_or_default()));
        }

        Ok(())
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
    }    /// Validates if a session key is in the correct format
    pub fn validate_session_key(&self, session_key: &str) -> bool {
        // Session keys should be non-empty strings
        !session_key.trim().is_empty() && session_key.len() >= 10
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
        signature_string.push_str(&self.shared_secret);

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
