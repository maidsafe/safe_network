from autonomi_client import Client, Wallet, PaymentOption, RegisterSecretKey
import hashlib

def handle_data_operations(client: Client, payment: PaymentOption):
    """Example of various data operations"""
    print("\n=== Data Operations ===")
    
    # Upload some text data
    text_data = b"Hello, Safe Network!"
    text_addr = client.data_put(text_data, payment)
    print(f"Text data uploaded to: {text_addr}")
    
    # Upload binary data (like an image)
    with open("example.jpg", "rb") as f:
        image_data = f.read()
        image_addr = client.data_put(image_data, payment)
        print(f"Image uploaded to: {image_addr}")
    
    # Download and verify data
    downloaded_text = client.data_get(text_addr)
    assert downloaded_text == text_data, "Text data verification failed!"
    print("Text data verified successfully")
    
    # Download and save image
    downloaded_image = client.data_get(image_addr)
    with open("downloaded_example.jpg", "wb") as f:
        f.write(downloaded_image)
    print("Image downloaded successfully")

def handle_register_operations(client: Client, wallet: Wallet):
    """Example of register operations"""
    print("\n=== Register Operations ===")
    
    # Create a register key
    register_key = client.register_generate_key()
    print(f"Generated register key")
    
    # Create a register with initial value
    register_name = "my_first_register"
    initial_value = b"Initial register value"
    register = client.register_create(
        initial_value,
        register_name,
        register_key,
        wallet
    )
    print(f"Created register at: {register.address()}")
    
    # Read current value
    values = register.values()
    print(f"Current register values: {[v.decode() for v in values]}")
    
    # Update register value
    new_value = b"Updated register value"
    client.register_update(register, new_value, register_key)
    print("Register updated")
    
    # Read updated value
    updated_register = client.register_get(register.address())
    updated_values = updated_register.values()
    print(f"Updated register values: {[v.decode() for v in updated_values]}")

def main():
    # Initialize wallet and client
    private_key = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    peers = ["/ip4/127.0.0.1/tcp/12000"]
    
    try:
        # Setup
        wallet = Wallet(private_key)
        print(f"Wallet address: {wallet.address()}")
        print(f"Wallet balance: {wallet.balance()}")
        
        client = Client.connect(peers)
        payment = PaymentOption.wallet(wallet)
        
        # Run examples
        handle_data_operations(client, payment)
        handle_register_operations(client, wallet)
        
    except Exception as e:
        print(f"Error: {e}")
        return 1
    
    print("\nAll operations completed successfully!")
    return 0

if __name__ == "__main__":
    exit(main()) 