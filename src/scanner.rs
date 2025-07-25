use std::path::PathBuf;
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::Path;
use tokio::sync::{mpsc, Semaphore};
use std::sync::Arc;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
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
    pub path_batch_size: usize,  // Number of paths to check in each DB query
    pub use_optimized_scanning: bool,  // Use new optimized scanning approach
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            music_path: "/mnt/shucked/Music".to_string(),
            show_progress: true,
            batch_size: 100,        // Smaller batches for more consistent performance
            path_batch_size: 2500,  // Balanced for good query efficiency
            use_optimized_scanning: true,
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

    // Count total files for progress estimation
    let total_files = count_files(path);

    // Create MultiProgress container for better log handling
    let multi = MultiProgress::new();

    let progress = if config.show_progress {
        let pb = multi.add(ProgressBar::new(total_files));
        pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg} ({eta} @ {per_sec})"
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  ")
        );
        pb.set_message("files processed");
        pb
    } else {
        let pb = multi.add(ProgressBar::new(total_files));
        pb.set_style(ProgressStyle::with_template("Scanning... {pos}/{len}")
            .unwrap());
        pb
    };

    // Temporarily allow initial log messages to display cleanly
    info!("Progress bar initialized, starting scan operations...");

    let (tx, mut rx) = mpsc::channel(2000);  // Balanced channel buffer for improved performance
    let tx_clone = tx.clone();

    // Use optimized scanning approach
    let scan_handle = if config.use_optimized_scanning {
        let db_clone = db.clone();
        tokio::spawn(async move {
            scan_dir_optimized(&path_buf, &tx_clone, &db_clone, config.path_batch_size).await;
        })
    } else {
        // Fallback to original approach
        let modified_by_path = get_all_modified_by_path(db).await?;
        tokio::spawn(async move {
            scan_dir(&path_buf, &tx_clone, &modified_by_path).await;
        })
    };

    drop(tx);

    let mut stack: Vec<track::ActiveModel> = Vec::with_capacity(config.batch_size);
    let mut tracks_processed = 0;

    while let Some(track) = rx.recv().await {
        stack.push(track);
        tracks_processed += 1;

        if stack.len() >= config.batch_size {
            upsert_tracks(&stack, db).await?;
            // Update progress after successful database operation
            progress.inc(stack.len() as u64);
            stack.clear();
        }
    }

    if !stack.is_empty() {
        upsert_tracks(&stack, db).await?;
        // Update progress after final database operation
        progress.inc(stack.len() as u64);
        stack.clear();
    }

    // Update progress for any remaining files that didn't need processing
    let remaining_files = total_files.saturating_sub(tracks_processed as u64);
    if remaining_files > 0 {
        progress.inc(remaining_files);
    }

    progress.finish_with_message("Scan completed");

    if let Err(e) = scan_handle.await {
        error!("Scan task failed: {:?}", e);
        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Scan task failed: {:?}", e))));
    }

    let scan_result = ScanResult {
        files_scanned: total_files,
        tracks_processed,
    };

    // Log completion with database count
    use entity::prelude::Track;
    use sea_orm::{EntityTrait, PaginatorTrait};
    let total_tracks_in_db = Track::find().count(db).await.unwrap_or(0);

    // Final logging
    info!("Scan completed: {} files scanned, {} tracks processed, {} tracks in database",
          scan_result.files_scanned, scan_result.tracks_processed, total_tracks_in_db);

    Ok(scan_result)
}

pub async fn get_all_modified_by_path(db: &DatabaseConnection) -> Result<HashMap<String, chrono::DateTime<chrono::Utc>>, sea_orm::DbErr> {
    use entity::prelude::Track;
    use sea_orm::EntityTrait;

    info!("Loading existing track metadata from database...");
    let tracks = Track::find().all(db).await?;

    let mut result = HashMap::new();
    for track in tracks {
        result.insert(track.path, track.modified);
    }

    info!("Loaded {} existing tracks", result.len());
    Ok(result)
}

/// Optimized version that queries database in batches instead of loading everything
pub async fn get_modified_times_for_paths(
    db: &DatabaseConnection,
    paths: &[String]
) -> Result<HashMap<String, chrono::DateTime<chrono::Utc>>, sea_orm::DbErr> {
    use entity::prelude::Track;
    use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};

    if paths.is_empty() {
        return Ok(HashMap::new());
    }

    let tracks = Track::find()
        .filter(track::Column::Path.is_in(paths.iter().cloned()))
        .all(db)
        .await?;

    let mut result = HashMap::new();
    for track in tracks {
        result.insert(track.path, track.modified);
    }

    Ok(result)
}

