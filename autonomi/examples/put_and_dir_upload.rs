use autonomi::{Bytes, Client, Wallet};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Default wallet of testnet.
    let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    let client = Client::connect(&["/ip4/127.0.0.1/udp/1234/quic-v1".parse()?]).await?;
    let wallet = Wallet::new_from_private_key(Default::default(), key)?;

    // Put and fetch data.
    let data_addr = client
        .data_put(Bytes::from("Hello, World"), (&wallet).into())
        .await?;
    let _data_fetched = client.data_get(data_addr).await?;

    // Put and fetch directory from local file system.
    let dir_addr = client.dir_upload("files/to/upload".into(), &wallet).await?;
    client
        .dir_download(dir_addr, "files/downloaded".into())
        .await?;

    Ok(())
}
