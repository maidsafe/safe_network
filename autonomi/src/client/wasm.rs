use libp2p::Multiaddr;
use wasm_bindgen::prelude::*;

use super::address::{addr_to_str, str_to_addr};

#[wasm_bindgen]
pub struct Client(super::Client);

#[wasm_bindgen]
pub struct AttoTokens(sn_evm::AttoTokens);
#[wasm_bindgen]
impl AttoTokens {
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub async fn connect(peers: Vec<String>) -> Result<Client, JsError> {
        let peers = peers
            .into_iter()
            .map(|peer| peer.parse())
            .collect::<Result<Vec<Multiaddr>, _>>()?;

        let client = super::Client::connect(&peers).await?;

        Ok(Client(client))
    }

    #[wasm_bindgen(js_name = chunkPut)]
    pub async fn chunk_put(&self, _data: Vec<u8>, _wallet: &Wallet) -> Result<String, JsError> {
        async { unimplemented!() }.await
    }

    #[wasm_bindgen(js_name = chunkGet)]
    pub async fn chunk_get(&self, addr: String) -> Result<Vec<u8>, JsError> {
        let addr = str_to_addr(&addr)?;
        let chunk = self.0.chunk_get(addr).await?;

        Ok(chunk.value().to_vec())
    }

    #[wasm_bindgen(js_name = dataPut)]
    pub async fn data_put(&self, data: Vec<u8>, wallet: &Wallet) -> Result<String, JsError> {
        let data = crate::Bytes::from(data);
        let xorname = self.0.data_put(data, &wallet.0).await?;

        Ok(addr_to_str(xorname))
    }

    #[wasm_bindgen(js_name = dataGet)]
    pub async fn data_get(&self, addr: String) -> Result<Vec<u8>, JsError> {
        let addr = str_to_addr(&addr)?;
        let data = self.0.data_get(addr).await?;

        Ok(data.to_vec())
    }

    #[wasm_bindgen(js_name = dataCost)]
    pub async fn data_cost(&self, data: Vec<u8>) -> Result<AttoTokens, JsValue> {
        let data = crate::Bytes::from(data);
        let cost = self.0.data_cost(data).await.map_err(JsError::from)?;

        Ok(AttoTokens(cost))
    }
}

mod archive {
    use super::*;
    use crate::client::{address::str_to_addr, archive::Archive};
    use std::{collections::HashMap, path::PathBuf};
    use xor_name::XorName;

    #[wasm_bindgen]
    impl Client {
        #[wasm_bindgen(js_name = archiveGet)]
        pub async fn archive_get(&self, addr: String) -> Result<js_sys::Map, JsError> {
            let addr = str_to_addr(&addr)?;
            let data = self.0.archive_get(addr).await?;

            // To `Map<K, V>` (JS)
            let data = serde_wasm_bindgen::to_value(&data.map)?;
            Ok(data.into())
        }

        #[wasm_bindgen(js_name = archivePut)]
        pub async fn archive_put(
            &self,
            map: JsValue,
            wallet: &super::Wallet,
        ) -> Result<String, JsError> {
            // From `Map<K, V>` or `Iterable<[K, V]>` (JS)
            let map: HashMap<PathBuf, XorName> = serde_wasm_bindgen::from_value(map)?;
            let archive = Archive { map };

            let addr = self.0.archive_put(archive, &wallet.0).await?;

            Ok(addr_to_str(addr))
        }
    }
}

#[wasm_bindgen]
pub struct Wallet(evmlib::wallet::Wallet);

/// Get a funded wallet for testing. This either uses a default private key or the `EVM_PRIVATE_KEY`
/// environment variable that was used during the build process of this library.
#[wasm_bindgen(js_name = getFundedWallet)]
pub fn funded_wallet() -> Wallet {
    Wallet(test_utils::evm::get_funded_wallet())
}

/// Enable tracing logging in the console.
///
/// A level could be passed like `trace` or `warn`. Or set for a specific module/crate
/// with `sn_networking=trace,autonomi=info`.
#[wasm_bindgen(js_name = logInit)]
pub fn log_init(directive: String) {
    use tracing_subscriber::prelude::*;

    console_error_panic_hook::set_once();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false) // Only partially supported across browsers
        .without_time() // std::time is not available in browsers
        .with_writer(tracing_web::MakeWebConsoleWriter::new()); // write events to the console
    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(tracing_subscriber::EnvFilter::new(directive))
        .init();
}
