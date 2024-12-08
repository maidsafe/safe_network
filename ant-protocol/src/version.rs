// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use lazy_static::lazy_static;

lazy_static! {
    /// The version used to identify the node
    pub static ref NODE_VERSION: String =
        format!(
            "ant/node/{}",
            get_protocol_version(),
        );

    /// The version used to identify the client
    pub static ref CLIENT_VERSION: String =
        format!(
            "ant/client/{}",
            get_protocol_version(),
        );

    /// The req/response version
    pub static ref REQ_RESPONSE_VERSION: String =
        format!(
            "/ant/node/{}",
            get_protocol_version(),
        );

    /// The current protocol version
    pub static ref PROTOCOL_VERSION: String =
        format!(
            "ant/{}",
            get_protocol_version(),
        );
}

pub fn get_protocol_version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() >= 2 {
        return format!("{}.{}", parts[0], parts[1]);
    }
    panic!("Cannot obtain protocol version from {version:?}: {parts:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_version_format() {
        let protocol_ver = get_protocol_version();
        let expected = format!("ant/node/{protocol_ver}");
        assert_eq!(*NODE_VERSION, expected);
    }

    #[test]
    fn test_client_version_format() {
        let protocol_ver = get_protocol_version();
        let expected = format!("ant/client/{protocol_ver}");
        assert_eq!(*CLIENT_VERSION, expected);
    }

    #[test]
    fn test_req_response_version_format() {
        let protocol_ver = get_protocol_version();
        let expected = format!("/ant/node/{protocol_ver}");
        assert_eq!(*REQ_RESPONSE_VERSION, expected);
    }

    #[test]
    fn test_protocol_version_format() {
        let protocol_ver = get_protocol_version();
        let expected = format!("ant/{protocol_ver}");
        assert_eq!(*PROTOCOL_VERSION, expected);
    }

    #[test]
    fn test_get_protocol_version() {
        let version = get_protocol_version();
        assert_eq!(version.chars().filter(|&c| c == '.').count(), 1);

        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 2);

        assert!(parts[0].parse::<u32>().is_ok());
        assert!(parts[1].parse::<u32>().is_ok());
    }

    #[test]
    fn test_version_consistency() {
        let cargo_version = env!("CARGO_PKG_VERSION");
        let protocol_version = get_protocol_version();

        let cargo_parts: Vec<&str> = cargo_version.split('.').collect();
        let protocol_parts: Vec<&str> = protocol_version.split('.').collect();

        assert_eq!(cargo_parts[0], protocol_parts[0]);
        assert_eq!(cargo_parts[1], protocol_parts[1]);
    }
}
