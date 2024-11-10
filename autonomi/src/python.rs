use crate::client::{
    archive::ArchiveAddr,
    archive_private::PrivateArchiveAccess,
    data_private::PrivateDataAccess,
    payment::PaymentOption as RustPaymentOption,
    vault::{UserData, VaultSecretKey},
    Client as RustClient,
};
use crate::{Bytes, Wallet as RustWallet};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use sn_evm::EvmNetwork;
use xor_name::XorName;

#[pyclass(name = "Client")]
pub(crate) struct PyClient {
    inner: RustClient,
}

#[pymethods]
impl PyClient {
    #[staticmethod]
    fn connect(peers: Vec<String>) -> PyResult<Self> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let peers = peers
            .into_iter()
            .map(|addr| addr.parse())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid multiaddr: {e}"))
            })?;

        let client = rt.block_on(RustClient::connect(&peers)).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to connect: {e}"))
        })?;

        Ok(Self { inner: client })
    }

    fn private_data_put(
        &self,
        data: Vec<u8>,
        payment: &PyPaymentOption,
    ) -> PyResult<PyPrivateDataAccess> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let access = rt
            .block_on(
                self.inner
                    .private_data_put(Bytes::from(data), payment.inner.clone()),
            )
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to put private data: {e}"))
            })?;

        Ok(PyPrivateDataAccess { inner: access })
    }

    fn private_data_get(&self, access: &PyPrivateDataAccess) -> PyResult<Vec<u8>> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let data = rt
            .block_on(self.inner.private_data_get(access.inner.clone()))
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to get private data: {e}"))
            })?;
        Ok(data.to_vec())
    }

    fn data_put(&self, data: Vec<u8>, payment: &PyPaymentOption) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let addr = rt
            .block_on(
                self.inner
                    .data_put(bytes::Bytes::from(data), payment.inner.clone()),
            )
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to put data: {e}"))
            })?;

        Ok(crate::client::address::addr_to_str(addr))
    }

    fn data_get(&self, addr: &str) -> PyResult<Vec<u8>> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let addr = crate::client::address::str_to_addr(addr).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid address: {e}"))
        })?;

        let data = rt.block_on(self.inner.data_get(addr)).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Failed to get data: {e}"))
        })?;

        Ok(data.to_vec())
    }

    fn vault_cost(&self, key: &PyVaultSecretKey) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let cost = rt
            .block_on(self.inner.vault_cost(&key.inner))
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to get vault cost: {e}"))
            })?;
        Ok(cost.to_string())
    }

    fn write_bytes_to_vault(
        &self,
        data: Vec<u8>,
        payment: &PyPaymentOption,
        key: &PyVaultSecretKey,
        content_type: u64,
    ) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let cost = rt
            .block_on(self.inner.write_bytes_to_vault(
                bytes::Bytes::from(data),
                payment.inner.clone(),
                &key.inner,
                content_type,
            ))
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to write to vault: {e}"))
            })?;
        Ok(cost.to_string())
    }

    fn fetch_and_decrypt_vault(&self, key: &PyVaultSecretKey) -> PyResult<(Vec<u8>, u64)> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let (data, content_type) = rt
            .block_on(self.inner.fetch_and_decrypt_vault(&key.inner))
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to fetch vault: {e}"))
            })?;
        Ok((data.to_vec(), content_type))
    }

    fn get_user_data_from_vault(&self, key: &PyVaultSecretKey) -> PyResult<PyUserData> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let user_data = rt
            .block_on(self.inner.get_user_data_from_vault(&key.inner))
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to get user data: {e}"))
            })?;
        Ok(PyUserData { inner: user_data })
    }

    fn put_user_data_to_vault(
        &self,
        key: &PyVaultSecretKey,
        payment: &PyPaymentOption,
        user_data: &PyUserData,
    ) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let cost = rt
            .block_on(self.inner.put_user_data_to_vault(
                &key.inner,
                payment.inner.clone(),
                user_data.inner.clone(),
            ))
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to put user data: {e}"))
            })?;
        Ok(cost.to_string())
    }
}

#[pyclass(name = "Wallet")]
pub(crate) struct PyWallet {
    inner: RustWallet,
}

