use std::path::PathBuf;
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::Path;
use tokio::sync::mpsc;
use ml_progress::progress_builder;
use log::{info, error};
use async_recursion::async_recursion;
use regex::Regex;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::error::LoftyError;
use sea_orm::ActiveValue::Set;
use sea_orm::{NotSet, DatabaseConnection};

use entity::track;

pub struct ScanConfig {
    pub music_path: String,
    pub show_progress: bool,
    pub batch_size: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            music_path: "/mnt/shucked/Music".to_string(),
            show_progress: true,
            batch_size: 100,
        }
    }
}

pub struct ScanResult {
    pub files_scanned: u64,
    pub tracks_processed: usize,
}

pub async fn scan_music_library(
    db: &DatabaseConnection,
    config: ScanConfig,
) -> Result<ScanResult, Box<dyn std::error::Error + Send + Sync>> {
    let path_buf = PathBuf::from(&config.music_path);
    let path = path_buf.as_path();

    info!("Starting music library scan at: {}", config.music_path);

    let modified_by_path = get_all_modified_by_path(db).await?;
    let total_files = count_files(path);

    let progress = if config.show_progress {
        progress_builder!(
            "[" percent "] " pos_group "/" total_group " " bar_fill " (" eta_hms " @ " speed "it/s)"
        )
            .total(Some(total_files))
            .thousands_separator(",")
            .build().unwrap()
    } else {
        // Create a minimal progress bar for background scans
        progress_builder!("Scanning...")
            .total(Some(total_files))
            .build().unwrap()
    };

    let (tx, mut rx) = mpsc::channel(100);
    let tx_clone = tx.clone();

    // Start the scanning process
    let scan_handle = tokio::spawn(async move {
        scan_dir(&path_buf, &tx_clone, &modified_by_path, &progress).await;
        progress.finish();
    });

    drop(tx);

    let mut stack: Vec<track::ActiveModel> = Vec::new();
    let mut tracks_processed = 0;

    while let Some(track) = rx.recv().await {
        stack.push(track);
        tracks_processed += 1;

        if stack.len() >= config.batch_size {
            upsert_tracks(&stack, db).await?;
            stack.clear();
        }
    }

    if !stack.is_empty() {
        upsert_tracks(&stack, db).await?;
        stack.clear();
    }

    scan_handle.await.unwrap();

    let scan_result = ScanResult {
        files_scanned: total_files,
        tracks_processed,
    };

    // Log completion with database count
    use entity::prelude::Track;
    use sea_orm::{EntityTrait, PaginatorTrait};
    let total_tracks_in_db = Track::find().count(db).await.unwrap_or(0);

    info!("Scan completed: {} files scanned, {} tracks processed, {} tracks in database",
          scan_result.files_scanned, scan_result.tracks_processed, total_tracks_in_db);

    Ok(scan_result)
}

pub async fn get_all_modified_by_path(db: &DatabaseConnection) -> Result<HashMap<String, chrono::DateTime<chrono::Utc>>, sea_orm::DbErr> {
    use entity::prelude::Track;
    use sea_orm::EntityTrait;

    let tracks = Track::find().all(db).await?;

    let mut result = HashMap::new();
    for track in tracks {
        result.insert(track.path, track.modified);
    }

    Ok(result)
}

pub fn count_files(path: &Path) -> u64 {
    let mut count = 0;
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            count += count_files(&path);
        } else {
            count += 1;
        }
    }
    count
}

#[async_recursion]
pub async fn scan_dir(path: &Path, tx: &tokio::sync::mpsc::Sender<track::ActiveModel>, modified_by_path: &HashMap<String, chrono::DateTime<chrono::Utc>>, progress: &ml_progress::Progress) {
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            scan_dir(&path, &tx, &modified_by_path, progress).await;
        } else if path.is_file() {
            let metadata = tokio::fs::metadata(&path).await.unwrap();
            let modified: chrono::DateTime<chrono::Utc> = chrono::DateTime::from(metadata.modified().unwrap());
            let modified_last_scan = match modified_by_path.get(path.to_str().unwrap()) {
                Some(modified) => modified.clone(),
                None => chrono::DateTime::from(std::time::SystemTime::UNIX_EPOCH)
            };
            if modified > modified_last_scan {
                // File has been modified since last scan
                let tx = tx.clone();
                tokio::spawn(async move {
                    let track = read_tags(&path, &metadata).await;
                    match track {
                        Ok(track) => tx.send(track).await.unwrap(),
                        Err(e) => {
                            // Only care about supported files
                            if lofty::file::FileType::from_path(&path).is_some() {
                                error!("Error reading tags: {:?}", e);
                                error!("In path: {}", path.to_str().unwrap());
                            }
                        }
                    }
                });
            }
            progress.inc(1);
        }
    }
}

