/// Configuration for getting records from the network
#[derive(Debug, Clone)]
pub struct GetRecordCfg {
    pub timeout_secs: u64,
    pub verification: VerificationKind,
}

/// Configuration for putting records to the network
#[derive(Debug, Clone)]
pub struct PutRecordCfg {
    pub timeout_secs: u64,
    pub replication: u8,
}

/// Verification requirements for records
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationKind {
    /// No verification required
    None,
    /// Verify record signature
    Signature,
    /// Verify both signature and data integrity
    Full,
} 