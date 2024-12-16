from autonomi_client import (
    Client, Wallet, PaymentOption, DataMapChunk,
    encrypt, hash_to_short_string
)
import json

def demonstrate_private_data(client: Client, payment: PaymentOption):
    """Show private data handling"""
    print("\n=== Private Data Operations ===")
    
    # Create some private data
    secret_data = {
        "password": "very_secret",
        "api_key": "super_secret_key"
    }
    data_bytes = json.dumps(secret_data).encode()
    
    # Store it privately
    access = client.data_put(data_bytes, payment)
    print(f"Stored private data, access token: {access.to_hex()}")
    print(f"Short reference: {access.address()}")
    
    # Retrieve it
    retrieved_bytes = client.data_get(access)
    retrieved_data = json.loads(retrieved_bytes.decode())
    print(f"Retrieved private data: {retrieved_data}")
    
    return access.to_hex()

def demonstrate_encryption():
    """Show self-encryption functionality"""
    print("\n=== Self-Encryption Operations ===")
    
    # Create test data
    test_data = b"This is some test data for encryption"
    
    # Encrypt it
    data_map, chunks = encrypt(test_data)
    print(f"Original data size: {len(test_data)} bytes")
    print(f"Data map size: {len(data_map)} bytes")
    print(f"Number of chunks: {len(chunks)}")
    print(f"Total chunks size: {sum(len(c) for c in chunks)} bytes")

def main():
    # Initialize
    private_key = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    peers = ["/ip4/127.0.0.1/tcp/12000"]
    
    try:
        # Setup
        wallet = Wallet(private_key)
        print(f"Wallet address: {wallet.address()}")
        print(f"Wallet balance: {wallet.balance()}")
        
        client = Client.connect(peers)
        payment = PaymentOption.wallet(wallet)
        
        # Run demonstrations
        access_token = demonstrate_private_data(client, payment)
        demonstrate_encryption()
        
        # Show utility function
        print("\n=== Utility Functions ===")
        short_hash = hash_to_short_string(access_token)
        print(f"Short hash of access token: {short_hash}")
        
    except Exception as e:
        print(f"Error: {e}")
        return 1
    
    print("\nAll operations completed successfully!")
    return 0

if __name__ == "__main__":
    exit(main()) 