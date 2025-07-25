use std::collections::HashMap;
use std::env;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use log::{debug, error, warn};
use md5;

use entity::prelude::Track;
use entity::track;
use sea_orm::EntityTrait;

use crate::api::AppState;

const LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

#[derive(Serialize)]
pub struct LastfmAuthResponse {
    pub auth_url: String,
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

        let signature = self.generate_signature(&params);
        params.insert("api_sig", &signature);

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
            return Err(format!("Last.fm API error {}: {}", error, api_response.message.unwrap_or_default()));
        }

        api_response.token.ok_or_else(|| "No token in response".to_string())
    }

    pub async fn get_session(&self, token: &str) -> Result<(String, String), String> {
        let mut params = HashMap::new();
        params.insert("method", "auth.getsession");
        params.insert("api_key", &self.api_key);
        params.insert("token", token);
        params.insert("format", "json");

        let signature = self.generate_signature(&params);
        params.insert("api_sig", &signature);

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
            return Err(format!("Last.fm API error {}: {}", error, api_response.message.unwrap_or_default()));
        }

        let session = api_response.session.ok_or_else(|| "No session in response".to_string())?;
        Ok((session.key, session.name))
    }

    pub async fn scrobble_track(&self, session_key: &str, track: &track::Model, timestamp: i64, album_artist: Option<&str>) -> Result<Option<String>, String> {
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

    fn generate_signature(&self, params: &HashMap<&str, &str>) -> String {
        let mut sorted_params: Vec<_> = params.iter()
            .filter(|(key, _)| **key != "format" && **key != "callback")
            .collect();
        sorted_params.sort_by_key(|(key, _)| *key);

        let mut signature_string = String::new();
        for (key, value) in sorted_params {
            signature_string.push_str(key);
            signature_string.push_str(value);
        }
        signature_string.push_str(&self.shared_secret);

        format!("{:x}", md5::compute(signature_string.as_bytes()))
    }
}

// API handlers

pub async fn get_auth_url(
    State(_state): State<AppState>,
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

    let auth_url = format!("https://www.last.fm/api/auth/?api_key={}&token={}",
                          client.api_key, token);

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
