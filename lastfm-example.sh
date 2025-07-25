#!/bin/bash

# Example script demonstrating Last.fm integration with Ongaku Server
# Set your Last.fm API credentials in environment variables before running

BASE_URL="https://ongaku-dev.m3r.dev/api/v1"

echo "🎵 Ongaku Server Last.fm Integration Example"
echo "==========================================="
echo

# Check if environment variables are set
if [ -z "$LASTFM_API_KEY" ] || [ -z "$LASTFM_SHARED_SECRET" ]; then
    echo "❌ Error: Please set LASTFM_API_KEY and LASTFM_SHARED_SECRET environment variables"
    echo "   You can get these from: https://www.last.fm/api/account/create"
    exit 1
fi

echo "📡 Step 1: Getting Last.fm authentication URL..."
# You can optionally add a callback URL parameter:
# auth_response=$(curl -s "$BASE_URL/lastfm/auth?callback_url=https://yourapp.com/callback")
auth_response=$(curl -s "$BASE_URL/lastfm/auth")
auth_url=$(echo "$auth_response" | grep -o '"auth_url":"[^"]*"' | cut -d'"' -f4)
token=$(echo "$auth_response" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

if [ -z "$auth_url" ] || [ -z "$token" ]; then
    echo "❌ Failed to get authentication URL"
    echo "Response: $auth_response"
    exit 1
fi

echo "✅ Authentication URL obtained!"
echo "🔗 Please visit this URL to authorize the application:"
echo "   $auth_url"
echo
echo "⏳ After authorization, press Enter to continue..."
read -r

echo "📱 Step 2: Creating Last.fm session..."
session_response=$(curl -s -X POST "$BASE_URL/lastfm/session" \
    -H "Content-Type: application/json" \
    -d "{\"token\":\"$token\"}")

session_key=$(echo "$session_response" | grep -o '"session_key":"[^"]*"' | cut -d'"' -f4)
username=$(echo "$session_response" | grep -o '"username":"[^"]*"' | cut -d'"' -f4)

if [ -z "$session_key" ]; then
    echo "❌ Failed to create session"
    echo "Response: $session_response"
    exit 1
fi

echo "✅ Session created successfully!"
echo "👤 Username: $username"
echo "🔑 Session key: ${session_key:0:10}..."
echo

echo "🎧 Step 3: Getting a track to test with..."
tracks_response=$(curl -s "$BASE_URL/tracks?per_page=1")
track_id=$(echo "$tracks_response" | grep -o '"id":[0-9]*' | head -1 | cut -d':' -f2)
track_title=$(echo "$tracks_response" | grep -o '"title":"[^"]*"' | head -1 | cut -d'"' -f4)
track_artist=$(echo "$tracks_response" | grep -o '"artist":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$track_id" ]; then
    echo "❌ No tracks found in the library"
    exit 1
fi

echo "🎵 Testing with track: $track_artist - $track_title (ID: $track_id)"
echo

echo "📻 Step 4: Updating 'Now Playing' status..."
now_playing_response=$(curl -s -X POST "$BASE_URL/tracks/$track_id/now-playing" \
    -H "Content-Type: application/json" \
    -d "{\"session_key\":\"$session_key\"}")

echo "Response: $now_playing_response"
echo

echo "⏰ Step 5: Simulating track play and scrobbling..."
echo "   (Waiting 5 seconds to simulate listening...)"
sleep 5

# Use current timestamp
timestamp=$(date +%s)
scrobble_response=$(curl -s -X POST "$BASE_URL/tracks/$track_id/scrobble" \
    -H "Content-Type: application/json" \
    -d "{\"session_key\":\"$session_key\",\"timestamp\":$timestamp}")

echo "Scrobble response: $scrobble_response"
echo

echo "✅ Last.fm integration test completed!"
echo "🎉 Check your Last.fm profile to see the scrobbled track!"
echo
echo "💡 Your session key is: $session_key"
echo "   Save this to use in your applications."
