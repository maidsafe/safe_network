// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use ledger_lib::{
    info::ConnInfo,
    transport::{BleTransport, GenericDevice, TcpInfo, TcpTransport, UsbTransport},
    Filters, LedgerProvider, Transport,
};

use color_eyre::Result;

pub(super) async fn connect_to_device() -> Result<GenericDevice> {
    let mut ledger_provider = LedgerProvider::init().await;

    let device = match ledger_provider.list(Filters::Any).await {
        Ok(devices) if devices.is_empty() => {
            println!("No USB devices detected automatically, triying with TCP on localhost...");
            connect_to_localhost().await?
        }
        Err(err) => {
            println!("Error when trying to detect devices through USB: {err:?}");
            println!("No devices detected through USB, triying with TCP on localhost...");
            connect_to_localhost().await?
        }
        Ok(devices) => {
            println!("Devices detected: {devices:?}");
            // TODO: allow the user to select which device
            // let's just choose the first one to connect to
            match &devices[0].conn {
                ConnInfo::Usb(usb_info) => {
                    println!("Connecting to Ledger through USB: {usb_info:?}");
                    let mut usb_transport = UsbTransport::new()?;
                    let usb_device = usb_transport.connect(usb_info.clone()).await?;
                    GenericDevice::from(usb_device)
                }
                ConnInfo::Tcp(tcp_info) => {
                    println!("Connecting to Ledger through TCP: {tcp_info:?}");
                    let tcp_device = TcpTransport::new()?.connect(tcp_info.clone()).await?;
                    GenericDevice::from(tcp_device)
                }
                ConnInfo::Ble(ble_info) => {
                    println!("Connecting to Ledger through BLE: {ble_info:?}");
                    let ble_device = BleTransport::new().await?.connect(ble_info.clone()).await?;
                    GenericDevice::from(ble_device)
                }
            }
        }
    };

    println!("Connected to device: {}", device.info());
    Ok(device)
}

async fn connect_to_localhost() -> Result<GenericDevice> {
    let tcp_device = TcpTransport::new()?
        .connect(TcpInfo::default()) // default socket is localhost::1237
        .await?;
    Ok(GenericDevice::from(tcp_device))
}