#[pymethods]
impl PyWallet {
    #[new]
    fn new(private_key: String) -> PyResult<Self> {
        let wallet = RustWallet::new_from_private_key(
            EvmNetwork::ArbitrumOne, // TODO: Make this configurable
            &private_key,
        )
        .map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid private key: {e}"))
        })?;

        Ok(Self { inner: wallet })
    }

    fn address(&self) -> String {
        format!("{:?}", self.inner.address())
    }

    fn balance(&self) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let balance = rt
            .block_on(async { self.inner.balance_of_tokens().await })
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to get balance: {e}"))
            })?;

        Ok(balance.to_string())
    }

    fn balance_of_gas(&self) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new().expect("Could not start tokio runtime");
        let balance = rt
            .block_on(async { self.inner.balance_of_gas_tokens().await })
            .map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Failed to get balance: {e}"))
            })?;

        Ok(balance.to_string())
    }
}

#[pyclass(name = "PaymentOption")]
pub(crate) struct PyPaymentOption {
    inner: RustPaymentOption,
}

#[pymethods]
impl PyPaymentOption {
    #[staticmethod]
    fn wallet(wallet: &PyWallet) -> Self {
        Self {
            inner: RustPaymentOption::Wallet(wallet.inner.clone()),
        }
    }
}

#[pyclass(name = "VaultSecretKey")]
pub(crate) struct PyVaultSecretKey {
    inner: VaultSecretKey,
}

#[pymethods]
impl PyVaultSecretKey {
    #[new]
    fn new() -> PyResult<Self> {
        Ok(Self {
            inner: VaultSecretKey::random(),
        })
    }

    #[staticmethod]
    fn from_hex(hex_str: &str) -> PyResult<Self> {
        VaultSecretKey::from_hex(hex_str)
            .map(|key| Self { inner: key })
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid hex key: {e}")))
    }

    fn to_hex(&self) -> String {
        self.inner.to_hex()
    }
}

#[pyclass(name = "UserData")]
pub(crate) struct PyUserData {
    inner: UserData,
}

#[pymethods]
impl PyUserData {
    #[new]
    fn new() -> Self {
        Self {
            inner: UserData::new(),
        }
    }

    fn add_file_archive(&mut self, archive: &str) -> Option<String> {
        let name = XorName::from_content(archive.as_bytes());
        let archive_addr = ArchiveAddr::from_content(&name);
        self.inner.add_file_archive(archive_addr)
    }

    fn add_private_file_archive(&mut self, archive: &str) -> Option<String> {
        let name = XorName::from_content(archive.as_bytes());
        let private_access = match PrivateArchiveAccess::from_hex(&name.to_string()) {
            Ok(access) => access,
            Err(_e) => return None,
        };
        self.inner.add_private_file_archive(private_access)
    }

    fn file_archives(&self) -> Vec<(String, String)> {
        self.inner
            .file_archives
            .iter()
            .map(|(addr, name)| (format!("{addr:x}"), name.clone()))
            .collect()
    }

    fn private_file_archives(&self) -> Vec<(String, String)> {
        self.inner
            .private_file_archives
            .iter()
            .map(|(addr, name)| (addr.to_hex(), name.clone()))
            .collect()
    }
}

#[pyclass(name = "PrivateDataAccess")]
#[derive(Clone)]
pub(crate) struct PyPrivateDataAccess {
    inner: PrivateDataAccess,
}

#[pymethods]
impl PyPrivateDataAccess {
    #[staticmethod]
    fn from_hex(hex: &str) -> PyResult<Self> {
        PrivateDataAccess::from_hex(hex)
            .map(|access| Self { inner: access })
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Invalid hex: {e}")))
    }

    fn to_hex(&self) -> String {
        self.inner.to_hex()
    }

    fn address(&self) -> String {
        self.inner.address().to_string()
    }
}

#[pyfunction]
fn encrypt(data: Vec<u8>) -> PyResult<(Vec<u8>, Vec<Vec<u8>>)> {
    let (data_map, chunks) = self_encryption::encrypt(Bytes::from(data))
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("Encryption failed: {e}")))?;

    let data_map_bytes = rmp_serde::to_vec(&data_map)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize data map: {e}")))?;

    let chunks_bytes: Vec<Vec<u8>> = chunks
        .into_iter()
        .map(|chunk| chunk.content.to_vec())
        .collect();

    Ok((data_map_bytes, chunks_bytes))
}

#[pymodule]
#[pyo3(name = "autonomi_client")]
fn autonomi_client_module(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyClient>()?;
    m.add_class::<PyWallet>()?;
    m.add_class::<PyPaymentOption>()?;
    m.add_class::<PyVaultSecretKey>()?;
    m.add_class::<PyUserData>()?;
    m.add_class::<PyPrivateDataAccess>()?;
    m.add_function(wrap_pyfunction!(encrypt, m)?)?;
    Ok(())
}
