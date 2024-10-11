// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use xor_name::XorName;

#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("Invalid XorName")]
    InvalidXorName,
    #[error("Input address is not a hex string")]
    InvalidHexString,
}

pub fn str_to_addr(addr: &str) -> Result<XorName, DataError> {
    let bytes = hex::decode(addr).map_err(|err| {
        error!("Failed to decode hex string: {err:?}");
        DataError::InvalidHexString
    })?;
    let xor = XorName(bytes.try_into().map_err(|err| {
        error!("Failed to convert bytes to XorName: {err:?}");
        DataError::InvalidXorName
    })?);
    Ok(xor)
}

pub fn addr_to_str(addr: XorName) -> String {
    hex::encode(addr)
}

#[cfg(test)]
mod test {
    use super::*;
    use xor_name::XorName;

    #[test]
    fn test_xorname_to_str() {
        let rng = &mut rand::thread_rng();
        let xorname = XorName::random(rng);
        let str = addr_to_str(xorname);
        let xorname2 = str_to_addr(&str).expect("Failed to convert back to xorname");
        assert_eq!(xorname, xorname2);
    }
}