pub fn count_files(path: &Path) -> u64 {
    let mut count = 0;
    let entries = match path.read_dir() {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read directory {}: {:?}", path.display(), e);
            return 0;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Failed to read directory entry: {:?}", e);
                continue;
            }
        };
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
pub async fn scan_dir(path: &Path, tx: &tokio::sync::mpsc::Sender<track::ActiveModel>, modified_by_path: &HashMap<String, chrono::DateTime<chrono::Utc>>) {
    let entries = match path.read_dir() {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read directory {}: {:?}", path.display(), e);
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Failed to read directory entry: {:?}", e);
                continue;
            }
        };
        let path = entry.path();

        if path.is_dir() {
            scan_dir(&path, &tx, &modified_by_path).await;
        } else if path.is_file() {
            let metadata = match tokio::fs::metadata(&path).await {
                Ok(metadata) => metadata,
                Err(e) => {
                    error!("Failed to read metadata for {}: {:?}", path.display(), e);
                    continue;
                }
            };
            let modified: chrono::DateTime<chrono::Utc> = match metadata.modified() {
                Ok(modified) => chrono::DateTime::from(modified),
                Err(e) => {
                    error!("Failed to get modified time for {}: {:?}", path.display(), e);
                    continue;
                }
            };
            let path_str = match path.to_str() {
                Some(path_str) => path_str,
                None => {
                    error!("Failed to convert path to string: {}", path.display());
                    continue;
                }
            };
            let modified_last_scan = match modified_by_path.get(path_str) {
                Some(modified) => modified.clone(),
                None => chrono::DateTime::from(std::time::SystemTime::UNIX_EPOCH)
            };
            if modified > modified_last_scan {
                // File has been modified since last scan
                let tx = tx.clone();
                tokio::spawn(async move {
                    let track = read_tags(&path, &metadata).await;
                    match track {
                        Ok(track) => {
                            if let Err(e) = tx.send(track).await {
                                error!("Failed to send track data through channel: {:?}", e);
                            }
                        },
                        Err(e) => {
                            // Only care about supported files
                            if lofty::file::FileType::from_path(&path).is_some() {
                                error!("Error reading tags: {:?}", e);
                                if let Some(path_str) = path.to_str() {
                                    error!("In path: {}", path_str);
                                } else {
                                    error!("In path: {:?}", path);
                                }
                            }
                        }
                    }
                });
            }
            // Progress will be updated after database upsert, not here
        }
    }
}

/// Optimized scanning that processes files in batches to avoid loading entire DB into memory
#[async_recursion]
pub async fn scan_dir_optimized(
    path: &Path,
    tx: &tokio::sync::mpsc::Sender<track::ActiveModel>,
    db: &DatabaseConnection,
    batch_size: usize,
) {
    // Collect all file paths first
    let mut file_paths = Vec::new();
    collect_file_paths(path, &mut file_paths);

    // Create a semaphore to limit concurrent file processing
    let semaphore = Arc::new(Semaphore::new(50)); // Limit to 50 concurrent file operations

    // Process files in batches
    for chunk in file_paths.chunks(batch_size) {
        let paths: Vec<String> = chunk.iter()
            .filter_map(|p| p.to_str().map(|s| s.to_string()))
            .collect();

        // Query database for this batch of paths
        let modified_by_path = match get_modified_times_for_paths(db, &paths).await {
            Ok(map) => map,
            Err(e) => {
                error!("Failed to query modified times from database: {:?}", e);
                continue;
            }
        };

        // Process each file in this batch
        for file_path in chunk {
            let metadata = match tokio::fs::metadata(&file_path).await {
                Ok(metadata) => metadata,
                Err(e) => {
                    error!("Failed to read metadata for {}: {:?}", file_path.display(), e);
                    continue;
                }
            };

            let modified: chrono::DateTime<chrono::Utc> = match metadata.modified() {
                Ok(modified) => chrono::DateTime::from(modified),
                Err(e) => {
                    error!("Failed to get modified time for {}: {:?}", file_path.display(), e);
                    continue;
                }
            };

            let path_str = match file_path.to_str() {
                Some(path_str) => path_str,
                None => {
                    error!("Failed to convert path to string: {}", file_path.display());
                    continue;
                }
            };

            let modified_last_scan = modified_by_path.get(path_str)
                .cloned()
                .unwrap_or_else(|| chrono::DateTime::from(std::time::SystemTime::UNIX_EPOCH));

            if modified > modified_last_scan {
                // File has been modified since last scan - spawn async task for processing
                let tx = tx.clone();
                let file_path = file_path.clone();
                let semaphore_permit = semaphore.clone();

                tokio::spawn(async move {
                    // Acquire a permit to limit concurrent operations
                    let _permit = semaphore_permit.acquire().await.unwrap();

                    let track = read_tags(&file_path, &metadata).await;
                    match track {
                        Ok(track) => {
                            if let Err(e) = tx.send(track).await {
                                error!("Failed to send track data through channel: {:?}", e);
                            }
                        },
                        Err(e) => {
                            // Only care about supported files
                            if lofty::file::FileType::from_path(&file_path).is_some() {
                                error!("Error reading tags: {:?}", e);
                                if let Some(path_str) = file_path.to_str() {
                                    error!("In path: {}", path_str);
                                } else {
                                    error!("In path: {:?}", file_path);
                                }
                            }
                        }
                    }
                    // Permit is automatically released when _permit is dropped
                });
            }
        }
    }
}

