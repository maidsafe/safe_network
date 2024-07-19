#!/usr/bin/env bash

# Check if the correct number of arguments is provided
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <source_server> <target_server>"
    exit 1
fi

# Server addresses from arguments
SOURCE_SERVER="$1"
TARGET_SERVER="$2"

# Fixed paths
SOURCE_PATH="/beta-rewards"
TARGET_PATH="/add-participant"

# Construct URLs
SOURCE_URL="http://$SOURCE_SERVER$SOURCE_PATH"
TARGET_URL="http://$TARGET_SERVER$TARGET_PATH"

# Fetch the JSON data from the source URL
echo "Fetching data from $SOURCE_URL"
json_data=$(curl -s $SOURCE_URL)

# Parse the JSON data to extract names, skipping unknown participants
names=$(echo $json_data | jq -r '.[] | select(.[0] | test("^unknown participant") | not) | .[0]')

# Function to HTML encode a string
html_encode() {
    local encoded=""
    local i
    local c
    for (( i=0; i<${#1}; i++ )); do
        c=$(printf "%d" "'${1:$i:1}")
        if [[ $c -eq 38 ]]; then
            encoded+="&amp;"
        elif [[ $c -eq 60 ]]; then
            encoded+="&lt;"
        elif [[ $c -eq 62 ]]; then
            encoded+="&gt;"
        elif [[ $c -eq 34 ]]; then
            encoded+="&quot;"
        elif [[ $c -eq 39 ]]; then
            encoded+="&#39;"
        elif [[ $c -gt 127 ]]; then
            encoded+="&#$c;"
        else
            encoded+="${1:$i:1}"
        fi
    done
    echo $encoded
}

# Loop through each name, HTML encode it, and make the GET request
for name in $names; do
    encoded_name=$(html_encode "$name")
    echo "Processing name: $name -> $encoded_name"
    # echo "Would call:$TARGET_URL/$encoded_name"
    response=$(curl -s "$TARGET_URL/$encoded_name")
    echo "Response for $encoded_name: $response"
done

echo "Verification step: Checking if all applied names exist on the second server."

# Fetch the JSON data from the target server's beta-rewards path
TARGET_JSON_URL="http://$TARGET_SERVER$SOURCE_PATH"
echo "Fetching data from $TARGET_JSON_URL"
target_json_data=$(curl -s $TARGET_JSON_URL)

# Parse the JSON data from the target server to extract names, skipping unknown participants
target_names=$(echo $target_json_data | jq -r '.[] | select(.[0] | test("^unknown participant") | not) | .[0]')

# Verify if all names exist on the target server
missing_names=0
for name in $names; do
    if echo "$target_names" | grep -q "^$name$"; then
        echo "Verified: $name exists on the target server."
    else
        echo "Error: $name does not exist on the target server."
        missing_names=$((missing_names + 1))
    fi
done

if [ $missing_names -eq 0 ]; then
    echo "Verification complete: All names exist on the target server."
else
    echo "Verification complete: $missing_names names do not exist on the target server."
fi
