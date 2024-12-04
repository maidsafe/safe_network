use crate::network::{
    error::{GetRecordError, NetworkError, RecordError, Result, RecordResult},
    record::{GetRecordCfg, PutRecordCfg, VerificationKind},
    types::{PayeeQuote, NetworkPrice, NetworkDistance, NetworkTimeout, NetworkTypeError},
};
use libp2p::PeerId;
use std::time::{Duration, SystemTime};

#[test]
fn test_network_error_display() {
    let peer_id = PeerId::random();
    let errors = vec![
        NetworkError::Record(RecordError::NotFound),
        NetworkError::Connection {
            peer_id,
            reason: "timeout".into(),
        },
        NetworkError::Protocol("invalid message".into()),
        NetworkError::Transport("connection refused".into()),
        NetworkError::Timeout(Duration::from_secs(30)),
    ];

    for error in errors {
        let display_string = format!("{}", error);
        match error {
            NetworkError::Record(_) => assert!(display_string.contains("Record operation failed")),
            NetworkError::Connection { .. } => assert!(display_string.contains("Connection to peer")),
            NetworkError::Protocol(_) => assert!(display_string.contains("Protocol error")),
            NetworkError::Transport(_) => assert!(display_string.contains("Transport error")),
            NetworkError::Timeout(_) => assert!(display_string.contains("Operation timed out")),
            _ => unreachable!(),
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
    let price = NetworkPrice::new(100).unwrap();
    let expiry = NetworkTimeout::new(Duration::from_secs(120)).unwrap();

    let quote = PayeeQuote {
        peer_id,
        price,
        expiry,
    };

    assert_eq!(quote.peer_id, peer_id);
    assert_eq!(quote.price.value(), 100);
    assert_eq!(quote.expiry.duration(), Duration::from_secs(120));
}

#[test]
fn test_network_error_conversion() {
    let record_err = RecordError::NotFound;
    let network_err: NetworkError = record_err.into();
    
    assert!(matches!(network_err, NetworkError::Record(RecordError::NotFound)));
}

#[test]
fn test_record_error_display() {
    let errors = vec![
        RecordError::NotFound,
        RecordError::VerificationFailed("invalid signature".into()),
        RecordError::SizeExceeded {
            size: 1000,
            max_size: 100,
        },
        RecordError::Expired(SystemTime::now()),
        RecordError::InvalidFormat("bad format".into()),
        RecordError::Storage("disk full".into()),
    ];

    for error in errors {
        let display_string = format!("{}", error);
        match error {
            RecordError::NotFound => assert!(display_string.contains("not found")),
            RecordError::VerificationFailed(_) => assert!(display_string.contains("verification failed")),
            RecordError::SizeExceeded { .. } => assert!(display_string.contains("exceeds maximum")),
            RecordError::Expired(_) => assert!(display_string.contains("expired")),
            RecordError::InvalidFormat(_) => assert!(display_string.contains("Invalid record format")),
            RecordError::Storage(_) => assert!(display_string.contains("Storage error")),
        }
    }
}

#[test]
fn test_error_handling() {
    // Test NetworkError Result
    let result: Result<()> = if true {
        Err(NetworkError::Protocol("example error".into()))
    } else {
        Ok(())
    };
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), NetworkError::Protocol(_)));

    // Test RecordError Result
    let record_result: RecordResult<()> = Err(RecordError::NotFound);
    assert!(record_result.is_err());
    assert!(matches!(record_result.unwrap_err(), RecordError::NotFound));
}

#[test]
fn test_network_price_validation() {
    // Valid price
    let price = NetworkPrice::new(1000).unwrap();
    assert_eq!(price.value(), 1000);
    
    // Price too high
    let result = NetworkPrice::new(2_000_000_000);
    assert!(matches!(result, Err(NetworkTypeError::PriceTooHigh(_))));
}

#[test]
fn test_network_distance_validation() {
    // Valid distance
    let distance = NetworkDistance::new(100).unwrap();
    assert_eq!(distance.value(), 100);
    
    // Distance too large
    let result = NetworkDistance::new(300);
    assert!(matches!(result, Err(NetworkTypeError::DistanceTooLarge(_))));
}

#[test]
fn test_network_timeout_validation() {
    use std::time::Duration;
    
    // Valid timeout
    let timeout = NetworkTimeout::new(Duration::from_secs(60)).unwrap();
    assert_eq!(timeout.duration(), Duration::from_secs(60));
    
    // Timeout too short
    let result = NetworkTimeout::new(Duration::from_millis(100));
    assert!(matches!(result, Err(NetworkTypeError::TimeoutTooShort(_, _))));
    
    // Timeout too long
    let result = NetworkTimeout::new(Duration::from_secs(600));
    assert!(matches!(result, Err(NetworkTypeError::TimeoutTooLong(_, _))));
}

#[test]
fn test_network_type_formatting() {
    let price = NetworkPrice::new(1000).unwrap();
    let distance = NetworkDistance::new(100).unwrap();
    let timeout = NetworkTimeout::new(Duration::from_secs(60)).unwrap();
    
    assert_eq!(format!("{}", price), "1000 credits");
    assert_eq!(format!("{}", distance), "distance 100");
    assert_eq!(format!("{}", timeout), "60s");
} 