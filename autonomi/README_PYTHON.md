## Python Bindings

The Autonomi client library provides Python bindings for easy integration with Python applications.

### Installation

```bash
pip install autonomi-client
```

### Quick Start

```python
from autonomi_client import Client, Wallet, PaymentOption

# Initialize wallet with private key
wallet = Wallet("your_private_key_here")
print(f"Wallet address: {wallet.address()}")
print(f"Balance: {wallet.balance()}")

# Connect to network
client = Client.connect(["/ip4/127.0.0.1/tcp/12000"])

# Create payment option
payment = PaymentOption.wallet(wallet)

# Upload data
data = b"Hello, Safe Network!"
addr = client.data_put_public(data, payment)
print(f"Data uploaded to: {addr}")

# Download data
retrieved = client.data_get_public(addr)
print(f"Retrieved: {retrieved.decode()}")
```

### Available Modules

#### Core Components

- `Client`: Main interface to the Autonomi network
  - `connect(peers: List[str])`: Connect to network nodes
  - `data_put_public(data: bytes, payment: PaymentOption)`: Upload data
  - `data_get_public(addr: str)`: Download data
  - `private_data_put(data: bytes, payment: PaymentOption)`: Store private data
  - `private_data_get(access: PrivateDataAccess)`: Retrieve private data
  - `register_generate_key()`: Generate register key

- `Wallet`: Ethereum wallet management
  - `new(private_key: str)`: Create wallet from private key
  - `address()`: Get wallet address
  - `balance()`: Get current balance

- `PaymentOption`: Payment configuration
  - `wallet(wallet: Wallet)`: Create payment option from wallet

#### Private Data

- `PrivateDataAccess`: Handle private data storage
  - `from_hex(hex: str)`: Create from hex string
  - `to_hex()`: Convert to hex string
  - `address()`: Get short reference address

```python
# Private data example
access = client.private_data_put(secret_data, payment)
print(f"Private data stored at: {access.to_hex()}")
retrieved = client.private_data_get(access)
```

#### Registers

- Register operations for mutable data
  - `register_create(value: bytes, name: str, key: RegisterSecretKey, wallet: Wallet)`
  - `register_get(address: str)`
  - `register_update(register: Register, value: bytes, key: RegisterSecretKey)`

```python
# Register example
key = client.register_generate_key()
register = client.register_create(b"Initial value", "my_register", key, wallet)
client.register_update(register, b"New value", key)
```

#### Vaults

- `VaultSecretKey`: Manage vault access
  - `new()`: Generate new key
  - `from_hex(hex: str)`: Create from hex string
  - `to_hex()`: Convert to hex string

- `UserData`: User data management
  - `new()`: Create new user data
  - `add_file_archive(archive: str)`: Add file archive
  - `add_private_file_archive(archive: str)`: Add private archive
  - `file_archives()`: List archives
  - `private_file_archives()`: List private archives

```python
# Vault example
vault_key = VaultSecretKey.new()
cost = client.vault_cost(vault_key)
client.write_bytes_to_vault(data, payment, vault_key, content_type=1)
data, content_type = client.fetch_and_decrypt_vault(vault_key)
```

#### Utility Functions

- `encrypt(data: bytes)`: Self-encrypt data
- `hash_to_short_string(input: str)`: Generate short reference

### Complete Examples

#### Data Management

```python
def handle_data_operations(client, payment):
    # Upload text
    text_data = b"Hello, Safe Network!"
    text_addr = client.data_put_public(text_data, payment)
    
    # Upload binary data
    with open("image.jpg", "rb") as f:
        image_data = f.read()
        image_addr = client.data_put_public(image_data, payment)
    
    # Download and verify
    downloaded = client.data_get_public(text_addr)
    assert downloaded == text_data
```

#### Private Data and Encryption

```python
def handle_private_data(client, payment):
    # Create and encrypt private data
    secret = {"api_key": "secret_key"}
    data = json.dumps(secret).encode()
    
    # Store privately
    access = client.private_data_put(data, payment)
    print(f"Access token: {access.to_hex()}")
    
    # Retrieve
    retrieved = client.private_data_get(access)
    secret = json.loads(retrieved.decode())
```

#### Vault Management

```python
def handle_vault(client, payment):
    # Create vault
    vault_key = VaultSecretKey.new()
    
    # Store user data
    user_data = UserData()
    user_data.add_file_archive("archive_address")
    
    # Save to vault
    cost = client.put_user_data_to_vault(vault_key, payment, user_data)
    
    # Retrieve
    retrieved = client.get_user_data_from_vault(vault_key)
    archives = retrieved.file_archives()
```

### Error Handling

All operations can raise exceptions. It's recommended to use try-except blocks:

```python
try:
    client = Client.connect(peers)
    # ... operations ...
except Exception as e:
    print(f"Error: {e}")
```

### Best Practices

1. Always keep private keys secure
2. Use error handling for all network operations
3. Clean up resources when done
4. Monitor wallet balance for payments
5. Use appropriate content types for vault storage

For more examples, see the `examples/` directory in the repository.
