# Autonomi JS API

Note: the JS API is experimental and will be subject to change.

The entry point for connecting to the network is {@link Client.connect}.

This API is a wrapper around the Rust API, found here: https://docs.rs/autonomi/latest/autonomi. The Rust API contains more detailed documentation on concepts and some types.

## Addresses

For addresses (chunk, data, archives, etc) we're using hex-encoded strings containing a 256-bit XOR addresse. For example: `abcdefg012345678900000000000000000000000000000000000000000000000`.

## Example

Note: `getEvmNetwork` will use hardcoded EVM network values that should be set during compilation of this library.

```javascript
import init, { Client, Wallet, getEvmNetwork } from 'autonomi';

let client = await new Client(["/ip4/127.0.0.1/tcp/36075/ws/p2p/12D3KooWALb...BhDAfJY"]);
console.log("connected");

let wallet = Wallet.new_from_private_key(getEvmNetwork, "your_private_key_here");
console.log("wallet retrieved");

let data = new Uint8Array([1, 2, 3]);
let result = await client.put(data, wallet);
console.log("Data stored at:", result);

let fetchedData = await client.get(result);
console.log("Data retrieved:", fetchedData);
```

## Funded wallet from custom local network

```js
const evmNetwork = getEvmNetworkCustom("http://localhost:4343", "<payment token addr>", "<data payments addr>");
const wallet = getFundedWalletWithCustomNetwork(evmNetwork, "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80");
```
