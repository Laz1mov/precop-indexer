pub mod models;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT};
use serde_json::Value;

pub use models::{VerifiedPayment, EvidenceFile, EvidenceItem, VerifyRequest, VerifyResponse};

/// Pure integer fixed-point decimal parser. Avoids binary floating-point (f64) precision loss.
pub fn parse_decimal_to_atoms(s: &str, decimals: u8) -> Option<u128> {
    let clean = s.trim();
    if clean.is_empty() {
        return None;
    }

    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() > 2 {
        return None;
    }

    let int_part: u128 = parts[0].parse().ok()?;
    let multiplier = 10u128.checked_pow(decimals as u32)?;
    let mut total = int_part.checked_mul(multiplier)?;

    if parts.len() == 2 {
        let frac_str = parts[1];
        if !frac_str.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let frac_len = frac_str.len();
        if frac_len <= decimals as usize {
            let shift = (decimals as usize) - frac_len;
            let frac_val: u128 = frac_str.parse().ok()?;
            let shift_mult = 10u128.checked_pow(shift as u32)?;
            let frac_scaled = frac_val.checked_mul(shift_mult)?;
            total = total.checked_add(frac_scaled)?;
        } else {
            let (allowed_frac, excess) = frac_str.split_at(decimals as usize);
            let frac_val: u128 = allowed_frac.parse().ok()?;
            total = total.checked_add(frac_val)?;
            // If excess contains non-zero digits, reject precision loss
            if excess.chars().any(|c| c != '0') {
                return None;
            }
        }
    }
    Some(total)
}

/// SSRF protection: Enforce HTTPS and restrict outbound fetching to trusted inscription indexers.
pub fn is_allowed_content_url(url_str: &str) -> bool {
    let parsed = match url::Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return false,
    };

    if parsed.scheme() != "https" {
        return false;
    }

    let host = match parsed.host_str() {
        Some(h) => h.to_lowercase(),
        None => return false,
    };

    // Explicitly reject private/loopback/link-local/metadata IP addresses
    if host == "localhost"
        || host == "127.0.0.1"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("169.254.")
        || host.starts_with("172.16.") || host.starts_with("172.17.") || host.starts_with("172.18.")
        || host.starts_with("172.19.") || host.starts_with("172.2") || host.starts_with("172.30.") || host.starts_with("172.31.")
        || host == "[::1]" || host == "::1"
    {
        return false;
    }

    // Trusted inscription indexer domain allowlist
    let trusted_domains = [
        "unisat.io",
        "ordinals.com",
        "api.hiro.so",
        "ordiscan.com",
        "ord.io",
        "mempool.space",
        "magiceden.io",
    ];

    trusted_domains.iter().any(|d| host == *d || host.ends_with(&format!(".{}", d)))
}

#[derive(Debug, Clone)]
pub struct SidecarConfig {
    pub target_output_file: PathBuf,
    pub dog_evidence_file: PathBuf,
    pub ordi_evidence_file: PathBuf,
    pub pending_mints_path: PathBuf,
    pub treasury_address: String,
    pub unisat_api_key: String,
    pub poll_interval_secs: u64,
    pub http_port: u16,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        let base_data = if Path::new("./data").exists() {
            PathBuf::from("./data")
        } else {
            PathBuf::from("/app/data")
        };

