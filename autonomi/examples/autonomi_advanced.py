from autonomi_client import Client, Wallet, PaymentOption
import sys

def init_wallet(private_key: str) -> Wallet:
    try:
        wallet = Wallet(private_key)
        print(f"Initialized wallet with address: {wallet.address()}")
        
        balance = wallet.balance()
        print(f"Wallet balance: {balance}")
        
        return wallet
    except Exception as e:
        print(f"Failed to initialize wallet: {e}")
        sys.exit(1)

def connect_to_network(peers: list[str]) -> Client:
    try:
        client = Client.connect(peers)
        print("Successfully connected to network")
        return client
    except Exception as e:
        print(f"Failed to connect to network: {e}")
        sys.exit(1)

def upload_data(client: Client, data: bytes, payment: PaymentOption) -> str:
    try:
        addr = client.data_put_public(data, payment)
        print(f"Successfully uploaded data to: {addr}")
        return addr
    except Exception as e:
        print(f"Failed to upload data: {e}")
        sys.exit(1)

def download_data(client: Client, addr: str) -> bytes:
    try:
        data = client.data_get_public(addr)
        print(f"Successfully downloaded {len(data)} bytes")
        return data
    except Exception as e:
        print(f"Failed to download data: {e}")
        sys.exit(1)

def main():
    # Configuration
    private_key = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    peers = ["/ip4/127.0.0.1/tcp/12000"]

    # Initialize
    wallet = init_wallet(private_key)
    client = connect_to_network(peers)
    payment = PaymentOption.wallet(wallet)

    # Upload test data
    test_data = b"Hello, Safe Network!"
    addr = upload_data(client, test_data, payment)

    # Download and verify
    downloaded = download_data(client, addr)
    assert downloaded == test_data, "Data verification failed!"
    print("Data verification successful!")

    # Example file handling
    try:
        with open("example.txt", "rb") as f:
            file_data = f.read()
            file_addr = upload_data(client, file_data, payment)
            
            # Download and save to new file
            downloaded = download_data(client, file_addr)
            with open("example_downloaded.txt", "wb") as f_out:
                f_out.write(downloaded)
            print("File operations completed successfully!")
    except IOError as e:
        print(f"File operation failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main() 