#!/bin/bash

# Subsonic API Test Script
# This script tests the basic functionality of the Subsonic API implementation

BASE_URL="http://localhost:4000/rest"
USERNAME="admin"
PASSWORD="admin"
VERSION="1.16.1"
CLIENT="test-script"
FORMAT="json"

# Common parameters
PARAMS="u=${USERNAME}&p=${PASSWORD}&v=${VERSION}&c=${CLIENT}&f=${FORMAT}"

echo "🎵 Testing Subsonic API Implementation"
echo "======================================="
echo

echo "1. Testing ping endpoint..."
curl -s "${BASE_URL}/ping?${PARAMS}" | jq '.' || echo "Failed to parse JSON"
echo
echo

echo "2. Testing getMusicFolders..."
curl -s "${BASE_URL}/getMusicFolders?${PARAMS}" | jq '.' || echo "Failed to parse JSON"
echo
echo

echo "3. Testing getIndexes..."
curl -s "${BASE_URL}/getIndexes?${PARAMS}" | jq '.' || echo "Failed to parse JSON"
echo
echo

echo "4. Testing getArtists..."
curl -s "${BASE_URL}/getArtists?${PARAMS}" | jq '.' || echo "Failed to parse JSON"
echo
echo

echo "5. Testing search3..."
curl -s "${BASE_URL}/search3?query=test&${PARAMS}" | jq '.' || echo "Failed to parse JSON"
echo
echo

echo "6. Testing invalid authentication..."
curl -s "${BASE_URL}/ping?u=invalid&p=invalid&v=${VERSION}&c=${CLIENT}&f=${FORMAT}" | jq '.' || echo "Failed to parse JSON"
echo
echo

echo "✅ Subsonic API tests completed!"
echo
echo "To test with a specific music library, make sure the server is running"
echo "and has scanned your music directory."
echo
echo "Example manual tests:"
echo "curl \"${BASE_URL}/ping?${PARAMS}\""
echo "curl \"${BASE_URL}/getArtists?${PARAMS}\""
echo "curl \"${BASE_URL}/search3?query=rock&${PARAMS}\""
