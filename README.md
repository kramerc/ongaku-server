# Ongaku Server

A music library server built with Rust, using Axum for the HTTP API and PostgreSQL for data storage.

## Features

- Music library scanning and indexing
- RESTful API for accessing track metadata
- **Subsonic API compatibility** for existing music clients
- Search functionality across tracks, artists, albums, and genres
- PostgreSQL database for reliable data storage
- Automatic music library rescanning
- Audio file streaming

## Prerequisites

- Rust (latest stable version)
- PostgreSQL server
- Music files in a directory accessible to the server

## Setup

### 1. Database Setup

This project uses PostgreSQL. You can either:

**Option A: Use an existing PostgreSQL server**
- Make sure you have access to a PostgreSQL server
- Create a database for Ongaku (optional - the application can use an existing database)
- Note your connection details (host, port, username, password, database name)

**Option B: Set up a new PostgreSQL instance**

Using Docker (easiest):
```bash
# Using the provided Docker Compose file
docker-compose up -d postgres
```

Or install PostgreSQL directly:
```bash
# Ubuntu/Debian:
sudo apt-get install postgresql postgresql-contrib

# macOS with Homebrew:
brew install postgresql
brew services start postgresql

# Create database and user
sudo -u postgres psql
CREATE DATABASE ongaku;
CREATE USER ongaku WITH ENCRYPTED PASSWORD 'ongaku_password';
GRANT ALL PRIVILEGES ON DATABASE ongaku TO ongaku;
\q
```

### 2. Configuration

Copy the example environment file and configure it:

```bash
cp .env.example .env
```

Edit `.env` to match your PostgreSQL server:

```bash
# Path to your music library
MUSIC_PATH=/path/to/your/music

# API server configuration
API_HOST=0.0.0.0
API_PORT=4000

# PostgreSQL database configuration
# Replace with your actual server details
DATABASE_URL=postgres://your_username:your_password@your_host:5432/your_database_name
```

### 3. Build and Run

```bash
# Build the project
cargo build --release

# Run database migrations
cargo run --bin migration

# Start the server
cargo run --release
```

The server will:
1. Connect to the PostgreSQL database
2. Run any pending migrations
3. Start an initial scan of your music library
4. Start the HTTP API server

## API Endpoints
## API Endpoints

### REST API

- `GET /api/v1/tracks` - List tracks with pagination and filters
- `GET /api/v1/tracks/:id` - Get a specific track by ID
- `GET /api/v1/tracks/search?q=query` - Search tracks
- `GET /api/v1/stats` - Get database statistics
- `GET /api/v1/artists` - Get list of unique artists
- `GET /api/v1/albums` - Get list of unique albums
- `GET /api/v1/genres` - Get list of unique genres
- `POST /api/v1/rescan` - Trigger a music library rescan

### Subsonic API

Ongaku Server implements a Subsonic-compatible API under the `/rest` path, making it compatible with existing Subsonic music clients:

- `GET /rest/ping` - Test connectivity
- `GET /rest/getMusicFolders` - Get music folders
- `GET /rest/getIndexes` - Get artist index
- `GET /rest/getArtists` - Get all artists (ID3)
- `GET /rest/getArtist` - Get artist details
- `GET /rest/getAlbum` - Get album details
- `GET /rest/getSong` - Get song details
- `GET /rest/search3` - Search for artists, albums, and songs
- `GET /rest/stream/:id` - Stream audio files

**Compatible Clients:**
- **Desktop**: Sublime Music, Supersonic, Sonixd
- **Mobile**: DSub, Ultrasonic, Audinaut, Subtracks
- **Web**: Jamstash, Supysonic Web UI

For detailed Subsonic API documentation, see [SUBSONIC_API.md](SUBSONIC_API.md).

**Authentication:** Use any non-empty username/password for demo purposes. Example:
```bash
curl "http://localhost:4000/rest/ping?u=admin&p=admin&v=1.16.1&c=test&f=json"
```

## Migration from SQLite

If you were previously using the SQLite version of this server, you'll need to:

1. Update your dependencies and rebuild the project
2. Set up PostgreSQL as described above
3. Update your `DATABASE_URL` in the `.env` file
4. Run the server - it will automatically create the necessary tables
5. The server will scan your music library again to populate the new database

## Development

### Running Migrations

The migration CLI is available in the `migration` directory:

```bash
cd migration

# Apply all pending migrations
cargo run

# Check migration status
cargo run -- status

# Create a new migration
cargo run -- generate MIGRATION_NAME
```

### Database Schema

The main entity is the `Track` table which stores:
- File metadata (path, extension)
- Audio metadata (title, artist, album, genre, etc.)
- Technical information (bitrate, sample rate, duration, etc.)
- Timestamps (created, modified)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

[Add your license information here]
