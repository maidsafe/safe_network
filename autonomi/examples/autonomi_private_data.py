from autonomi_client import Client, Wallet, PaymentOption, RegisterSecretKey, RegisterPermissions
from typing import List, Optional
import json

class DataManager:
    def __init__(self, client: Client, wallet: Wallet):
        self.client = client
        self.wallet = wallet
        self.payment = PaymentOption.wallet(wallet)
        
    def store_private_data(self, data: bytes) -> str:
        """Store data privately and return its address"""
        addr = self.client.private_data_put(data, self.payment)
        return addr
        
    def retrieve_private_data(self, addr: str) -> bytes:
        """Retrieve privately stored data"""
        return self.client.private_data_get(addr)
        
    def create_shared_register(self, name: str, initial_value: bytes, 
                             allowed_writers: List[str]) -> str:
        """Create a register that multiple users can write to"""
        register_key = self.client.register_generate_key()
        
        # Create permissions for all writers
        permissions = RegisterPermissions.new_with(allowed_writers)
        
        register = self.client.register_create_with_permissions(
            initial_value,
            name,
            register_key,
            permissions,
            self.wallet
        )
        
        return register.address()

def main():
    # Initialize
    private_key = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    peers = ["/ip4/127.0.0.1/tcp/12000"]
    
    try:
        wallet = Wallet(private_key)
        client = Client.connect(peers)
        manager = DataManager(client, wallet)
        
        # Store private data
        user_data = {
            "username": "alice",
            "preferences": {
                "theme": "dark",
                "notifications": True
            }
        }
        private_data = json.dumps(user_data).encode()
        private_addr = manager.store_private_data(private_data)
        print(f"Stored private data at: {private_addr}")
        
        # Retrieve and verify private data
        retrieved_data = manager.retrieve_private_data(private_addr)
        retrieved_json = json.loads(retrieved_data.decode())
        print(f"Retrieved data: {retrieved_json}")
        
        # Create shared register
        allowed_writers = [
            wallet.address(),  # self
            "0x1234567890abcdef1234567890abcdef12345678"  # another user
        ]
        register_addr = manager.create_shared_register(
            "shared_config",
            b"initial shared data",
            allowed_writers
        )
        print(f"Created shared register at: {register_addr}")
        
        # Verify register
        register = client.register_get(register_addr)
        values = register.values()
        print(f"Register values: {[v.decode() for v in values]}")
        
    except Exception as e:
        print(f"Error: {e}")
        return 1
        
    print("All operations completed successfully!")
    return 0

if __name__ == "__main__":
    exit(main()) 