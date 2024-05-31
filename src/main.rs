use std::collections::HashMap;
use std::fmt::Debug;

use lofty::prelude::*;
use lofty::probe::Probe;
use log::{error, info};
use ml_progress::{progress_builder};
use regex::Regex;
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::library::Track;

mod library;
mod logger;

fn main() {
    logger::init().unwrap();

    let connection = Connection::open("ongaku.db").unwrap();
    let query = "CREATE TABLE IF NOT EXISTS tracks (
        id INTEGER PRIMARY KEY,
        uuid TEXT NOT NULL UNIQUE,
        path TEXT NOT NULL,
        extension TEXT NOT NULL,
        title TEXT NOT NULL,
        artist TEXT NOT NULL,
        album TEXT NOT NULL,
        genre TEXT NOT NULL,
        album_artist TEXT NOT NULL,
        publisher TEXT NOT NULL,
        catalog_number TEXT NOT NULL,
        duration_seconds INTEGER NOT NULL,
        audio_bitrate INTEGER NOT NULL,
        overall_bitrate INTEGER NOT NULL,
        sample_rate INTEGER NOT NULL,
        bit_depth INTEGER NOT NULL,
        channels INTEGER NOT NULL,
        tags TEXT NOT NULL,
        created TEXT NOT NULL,
        modified TEXT NOT NULL
    )";
    connection.execute(query, ()).expect("Failed to create table");

    let path = std::path::Path::new("E:\\Music");

    println!("Path: {:?}", path);
    println!("Path exists: {}", path.exists());

    let count = count_files(path);
    // let progress = progress!(count).unwrap();
    let progress = progress_builder!(
        "[" percent "] " pos_group "/" total_group " " bar_fill " (" eta_hms " @ " speed "it/s)"
    )
        .total(Some(count))
        .thousands_separator(",")
        .build().unwrap();
    read_files(path, &connection, &progress);
    progress.finish();
    println!("{} tracks are in the database", count_tracks(&connection));
}

fn get_uuid_for_track(path: &std::path::Path, connection: &Connection) -> String {
    let select_query = "SELECT uuid FROM tracks WHERE path = ?1";
    let mut stmt = connection.prepare(select_query).unwrap();

    stmt.query_row([path.to_str().unwrap()], |row| {
        row.get(0)
    }).unwrap_or(Uuid::new_v4().to_string())
}

fn read_tags(path: &std::path::Path, connection: &Connection) -> Result<Option<Track>, OngakuError> {
    if !path.is_file() {
        return Err(OngakuError::NotFile());
    }

    // stat file
    let metadata = std::fs::metadata(path).unwrap();
    let created = chrono::DateTime::from(metadata.created().unwrap());
    let modified = chrono::DateTime::from(metadata.modified().unwrap());

    let existing_track = get_track(path, &connection).unwrap();
    match existing_track {
        Some(track) => {
            if track.modified <= modified {
                // File has not been modified since last scan
                return Ok(None);
            }
        },
        None => {}
    }

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

    Ok(Some(Track {
        uuid: get_uuid_for_track(path, &connection),
        path: path.to_str().unwrap_or("").to_string(),
        extension: path.extension().unwrap_or_default().to_str().unwrap_or("").to_string(),
        title: tag.title().as_deref().unwrap_or("").to_string(),
        artist: tag.artist().as_deref().unwrap_or("").to_string(),
        album: tag.album().as_deref().unwrap_or("").to_string(),
        genre: tag.genre().as_deref().unwrap_or("").to_string(),
        album_artist: tag.get_string(&ItemKey::AlbumArtist).unwrap_or("").to_string(),
        publisher: tag.get_string(&ItemKey::Publisher).unwrap_or("").to_string(),
        catalog_number: tag.get_string(&ItemKey::CatalogNumber).unwrap_or("").to_string(),
        duration_seconds: duration.as_secs(),
        audio_bitrate: properties.audio_bitrate().unwrap_or(0),
        overall_bitrate: properties.overall_bitrate().unwrap_or(0),
        sample_rate: properties.sample_rate().unwrap_or(0),
        bit_depth: properties.bit_depth().unwrap_or(0),
        channels: properties.channels().unwrap_or(0),
        tags: all_tags,
        created,
        modified,
    }))
}

fn insert_track(track: &library::Track, connection: &Connection) -> Result<(), rusqlite::Error> {
    let insert_query = "INSERT INTO tracks (
        uuid,
        path,
        extension,
        title,
        artist,
        album,
        genre,
        album_artist,
        publisher,
        catalog_number,
        duration_seconds,
        audio_bitrate,
        overall_bitrate,
        sample_rate,
        bit_depth,
        channels,
        tags,
        created,
        modified
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)";
    connection.execute(insert_query, params![
        &track.uuid,
        &track.path,
        &track.extension,
        &track.title,
        &track.artist,
        &track.album,
        &track.genre,
        &track.album_artist,
        &track.publisher,
        &track.catalog_number,
        &track.duration_seconds,
        &track.audio_bitrate,
        &track.overall_bitrate,
        &track.sample_rate,
        &track.bit_depth,
        &track.channels,
        &serde_json::to_string(&track.tags).unwrap(),
        &track.created,
        &track.modified,
    ]).expect("Failed to insert");

    info!("Inserted track: {}", track.uuid);
    Ok(())
}