        Self {
            target_output_file: std::env::var("TARGET_OUTPUT_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base_data.join("external_payments.json")),
            dog_evidence_file: std::env::var("DOG_EVIDENCE_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base_data.join("dog-evidence.json")),
            ordi_evidence_file: std::env::var("ORDI_EVIDENCE_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base_data.join("ordi-evidence.json")),
            pending_mints_path: std::env::var("PENDING_MINTS_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| base_data.join("pending_mints.jsonl")),
            treasury_address: std::env::var("TREASURY_ADDRESS")
                .unwrap_or_else(|_| "bc1p8q4hnqrve9tdd7yzx650m4ct007w38p8gkcylrmffherq25987fq4e6ajr".to_string()),
            unisat_api_key: std::env::var("UNISAT_API_KEY").unwrap_or_default(),
            poll_interval_secs: std::env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(15),
            http_port: std::env::var("SIDECAR_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8085),
        }
    }
}

pub struct SidecarEngine {
    pub config: SidecarConfig,
    pub client: reqwest::Client,
    pub payments: Arc<RwLock<HashMap<String, VerifiedPayment>>>,
    pub dog_evidence: Arc<RwLock<EvidenceFile>>,
    pub ordi_evidence: Arc<RwLock<EvidenceFile>>,
}

impl SidecarEngine {
    pub fn new(config: SidecarConfig) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Precop Consensus Sidecar)"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(12))
            .build()
            .unwrap_or_default();

        let mut payments_map = HashMap::new();
        if let Ok(content) = std::fs::read_to_string(&config.target_output_file) {
            if let Ok(mut parsed) = serde_json::from_str::<HashMap<String, VerifiedPayment>>(&content) {
                let secret = std::env::var("SIDECAR_ATTESTATION_SECRET")
                    .unwrap_or_else(|_| tusm_core::attestation::DEFAULT_ATTESTATION_SECRET.to_string());
                let mut upgraded = false;
                for (txid, payment) in parsed.iter_mut() {
                    if payment.signature.is_none() {
                        let sig = tusm_core::attestation::create_attestation_hmac(
                            txid,
                            &payment.asset,
                            payment.amount,
                            payment.rate,
                            &secret,
                        );
                        payment.signature = Some(sig);
                        upgraded = true;
                    }
                }
                if upgraded {
                    let _ = Self::atomic_write_json(&config.target_output_file, &parsed);
                }
                payments_map = parsed;
            }
        }

        let mut dog_ev = EvidenceFile::new("DOG", "runes");
        if let Ok(content) = std::fs::read_to_string(&config.dog_evidence_file) {
            if let Ok(parsed) = serde_json::from_str::<EvidenceFile>(&content) {
                dog_ev = parsed;
            }
        }

        let mut ordi_ev = EvidenceFile::new("ORDI", "brc-20");
        if let Ok(content) = std::fs::read_to_string(&config.ordi_evidence_file) {
            if let Ok(parsed) = serde_json::from_str::<EvidenceFile>(&content) {
                ordi_ev = parsed;
            }
        }

        Self {
            config,
            client,
            payments: Arc::new(RwLock::new(payments_map)),
            dog_evidence: Arc::new(RwLock::new(dog_ev)),
            ordi_evidence: Arc::new(RwLock::new(ordi_ev)),
        }
    }

    /// 1. Runes Verification via UniSat (Strict Rune ID matching: 840000:3 only)
    pub async fn verify_runes_payment(&self, txid: &str) -> Option<VerifiedPayment> {
        let url = format!(
            "https://wallet-api.unisat.io/v5/runes/utxos?address={}&runeid=840000:3",
            self.config.treasury_address
        );

        if let Ok(res) = self.client.get(&url).send().await {
            if res.status().is_success() {
                if let Ok(body) = res.json::<Value>().await {
                    if let Some(utxos) = body.get("data").and_then(|d| d.as_array()) {
                        for u in utxos {
                            if u.get("txid").and_then(|t| t.as_str()) == Some(txid) {
                                if let Some(runes) = u.get("runes").and_then(|r| r.as_array()) {
                                    for r in runes {
                                        let rid = r.get("runeid").and_then(|s| s.as_str()).unwrap_or_default();
                                        // PRECOP-02 FIX: Strict matching on Rune ID 840000:3 ONLY, never loose substring
                                        if rid == "840000:3" {
                                            let amt = r.get("amount")
                                                .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
                                                .unwrap_or(0) as u128;
                                            if amt > 0 {
                                                let dest_vout = u.get("vout")
                                                    .and_then(|v| v.as_u64())
                                                    .map(|n| n as u32)
                                                    .or(Some(1));
                                                let display_str = format!("{}.{:05}", amt / 100_000, amt % 100_000);
                                                let secret = std::env::var("SIDECAR_ATTESTATION_SECRET")
                                                    .unwrap_or_else(|_| tusm_core::attestation::DEFAULT_ATTESTATION_SECRET.to_string());
                                                let signature = Some(tusm_core::attestation::create_attestation_hmac(
                                                    txid, "DOG", amt, Some(47913), &secret
                                                ));

                                                return Some(VerifiedPayment {
                                                    asset: "DOG".to_string(),
                                                    amount: amt,
                                                    amount_display: Some(display_str),
                                                    rate: Some(47913),
                                                    payment_type: "runes".to_string(),
                                                    protocol: "runes".to_string(),
                                                    runeid: Some("840000:3".to_string()),
                                                    symbol: Some("DOG•GO•TO•THE•MOON".to_string()),
                                                    decimals: Some(5),
                                                    ticker: None,
                                                    destination: Some(self.config.treasury_address.clone()),
                                                    destination_vout: dest_vout,
                                                    seal_vout: Some(2),
                                                    block_height: None,
                                                    inscription_id: None,
                                                    inscription_number: None,
                                                    signature,
                                                    status: "confirmed".to_string(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// 2. BRC-20 Verification via Inscription Info (With strict treasury delivery proof & SSRF protection)
    pub async fn verify_brc20_payment(&self, txid: &str) -> Option<VerifiedPayment> {
        let mempool_url = format!("https://mempool.space/api/tx/{}", txid);
        if let Ok(res) = self.client.get(&mempool_url).send().await {
            if res.status().is_success() {
                if let Ok(tx_val) = res.json::<Value>().await {
                    // PRECOP-02 FIX: Verify that the transaction actually delivers an output to treasury!
                    let mut matched_treasury_vout = None;
                    if let Some(vouts) = tx_val.get("vout").and_then(|v| v.as_array()) {
                        for (idx, out) in vouts.iter().enumerate() {
                            let addr = out.get("scriptpubkey_address").and_then(|a| a.as_str());
                            if addr == Some(&self.config.treasury_address) {
                                matched_treasury_vout = Some(idx as u32);
                                break;
                            }
                        }
                    }

                    let destination_vout = match matched_treasury_vout {
                        Some(idx) => idx,
                        None => {
                            // Transaction does NOT send any output to protocol treasury!
                            return None;
                        }
                    };

                    if let Some(vins) = tx_val.get("vin").and_then(|v| v.as_array()) {
                        for vin in vins {
                            let in_txid = vin.get("txid").and_then(|t| t.as_str()).unwrap_or_default();
                            let in_vout = vin.get("vout").and_then(|v| v.as_u64()).unwrap_or(0);
                            if in_txid.is_empty() {
                                continue;
                            }
                            let insc_id = format!("{}i{}", in_txid, in_vout);
                            let u_url = format!("https://wallet-api.unisat.io/v5/inscription/info?inscriptionId={}", insc_id);
                            if let Ok(u_res) = self.client.get(&u_url).send().await {
                                if u_res.status().is_success() {
                                    if let Ok(u_val) = u_res.json::<Value>().await {
                                        if let Some(content_url) = u_val.get("data").and_then(|d| d.get("content")).and_then(|c| c.as_str()) {
                                            // PRECOP-06 FIX: Strict SSRF protection
                                            if !is_allowed_content_url(content_url) {
                                                continue;
                                            }

                                            if let Ok(c_res) = self.client.get(content_url).send().await {
                                                if c_res.status().is_success() {
                                                    if let Ok(c_val) = c_res.json::<Value>().await {
                                                        let p = c_val.get("p").and_then(|v| v.as_str()).unwrap_or_default();
                                                        let op = c_val.get("op").and_then(|v| v.as_str()).unwrap_or_default();
                                                        if p == "brc-20" && op == "transfer" {
                                                            let tick = c_val.get("tick").and_then(|v| v.as_str()).unwrap_or_default().to_uppercase();
                                                            let amt_str = c_val.get("amt").and_then(|v| v.as_str()).unwrap_or("0");
                                                            let dec = if tick == "ORDI" { 18u8 } else { 8u8 };

                                                            // PRECOP-07 FIX: Pure integer fixed-point decimal arithmetic (no binary f64)
                                                            let amt_atoms = parse_decimal_to_atoms(amt_str, dec)
                                                                .or_else(|| amt_str.parse::<u128>().ok())
                                                                .unwrap_or(0);

                                                            let secret = std::env::var("SIDECAR_ATTESTATION_SECRET")
                                                                .unwrap_or_else(|_| tusm_core::attestation::DEFAULT_ATTESTATION_SECRET.to_string());
                                                            let signature = Some(tusm_core::attestation::create_attestation_hmac(
                                                                txid, &tick, amt_atoms, None, &secret
                                                            ));

                                                            return Some(VerifiedPayment {
                                                                asset: tick.clone(),
                                                                amount: amt_atoms,
                                                                amount_display: Some(amt_str.to_string()),
                                                                rate: None,
                                                                payment_type: "brc-20".to_string(),
                                                                protocol: "brc-20".to_string(),
                                                                runeid: None,
                                                                symbol: None,
                                                                decimals: Some(dec),
                                                                ticker: Some(tick.to_lowercase()),
                                                                destination: Some(self.config.treasury_address.clone()),
                                                                destination_vout: Some(destination_vout),
                                                                seal_vout: Some(2),
                                                                block_height: None,
                                                                inscription_id: Some(insc_id),
                                                                inscription_number: None,
                                                                signature,
                                                                status: "confirmed".to_string(),
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// 3. Polymorphic Dispatcher (Runes -> BRC-20)
    pub async fn verify_polymorphic_payment(&self, txid: &str) -> Option<VerifiedPayment> {
        if let Some(p) = self.verify_runes_payment(txid).await {
            return Some(p);
        }
        if let Some(p) = self.verify_brc20_payment(txid).await {
            return Some(p);
        }
        None
    }

    /// 4. Continuous Treasury History Streaming
    pub async fn scan_treasury_history(&self) -> usize {
        let mut discovered = 0;
        let mut start = 0;
        let limit = 50;

        loop {
            let url = format!(
                "https://wallet-api.unisat.io/v5/address/activities?address={}&start={}&limit={}",
                self.config.treasury_address, start, limit
            );

            let res = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(_) => break,
            };

            if !res.status().is_success() {
                break;
            }

            let body = match res.json::<Value>().await {
                Ok(b) => b,
                Err(_) => break,
            };

            let data = match body.get("data") {
                Some(d) => d,
                None => break,
            };

            let total = data.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
            let items = match data.get("list").and_then(|l| l.as_array()) {
                Some(it) => it,
                None => break,
            };

            if items.is_empty() {
                break;
            }

            for item in items {
                let txid = match item.get("txid").and_then(|t| t.as_str()) {
                    Some(t) => t,
                    None => continue,
                };

                if self.payments.read().contains_key(txid) {
                    continue;
                }

                let height = item.get("height").and_then(|h| h.as_u64()).map(|n| n as u32);

                if let Some(received) = item.get("received").and_then(|r| r.as_array()) {
                    for r in received {
                        let kind = r.get("kind").and_then(|k| k.as_str()).unwrap_or_default();
                        if kind == "runes" {
                            let asset_id = r.get("assetId").and_then(|s| s.as_str()).unwrap_or_default();
                            // PRECOP-07 FIX: Do NOT use as_f64(), preserve exact literal JSON number representation
                            let amt_str = r.get("amount")
                                .and_then(|v| match v {
                                    serde_json::Value::String(s) => Some(s.clone()),
                                    serde_json::Value::Number(n) => Some(n.to_string()),
                                    _ => None,
                                })
                                .unwrap_or_else(|| "0".to_string());

                            // PRECOP-02 FIX: Strict matching on 840000:3 only
                            if asset_id == "840000:3" {
                                // PRECOP-07 FIX: Pure integer decimal parsing
                                let amt_atoms = parse_decimal_to_atoms(&amt_str, 5).unwrap_or(0);
                                let secret = std::env::var("SIDECAR_ATTESTATION_SECRET")
                                    .unwrap_or_else(|_| tusm_core::attestation::DEFAULT_ATTESTATION_SECRET.to_string());
                                let signature = Some(tusm_core::attestation::create_attestation_hmac(
                                    txid, "DOG", amt_atoms, Some(47913), &secret
                                ));
                                let entry = VerifiedPayment {
                                    asset: "DOG".to_string(),
                                    amount: amt_atoms,
                                    amount_display: Some(amt_str),
                                    rate: Some(47913),
                                    payment_type: "runes".to_string(),
                                    protocol: "runes".to_string(),
                                    runeid: Some("840000:3".to_string()),
                                    symbol: Some("DOG•GO•TO•THE•MOON".to_string()),
                                    decimals: Some(5),
                                    ticker: None,
                                    destination: Some(self.config.treasury_address.clone()),
                                    destination_vout: Some(1),
                                    seal_vout: Some(2),
                                    block_height: height,
                                    inscription_id: None,
                                    inscription_number: None,
                                    signature,
                                    status: "confirmed".to_string(),
                                };
                                self.payments.write().insert(txid.to_string(), entry);
                                discovered += 1;
                            }
                        } else if kind == "brc20" {
                            let tick = r.get("symbol")
                                .or_else(|| r.get("ticker"))
                                .and_then(|s| s.as_str())
                                .unwrap_or("BRC20")
                                .to_uppercase();
                            // PRECOP-07 FIX: Do NOT use as_f64(), preserve exact literal JSON number representation
                            let amt_str = r.get("amount")
                                .and_then(|v| match v {
                                    serde_json::Value::String(s) => Some(s.clone()),
                                    serde_json::Value::Number(n) => Some(n.to_string()),
                                    _ => None,
                                })
                                .unwrap_or_else(|| "0".to_string());
                            let dec = if tick == "ORDI" { 18u8 } else { 8u8 };

                            // PRECOP-07 FIX: Pure integer decimal parsing
                            let amt_atoms = parse_decimal_to_atoms(&amt_str, dec).unwrap_or(0);
                            let secret = std::env::var("SIDECAR_ATTESTATION_SECRET")
                                .unwrap_or_else(|_| tusm_core::attestation::DEFAULT_ATTESTATION_SECRET.to_string());
                            let signature = Some(tusm_core::attestation::create_attestation_hmac(
                                txid, &tick, amt_atoms, None, &secret
                            ));

                            let entry = VerifiedPayment {
                                asset: tick.clone(),
                                amount: amt_atoms,
                                amount_display: Some(amt_str),
                                rate: None,
                                payment_type: "brc-20".to_string(),
                                protocol: "brc-20".to_string(),
                                runeid: None,
                                symbol: None,
                                decimals: Some(dec),
                                ticker: Some(tick.to_lowercase()),
                                destination: Some(self.config.treasury_address.clone()),
                                destination_vout: Some(1),
                                seal_vout: Some(2),
                                block_height: height,
                                inscription_id: None,
                                inscription_number: None,
                                signature,
                                status: "confirmed".to_string(),
                            };
                            self.payments.write().insert(txid.to_string(), entry);
                            discovered += 1;
                        }
                    }
                }
            }

            start += limit;
            if start >= total as usize {
                break;
            }
        }

        if discovered > 0 {
            self.save_files();
        }

        discovered
    }

    /// 5. Monitor and process pending candidate mints
    pub async fn process_pending_mints(&self) -> usize {
        let path = &self.config.pending_mints_path;
        if !path.exists() {
            return 0;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return 0,
        };

        let mut lines: Vec<String> = content.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if lines.is_empty() {
            return 0;
        }

        let mut remaining = Vec::new();
        let mut processed = 0;

        for txid in lines.drain(..) {
            if self.payments.read().contains_key(&txid) {
                continue;
            }

            if let Some(payment) = self.verify_polymorphic_payment(&txid).await {
                println!("[Rust Sidecar] Verified Pending TXID {} -> Asset: {}, Amount: {}", txid, payment.asset, payment.amount);
                self.payments.write().insert(txid, payment);
                processed += 1;
            } else {
                remaining.push(txid);
            }
        }

        let new_content = if remaining.is_empty() {
            String::new()
        } else {
            remaining.join("\n") + "\n"
        };
        let _ = std::fs::write(path, new_content);

        if processed > 0 {
            self.save_files();
        }

        processed
    }

    /// Persist memory state to disk files (atomic save)
    pub fn save_files(&self) {
        let payments = self.payments.read().clone();
        let mut dog_ev = self.dog_evidence.write();
        let mut ordi_ev = self.ordi_evidence.write();

        // Sync dog_evidence & ordi_evidence with all payments
        for (txid, p) in &payments {
            if p.asset == "DOG" {
                if !dog_ev.items.iter().any(|i| &i.txid == txid) {
                    let display_str = p.amount_display.clone()
                        .unwrap_or_else(|| format!("{}.{:05}", p.amount / 100_000, p.amount % 100_000));
                    dog_ev.items.push(EvidenceItem {
                        txid: txid.clone(),
                        block_height: p.block_height,
                        amount_atoms: p.amount.to_string(),
                        amount_dog: Some(display_str),
                        amount_ordi: None,
                        rate: p.rate.or(Some(47913)),
                        destination_vout: p.destination_vout.or(Some(1)),
                        seal_vout: Some(2),
                        inscription_id: None,
                        note: Some("Verified by Rust native UniSat sidecar".to_string()),
                    });
                }
            } else if p.asset == "ORDI" {
                if !ordi_ev.items.iter().any(|i| &i.txid == txid) {
                    let display_str = p.amount_display.clone()
                        .unwrap_or_else(|| format!("{}.{:018}", p.amount / 1_000_000_000_000_000_000, p.amount % 1_000_000_000_000_000_000));
                    ordi_ev.items.push(EvidenceItem {
                        txid: txid.clone(),
                        block_height: p.block_height,
                        amount_atoms: p.amount.to_string(),
                        amount_dog: None,
                        amount_ordi: Some(display_str),
                        rate: None,
                        destination_vout: p.destination_vout.or(Some(1)),
                        seal_vout: Some(2),
                        inscription_id: p.inscription_id.clone(),
                        note: Some("Verified by Rust native UniSat sidecar".to_string()),
                    });
                }
            }
        }
        dog_ev.count = dog_ev.items.len();
        ordi_ev.count = ordi_ev.items.len();

        let _ = Self::atomic_write_json(&self.config.target_output_file, &payments);
        let _ = Self::atomic_write_json(&self.config.dog_evidence_file, &*dog_ev);
        let _ = Self::atomic_write_json(&self.config.ordi_evidence_file, &*ordi_ev);
    }

    fn atomic_write_json<T: serde::Serialize>(path: &Path, data: &T) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        let content = serde_json::to_string_pretty(data)?;
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(tmp_path, path)?;
        Ok(())
    }

    /// Background event loop: streams continuous activities and processes pending mint requests
    pub async fn run_loop(&self) {
        let interval = Duration::from_secs(self.config.poll_interval_secs);
        println!("[Rust Sidecar] Continuous poller started with interval {}s", self.config.poll_interval_secs);

        loop {
            let _ = self.scan_treasury_history().await;
            let _ = self.process_pending_mints().await;
            tokio::time::sleep(interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_integer_decimal_parsing_no_f64_precision_loss() {
        // Test case from the auditor's PoC that failed on binary f64:
        let s = "9007199254740.993";
        let atoms = parse_decimal_to_atoms(s, 18).expect("Must parse exactly without f64");
        // Exact integer representation: 9007199254740 * 10^18 + 993 * 10^15
        let expected: u128 = 9007199254740993000000000000000;
        assert_eq!(atoms, expected, "Must be exact to the atom, zero float precision drift");

        // DOG runes (5 decimals)
        let dog_s = "100.5";
        let dog_atoms = parse_decimal_to_atoms(dog_s, 5).expect("Must parse DOG");
        assert_eq!(dog_atoms, 10050000);

        // Whole number
        let whole = "50000";
        let whole_atoms = parse_decimal_to_atoms(whole, 8).expect("Must parse whole");
        assert_eq!(whole_atoms, 50000_00000000);
    }

    #[test]
    fn test_ssrf_allowed_urls() {
        // Disallowed / dangerous URLs:
        assert!(!is_allowed_content_url("http://169.254.169.254/latest/meta-data"));
        assert!(!is_allowed_content_url("http://127.0.0.1:8080/admin"));
        assert!(!is_allowed_content_url("http://localhost:8085/v1/verify"));
        assert!(!is_allowed_content_url("https://malicious-attacker.com/payload.json"));
        assert!(!is_allowed_content_url("ftp://ordinals.com/file"));

        // Allowed trusted indexers:
        assert!(is_allowed_content_url("https://ordinals.com/content/27d4e5488f5c47de6794b84551502a3c60c1a221908741495fa0b7i0"));
        assert!(is_allowed_content_url("https://wallet-api.unisat.io/v5/content/abc"));
        assert!(is_allowed_content_url("https://api.hiro.so/ordinals/v1/inscriptions/abc/content"));
    }
}
