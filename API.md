# Ongaku Server REST API

This project exposes a REST API for managing and querying your music library database.

## Configuration

The server can be configured using environment variables:

- `MUSIC_PATH`: Path to your music library (default: `/mnt/shucked/Music`)
- `API_HOST`: Host to bind the API server (default: `0.0.0.0`)
- `API_PORT`: Port for the API server (default: `4000`)
- `DATABASE_URL`: Database connection string (default: `sqlite://ongaku.db?mode=rwc`)

Copy `.env.example` to `.env` and modify as needed.

## API Endpoints

### Base URL
```
http://localhost:4000/api/v1
```

### Endpoints

#### GET /tracks
List tracks with pagination and optional filters.

**Query Parameters:**
- `page` (optional): Page number (default: 1)
- `per_page` (optional): Items per page (default: 20, max: 100)
- `title` (optional): Filter by title (contains search)
- `artist` (optional): Filter by artist (contains search)
- `album` (optional): Filter by album (contains search)
- `genre` (optional): Filter by genre (contains search)
- `album_artist` (optional): Filter by album artist (contains search)

**Example:**
```bash
curl "http://localhost:4000/api/v1/tracks?page=1&per_page=10&artist=Beatles"
```

#### GET /tracks/:id
Get a specific track by ID.

**Example:**
```bash
curl "http://localhost:4000/api/v1/tracks/123"
```

#### GET /tracks/:id/play
Stream audio file for the specified track. This endpoint supports HTTP range requests for efficient streaming in web browsers.

**Features:**
- HTTP Range support for partial content streaming
- Proper MIME type detection based on file extension
- CORS headers for web browser compatibility
- Efficient file streaming with caching headers

**Example:**
```bash
# Stream full file
curl "http://localhost:4000/api/v1/tracks/123/play" -o song.mp3

# Request specific byte range (browsers do this automatically)
curl -H "Range: bytes=0-1023" "http://localhost:4000/api/v1/tracks/123/play"
```

**HTML5 Audio Example:**
```html
<audio controls>
  <source src="http://localhost:4000/api/v1/tracks/123/play" type="audio/mpeg">
  Your browser does not support the audio element.
</audio>
```

**Response Headers:**
- `Content-Type`: Detected MIME type (e.g., `audio/mpeg`, `audio/flac`)
- `Accept-Ranges`: `bytes` (indicates range support)
- `Content-Length`: File size or range size
- `Content-Range`: Byte range for partial content (206 responses)
- `Cache-Control`: `public, max-age=3600` (1 hour cache)
- CORS headers for web browser compatibility

**Status Codes:**
- `200 OK`: Full file content
- `206 Partial Content`: Range request response  
- `404 Not Found`: Track or file not found
- `416 Range Not Satisfiable`: Invalid range request

#### GET /tracks/search
Search tracks across multiple fields.

**Query Parameters:**
- `q` (required): Search query
- `page` (optional): Page number (default: 1)
- `per_page` (optional): Items per page (default: 20, max: 100)

**Example:**
```bash
curl "http://localhost:4000/api/v1/tracks/search?q=rock&page=1"
```

#### GET /stats
Get database statistics including total tracks, duration, and unique counts.

**Example:**
```bash
curl "http://localhost:4000/api/v1/stats"
```

#### GET /artists
Get list of unique artists.

**Example:**
```bash
curl "http://localhost:4000/api/v1/artists"
```

#### GET /albums
Get list of unique albums.

**Example:**
```bash
curl "http://localhost:4000/api/v1/albums"
```

#### GET /genres
Get list of unique genres.

**Example:**
```bash
curl "http://localhost:4000/api/v1/genres"
```

#### POST /rescan
Trigger a rescan of the music library. This will scan for new, modified, or deleted files and update the database accordingly.

**Example:**
```bash
curl -X POST "http://localhost:4000/api/v1/rescan"
```

**Response:**
```json
{
  "message": "Music library rescan initiated",
  "status": "success"
}
```

**Note:** The rescan runs in the background with a proper progress bar displayed in the server logs. You can monitor progress by watching the server console output or polling the `/stats` endpoint to see track count changes.

## Response Format

All endpoints return JSON responses. List endpoints include pagination metadata.

The `tags` field contains all the metadata tags extracted from the audio file as a JSON object. If the stored tags string cannot be parsed as JSON, an empty object `{}` is returned.

### Track Object
```json
{
  "id": 123,
  "path": "/path/to/song.mp3",
  "extension": "mp3",
  "title": "Song Title",
  "artist": "Artist Name",
  "album": "Album Name",
  "genre": "Rock",
  "album_artist": "Album Artist",
  "publisher": "Publisher",
  "catalog_number": "CAT123",
  "duration_seconds": 240,
  "audio_bitrate": 320,
  "overall_bitrate": 320,
  "sample_rate": 44100,
  "bit_depth": 16,
  "channels": 2,
  "tags": {
    "GENRE": "Rock",
    "ARTIST": "Artist Name",
    "ALBUM": "Album Name",
    "TITLE": "Song Title",
    "DATE": "2024",
    "ALBUMARTIST": "Album Artist"
  },
  "created": "2024-01-01T00:00:00Z",
  "modified": "2024-01-01T00:00:00Z"
}
```

### Paginated Response
```json
{
  "tracks": [...],
  "total": 1000,
  "page": 1,
  "per_page": 20,
  "total_pages": 50
}
```

## Running the Server

1. Build the project:
```bash
cargo build --release
```

2. Run the server:
```bash
cargo run
```

The server will:
1. Start scanning your music library (configured path in main.rs)
2. Start the REST API server on http://localhost:4000
3. Both processes run concurrently

## CORS

The API includes permissive CORS headers, allowing requests from any origin during development.

## Error Handling

- `200 OK`: Successful request
- `400 Bad Request`: Invalid request parameters
- `404 Not Found`: Resource not found
- `500 Internal Server Error`: Server error
