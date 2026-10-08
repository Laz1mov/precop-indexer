use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LegacyDeployPayload {
    pub p: String,
    pub op: String,
    pub tick: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub max: String,
    pub lim: String,
    pub leaf: Option<String>,
    pub ordi: Option<String>,
    pub btc: Option<String>,
}

pub struct LegacyV1Router;

impl LegacyV1Router {
    pub fn parse_deploy_payload(json_str: &str) -> Result<LegacyDeployPayload, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    pub fn extract_weights(payload: &LegacyDeployPayload) -> (u128, u128, u128) {
        let leaf_weight = payload.leaf.as_deref().unwrap_or("0").parse().unwrap_or(0);
        let ordi_weight = payload.ordi.as_deref().unwrap_or("0").parse().unwrap_or(0);
        let btc_weight = payload.btc.as_deref().unwrap_or("0").parse().unwrap_or(0);
        (leaf_weight, ordi_weight, btc_weight)
    }
}
