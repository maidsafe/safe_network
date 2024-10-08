## JavaScript Autonomi API Documentation

Note that this is a first version and will be subject to change.

### **Client**

The `Client` object allows interaction with the network to store and retrieve data. Below are the available methods for the `Client` class.

#### **Constructor**

```javascript
let client = await new Client([multiaddress]);
```

- **multiaddress** (Array of Strings): A list of network addresses for the client to connect to.
  
Example:
```javascript
let client = await new Client(["/ip4/127.0.0.1/tcp/36075/ws/p2p/12D3KooWALb...BhDAfJY"]);
```

#### **Methods**

##### **put(data, wallet)**

Uploads a piece of encrypted data to the network.

```javascript
let result = await client.put(data, wallet);
```

- **data** (Uint8Array): The data to be stored.
- **wallet** (Wallet): The wallet used to pay for the storage.

Returns:
- **result** (XorName): The XOR address of the stored data.

Example:
```javascript
let wallet = getFundedWallet();
let data = new Uint8Array([1, 2, 3]);
let result = await client.put(data, wallet);
```

##### **get(data_map_addr)**

Fetches encrypted data from the network using its XOR address.

```javascript
let data = await client.get(data_map_addr);
```

- **data_map_addr** (XorName): The XOR address of the data to fetch.

Returns:
- **data** (Uint8Array): The fetched data.

Example:
```javascript
let data = await client.get(result);
```

##### **cost(data)**

Gets the cost of storing the provided data on the network.

```javascript
let cost = await client.cost(data);
```

- **data** (Uint8Array): The data whose storage cost you want to calculate.

Returns:
- **cost** (AttoTokens): The calculated cost for storing the data.

Example:
```javascript
let cost = await client.cost(new Uint8Array([1, 2, 3]));
```

---

### **Wallet**

The `Wallet` object represents an Ethereum wallet used for data payments.

#### **Methods**

##### **new_from_private_key(network, private_key)**

Creates a new wallet using the given private key.

```javascript
let wallet = Wallet.new_from_private_key(network, private_key);
```

- **network** (EvmNetwork): The network to which the wallet connects.
- **private_key** (String): The private key of the wallet.

Returns:
- **wallet** (Wallet): The created wallet.

Example:
```javascript
let wallet = Wallet.new_from_private_key(EvmNetwork.default(), "your_private_key_here");
```

##### **address()**

Gets the wallet’s address.

```javascript
let address = wallet.address();
```

Returns:
- **address** (Address): The wallet's address.

Example:
```javascript
let wallet = Wallet.new_from_private_key(EvmNetwork.default(), "your_private_key_here");
let address = wallet.address();
```

---

### **EvmNetwork**

The `EvmNetwork` object represents the blockchain network.

#### **Methods**

##### **default()**

Connects to the default network.

```javascript
let network = EvmNetwork.default();
```

Returns:
- **network** (EvmNetwork): The default network.

Example:
```javascript
let network = EvmNetwork.default();
```

---

### Example Usage:

```javascript
let client = await new Client(["/ip4/127.0.0.1/tcp/36075/ws/p2p/12D3KooWALb...BhDAfJY"]);
console.log("connected");

let wallet = Wallet.new_from_private_key(EvmNetwork.default(), "your_private_key_here");
console.log("wallet retrieved");

let data = new Uint8Array([1, 2, 3]);
let result = await client.put(data, wallet);
console.log("Data stored at:", result);

let fetchedData = await client.get(result);
console.log("Data retrieved:", fetchedData);
```

---

This documentation covers the basic usage of `Client`, `Wallet`, and `EvmNetwork` types in the JavaScript API.