pub async fn upsert_tracks(tracks: &Vec<track::ActiveModel>, db: &DatabaseConnection) -> Result<sea_orm::InsertResult<track::ActiveModel>, sea_orm::DbErr> {
    use sea_orm::EntityTrait;

    let on_conflict = sea_query::OnConflict::column(track::Column::Path)
        .update_columns(vec![
            track::Column::Extension,
            track::Column::Title,
            track::Column::Artist,
            track::Column::Album,
            track::Column::Genre,
            track::Column::AlbumArtist,
            track::Column::Publisher,
            track::Column::CatalogNumber,
            track::Column::DurationSeconds,
            track::Column::AudioBitrate,
            track::Column::OverallBitrate,
            track::Column::SampleRate,
            track::Column::BitDepth,
            track::Column::Channels,
            track::Column::Tags,
            track::Column::Modified,
        ])
        .to_owned();
    track::Entity::insert_many(tracks.clone())
        .on_conflict(on_conflict)
        .exec(db)
        .await
}

async fn read_tags(path: &Path, metadata: &Metadata) -> Result<track::ActiveModel, TagError> {
    let created = chrono::DateTime::from(metadata.created().unwrap());
    let modified = chrono::DateTime::from(metadata.modified().unwrap());

    let probe = Probe::open(path)?;
    let tagged_file = probe.read()?;

    let tag_option = match tagged_file.primary_tag() {
        Some(primary_tag) => Option::from(primary_tag),
        None => tagged_file.first_tag(),
    };
    if tag_option.is_none() {
        return Err(TagError::NoTags);
    }
    let tag = tag_option.unwrap();

    let properties = tagged_file.properties();
    let duration = properties.duration();

    let mut all_tags = HashMap::new();
    for item in tag.items() {
        let key = format!("{:?}", item.key());
        let re = Regex::new(r#"Unknown\("(.+)"\)"#).unwrap();
        let key = re.replace_all(&key, "$1").to_string();
        let value = item.value().clone().into_string().unwrap_or("".to_string());
        all_tags.insert(key, value);
    }

    Ok(track::ActiveModel {
        id: NotSet,
        path: Set(path.to_str().unwrap_or("").to_string()),
        extension: Set(path.extension().unwrap_or_default().to_str().unwrap_or("").to_string()),
        title: Set(tag.title().as_deref().unwrap_or("").to_string()),
        artist: Set(tag.artist().as_deref().unwrap_or("").to_string()),
        album: Set(tag.album().as_deref().unwrap_or("").to_string()),
        genre: Set(tag.genre().as_deref().unwrap_or("").to_string()),
        album_artist: Set(tag.get_string(&ItemKey::AlbumArtist).unwrap_or("").to_string()),
        publisher: Set(tag.get_string(&ItemKey::Publisher).unwrap_or("").to_string()),
        catalog_number: Set(tag.get_string(&ItemKey::CatalogNumber).unwrap_or("").to_string()),
        duration_seconds: Set(duration.as_secs() as i32),
        audio_bitrate: Set(properties.audio_bitrate().unwrap_or(0) as i32),
        overall_bitrate: Set(properties.overall_bitrate().unwrap_or(0) as i32),
        sample_rate: Set(properties.sample_rate().unwrap_or(0) as i32),
        bit_depth: Set(properties.bit_depth().unwrap_or(0) as i32),
        channels: Set(properties.channels().unwrap_or(0) as i32),
        tags: Set(serde_json::to_string(&all_tags).unwrap()),
        created: Set(created),
        modified: Set(modified),
    })
}

#[derive(Debug)]
#[allow(dead_code)]
enum TagError {
    ReadTag(LoftyError),
    NoTags,
}

impl From<LoftyError> for TagError {
    fn from(e: LoftyError) -> Self {
        TagError::ReadTag(e)
    }
}
