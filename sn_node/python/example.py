from safenode import SafeNode

# Create a new node instance
node = SafeNode()
initial_peers = ["/ip4/142.93.37.4/udp/40184/quic-v1/p2p/12D3KooWPC8q7QGZsmuTtCYxZ2s3FPXPZcS8LVKkayXkVFkqDEQB",
  "/ip4/157.245.40.2/udp/33698/quic-v1/p2p/12D3KooWNyNNTGfwGf6fYyvrk4zp5EHxPhNDVNB25ZzEt2NXbCq2",
  "/ip4/157.245.40.2/udp/33991/quic-v1/p2p/12D3KooWHPyZVAHqp2ebzKyxxsYzJYS7sNysfcLg2s1JLtbo6vhC"]
# Start the node with initial rewards address
initial_rewards_address = "0x1234567890123456789012345678901234567890"
print(f"Starting node with rewards address: {initial_rewards_address}")

node.run(
    rewards_address=initial_rewards_address,
    evm_network="arbitrum_sepolia",
    ip="0.0.0.0",
    port=12000,
    initial_peers=initial_peers,
    local=True,
    root_dir=None,
    home_network=False
)

# Get the current rewards address
current_address = node.get_rewards_address()
print(f"Current rewards address: {current_address}")

# Verify it matches what we set
assert current_address.lower() == initial_rewards_address.lower(), "Rewards address mismatch!"

# Try to set a new rewards address (this will raise an error since it requires restart)
new_address = "0x9876543210987654321098765432109876543210"
try:
    node.set_rewards_address(new_address)
    print("This line won't be reached due to the error")
except RuntimeError as e:
    print(f"Expected error when trying to change address: {e}")

# Get the node's peer ID
peer_id = node.peer_id()
print(f"Node peer ID: {peer_id}")

# Get all record addresses
addresses = node.get_all_record_addresses()
print(f"Record addresses: {addresses}")

# Get kbuckets information
kbuckets = node.get_kbuckets()
for distance, peers in kbuckets:
    print(f"Distance {distance}: {len(peers)} peers")

# To actually change the rewards address, you would need to:
# 1. Stop the current node
# 2. Create a new node with the new address
print("\nDemonstrating rewards address change with node restart:")
node = SafeNode()  # Create new instance
print(f"Starting node with new rewards address: {new_address}")

node.run(
    rewards_address=new_address,
    evm_network="arbitrum_sepolia",
    ip="0.0.0.0",
    port=12000,
    initial_peers=[],
    local=True,
    root_dir=None,
    home_network=False
)

# Verify the new address was set
current_address = node.get_rewards_address()
print(f"New current rewards address: {current_address}")
assert current_address.lower() == new_address.lower(), "New rewards address mismatch!"