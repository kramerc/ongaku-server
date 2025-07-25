use log::{Level, LevelFilter, Metadata, Record};
use std::env;

static LOGGER: SimpleLogger = SimpleLogger;

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= get_log_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // Filter out noisy warnings during scanning
            let message = record.args().to_string();
            if message.contains("MPEG: Using bitrate to estimate duration") ||
               message.contains("Skipping empty \"data\" atom") ||
               message.contains("Encountered an ID3v2 tag. This tag cannot be rewritten to the FLAC file!") {
                return; // Skip these warnings
            }

            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

fn get_log_level() -> Level {
    match env::var("RUST_LOG").as_deref() {
        Ok("trace") => Level::Trace,
        Ok("debug") => Level::Debug,
        Ok("info") => Level::Info,
        Ok("warn") => Level::Warn,
        Ok("error") => Level::Error,
        _ => Level::Info, // default
    }
}

fn get_log_level_filter() -> LevelFilter {
    match env::var("RUST_LOG").as_deref() {
        Ok("trace") => LevelFilter::Trace,
        Ok("debug") => LevelFilter::Debug,
        Ok("info") => LevelFilter::Info,
        Ok("warn") => LevelFilter::Warn,
        Ok("error") => LevelFilter::Error,
        _ => LevelFilter::Info, // default
    }
}

pub fn init() -> Result<(), log::SetLoggerError> {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(get_log_level_filter()))
}
