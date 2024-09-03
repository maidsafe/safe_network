use alloy::primitives::{keccak256, Address};
use alloy::signers::k256::ecdsa::{signature, RecoveryId, Signature, SigningKey, VerifyingKey};
use alloy::signers::k256::elliptic_curve::rand_core::OsRng;

/// Generate a new ECDSA keypair.
pub fn generate_ecdsa_keypair() -> (SigningKey, VerifyingKey) {
    let secret_key = SigningKey::random(&mut OsRng);
    let public_key = *secret_key.verifying_key();
    (secret_key, public_key)
}

/// Get an Ethereum address for a public key.
pub fn public_key_to_address(public_key: &VerifyingKey) -> Address {
    alloy::signers::utils::public_key_to_address(public_key)
}

/// Hash a message using Keccak256, then add the Ethereum prefix and hash it again.
pub fn to_eth_signed_message_hash<T: AsRef<[u8]>>(message: T) -> [u8; 32] {
    const PREFIX: &str = "\x19Ethereum Signed Message:\n32";

    let hashed_message = keccak256(message).0;

    let mut eth_message = Vec::with_capacity(PREFIX.len() + 32);
    eth_message.extend_from_slice(PREFIX.as_bytes());
    eth_message.extend_from_slice(&hashed_message);

    keccak256(&eth_message).0
}

/// Sign a message with a recoverable public key.
pub fn sign_message_recoverable<T: AsRef<[u8]>>(
    secret_key: &SigningKey,
    message: T,
) -> Result<(Signature, RecoveryId), signature::Error> {
    let hash = to_eth_signed_message_hash(message);
    secret_key.sign_prehash_recoverable(&hash)
}

/// Recover a public key from an Ethereum signed message hash, the signature and the recovery id.
pub fn recover_public_key_from_signed_hash(
    hash: &[u8; 32],
    signature: &Signature,
    recovery_id: RecoveryId,
) -> Result<VerifyingKey, signature::Error> {
    VerifyingKey::recover_from_prehash(hash, signature, recovery_id)
}

#[cfg(test)]
mod tests {
    use crate::cryptography::{
        generate_ecdsa_keypair, recover_public_key_from_signed_hash, sign_message_recoverable,
        to_eth_signed_message_hash,
    };

    #[tokio::test]
    async fn test_sign_verify_message() {
        let (secret_key, public_key) = generate_ecdsa_keypair();
        let message = "test message";
        let signed_message_hash = to_eth_signed_message_hash(message.as_bytes());
        let (signature, v) = sign_message_recoverable(&secret_key, message).unwrap();

        let recovered_public_key =
            recover_public_key_from_signed_hash(&signed_message_hash, &signature, v).unwrap();

        assert_eq!(recovered_public_key, public_key);
    }
}
