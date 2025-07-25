#!/bin/bash

# Example API calls for testing the Ongaku Server REST API

BASE_URL="http://localhost:3000/api/v1"

echo "=== Testing Ongaku Server REST API ==="
echo

# Test 1: Get statistics
echo "1. Getting database statistics:"
curl -s "$BASE_URL/stats" | jq .
echo
echo

# Test 2: Get list of artists
echo "2. Getting first 10 artists:"
curl -s "$BASE_URL/artists" | jq '.[0:10]'
echo
echo

# Test 3: Get tracks with pagination
echo "3. Getting first 5 tracks (notice tags are now JSON objects):"
curl -s "$BASE_URL/tracks?per_page=5&page=1" | jq .
echo
echo

# Test 4: Search for tracks
echo "4. Searching for tracks (replace 'rock' with actual term):"
curl -s "$BASE_URL/tracks/search?q=rock&per_page=3" | jq .
echo
echo

# Test 5: Filter tracks by artist
echo "5. Filtering tracks by artist (replace 'Beatles' with actual artist):"
curl -s "$BASE_URL/tracks?artist=Beatles&per_page=3" | jq .
echo
echo

# Test 6: Get genres
echo "6. Getting all genres:"
curl -s "$BASE_URL/genres" | jq .
echo
echo

# Test 7: Get albums
echo "7. Getting first 10 albums:"
curl -s "$BASE_URL/albums" | jq '.[0:10]'
echo
echo

echo "=== API Testing Complete ==="
echo "Note: Some queries may return empty results if your database doesn't contain matching data."
echo "Make sure jq is installed for pretty JSON formatting."
