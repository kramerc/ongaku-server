# API Documentation

This project includes comprehensive OpenAPI 3.0 documentation for the REST API.

## Accessing the Documentation

Once the server is running, you can access the API documentation in several ways:

### Interactive Web UI (Swagger UI)
- **URL**: `https://ongaku-dev.m3r.dev/api/v1/docs`
- **Description**: Interactive web interface where you can explore endpoints, view request/response schemas, and test API calls directly from your browser.
- **Features**:
  - Browse all available endpoints
  - View detailed request/response schemas
  - Test API calls with "Try it out" functionality
  - View example requests and responses

### OpenAPI Specification File
- **URL**: `https://ongaku-dev.m3r.dev/api/v1/openapi.yaml`
- **Description**: Raw OpenAPI 3.0 specification in YAML format
- **Use cases**:
  - Import into API testing tools (Postman, Insomnia, etc.)
  - Generate client SDKs
  - Integration with API gateways
  - Documentation generation tools

## Available Endpoints

The API provides the following main functionality:

### Tracks
- `GET /api/v1/tracks` - List tracks with pagination and filtering
- `GET /api/v1/tracks/{id}` - Get specific track by ID
- `GET /api/v1/tracks/search?q={query}` - Search tracks across all fields

### Library Browsing
- `GET /api/v1/artists` - Get list of all artists
- `GET /api/v1/albums` - Get list of all albums  
- `GET /api/v1/genres` - Get list of all genres
- `GET /api/v1/stats` - Get library statistics

### Management
- `POST /api/v1/rescan` - Trigger music library rescan

## Using the API

### Pagination
Most list endpoints support pagination with these parameters:
- `page` - Page number (1-based, default: 1)
- `per_page` - Items per page (max: 100, default: 20)

### Filtering
The tracks endpoint supports filtering by:
- `title` - Filter by track title (partial match)
- `artist` - Filter by artist name (partial match)
- `album` - Filter by album name (partial match)
- `genre` - Filter by genre (partial match)
- `album_artist` - Filter by album artist (partial match)

### Search
The search endpoint (`/tracks/search`) performs full-text search across:
- Track title
- Artist name
- Album name
- Genre
- Album artist

## Response Format

All successful responses return JSON with appropriate HTTP status codes:
- `200 OK` - Successful request
- `400 Bad Request` - Invalid request parameters
- `404 Not Found` - Resource not found
- `500 Internal Server Error` - Server error

Error responses include a JSON object with an `error` field containing the error message.

## Example Usage

### Get first page of tracks
```bash
curl "https://ongaku-dev.m3r.dev/api/v1/tracks?page=1&per_page=10"
```

### Search for tracks
```bash
curl "https://ongaku-dev.m3r.dev/api/v1/tracks/search?q=beatles"
```

### Get library statistics
```bash
curl "https://ongaku-dev.m3r.dev/api/v1/stats"
```

### Trigger rescan
```bash
curl -X POST "https://ongaku-dev.m3r.dev/api/v1/rescan"
```

## Development

The OpenAPI specification is automatically served by the application and stays in sync with the actual API implementation. The specification file is located at `openapi.yaml` in the project root.

To regenerate or modify the documentation:
1. Edit the `openapi.yaml` file
2. Restart the server
3. Access the updated documentation at `/api/v1/docs`
