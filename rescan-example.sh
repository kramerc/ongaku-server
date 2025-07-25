#!/bin/bash

# Example of triggering a music library rescan via the API

BASE_URL="http://localhost:4000/api/v1"

echo "=== Triggering Music Library Rescan ==="
echo

# Get current stats before rescan
echo "1. Current database statistics:"
curl -s "$BASE_URL/stats" | jq .
echo
echo

# Trigger the rescan
echo "2. Triggering rescan..."
response=$(curl -s -X POST "$BASE_URL/rescan")
echo "$response" | jq .

# Check if rescan was successful
status=$(echo "$response" | jq -r '.status')
if [ "$status" = "success" ]; then
    echo
    echo "✅ Rescan initiated successfully!"
    echo
    echo "3. The rescan is now running in the background."
    echo "   You can check the server logs for progress updates."
    echo "   Or monitor the stats endpoint for changes:"
    echo
    echo "   curl $BASE_URL/stats"
    echo
else
    echo
    echo "❌ Failed to initiate rescan"
    echo
fi

echo "=== Rescan Complete ==="
