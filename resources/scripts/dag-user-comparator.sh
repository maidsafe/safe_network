#!/usr/bin/env bash

# Check if the correct number of arguments is provided
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <source_server> <target_server>"
    exit 1
fi

# Function to check if a command is available
check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo "Error: $1 is not installed."
        case "$1" in
            curl)
                echo "Install curl using:"
                echo "  sudo apt-get install curl    # Ubuntu/Debian"
                echo "  sudo dnf install curl        # Fedora"
                echo "  brew install curl            # MacOS"
                ;;
            jq)
                echo "Install jq using:"
                echo "  sudo apt-get install jq      # Ubuntu/Debian"
                echo "  sudo dnf install jq          # Fedora"
                echo "  brew install jq              # MacOS"
                ;;
            rg)
                echo "Install ripgrep using:"
                echo "  sudo apt-get install ripgrep # Ubuntu/Debian"
                echo "  sudo dnf install ripgrep     # Fedora"
                echo "  brew install ripgrep         # MacOS"
                ;;
        esac
        exit 1
    fi
}

# Check for required commands
check_command curl
check_command jq
check_command rg

# Set PATH variable to ensure the script can find curl, jq, and rg
export PATH=$PATH:/usr/bin:/usr/local/bin

# Server addresses from arguments
SOURCE_SERVER="$1"
TARGET_SERVER="$2"

# Fixed path
FIXED_PATH="/beta-rewards"

# Construct URLs
SOURCE_URL="http://$SOURCE_SERVER$FIXED_PATH"
TARGET_URL="http://$TARGET_SERVER$FIXED_PATH"

# Fetch the JSON data from the source URL
echo "Fetching data from $SOURCE_URL"
source_json=$(curl -s $SOURCE_URL)

# Fetch the JSON data from the target URL
echo "Fetching data from $TARGET_URL"
target_json=$(curl -s $TARGET_URL)

# Parse the JSON data to extract names, skipping unknown participants
source_names=$(echo $source_json | jq -r '.[] | select(.[0] | test("^unknown participant") | not) | .[0]')
target_names=$(echo $target_json | jq -r '.[] | select(.[0] | test("^unknown participant") | not) | .[0]')

# Convert names to arrays
source_names_array=($source_names)
target_names_array=($target_names)

# Compare and find missing names on target server
echo "Comparing source to target..."
missing_on_target=()
for name in "${source_names_array[@]}"; do
    if ! printf '%s\n' "${target_names_array[@]}" | rg -q "^$name$"; then
        missing_on_target+=("$name")
    fi
done

# Compare and find missing names on source server
echo "Comparing target to source..."
missing_on_source=()
for name in "${target_names_array[@]}"; do
    if ! printf '%s\n' "${source_names_array[@]}" | rg -q "^$name$"; then
        missing_on_source+=("$name")
    fi
done

# Output the results with URLs for clarity
echo "Missing on target server (${#missing_on_target[@]}) from $TARGET_URL:"
for name in "${missing_on_target[@]}"; do
    echo "$name"
done

echo "Missing on source server (${#missing_on_source[@]}) from $SOURCE_URL:"
for name in "${missing_on_source[@]}"; do
    echo "$name"
done

# Summarize user counts
source_count=${#source_names_array[@]}
target_count=${#target_names_array[@]}
echo "Summary:"
echo "Total users on source server ($SOURCE_URL): $source_count"
echo "Total users on target server ($TARGET_URL): $target_count"

# Summarize if target is missing anything or if it is ahead of source
if [ ${#missing_on_target[@]} -gt 0 ]; then
    echo "The target server ($TARGET_URL) is missing ${#missing_on_target[@]} users compared to the source server."
else
    echo "The target server ($TARGET_URL) is up-to-date with the source server."
fi

if [ ${#missing_on_source[@]} -gt 0 ]; then
    echo "The target server ($TARGET_URL) has ${#missing_on_source[@]} users that are not present on the source server."
else
    echo "The target server ($TARGET_URL) has no additional users compared to the source server."
fi

echo "Comparison complete."
