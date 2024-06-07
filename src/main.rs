use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use lofty::prelude::*;
use lofty::probe::Probe;
use log::{debug, error};
use ml_progress::progress_builder;
use regex::Regex;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, DbErr, EntityTrait, NotSet, PaginatorTrait, QueryFilter};
use sea_orm::ActiveValue::{Set, Unchanged};
use uuid::Uuid;

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

    let path = std::path::Path::new("E:\\Music");

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

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        scan_dir(path, &tx, &modified_by_path, &progress);
        progress.finish();
    });

    for track in rx {
        // TODO: Improve performance, currently takes a while after an initial scan
        let result = upsert_track(&track, &db).await;
        match result {
            Ok(_) => {
                debug!("Inserted/updated track: {}", path.to_str().unwrap_or(""));
            },
            Err(e) => {
                error!("Error inserting {}: {:?}", path.to_str().unwrap_or(""), e);
            }
        }
    }

    println!("{} tracks are in the database", Track::find().count(&db).await?);
    Ok(())
}

async fn get_all_modified_by_path(db: &DatabaseConnection) -> Result<HashMap<String, chrono::DateTime<chrono::Utc>>, DbErr> {
    let tracks: Vec<track::Model> = Track::find().all(db).await?;

    let mut result = HashMap::new();
    for track in tracks {
        result.insert(track.path, track.modified);
    }

    Ok(result)
}

fn read_tags(path: &std::path::Path) -> Result<track::ActiveModel, OngakuError> {
    if !path.is_file() {
        return Err(OngakuError::NotFile);
    }

    // stat file
    let metadata = std::fs::metadata(path).unwrap();
    let created = chrono::DateTime::from(metadata.created().unwrap());
    let modified = chrono::DateTime::from(metadata.modified().unwrap());

    let probe = Probe::open(path);
    if let Err(e) = probe {
        return Err(OngakuError::ReadTag(e));
    }
    let tagged_file_result = probe.unwrap().read();
    if let Err(e) = tagged_file_result {
        return Err(OngakuError::ReadTag(e));
    }
    let tagged_file = tagged_file_result.unwrap();

    let tag_option = match tagged_file.primary_tag() {
        Some(primary_tag) => Option::from(primary_tag),
        None => tagged_file.first_tag(),
    };
    if tag_option.is_none() {
        return Err(OngakuError::ReadTagNoTags);
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
        uuid: NotSet,
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

async fn upsert_track(track: &track::ActiveModel, db: &DatabaseConnection) -> Result<track::Model, DbErr> {
    let existing_track = Track::find().filter(track::Column::Path.eq(track.path.clone().unwrap())).one(db).await?;
    if existing_track.is_none() {
        let mut track = track.clone();
        track.uuid = Set(Uuid::new_v4().to_string());
        track.insert(db).await
    } else {
        let existing_track = existing_track.unwrap();
        let mut track = track.clone();
        track.id = Unchanged(existing_track.id);
        track.uuid = Unchanged(existing_track.uuid);
        track.update(db).await
    }
}

fn scan_dir(path: &std::path::Path, tx: &Sender<track::ActiveModel>, modified_by_path: &HashMap<String, chrono::DateTime<chrono::Utc>>, progress: &ml_progress::Progress) {
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            scan_dir(&path, &tx, &modified_by_path, progress);
        } else {
            let metadata = std::fs::metadata(&path).unwrap();
            let modified: chrono::DateTime<chrono::Utc> = chrono::DateTime::from(metadata.modified().unwrap());
            let modified_last_scan = match modified_by_path.get(path.to_str().unwrap()) {
                Some(modified) => modified.clone(),
                None => chrono::DateTime::from(std::time::SystemTime::UNIX_EPOCH)
            };
            if modified > modified_last_scan {
                // File has been modified since last scan
                match read_tags(&path) {
                    Ok(track) => tx.send(track).unwrap(),
                    Err(e) => {
                        // Only care about supported files
                        if lofty::file::FileType::from_path(&path).is_some() {
                            error!("Error reading tags: {:?}", e)
                        }
                    }
                }
            }
            progress.inc(1);
        }
    }
}

fn count_files(path: &std::path::Path) -> u64 {
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
enum OngakuError {
    ReadTag(lofty::error::LoftyError),
    ReadTagNoTags,
    NotFile,
}
