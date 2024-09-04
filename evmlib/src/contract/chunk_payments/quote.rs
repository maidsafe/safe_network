use crate::cryptography::{
    recover_public_key_from_signed_hash, sign_message_recoverable, to_eth_signed_message_hash,
};
use alloy::hex;
use alloy::primitives::{Address, FixedBytes, U256};
use alloy::signers::k256::ecdsa::{signature, RecoveryId, SigningKey, VerifyingKey};
use alloy::sol_types::SolValue;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid signature")]
    SignatureInvalid,
    #[error("Invalid recovery id. It should be less than 4")]
    RecoveryIdInvalid,
    #[error(transparent)]
    SignatureError(#[from] signature::Error),
}

#[derive(Clone, Debug)]
pub struct Signature {
    pub r: FixedBytes<32>,
    pub s: FixedBytes<32>,
    /// Recovery id
    pub v: u8,
}

#[derive(Clone, Debug)]
pub struct Quote {
    /// Keccak256 hash of the chunk address
    pub chunk_address_hash: FixedBytes<32>,
    /// Price for the chunk
    pub cost: U256,
    /// Expiration timestamp as seconds since UNIX epoch
    pub expiration_timestamp: U256,
    /// Wallet address receiving payment
    pub payment_address: Address,
}

#[derive(Clone, Debug)]
pub struct SignedQuote {
    pub quote: Quote,
    /// Signature by the Node of this quote
    pub signature: Signature,
}

impl Quote {
    pub fn to_packed(&self) -> Vec<u8> {
        (
            self.chunk_address_hash,
            self.cost,
            self.expiration_timestamp,
            self.payment_address,
        )
            .abi_encode_packed()
    }

    /// Sign a quote using a secret key.
    pub fn sign_quote(&self, secret_key: &SigningKey) -> Result<SignedQuote, signature::Error> {
        let (signature, recovery_id) =
            sign_message_recoverable(secret_key, self.to_packed().as_slice())?;

        Ok(SignedQuote {
            quote: self.clone(),
            signature: Signature {
                r: FixedBytes::from_slice(signature.r().to_bytes().as_slice()),
                s: FixedBytes::from_slice(signature.s().to_bytes().as_slice()),
                v: u8::from(recovery_id),
            },
        })
    }
}

impl SignedQuote {
    // TODO: See why legacy recovery id's do not produce the expected result
    /// Recover the public key of the signer of this quote.
    pub fn recover_public_key(&self) -> Result<VerifyingKey, Error> {
        let hash = to_eth_signed_message_hash(self.quote.to_packed().as_slice());

        let signature_bytes: [u8; 64] = {
            let mut bytes = [0u8; 64];
            bytes[0..32].copy_from_slice(self.signature.r.as_slice());
            bytes[32..64].copy_from_slice(self.signature.s.as_slice());
            bytes
        };

        let signature =
            alloy::signers::k256::ecdsa::Signature::from_slice(signature_bytes.as_slice())
                .map_err(|_| Error::SignatureInvalid)?;

        let adjusted_v = if self.signature.v >= 27 {
            self.signature.v - 27
        } else {
            self.signature.v
        };

        let recovery_id = RecoveryId::from_byte(adjusted_v).ok_or(Error::RecoveryIdInvalid)?;

        recover_public_key_from_signed_hash(&hash, &signature, recovery_id).map_err(Error::from)
    }
}
