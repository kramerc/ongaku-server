# Last.fm Integration

Ongaku Server includes built-in Last.fm integration for scrobbling tracks and updating "now playing" status.

## Setup

1. **Get Last.fm API credentials:**
   - Visit https://www.last.fm/api/account/create
   - Create a new application
   - Note down your API Key and Shared Secret

2. **Set environment variables:**
   ```bash
   export LASTFM_API_KEY="your_api_key_here"
   export LASTFM_SHARED_SECRET="your_shared_secret_here"
   ```

3. **Add to your .env file (optional):**
   ```env
   LASTFM_API_KEY=your_api_key_here
   LASTFM_SHARED_SECRET=your_shared_secret_here
   ```

## Authentication Flow

### 1. Get Authentication URL
```bash
curl https://ongaku-dev.m3r.dev/api/v1/lastfm/auth
```

Response:
```json
{
  "auth_url": "https://www.last.fm/api/auth/?api_key=YOUR_API_KEY&token=TOKEN",
  "token": "abcd1234efgh5678"
}
```

### 2. User Authorization
Direct users to visit the `auth_url` to authorize your application.

### 3. Create Session
After authorization, exchange the token for a session key:

```bash
curl -X POST https://ongaku-dev.m3r.dev/api/v1/lastfm/session \
  -H "Content-Type: application/json" \
  -d '{"token": "abcd1234efgh5678"}'
```

Response:
```json
{
  "session_key": "xyz789abc123def456",
  "username": "music_lover_2024",
  "message": "Last.fm session created successfully"
}
```

## Scrobbling

### Update "Now Playing"
Call this when a track starts playing:

```bash
curl -X POST https://ongaku-dev.m3r.dev/api/v1/tracks/123/now-playing \
  -H "Content-Type: application/json" \
  -d '{"session_key": "xyz789abc123def456"}'
```

### Scrobble Track
Call this when a track has been played for at least 50% of its duration or 4 minutes:

```bash
curl -X POST https://ongaku-dev.m3r.dev/api/v1/tracks/123/scrobble \
  -H "Content-Type: application/json" \
  -d '{
    "session_key": "xyz789abc123def456",
    "timestamp": 1640995200,
    "album_artist": "The Beatles"
  }'
```

## Best Practices

1. **Scrobbling Rules:**
   - Only scrobble tracks that have been played for at least 50% of their duration
   - Or at least 4 minutes for longer tracks
   - Don't scrobble tracks shorter than 30 seconds

2. **Session Management:**
   - Store session keys securely
   - Session keys don't expire but can be revoked by users
   - Handle authentication errors gracefully

3. **Rate Limiting:**
   - Respect Last.fm's rate limits
   - The server handles this automatically but don't spam requests

## Example Usage

Run the included example script:

```bash
./lastfm-example.sh
```

This script demonstrates the complete authentication flow and scrobbling process.

## Frontend Integration

For web frontends, you can implement a simple flow:

1. **Check Authentication:**
   ```javascript
   // Check if user has a stored session key
   const sessionKey = localStorage.getItem('lastfm_session_key');
   ```

2. **Authenticate if needed:**
   ```javascript
   // Get auth URL and redirect user
   const response = await fetch('/api/v1/lastfm/auth');
   const { auth_url, token } = await response.json();
   
   // Store token for later
   localStorage.setItem('lastfm_token', token);
   
   // Redirect to Last.fm
   window.location.href = auth_url;
   ```

3. **Create session after redirect:**
   ```javascript
   // After user returns from Last.fm
   const token = localStorage.getItem('lastfm_token');
   const response = await fetch('/api/v1/lastfm/session', {
     method: 'POST',
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({ token })
   });
   
   const { session_key } = await response.json();
   localStorage.setItem('lastfm_session_key', session_key);
   ```

4. **Scrobble during playback:**
   ```javascript
   // When track starts
   await fetch(`/api/v1/tracks/${trackId}/now-playing`, {
     method: 'POST',
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({ session_key: sessionKey })
   });
   
   // When track should be scrobbled
   await fetch(`/api/v1/tracks/${trackId}/scrobble`, {
     method: 'POST', 
     headers: { 'Content-Type': 'application/json' },
     body: JSON.stringify({
       session_key: sessionKey,
       timestamp: Math.floor(Date.now() / 1000)
     })
   });
   ```

## Troubleshooting

- **"Invalid API key"**: Check your LASTFM_API_KEY environment variable
- **"Invalid session"**: The session key may have been revoked, re-authenticate
- **"Track not found"**: Ensure the track ID exists in your library
- **"Authentication failed"**: Check that the user completed the authorization flow

## API Documentation

Full API documentation with request/response schemas is available at:
- Interactive docs: https://ongaku-dev.m3r.dev/api/v1/docs
- OpenAPI spec: https://ongaku-dev.m3r.dev/api/v1/openapi.yaml
