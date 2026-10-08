use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedPayment {
    pub asset: String,
    pub amount: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u64>,
    #[serde(rename = "type")]
    pub payment_type: String,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runeid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_vout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seal_vout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inscription_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inscription_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceFile {
    pub asset: String,
    pub protocol: String,
    pub count: usize,
    pub items: Vec<EvidenceItem>,
}

impl EvidenceFile {
    pub fn new(asset: &str, protocol: &str) -> Self {
        Self {
            asset: asset.to_string(),
            protocol: protocol.to_string(),
            count: 0,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub txid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_height: Option<u32>,
    pub amount_atoms: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_dog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_ordi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_vout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seal_vout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inscription_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub txid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<VerifiedPayment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
