#!/bin/bash

# Test Last.fm Authentication Flow
# Based on the official Last.fm API Authentication documentation

set -e

echo "🎵 Testing Last.fm Authentication Flow"
echo "======================================"

# Base URL for the API
BASE_URL="http://localhost:3000"
CALLBACK_URL="http://localhost:3000/lastfm/callback"

echo ""
echo "Step 1: Request authentication token and URL"
echo "--------------------------------------------"

# Get auth URL and token (callback URL is pre-configured in Last.fm app)
AUTH_RESPONSE=$(curl -s "${BASE_URL}/lastfm/auth")
echo "Auth response: $AUTH_RESPONSE"

# Extract token and auth URL using jq
TOKEN=$(echo "$AUTH_RESPONSE" | jq -r '.token')
AUTH_URL=$(echo "$AUTH_RESPONSE" | jq -r '.auth_url')

echo "Token: $TOKEN"
echo "Auth URL: $AUTH_URL"

echo ""
echo "Step 2: User Authorization (with automatic callback)"
echo "---------------------------------------------------"
echo "🔗 Please open this URL in your browser and authorize the application:"
echo "   $AUTH_URL"
echo ""
echo "After authorization, Last.fm will redirect you to:"
echo "   ${CALLBACK_URL}?token=${TOKEN}"
echo ""
echo "The callback endpoint will automatically create your session and display"
echo "your session key. You can also continue with manual session creation below."
echo ""
echo "Press Enter if you want to test manual session creation, or Ctrl+C to exit..."
read -r

echo ""
echo "Step 3: Create session with authorized token (Manual API)"
echo "--------------------------------------------------------"

# Create session
SESSION_RESPONSE=$(curl -s -X POST "${BASE_URL}/lastfm/session" \
  -H "Content-Type: application/json" \
  -d "{\"token\": \"$TOKEN\"}")

echo "Session response: $SESSION_RESPONSE"

# Check if session creation was successful
if echo "$SESSION_RESPONSE" | jq -e '.session_key' > /dev/null; then
    SESSION_KEY=$(echo "$SESSION_RESPONSE" | jq -r '.session_key')
    USERNAME=$(echo "$SESSION_RESPONSE" | jq -r '.username')

    echo "✅ Session created successfully!"
    echo "   Session key: ${SESSION_KEY:0:10}..."
    echo "   Username: $USERNAME"

    echo ""
    echo "Step 4: Test scrobbling functionality"
    echo "------------------------------------"

    # Get first track from database
    TRACKS_RESPONSE=$(curl -s "${BASE_URL}/tracks?limit=1")
    TRACK_ID=$(echo "$TRACKS_RESPONSE" | jq -r '.[0].id')

    if [ "$TRACK_ID" != "null" ]; then
        echo "Testing with track ID: $TRACK_ID"

        # Test now playing
        echo "Setting 'now playing'..."
        NOW_PLAYING_RESPONSE=$(curl -s -X POST "${BASE_URL}/tracks/${TRACK_ID}/lastfm/now-playing" \
          -H "Content-Type: application/json" \
          -d "{\"session_key\": \"$SESSION_KEY\"}")

        echo "Now playing response: $NOW_PLAYING_RESPONSE"

        # Test scrobbling
        echo "Scrobbling track..."
        TIMESTAMP=$(date +%s)
        SCROBBLE_RESPONSE=$(curl -s -X POST "${BASE_URL}/tracks/${TRACK_ID}/lastfm/scrobble" \
          -H "Content-Type: application/json" \
          -d "{\"session_key\": \"$SESSION_KEY\", \"timestamp\": $TIMESTAMP}")

        echo "Scrobble response: $SCROBBLE_RESPONSE"

        if echo "$SCROBBLE_RESPONSE" | jq -e '.success' > /dev/null; then
            echo "✅ Scrobbling test successful!"
        else
            echo "❌ Scrobbling test failed"
        fi
    else
        echo "❌ No tracks found in database to test scrobbling"
    fi

else
    echo "❌ Session creation failed!"
    echo "Response: $SESSION_RESPONSE"
fi

echo ""
echo "🎵 Last.fm Authentication Flow Test Complete"
echo "============================================"
