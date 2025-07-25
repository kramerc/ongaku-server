use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::Metadata;
use std::path::Path;
use std::time::Duration;

use async_recursion::async_recursion;
use lofty::error::LoftyError;
use lofty::prelude::*;
use lofty::probe::Probe;
use log::error;
use ml_progress::progress_builder;
use regex::Regex;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr, EntityTrait, InsertResult, NotSet, PaginatorTrait};
use sea_orm::ActiveValue::Set;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

use entity::prelude::Track;
use entity::track;
use migration::{Migrator, MigratorTrait};

mod logger;

#[tokio::main]
async fn main() -> Result<(), DbErr> {
    logger::init().unwrap();

    let mut opt = ConnectOptions::new("sqlite://ongaku.db?mode=rwc");
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info);
    let db: DatabaseConnection = Database::connect(opt).await?;
    Migrator::up(&db, None).await?;

    let path = Path::new("/mnt/shucked/Music");

    println!("Path: {:?}", path);
    println!("Path exists: {}", path.exists());

    let modified_by_path = get_all_modified_by_path(&db).await?;
    let count = count_files(path);
    let progress = progress_builder!(
        "[" percent "] " pos_group "/" total_group " " bar_fill " (" eta_hms " @ " speed "it/s)"
    )
        .total(Some(count))
        .thousands_separator(",")
        .build().unwrap();

    let (tx, mut rx) = mpsc::channel(100);
    let tx_clone = tx.clone();
    let scan_handle = tokio::spawn(async move {
        scan_dir(path, &tx_clone, &modified_by_path, &progress).await;
        progress.finish();
    });

    drop(tx);

    let mut stack: Vec<track::ActiveModel> = Vec::new();
    while let Some(track) = rx.recv().await {
        stack.push(track);

        if stack.len() >= 100 {
            upsert_tracks(&stack, &db).await?;
            stack.clear();
        }
    }

    if !stack.is_empty() {
        upsert_tracks(&stack, &db).await?;
        stack.clear();
    }

    scan_handle.await.unwrap();

    println!("{} tracks are in the database", Track::find().count(&db).await?);
    Ok(())
}

async fn get_all_modified_by_path(db: &DatabaseConnection) -> Result<HashMap<String, chrono::DateTime<chrono::Utc>>, DbErr> {
    let tracks = Track::find().all(db).await?;

    let mut result = HashMap::new();
    for track in tracks {
        result.insert(track.path, track.modified);
    }

    Ok(result)
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

async fn upsert_tracks(tracks: &Vec<track::ActiveModel>, db: &DatabaseConnection) -> Result<InsertResult<track::ActiveModel>, DbErr> {
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

#[async_recursion]
async fn scan_dir(path: &Path, tx: &Sender<track::ActiveModel>, modified_by_path: &HashMap<String, chrono::DateTime<chrono::Utc>>, progress: &ml_progress::Progress) {
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

fn count_files(path: &Path) -> u64 {
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
