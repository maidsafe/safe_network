use crate::network::{
    error::{GetRecordError, NetworkError},
    record::{GetRecordCfg, PutRecordCfg, VerificationKind},
    types::PayeeQuote,
};
use libp2p::PeerId;
use std::time::Duration;

#[test]
fn test_network_error_display() {
    let errors = vec![
        NetworkError::Record("missing data".into()),
        NetworkError::Connection("timeout".into()),
        NetworkError::Other("unknown error".into()),
    ];

    for error in errors {
        let display_string = format!("{}", error);
        match error {
            NetworkError::Record(_) => assert!(display_string.contains("Record error")),
            NetworkError::Connection(_) => assert!(display_string.contains("Connection error")),
            NetworkError::Other(_) => assert!(display_string.contains("Network error")),
        }
    }
}

#[test]
fn test_get_record_error_display() {
    let errors = vec![
        GetRecordError::NotFound,
        GetRecordError::VerificationFailed("invalid signature".into()),
        GetRecordError::Network(NetworkError::Other("network down".into())),
    ];

    for error in errors {
        let display_string = format!("{}", error);
        match error {
            GetRecordError::NotFound => assert!(display_string.contains("Record not found")),
            GetRecordError::VerificationFailed(_) => assert!(display_string.contains("Verification failed")),
            GetRecordError::Network(_) => assert!(display_string.contains("Network error")),
        }
    }
}

#[test]
fn test_record_configs() {
    // Test GetRecordCfg
    let get_cfg = GetRecordCfg {
        timeout_secs: 30,
        verification: VerificationKind::Full,
    };
    assert_eq!(get_cfg.timeout_secs, 30);
    assert_eq!(get_cfg.verification, VerificationKind::Full);

    // Test PutRecordCfg
    let put_cfg = PutRecordCfg {
        timeout_secs: 60,
        replication: 3,
    };
    assert_eq!(put_cfg.timeout_secs, 60);
    assert_eq!(put_cfg.replication, 3);
}

#[test]
fn test_verification_kinds() {
    assert_ne!(VerificationKind::None, VerificationKind::Signature);
    assert_ne!(VerificationKind::None, VerificationKind::Full);
    assert_ne!(VerificationKind::Signature, VerificationKind::Full);
}

#[test]
fn test_payee_quote() {
    let peer_id = PeerId::random();
    let price = 100;
    let expiry = Duration::from_secs(3600);

    let quote = PayeeQuote {
        peer_id,
        price,
        expiry,
    };

    assert_eq!(quote.peer_id, peer_id);
    assert_eq!(quote.price, price);
    assert_eq!(quote.expiry, expiry);
} 