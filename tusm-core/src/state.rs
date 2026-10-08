
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    InvalidTopology,
    InvalidSpeech,
    UnknownTicker,
    Halted,
    InvalidSequence,
    InvalidCsvCondition,
    InvalidTreasury,
    InvalidVault,
    InvalidAmount,
    Overflow,
    Underflow,
    InvalidAnchor,
    InvalidTaprootCommitment,
    InvalidInputContext,
    RustTypeScriptDivergence,
    NonCanonicalEncoding,
    Reorg,
    UnsupportedVersion,
    InvalidMerkleProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyResult {
    Sign,
    Reject(RejectReason),
}

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigmaLeaf {
    pub supply: u128,
    pub max_supply: u128,
    pub limit_per_mint: u128,
    pub btc_rate: u128,
    pub dog_rate: u128,
    pub leaf_rate: u128,
    pub ordi_rate: u128,
    pub anchor: Vec<u8>, // OutPoint mock
    pub height: u32,
    pub version: u8,
    pub halted: bool,
    pub consumed_points: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigmaAbtc {
    pub supply: u128,
    pub anchor: [u8; 36],
    pub vault: [u8; 36],
    pub height: u32,
    pub version: u8,
    pub halted: bool,
}
