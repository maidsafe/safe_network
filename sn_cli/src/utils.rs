/// Returns whether a hex string is a valid secret key in hex format.
pub fn is_valid_key_hex(hex: &str) -> bool {
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}
