use libp2p::Multiaddr;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Client(super::Client);

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub async fn connect(peers: Vec<String>) -> Result<Client, JsValue> {
        let peers = peers
            .into_iter()
            .map(|peer| peer.parse())
            .collect::<Result<Vec<Multiaddr>, _>>()
            // .map_err(|err| serde_wasm_bindgen::to_value(&err).unwrap());
            .map_err(|_err| JsValue::NULL)?;

        let client = super::Client::connect(&peers)
            .await
            .map_err(|err| serde_wasm_bindgen::to_value(&err).expect("serialization to succeed"))?;

        Ok(Client(client))
    }

    #[wasm_bindgen]
    pub async fn put(&self, data: Vec<u8>, wallet: Wallet) -> Result<Vec<u8>, JsValue> {
        let data = crate::Bytes::from(data);
        self.0
            .put(data, &wallet.0)
            .await
            .map_err(|err| serde_wasm_bindgen::to_value(&err).expect("serialization to succeed"))
            .map(|xorname| xorname.to_vec())
    }
}

#[wasm_bindgen]
pub struct Wallet(evmlib::wallet::Wallet);

#[wasm_bindgen(js_name = getFundedWallet)]
pub fn funded_wallet() -> Wallet {
    let network = evmlib::utils::evm_network_from_env().expect("network init from env");

    let private_key = option_env!("EVM_PRIVATE_KEY")
        .unwrap_or_else(|| "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80");

    Wallet(
        evmlib::wallet::Wallet::new_from_private_key(network, private_key)
            .expect("Invalid private key"),
    )
}

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
