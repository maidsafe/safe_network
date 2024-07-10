#!/usr/bin/env bash

# Function to print a message in a box
print_in_box() {
    local message="$1"
    local border_length=$((${#message} + 4))
    local border=$(printf '%*s' "$border_length" | tr ' ' '#')

    echo "$border"
    echo "# $message #"
    echo "$border"
}

# Check for correct number of arguments
if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
    echo "Usage: $0 <number_of_wallets> <amount_per_wallet> [contact_peer]"
    exit 1
fi

# Get the input arguments
NUM_WALLETS=$1
AMOUNT_PER_WALLET=$2
CONTACT_PEER="${3:-}"

# Prepare contact peer argument
CONTACT_PEER_ARG=""
if [ -n "$CONTACT_PEER" ]; then
    CONTACT_PEER_ARG="--peer $CONTACT_PEER"
fi

# Define directories relative to the current working directory
APP_DIR=$(pwd)
CLIENT_DIR="${APP_DIR}/client"
NEW_CLIENT_DIR_TEMPLATE="${APP_DIR}/client_"

# Initialize an array to store the wallet details
declare -a WALLETS

# Loop to create and fund wallets
for ((i=1; i<=NUM_WALLETS; i++)); do
    # Step 1: Create a new wallet address and capture the new address
    safe wallet address
    start_address=$(safe wallet address | awk 'NR==3')

    # Step 2: Extract the first 5 characters of the wallet address
    start_prefix=$(echo "$start_address" | cut -c1-5)

    # Step 3: Define the new client directory names
    SOURCE_CLIENT_DIR="${NEW_CLIENT_DIR_TEMPLATE}${start_prefix}"

    # Step 4: Rename the original client directory to client_<5PK>
    mv "$CLIENT_DIR" "$SOURCE_CLIENT_DIR"
    print_in_box "Moving source client to $SOURCE_CLIENT_DIR"

    # Step 5: Create a new wallet address and capture the new address
    safe wallet address
    new_address=$(safe wallet address | awk 'NR==3')
    new_prefix=$(echo "$new_address" | cut -c1-5)
    NEW_CLIENT_DIR="${NEW_CLIENT_DIR_TEMPLATE}${new_prefix}"

    print_in_box "Moving new client to $NEW_CLIENT_DIR"

    # Step 6: Move the new client
    mv "$CLIENT_DIR" "$NEW_CLIENT_DIR"

    # Step 7: Restore the original client directory
    mv "$SOURCE_CLIENT_DIR" "$CLIENT_DIR"

    # Step 8: Send tokens to the new address
    echo "New address is: $new_address"
    transfer_output=$(safe $CONTACT_PEER_ARG wallet send "$AMOUNT_PER_WALLET" "$new_address")
    echo "Transfer output is: $transfer_output"

    # Extract the transfer note
    transfer_note=$(echo "$transfer_output" | awk '/Please share this to the recipient:/{getline; getline; print}')

    echo "Transfer note is: $transfer_note"

    # Store the wallet details in the array
    WALLETS+=("Wallet: client_${new_prefix}, Address: $new_address")

    # Step 9: Rename current client directory to client_<5PK>
    mv "$CLIENT_DIR" "$SOURCE_CLIENT_DIR"

    # Step 10: Restore the client_<5PK> to client
    mv "$NEW_CLIENT_DIR" "$CLIENT_DIR"

    # Step 11: Receive the funds using the transfer note
    safe $CONTACT_PEER_ARG wallet receive "$transfer_note"

    # Step 12: Rename the client directories back to their original names
    mv "$CLIENT_DIR" "$NEW_CLIENT_DIR"
    mv "$SOURCE_CLIENT_DIR" "$CLIENT_DIR"
done

# Print the summary of wallets created
print_in_box "Summary of Wallets Created"
for wallet in "${WALLETS[@]}"; do
    echo "$wallet"
done
print_in_box "End of Summary"
