use std::collections::HashMap;

#[derive(Debug)]
pub struct Track {
    pub uuid: String,
    pub path: String,
    pub extension: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub album_artist: String,
    pub publisher: String,
    pub catalog_number: String,
    pub duration_seconds: u64,
    pub audio_bitrate: u32,
    pub overall_bitrate: u32,
    pub sample_rate: u32,
    pub bit_depth: u8,
    pub channels: u8,
    pub tags: HashMap<String, String>,
    pub created: chrono::DateTime<chrono::Utc>,
    pub modified: chrono::DateTime<chrono::Utc>,
}
