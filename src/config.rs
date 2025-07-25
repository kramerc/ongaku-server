use std::env;
use std::path::Path;

pub struct Config {
    pub music_path: String,
    pub api_host: String,
    pub api_port: u16,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            music_path: env::var("MUSIC_PATH").unwrap_or_else(|_| "/mnt/shucked/Music".to_string()),
            api_host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            api_port: env::var("API_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://ongaku.db?mode=rwc".to_string()),
        }
    }

    pub fn music_path(&self) -> &Path {
        Path::new(&self.music_path)
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.api_host, self.api_port)
    }
}
