#!/bin/bash

# Target rate of 1.5mb/s


# Get the input argument
CONTACT_PEER="${1:-}"

# Prepare contact peer argument
CONTACT_PEER_ARG=""
if [ -n "$CONTACT_PEER" ]; then
    CONTACT_PEER_ARG="--peer $CONTACT_PEER"
fi


# Function to check if the `safe` command exists, and install it if not
check_and_install_safe() {
  if ! command -v safe &> /dev/null; then
    echo "'safe' command not found. Installing..."
    curl -sSL https://raw.githubusercontent.com/maidsafe/safeup/main/install.sh | sudo bash
    safeup client
  else
    echo "'safe' command is already installed."
  fi
}

# Function to generate a 10MB file of random data
generate_random_data_file() {
  tmpfile=$(mktemp)
  dd if=/dev/urandom of="$tmpfile" bs=2M count=1 iflag=fullblock &> /dev/null

  echo "Generated random data file at $tmpfile"

  # Upload the random data file using SAFE CLI
  safe $CONTACT_PEER_ARG files upload "$tmpfile"
  if [ $? -eq 0 ]; then
    echo "Successfully uploaded $tmpfile using SAFE CLI"
  else
    echo "Failed to upload $tmpfile using SAFE CLI"
  fi

  # Remove the temporary file
  rm "$tmpfile"

  # Log and sleep for 60 seconds
  echo "Sleeping for 60 seconds..."
  sleep 60
}

# Check and install 'safe' if necessary
check_and_install_safe

# Check for correct number of arguments
if [ "$#" -gt 1 ]; then
    echo "Usage: $0 [contact_peer]"
    exit 1
fi

# Example usage
total_files=10000  # Total number of files to generate and upload

# Loop to generate and upload random data files
for i in $(seq 1 $total_files); do
  date +"%A, %B %d, %Y %H:%M:%S"
  echo "Generating and uploading file $i of $total_files..."
  generate_random_data_file

  safe $CONTACT_PEER_ARG wallet balance
done
