from autonomi_client import Client, Wallet, PaymentOption

def main():
    # Initialize a wallet with a private key
    # This should be a valid Ethereum private key (64 hex chars without '0x' prefix)
    private_key = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    wallet = Wallet(private_key)
    print(f"Wallet address: {wallet.address()}")
    print(f"Wallet balance: {wallet.balance()}")

    # Connect to the network
    # These should be valid multiaddresses of network nodes
    peers = [
        "/ip4/127.0.0.1/tcp/12000",
        "/ip4/127.0.0.1/tcp/12001"
    ]
    client = Client.connect(peers)

    # Create payment option using the wallet
    payment = PaymentOption.wallet(wallet)

    # Upload some data
    data = b"Hello, Safe Network!"
    addr = client.data_put(data, payment)
    print(f"Data uploaded to address: {addr}")

    # Download the data back
    downloaded = client.data_get(addr)
    print(f"Downloaded data: {downloaded.decode()}")

    # You can also upload files
    with open("example.txt", "rb") as f:
        file_data = f.read()
        file_addr = client.data_put(file_data, payment)
        print(f"File uploaded to address: {file_addr}")

if __name__ == "__main__":
    main() 