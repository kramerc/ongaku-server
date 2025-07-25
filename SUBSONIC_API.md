# Subsonic API Implementation

This implementation provides a Subsonic-compatible API for the Ongaku music server. The Subsonic API is a REST API that enables music client applications to browse, search, and stream music from the server.

## Base URL

All Subsonic API endpoints are available under the `/rest` path:

```
http://localhost:4000/rest/
```

## Authentication

The Subsonic API requires authentication for all endpoints. You can authenticate using:

- **Username/Password**: Pass `u` (username) and `p` (password) parameters
- **Token/Salt**: Pass `u` (username), `t` (token), and `s` (salt) parameters

For demo purposes, any non-empty username will be accepted. In production, implement proper authentication.

## Common Parameters

All endpoints accept these common parameters:

- `u` - Username (required)
- `p` - Password in clear text or hex-encoded with "enc:" prefix
- `t` - Authentication token (alternative to password)
- `s` - Salt used for token generation (required when using token)
- `v` - API version (e.g., "1.16.1")
- `c` - Client name/identifier
- `f` - Response format: "xml" (default) or "json"

## Response Format

All responses follow the Subsonic response format:

```json
{
  "subsonic-response": {
    "status": "ok",
    "version": "1.16.1",
    "type": "ongaku-server", 
    "serverVersion": "0.1.0",
    ... // endpoint-specific data
  }
}
```

For errors:

```json
{
  "subsonic-response": {
    "status": "failed",
    "version": "1.16.1", 
    "type": "ongaku-server",
    "serverVersion": "0.1.0",
    "error": {
      "code": 40,
      "message": "Wrong username or password"
    }
  }
}
```

## Available Endpoints

### System

#### `GET /rest/ping`
Test server connectivity and authentication.

**Example:**
```
GET /rest/ping?u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

### Library Browsing

#### `GET /rest/getMusicFolders`
Get all music folders.

**Example:**
```
GET /rest/getMusicFolders?u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

#### `GET /rest/getIndexes`
Get artist index (A-Z grouped artists).

**Parameters:**
- `musicFolderId` - Music folder ID (optional)
- `ifModifiedSince` - Return only if modified since timestamp (optional)

**Example:**
```
GET /rest/getIndexes?u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

#### `GET /rest/getMusicDirectory`
Get directory contents (albums for artist, or tracks for album).

**Parameters:**
- `id` - Directory ID (required)

**Example:**
```
GET /rest/getMusicDirectory?id=artist-QWJiZXk%3D&u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

### ID3 Browsing  

#### `GET /rest/getArtists`
Get all artists organized alphabetically.

**Parameters:**
- `musicFolderId` - Music folder ID (optional)

**Example:**
```
GET /rest/getArtists?u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

#### `GET /rest/getArtist`
Get artist details including albums.

**Parameters:**
- `id` - Artist ID (required)

**Example:**
```
GET /rest/getArtist?id=artist-QWJiZXk%3D&u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

#### `GET /rest/getAlbum`
Get album details including tracks.

**Parameters:**
- `id` - Album ID (required)

**Example:**
```
GET /rest/getAlbum?id=album-QWJiZXk%3D-QWJiZXkgUm9hZA%3D%3D&u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

#### `GET /rest/getSong`
Get song/track details.

**Parameters:**
- `id` - Song ID (required)

**Example:**
```
GET /rest/getSong?id=123&u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

### Search

#### `GET /rest/search3`
Search for artists, albums, and songs.

**Parameters:**
- `query` - Search query (required)
- `artistCount` - Max artists to return (default: 20)
- `artistOffset` - Artist search offset (default: 0)
- `albumCount` - Max albums to return (default: 20)
- `albumOffset` - Album search offset (default: 0)
- `songCount` - Max songs to return (default: 20)
- `songOffset` - Song search offset (default: 0)

**Example:**
```
GET /rest/search3?query=Abbey&u=admin&p=admin&v=1.16.1&c=myapp&f=json
```

### Streaming

#### `GET /rest/stream/:id`
Stream an audio file.

**Parameters:**
- `id` - Song ID (required, in URL path)
- `maxBitRate` - Max bitrate in kbps (optional)
- `format` - Audio format (optional)
- `timeOffset` - Start offset in seconds (optional)
- `size` - Scaled image size (optional)
- `estimateContentLength` - Estimate content length (optional)
- `converted` - Return converted/transcoded stream (optional)

**Example:**
```
GET /rest/stream/123?u=admin&p=admin&v=1.16.1&c=myapp
```

## ID Format

The API uses encoded IDs to identify resources:

- **Artists**: `artist-{base64_encoded_artist_name}`
- **Albums**: `album-{base64_encoded_artist_name}-{base64_encoded_album_name}`  
- **Songs**: Database ID as integer

## Client Compatibility

This implementation aims to be compatible with popular Subsonic clients such as:

- **Desktop**: Sublime Music, Supersonic, Sonixd
- **Mobile**: DSub, Ultrasonic, Audinaut, Subtracks
- **Web**: Jamstash, Supysonic Web UI

## Implementation Notes

- XML responses are currently returned as JSON with XML content-type (full XML serialization can be added)
- Authentication is simplified for demo purposes
- Cover art/album artwork is not yet implemented
- User ratings, stars, and play counts are not implemented
- Playlists are not implemented in this initial version
- Transcoding/format conversion is not implemented

## Error Codes

Common Subsonic error codes:

- `0` - Generic error
- `10` - Required parameter is missing
- `20` - Incompatible Subsonic REST protocol version
- `30` - Incompatible Subsonic REST protocol version  
- `40` - Wrong username or password
- `50` - User is not authorized for the given operation
- `60` - Trial period for the Subsonic server is over
- `70` - The requested data was not found

## Testing

You can test the API using curl:

```bash
# Ping test
curl "http://localhost:4000/rest/ping?u=admin&p=admin&v=1.16.1&c=test&f=json"

# Get artists
curl "http://localhost:4000/rest/getArtists?u=admin&p=admin&v=1.16.1&c=test&f=json"

# Search
curl "http://localhost:4000/rest/search3?query=rock&u=admin&p=admin&v=1.16.1&c=test&f=json"
```
