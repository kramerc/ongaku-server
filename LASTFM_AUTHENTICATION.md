# Last.fm Authentication API Implementation

This document explains how our Last.fm integration follows the official Last.fm Authentication API specifica## Usage Example

### Automatic Flow (Recommended)
```bash
# 1. Get auth URL (callback is pre-configured in Last.fm app)
curl "http://localhost:3000/lastfm/auth"

# 2. User visits auth URL, authorizes, gets redirected to pre-configured callback
# 3. Callback automatically creates session and displays HTML page with session key
```

### Manual/Programmatic Flow
```bash
# 1. Get auth URL
curl "http://localhost:3000/lastfm/auth"

# 2. User visits auth URL and authorizes

# 3. Create session manually
curl -X POST "http://localhost:3000/lastfm/session" \
  -H "Content-Type: application/json" \
  -d '{"token": "authorized_token"}'

# 4. Scrobble track
curl -X POST "http://localhost:3000/tracks/1/scrobble" \
  -H "Content-Type: application/json" \
  -d '{"session_key": "session_key", "timestamp": 1234567890}'
```

Our implementation supports the complete Last.fm Web-based Authentication flow as specified in the official documentation, including:

1. **Request authorization from user** - Getting authentication tokens
2. **Create an authentication handler** - Building proper auth URLs with signatures
3. **Create a Web Service Session** - Converting authorized tokens to session keys

## Authentication Flow

### Step 1: Get Authentication Token

```rust
pub async fn get_token(&self) -> Result<String, String>
```

- **Method**: `auth.gettoken`
- **Parameters**: `api_key`, `api_sig`
- **Signature**: Generated excluding `format` and `api_sig` parameters
- **Error Handling**: Comprehensive error mapping for all Last.fm error codes
- **Callback URL**: Pre-configured in Last.fm application settings

### Step 2: Build Authentication URL

```rust
pub fn build_auth_url(&self, token: &str) -> String
```

- **URL Format**: `https://www.last.fm/api/auth?api_key=XXX&token=XXX`
- **No Signature**: User-facing auth URLs don't require API signatures
- **URL Encoding**: Proper encoding of all parameters
- **Validation**: Input validation for token
- **Callback URL**: Uses the callback URL configured in Last.fm app settings

### Step 3: User Authorization

User visits the authentication URL and authorizes the application. Last.fm redirects to the callback URL with the token.

### Step 4: Handle Callback (Automatic)

```rust
pub async fn auth_callback(State, Query<CallbackQuery>) -> Result<Html<String>, StatusCode>
```

- **Route**: `GET /lastfm/callback?token=XXX`
- **Purpose**: Automatically handles the redirect from Last.fm
- **Functionality**: Creates session and displays HTML page with session key
- **User Experience**: No manual token handling required

### Step 5: Create Session (Alternative/Manual)

```rust
pub async fn get_session(&self, token: &str) -> Result<(String, String), String>
```

- **Method**: `auth.getsession`
- **Parameters**: `api_key`, `token`, `api_sig`
- **Returns**: Session key and username
- **Error Handling**: Specific handling for authentication-related errors (4, 14, 15)

## API Error Handling

Following the Last.fm documentation, we handle these specific error codes:

| Code | Description |
|------|-------------|
| 2    | Invalid service |
| 3    | Invalid Method |
| 4    | Authentication Failed |
| 5    | Invalid format |
| 6    | Invalid parameters |
| 7    | Invalid resource specified |
| 8    | Operation failed |
| 9    | Invalid session key |
| 10   | Invalid API key |
| 11   | Service Offline |
| 13   | Invalid method signature |
| 14   | Unauthorized Token |
| 15   | Token has expired |
| 16   | Temporary error |
| 26   | Suspended API key |
| 29   | Rate limit exceeded |

## Signature Generation

Our signature generation follows the Last.fm specification exactly:

1. **Parameters**: All parameters except `format` and `api_sig`
2. **Sorting**: Alphabetical by parameter name
3. **Concatenation**: `key1value1key2value2...shared_secret`
4. **Hashing**: MD5 hash of the concatenated string
5. **Format**: Lowercase hexadecimal

```rust
fn generate_signature(&self, params: &HashMap<&str, &str>) -> String {
    let mut sorted_params: Vec<_> = params.iter()
        .filter(|(key, _)| **key != "format" && **key != "api_sig")
        .collect();
    sorted_params.sort_by_key(|(key, _)| *key);

    let mut signature_string = String::new();
    for (key, value) in sorted_params {
        signature_string.push_str(key);
        signature_string.push_str(value);
    }
    signature_string.push_str(&self.shared_secret);

    format!("{:x}", md5::compute(signature_string.as_bytes()))
}
```

## API Endpoints

### Authentication Endpoints

1. **GET /lastfm/auth** - Get authentication URL and token
   - No parameters required (callback URL is pre-configured)
   - Returns: `auth_url` and `token`

2. **GET /lastfm/callback** - Handle Last.fm authorization callback
   - Query parameter: `token` (from Last.fm redirect)
   - Returns: HTML page with session information

3. **POST /lastfm/session** - Create session from authorized token (alternative to callback)
   - Body: `{"token": "authorized_token"}`
   - Returns: `session_key`, `username`, and `message`

### Scrobbling Endpoints

3. **POST /tracks/{id}/lastfm/scrobble** - Scrobble a track
   - Body: `{"session_key": "key", "timestamp": 1234567890, "album_artist": "optional"}`
   - Returns: `success`, `message`, and optional `scrobble_id`

4. **POST /tracks/{id}/lastfm/now-playing** - Update now playing status
   - Body: `{"session_key": "key"}`
   - Returns: `success` and `message`

## Validation

### Session Key Validation
```rust
pub fn validate_session_key(&self, session_key: &str) -> bool {
    !session_key.trim().is_empty() && session_key.len() >= 10
}
```

### Track Data Validation
- Artist and title must be non-empty
- Album information is optional but recommended
- Duration and track number are optional

## Usage Example

```bash
# 1. Get auth URL
curl "http://localhost:3000/lastfm/auth?callback_url=http://localhost:3000/callback"

# 2. User visits auth URL and authorizes

# 3. Create session
curl -X POST "http://localhost:3000/lastfm/session" \
  -H "Content-Type: application/json" \
  -d '{"token": "authorized_token"}'

# 4. Scrobble track
curl -X POST "http://localhost:3000/tracks/1/lastfm/scrobble" \
  -H "Content-Type: application/json" \
  -d '{"session_key": "session_key", "timestamp": 1234567890}'
```

## Environment Variables

Required environment variables:
- `LASTFM_API_KEY` - Your Last.fm API key
- `LASTFM_SHARED_SECRET` - Your Last.fm shared secret

## Testing

Use the provided test script:
```bash
./test-lastfm-auth.sh
```

This script tests the complete authentication flow including token generation, URL building, session creation, and scrobbling functionality.
