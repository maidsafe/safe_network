use libp2p::kad::{KBucketKey, KBucketDistance, Record};
use libp2p::PeerId;
use ant_protocol::NetworkAddress;

pub use KBucketKey;
pub use KBucketDistance;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordKey(pub NetworkAddress);

impl From<NetworkAddress> for RecordKey {
    fn from(addr: NetworkAddress) -> Self {
        RecordKey(addr)
    }
}

impl AsRef<[u8]> for RecordKey {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<RecordKey> for KBucketKey<PeerId> {
    fn from(key: RecordKey) -> Self {
        KBucketKey::new(key.0.into())
    }
}