/// Recursively collect all file paths
fn collect_file_paths(path: &Path, file_paths: &mut Vec<PathBuf>) {
    let entries = match path.read_dir() {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read directory {}: {:?}", path.display(), e);
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Failed to read directory entry: {:?}", e);
                continue;
            }
        };
        let path = entry.path();

        if path.is_dir() {
            collect_file_paths(&path, file_paths);
        } else if path.is_file() {
            file_paths.push(path);
        }
    }
}

pub async fn upsert_tracks(tracks: &Vec<track::ActiveModel>, db: &DatabaseConnection) -> Result<sea_orm::InsertResult<track::ActiveModel>, sea_orm::DbErr> {
    use sea_orm::EntityTrait;

    if tracks.is_empty() {
        return Ok(sea_orm::InsertResult { last_insert_id: 0 });
    }

    // Use optimized bulk upsert with proper conflict resolution
    let on_conflict = sea_query::OnConflict::column(track::Column::Path)
        .update_columns(vec![
            track::Column::Extension,
            track::Column::Title,
            track::Column::Artist,
            track::Column::Album,
            track::Column::DiscNumber,
            track::Column::TrackNumber,
            track::Column::Year,
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

    // Log only every 5th batch to reduce noise
    if tracks.len() >= 500 || tracks.len() % 500 == 0 {
        info!("Upserting batch of {} tracks", tracks.len());
    }

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

    // Extract disc number - try multiple approaches
    let disc_number = tag.get_string(&ItemKey::DiscNumber)
        .and_then(|s| s.parse::<i32>().ok())
        .or_else(|| {
            // Try to parse from DiscTotal format like "1/2"
            tag.get_string(&ItemKey::DiscNumber)
                .and_then(|s| s.split('/').next()?.parse::<i32>().ok())
        })
        .or_else(|| {
            // Try alternative tag names from all_tags
            all_tags.get("DISCNUMBER")
                .or_else(|| all_tags.get("DISC"))
                .or_else(|| all_tags.get("TPOS"))
                .and_then(|s| s.split('/').next()?.parse::<i32>().ok())
        });

    // Extract track number - try multiple approaches
    let track_number = tag.track()
        .map(|t| t as i32)
        .or_else(|| {
            // Try to parse from TrackTotal format like "1/12"
            tag.get_string(&ItemKey::TrackNumber)
                .and_then(|s| s.split('/').next()?.parse::<i32>().ok())
        })
        .or_else(|| {
            // Try alternative tag names from all_tags
            all_tags.get("TRACKNUMBER")
                .or_else(|| all_tags.get("TRACK"))
                .or_else(|| all_tags.get("TRCK"))
                .and_then(|s| s.split('/').next()?.parse::<i32>().ok())
        });

    // Extract year - try multiple approaches
    let year = tag.year()
        .map(|y| y as i32)
        .or_else(|| {
            // Try alternative tag names from all_tags
            all_tags.get("DATE")
                .or_else(|| all_tags.get("YEAR"))
                .or_else(|| all_tags.get("TDRC"))
                .or_else(|| all_tags.get("TYER"))
                .and_then(|s| {
                    // Parse year from various date formats
                    if s.len() >= 4 {
                        s[0..4].parse::<i32>().ok()
                    } else {
                        s.parse::<i32>().ok()
                    }
                })
        });

    Ok(track::ActiveModel {
        id: NotSet,
        path: Set(path.to_str().unwrap_or("").to_string()),
        extension: Set(path.extension().unwrap_or_default().to_str().unwrap_or("").to_string()),
        title: Set(tag.title().as_deref().unwrap_or("").to_string()),
        artist: Set(tag.artist().as_deref().unwrap_or("").to_string()),
        album: Set(tag.album().as_deref().unwrap_or("").to_string()),
        disc_number: Set(disc_number),
        track_number: Set(track_number),
        year: Set(year),
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
        tags: Set(serde_json::to_value(all_tags).unwrap_or_else(|e| {
            error!("Failed to serialize tags to JSON: {:?}", e);
            serde_json::Value::Object(serde_json::Map::new())
        })),
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
