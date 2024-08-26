# sn_registers

Provides utilities for working with registers on the Safe Network.

## Introduction to Registers

Registers are a fundamental data structure in the Safe Network,
designed for storing and managing mutable data with strong consistency guarantees.
They are particularly useful for scenarios requiring atomic updates
and conflict resolution in a distributed environment.

### General Purpose and Structure

A register consists of:
- A unique address on the network, determined by its meta and owner
  - meta being a user specific string, or a `name` of the register
- `Permissions` showing the who can mutate the register
- An inner CRDT data, which holds the actuall content and the update history

Registers are:
- Replicated: Stored across multiple nodes for redundancy and availability.
- Versioned: Each update creates a new version, allowing for history tracking.
- Conflict-resistant: Uses a Conflict-free Replicated Data Type (CRDT) approach.

### API and Workflow

The `sn_registers` crate provides a high-level API for interacting with registers:

1. Create a new register
2. Read the current state of a register
3. Write new data to a register
4. Merge register with different versions

Basic workflow:
1. Initialize a connection to the Safe Network
2. Create or retrieve a register by its address
3. Perform operations (read/write) on the register
4. Handle any conflicts that may arise during concurrent updates

### Constraints and Limitations

- Size limits: Individual entry has a maximum size (1024 bytes),
               and a register shall have max 1024 entires
- Write permissions: Only authorized owners can modify a register
- Network dependency: Operations require a connection to the Safe Network

### Understanding MerkleReg in the crdts Crate
1. Purpose of MerkleReg

MerkleReg is a CRDT that maintains a single value but keeps track of all the changes (mutations) made to that value.
It uses a Merkle tree to store and verify the history of mutations.
This allows for efficient verification of the state of the register and the history of changes,
which is particularly useful in distributed systems where you may need to prove the integrity of data.

2. Structure of MerkleReg

The MerkleReg CRDT typically consists of:
	* Value: The current value stored in the register.
	* History: A Merkle tree that stores the history of all previous values.
	Each mutation adds a new node to the tree, which is cryptographically linked to its predecessors,
	forming a secure chain of updates.

3. Mutating the Register

When you mutate the MerkleReg, the following happens:
	* The current value is replaced with the new value.
	* The mutation is recorded in the Merkle tree by creating a new node
	that includes a cryptographic hash of the new value and the hash of the previous state (root of the Merkle tree).

4. Conflict Resolution

Like other CRDTs, MerkleReg resolves conflicts automatically.
If two or more replicas concurrently update the register with different values,
the CRDT framework handles merging these changes.
The Merkle tree structure helps in efficiently reconciling these updates by comparing the histories.

5. Showing Mutation History in MerkleReg

To show the mutation history in a MerkleReg, you can traverse the Merkle tree,
listing all previous values and their associated metadata (such as timestamps or versions).
Here’s how you might approach this in practice:
- Traversing the Merkle Tree: 
    To retrieve the mutation history, you need to walk through the Merkle tree stored in the MerkleReg.
	The tree is composed of nodes where each node represents a mutation, containing:
  - The value after the mutation.
  - A hash that links back to the previous state.
- Displaying the History: 
	You can then display each value along with its position in the Merkle tree (e.g., the hash or index).
	This provides a chronological view of the register’s state over time.

## Examples

Here are some simple scenarios using the `sn_registers` crate:

1. Creating and writing to a register:
```rust
// `permissions` defines the owner of the register
let mut register = Register::new(owner.pub_key, meta, permissions);
let entry = Entry::new("Hello, Safe Network!".as_bytes().to_vec());
// Children being an empty list for a newly created register
register.write(entry, children, owner.priv_key).await?;
```

2. Reading from a register:
```rust
/// Note this reads the root content (i.e. the last entry) of the inner crdt.
/// It will return with multiple `roots`, when there are branches of the inner crdt.
let root_contents = register.read().await?;
for content in root_contents {
	println!("content: {:?}", String::from_utf8(content.value)?);
}
```

3. Merge registers:
```rust
/// Note two registers are only mergeable when they are for the same address and permissions.
/// And it is the inner crdt to be merged.
pub fn merge(&mut self, other: &Register) -> Result<()> {
    self.verify_is_mergeable(other)?;
    self.crdt.merge(other.crdt.clone());
    Ok(())
}
```
