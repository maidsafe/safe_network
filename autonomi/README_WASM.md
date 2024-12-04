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

# Developing

## WebAssembly

To run a WASM test

- Install `wasm-pack`
- Make sure your Rust supports the `wasm32-unknown-unknown` target. (If you
  have `rustup`: `rustup target add wasm32-unknown-unknown`.)
- Pass a bootstrap peer via `ANT_PEERS`. This *has* to be the websocket address,
  e.g. `/ip4/<ip>/tcp/<port>/ws/p2p/<peer ID>`.
    - As well as the other environment variables needed for EVM payments (e.g. `RPC_URL`).
- Optionally specify the specific test, e.g. `-- put` to run `put()` in `wasm.rs` only.

Example:

```sh
ANT_PEERS=/ip4/<ip>/tcp/<port>/ws/p2p/<peer ID> wasm-pack test --release --firefox autonomi --features=files --test wasm -- put
```

### Test from JS in the browser

`wasm-pack test` does not execute JavaScript, but runs mostly WebAssembly. Again make sure the environment variables are
set and build the JS package:

```sh
wasm-pack build --dev --target web autonomi --features=vault
```

Then cd into `autonomi/tests-js`, and use `npm` to install and serve the test html file.

```
cd autonomi/tests-js
npm install
npm run serve
```

Then go to `http://127.0.0.1:8080/tests-js` in the browser. Here, enter a `ws` multiaddr of a local node and press '
run'.

## MetaMask example

There is a MetaMask example for doing a simple put operation.

Build the package with the `external-signer` feature (and again with the env variables) and run a webserver, e.g. with
Python:

```sh
wasm-pack build --dev --target web autonomi --features=external-signer
python -m http.server --directory autonomi 8000
```

Then visit `http://127.0.0.1:8000/examples/metamask` in your (modern) browser.

Here, enter a `ws` multiaddr of a local node and press 'run'.
