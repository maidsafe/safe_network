use rand::Rng;
use std::num::NonZeroU32;
use ring::aead::{BoundKey, Nonce, NonceSequence};
use ring::error::Unspecified;
use crate::EvmError;
struct NonceSeq([u8; 12]);

impl NonceSequence for NonceSeq {
    fn advance(&mut self) -> std::result::Result<Nonce, Unspecified> {
        Nonce::try_assume_unique_for_key(&self.0)
    }
}


/// Number of iterations for pbkdf2.
const ITERATIONS: NonZeroU32 = match NonZeroU32::new(100_000) {
    Some(v) => v,
    None => panic!("`100_000` is not be zero"),
};

const SALT_LENGTH: usize = 8;
const NONCE_LENGTH: usize = 12;

pub fn encrypt_secret_key(
    secret_key: &str,
    password: &str,
) -> String {
    // Generate a random salt
    // Salt is used to ensure unique derived keys even for identical passwords
    let mut salt = [0u8; SALT_LENGTH];
    rand::thread_rng().fill(&mut salt);

    // Generate a random nonce
    // Nonce is used to ensure unique encryption outputs even for identical inputs
    let mut nonce = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill(&mut nonce);

    let mut key = [0; 32];

    // Derive a key from the password using PBKDF2 with HMAC<Sha512>
    // PBKDF2 is used for key derivation to mitigate brute-force attacks by making key derivation computationally expensive
    // HMAC<Sha512> is used as the pseudorandom function for its security properties
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA512,
        ITERATIONS,
        &salt,
        password.as_bytes(),
        &mut key,
    );

    // Create an unbound key using CHACHA20_POLY1305 algorithm
    // CHACHA20_POLY1305 is a fast and secure AEAD (Authenticated Encryption with Associated Data) algorithm
    let unbound_key = ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
        .map_err(|_| EvmError::FailedToEncryptKey(String::from("Could not create unbound key."))).expect("error");

    // Create a sealing key with the unbound key and nonce
    let mut sealing_key = ring::aead::SealingKey::new(unbound_key, NonceSeq(nonce));
    let aad = ring::aead::Aad::from(&[]);

    // Convert the secret key to bytes
    let secret_key_bytes = String::from(secret_key).into_bytes();
    let mut encrypted_secret_key = secret_key_bytes;

    // seal_in_place_append_tag encrypts the data and appends an authentication tag to ensure data integrity
    sealing_key
        .seal_in_place_append_tag(aad, &mut encrypted_secret_key)
        .map_err(|_| EvmError::FailedToEncryptKey(String::from("Could not seal sealing key."))).expect("error");

    // encrypted_secret_key.extend_from_slice(&salt);
    // encrypted_secret_key.extend_from_slice(&salt);
    let mut encrypted_data = Vec::new();
    encrypted_data.extend_from_slice(&salt);
    encrypted_data.extend_from_slice(&nonce);
    encrypted_data.extend_from_slice(&encrypted_secret_key);
    
    // Return the encrypted secret key along with salt and nonce encoded as hex strings
    hex::encode(encrypted_data)
}


pub fn decrypt_secret_key(
    encrypted_data: &str, 
    password: &str
    ) -> String {

    let encrypted_data = hex::decode(encrypted_data).expect("error");
    let salt: [u8; SALT_LENGTH]  = encrypted_data[..SALT_LENGTH].try_into().expect("error");
    let nonce:[u8; NONCE_LENGTH]  = encrypted_data[SALT_LENGTH..SALT_LENGTH+NONCE_LENGTH].try_into().expect("error");
    let encrypted_secretkey = &encrypted_data[SALT_LENGTH+ NONCE_LENGTH ..];
    
    let mut key = [0; 32];

    // Reconstruct the key from salt and password
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA512,
        ITERATIONS,
        &salt,
        password.as_bytes(),
        &mut key,
    );

    // Create an unbound key from the previously reconstructed key
    let unbound_key = ring::aead::UnboundKey::new(&ring::aead::CHACHA20_POLY1305, &key)
        .map_err(|_| {
            EvmError::FailedToDecryptKey(String::from("Could not create unbound key."))
        }).expect("error");


    // Create an opening key using the unbound key and original nonce
    let mut opening_key = ring::aead::OpeningKey::new(unbound_key, NonceSeq(nonce));
    let aad = ring::aead::Aad::from(&[]);

    // Convert the hex encoded and encrypted secret key to bytes
    // let mut encrypted_secret_key = hex::decode(encrypted_secretkey).map_err(|_| {
    //     EvmError::FailedToDecryptKey(String::from("Invalid encrypted secret key encoding."))
    // }).expect("error");

    let mut encrypted_secret_key = encrypted_secretkey.to_vec();
    // Decrypt the encrypted secret key bytes
    let decrypted_data = opening_key
        .open_in_place(aad, &mut encrypted_secret_key)
        .map_err(|_| EvmError::FailedToDecryptKey(String::from("Could not open encrypted key"))).expect("error");

    let mut secret_key_bytes = [0u8; 66];
    secret_key_bytes.copy_from_slice(&decrypted_data[0..66]);

    // Create secret key from decrypted bytes
    String::from_utf8(secret_key_bytes.to_vec()).expect("error")
}