fn update_track(track: &Track, connection: &Connection) -> Result<(), rusqlite::Error> {
    let update_query = "UPDATE tracks SET
        title = ?1,
        artist = ?2,
        album = ?3,
        genre = ?4,
        album_artist = ?5,
        publisher = ?6,
        catalog_number = ?7,
        duration_seconds = ?8,
        audio_bitrate = ?9,
        overall_bitrate = ?10,
        sample_rate = ?11,
        bit_depth = ?12,
        channels = ?13,
        tags = ?14,
        created = ?15,
        modified = ?16
        WHERE uuid = ?17";
    connection.execute(update_query, params![
        &track.title,
        &track.artist,
        &track.album,
        &track.genre,
        &track.album_artist,
        &track.publisher,
        &track.catalog_number,
        &track.duration_seconds,
        &track.audio_bitrate,
        &track.overall_bitrate,
        &track.sample_rate,
        &track.bit_depth,
        &track.channels,
        &serde_json::to_string(&track.tags).unwrap(),
        &track.created,
        &track.modified,
        &track.uuid
    ])?;

    info!("Updated track: {}", track.uuid);
    Ok(())
}

fn upsert_track(track: &Track, connection: &Connection) -> Result<(), rusqlite::Error> {
    let select_query = "SELECT COUNT(*) FROM tracks WHERE uuid = ?1";
    let mut stmt = connection.prepare(select_query)?;
    let count: i32 = stmt.query_row([&track.uuid], |row| {
        row.get(0)
    })?;

    if count == 0 {
        insert_track(&track, &connection)
    } else {
        update_track(&track, &connection)
    }
}

fn get_track(path: &std::path::Path, connection: &Connection) -> Result<Option<Track>, rusqlite::Error> {
    let select_query = "SELECT
            uuid,
            title,
            artist,
            album,
            genre,
            album_artist,
            publisher,
            catalog_number,
            duration_seconds,
            audio_bitrate,
            overall_bitrate,
            sample_rate,
            bit_depth,
            channels,
            path,
            extension,
            tags,
            created,
            modified
        FROM tracks WHERE path = ?1 LIMIT 1";
    let mut stmt = connection.prepare(select_query)?;
    let tracks = stmt.query_map([path.to_str()], |row| {
        let tags_json: String = row.get(16)?;
        let tags: HashMap<String, String> = serde_json::from_str(tags_json.as_str()).unwrap();

        Ok(Track {
            uuid: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            album: row.get(3)?,
            genre: row.get(4)?,
            album_artist: row.get(5)?,
            publisher: row.get(6)?,
            catalog_number: row.get(7)?,
            duration_seconds: row.get(8)?,
            audio_bitrate: row.get(9)?,
            overall_bitrate: row.get(10)?,
            sample_rate: row.get(11)?,
            bit_depth: row.get(12)?,
            channels: row.get(13)?,
            path: row.get(14)?,
            extension: row.get(15)?,
            created: row.get(17)?,
            modified: row.get(18)?,
            tags
        })
    })?;
    let result = tracks.last();
    match result {
        Some(Ok(track)) => Ok(Some(track)),
        _ => Ok(None)
    }
}

fn count_tracks(connection: &Connection) -> u64 {
    let mut stmt = connection.prepare("SELECT COUNT(*) FROM tracks").unwrap();
    stmt.query_row([], |row| {
        row.get(0)
    }).unwrap()
}

fn read_files(path: &std::path::Path, connection: &Connection, progress: &ml_progress::Progress) {
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            read_files(&path, &connection, progress);
        } else {
            let result = read_tags(&path, connection);
            match result {
                Ok(track) => {
                    if track.is_none() {
                        progress.inc(1);
                        continue;
                    }
                    let track = track.unwrap();
                    let result = upsert_track(&track, &connection);
                    match result {
                        Ok(_) => {},
                        Err(e) => {
                            error!("Error inserting {}: {:?}", path.to_str().unwrap_or(""), e);
                        }
                    }
                    // print!("\rScanning tracks... {}", COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1);
                },
                Err(e) => {
                    // Only care about supported files
                    if lofty::file::FileType::from_path(&path).is_some() {
                        error!("Error reading {}: {:?}", path.to_str().unwrap_or(""), e);
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
    NotFile(),
}
