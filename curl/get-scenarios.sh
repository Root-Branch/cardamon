#!/bin/sh

# Get the HTTP status code
status_code=$(curl -s -o /dev/null -I -w "%{http_code}" 'http://127.0.0.1:7001/api/scenarios')
# Check if the status code is 200
if [ "$status_code" = "200" ]; then
    # Status is 200, proceed with getting and processing the data
    curl 'http://127.0.0.1:7001/api/scenarios' > scenarios.json
    
    # Try to format the JSON with jq
    if ! jq . scenarios.json > scenarios-formatted.json 2>/dev/null; then
        echo "Error: Failed to parse JSON. Raw response:"
        cat scenarios.json
        rm scenarios.json
        exit 1
    fi
    
    # If jq succeeds, continue with the original flow
    rm scenarios.json
    mv scenarios-formatted.json scenarios.json
    nvim scenarios.json
else
    curl 'http://127.0.0.1:7001/api/scenarios'
    echo "\nError: Server responded with status code $status_code"

    exit 1
fi
