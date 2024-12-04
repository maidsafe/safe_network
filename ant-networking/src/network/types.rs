use libp2p::PeerId;
use std::{fmt, time::Duration};
use thiserror::Error;

/// A newtype wrapper for network prices to ensure type safety
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkPrice(u64);

impl NetworkPrice {
    /// Creates a new NetworkPrice if the value is within valid range
    pub fn new(price: u64) -> Result<Self, NetworkTypeError> {
        if price > 1_000_000_000 {
            return Err(NetworkTypeError::PriceTooHigh(price));
        }
        Ok(Self(price))
    }

    /// Get the raw price value
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A newtype wrapper for network distances to ensure valid ranges
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkDistance(u32);

impl NetworkDistance {
    /// Creates a new NetworkDistance if the value is within valid range
    pub fn new(distance: u32) -> Result<Self, NetworkTypeError> {
        if distance > 256 {
            return Err(NetworkTypeError::DistanceTooLarge(distance));
        }
        Ok(Self(distance))
    }

    /// Get the raw distance value
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// A wrapper for network timeouts to ensure they're within reasonable bounds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkTimeout(Duration);

impl NetworkTimeout {
    const MIN_TIMEOUT: Duration = Duration::from_secs(1);
    const MAX_TIMEOUT: Duration = Duration::from_secs(300);

    /// Creates a new NetworkTimeout if the duration is within valid range
    pub fn new(duration: Duration) -> Result<Self, NetworkTypeError> {
        if duration < Self::MIN_TIMEOUT {
            return Err(NetworkTypeError::TimeoutTooShort(duration, Self::MIN_TIMEOUT));
        }
        if duration > Self::MAX_TIMEOUT {
            return Err(NetworkTypeError::TimeoutTooLong(duration, Self::MAX_TIMEOUT));
        }
        Ok(Self(duration))
    }

    /// Get the raw duration value
    pub fn duration(&self) -> Duration {
        self.0
    }
}

/// Quote information for network payments with stronger typing
#[derive(Debug, Clone)]
pub struct PayeeQuote {
    pub peer_id: PeerId,
    pub price: NetworkPrice,
    pub expiry: NetworkTimeout,
}

/// Errors that can occur when creating network types
#[derive(Debug, Error)]
pub enum NetworkTypeError {
    #[error("Price {0} exceeds maximum allowed value")]
    PriceTooHigh(u64),
    
    #[error("Distance {0} exceeds maximum allowed value")]
    DistanceTooLarge(u32),
    
    #[error("Timeout {0:?} is too short (minimum {1:?})")]
    TimeoutTooShort(Duration, Duration),
    
    #[error("Timeout {0:?} is too long (maximum {1:?})")]
    TimeoutTooLong(Duration, Duration),
}

impl fmt::Display for NetworkPrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} credits", self.0)
    }
}

impl fmt::Display for NetworkDistance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "distance {}", self.0)
    }
}

impl fmt::Display for NetworkTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
} 