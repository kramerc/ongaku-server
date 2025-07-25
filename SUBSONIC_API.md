# Ongaku Server - Subsonic API

This document describes the Subsonic API implementation in Ongaku Server.

## Overview

Ongaku Server implements a subset of the Subsonic API specification, providing compatibility with existing Subsonic clients. The API follows the standard Subsonic protocol and returns responses in both JSON and XML formats.

## Base URL

All Subsonic API endpoints are available under: `/rest/`

## Authentication

Currently, authentication is not enforced. Any username and password combination will be accepted. This may change in future versions.

## Common Parameters

All endpoints require these parameters:
- `u` - Username (any value accepted)
- `p` - Password (any value accepted) OR `t` + `s` for token auth
- `v` - API version (recommend: 1.16.1)
- `c` - Client identifier (any value)
- `f` - Response format (json or xml, defaults to xml)

## Supported Endpoints

### System Endpoints

#### ping
Tests connectivity to the server.

**URL:** `/rest/ping`  
**Method:** GET  
**Parameters:** Standard authentication parameters  
**Response:** Empty success response

```bash
curl "http://localhost:8080/rest/ping?u=test&v=1.16.1&c=test&f=json"
```

#### getLicense
Returns server license information.

**URL:** `/rest/getLicense`  
**Method:** GET  
**Parameters:** Standard authentication parameters  
**Response:** License object (always shows as valid)

```bash
curl "http://localhost:8080/rest/getLicense?u=test&v=1.16.1&c=test&f=json"
```

### Browsing Endpoints

#### getMusicFolders
Returns available music folders.

**URL:** `/rest/getMusicFolders`  
**Method:** GET  
**Parameters:** Standard authentication parameters  
**Response:** Array of music folder objects

```bash
curl "http://localhost:8080/rest/getMusicFolders?u=test&v=1.16.1&c=test&f=json"
```

#### getIndexes
Returns an index of all artists, organized by first letter.

**URL:** `/rest/getIndexes`  
**Method:** GET  
**Parameters:** 
- Standard authentication parameters
- `musicFolderId` (optional) - Music folder ID
- `ifModifiedSince` (optional) - Timestamp

**Response:** Indexes object with artists grouped by first letter

```bash
curl "http://localhost:8080/rest/getIndexes?u=test&v=1.16.1&c=test&f=json"
```

#### getMusicDirectory
Returns the contents of a music directory (albums for artists, tracks for albums).

**URL:** `/rest/getMusicDirectory`  
**Method:** GET  
**Parameters:**
- Standard authentication parameters
- `id` (required) - Directory ID (format: `artist-{encoded_name}` or `album-{encoded_artist}-{encoded_album}`)

**Response:** Directory object with child entries

```bash
# Get albums for an artist
curl "http://localhost:8080/rest/getMusicDirectory?u=test&v=1.16.1&c=test&f=json&id=artist-The%20Beatles"

# Get tracks for an album
curl "http://localhost:8080/rest/getMusicDirectory?u=test&v=1.16.1&c=test&f=json&id=album-The%20Beatles-Abbey%20Road"
```

#### getGenres
Returns all genres.

**URL:** `/rest/getGenres`  
**Method:** GET  
**Parameters:** Standard authentication parameters  
**Response:** Array of genre objects

```bash
curl "http://localhost:8080/rest/getGenres?u=test&v=1.16.1&c=test&f=json"
```

### Search Endpoints

#### search3
Searches for artists, albums, and songs.

**URL:** `/rest/search3`  
**Method:** GET  
**Parameters:**
- Standard authentication parameters
- `query` (optional) - Search query string
- `artistCount` (optional) - Max artists to return (default: 20)
- `artistOffset` (optional) - Artist result offset (default: 0)
- `albumCount` (optional) - Max albums to return (default: 20)
- `albumOffset` (optional) - Album result offset (default: 0)
- `songCount` (optional) - Max songs to return (default: 20)
- `songOffset` (optional) - Song result offset (default: 0)

**Response:** SearchResult3 object with artists, albums, and songs

```bash
curl "http://localhost:8080/rest/search3?u=test&v=1.16.1&c=test&f=json&query=beatles"
```

### Media Endpoints

#### stream
Streams a media file.

**URL:** `/rest/stream`  
**Method:** GET  
**Parameters:**
- Standard authentication parameters
- `id` (required) - Track ID
- `maxBitRate` (optional) - Maximum bit rate
- `format` (optional) - Preferred format
- `timeOffset` (optional) - Start time offset
- Additional parameters supported but not used

**Response:** Audio file stream

```bash
curl "http://localhost:8080/rest/stream?u=test&v=1.16.1&c=test&id=123" --output song.mp3
```

## ID Formats

Ongaku Server uses the following ID formats:

- **Artist IDs:** `artist-{url_encoded_artist_name}`
- **Album IDs:** `album-{url_encoded_artist_name}-{url_encoded_album_name}`
- **Track IDs:** `{database_track_id}` (integer)

## Response Format

All responses follow the standard Subsonic API format:

```json
{
  "subsonic-response": {
    "status": "ok",
    "version": "1.16.1",
    "type": "ongaku",
    "serverVersion": "0.1.0",
    // ... response data
  }
}
```

For XML format, the structure is equivalent:

```xml
<subsonic-response status="ok" version="1.16.1" type="ongaku" serverVersion="0.1.0">
  <!-- response data -->
</subsonic-response>
```

## Error Handling

Errors are returned with status "failed" and typically include minimal error information:

```json
{
  "subsonic-response": {
    "status": "failed",
    "version": "1.16.1",
    "type": "ongaku",
    "serverVersion": "0.1.0"
  }
}
```

## Client Compatibility

This Subsonic API implementation has been designed to work with standard Subsonic clients. However, note that:

1. Authentication is currently not enforced
2. Some advanced features may not be implemented
3. Album art is not yet supported
4. Playlists are not yet supported

## Future Enhancements

Planned improvements include:
- Proper authentication
- Album art support
- Playlist management
- Additional browsing endpoints
- Star/favorite functionality
