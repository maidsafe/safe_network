#!/bin/bash

BUCKET_NAME="your-bucket-name"
DESTINATION_PATH="s3://nat-detection/nat-detection-0.1.0-aarch64-unknown-linux-musl.zip"
LOCAL_FILE_PATH="path/to/local/file"

if aws s3 ls "$DESTINATION_PATH" > /dev/null 2>&1; then
    echo "Error: Destination file already exists in the bucket."
    exit 1
else
    echo "will upload"
fi
