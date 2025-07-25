#!/bin/bash

# Test script for Ongaku Subsonic API (XML responses)
# Run this after starting the server

SERVER_URL="https://ongaku-dev.m3r.dev"
USERNAME="test"
VERSION="1.16.1"
CLIENT="test-client"

echo "Testing Ongaku Subsonic API (XML format)..."
echo "==========================================="

# Test root endpoint (server info)
echo "0. Testing root endpoint (server info)..."
curl -s "${SERVER_URL}/rest" | xmllint --format -

# Test ping endpoint
echo -e "\n1. Testing ping endpoint..."
curl -s "${SERVER_URL}/rest/ping?u=${USERNAME}&v=${VERSION}&c=${CLIENT}" | xmllint --format -

echo -e "\n2. Testing getLicense endpoint..."
curl -s "${SERVER_URL}/rest/getLicense?u=${USERNAME}&v=${VERSION}&c=${CLIENT}" | xmllint --format -

echo -e "\n3. Testing getMusicFolders endpoint..."
curl -s "${SERVER_URL}/rest/getMusicFolders?u=${USERNAME}&v=${VERSION}&c=${CLIENT}" | xmllint --format -

echo -e "\n4. Testing getIndexes endpoint..."
curl -s "${SERVER_URL}/rest/getIndexes?u=${USERNAME}&v=${VERSION}&c=${CLIENT}" | xmllint --format -

echo -e "\n5. Testing getGenres endpoint..."
curl -s "${SERVER_URL}/rest/getGenres?u=${USERNAME}&v=${VERSION}&c=${CLIENT}" | xmllint --format -

echo -e "\n6. Testing search3 endpoint..."
curl -s "${SERVER_URL}/rest/search3?u=${USERNAME}&v=${VERSION}&c=${CLIENT}&query=test" | xmllint --format -

echo -e "\n7. Testing getMusicDirectory for first artist..."
# This would need an actual artist ID from your database
# curl -s "${SERVER_URL}/rest/getMusicDirectory?u=${USERNAME}&v=${VERSION}&c=${CLIENT}&id=artist-..." | xmllint --format -

echo -e "\nAPI tests completed!"
