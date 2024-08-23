use std::time::Duration;

use helpers::enable_logging;
use libautonomi::Client;
use tokio::time::sleep;

#[tokio::test]
async fn put() {
    enable_logging();

    let mut client = Client::connect(&[]).await.unwrap();
    let mut wallet = helpers::load_hot_wallet_from_faucet();
    let data = helpers::gen_random_data(1024 * 1024 * 1000);

    // let quote = client.quote(data.clone()).await.unwrap();
    // let payment = client.pay(quote, &mut wallet).await.unwrap();
    let addr = client.put(data.clone(), &mut wallet).await.unwrap();

    sleep(Duration::from_secs(2)).await;

    let data_fetched = client.get(addr).await.unwrap();
    assert_eq!(data, data_fetched, "data fetched should match data put");
}

mod helpers {
    use bytes::Bytes;
    use rand::Rng;
    use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
    use sn_transfers::{get_faucet_data_dir, HotWallet};

    /// When launching a testnet locally, we can use the faucet wallet.
    pub(super) fn load_hot_wallet_from_faucet() -> HotWallet {
        let root_dir = get_faucet_data_dir();
        load_account_wallet_or_create_with_mnemonic(&root_dir, None)
            .expect("faucet wallet should be available for tests")
    }

    pub(super) fn gen_random_data(len: usize) -> Bytes {
        let mut data = vec![0u8; len];
        rand::thread_rng().fill(&mut data[..]);
        Bytes::from(data)
    }

    /// Enable logging for tests. E.g. use `RUST_LOG=libautonomi` to see logs.
    pub(super) fn enable_logging() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
    }
}
