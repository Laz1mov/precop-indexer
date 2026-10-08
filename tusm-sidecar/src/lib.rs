pub mod models;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT};
use serde_json::Value;

pub use models::{VerifiedPayment, EvidenceFile, EvidenceItem, VerifyRequest, VerifyResponse};

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
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, VerifiedPayment>>(&content) {
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

    /// 1. Runes Verification via UniSat (Zero API Key)
    pub async fn verify_runes_payment(&self, txid: &str) -> Option<VerifiedPayment> {
        // Query DOG UTXOs (runeid 840000:3)
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
                                        let rune_name = r.get("rune").and_then(|s| s.as_str()).unwrap_or_default();
                                        if rid == "840000:3" || rune_name.contains("DOG") {
                                            let amt = r.get("amount")
                                                .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())))
                                                .unwrap_or(0) as u128;
                                            if amt > 0 {
                                                return Some(VerifiedPayment {
                                                    asset: "DOG".to_string(),
                                                    amount: amt,
                                                    amount_display: Some((amt as f64 / 100_000.0).to_string()),
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
                                                    block_height: None,
                                                    inscription_id: None,
                                                    inscription_number: None,
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

    /// 2. BRC-20 Verification via UniSat Inscription Info (Zero API Key)
    pub async fn verify_brc20_payment(&self, txid: &str) -> Option<VerifiedPayment> {
        let mempool_url = format!("https://mempool.space/api/tx/{}", txid);
        if let Ok(res) = self.client.get(&mempool_url).send().await {
            if res.status().is_success() {
                if let Ok(tx_val) = res.json::<Value>().await {
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
                                            if let Ok(c_res) = self.client.get(content_url).send().await {
                                                if c_res.status().is_success() {
                                                    if let Ok(c_val) = c_res.json::<Value>().await {
                                                        let p = c_val.get("p").and_then(|v| v.as_str()).unwrap_or_default();
                                                        let op = c_val.get("op").and_then(|v| v.as_str()).unwrap_or_default();
                                                        if p == "brc-20" && op == "transfer" {
                                                            let tick = c_val.get("tick").and_then(|v| v.as_str()).unwrap_or_default().to_uppercase();
                                                            let amt_str = c_val.get("amt").and_then(|v| v.as_str()).unwrap_or("0");
                                                            let dec = if tick == "ORDI" { 18u8 } else { 8u8 };
                                                            let amt_atoms = if let Ok(f) = amt_str.parse::<f64>() {
                                                                (f * 10f64.powi(dec as i32)).round() as u128
                                                            } else {
                                                                amt_str.parse::<u128>().unwrap_or(0)
                                                            };
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
                                                                destination_vout: Some(1),
                                                                seal_vout: Some(2),
                                                                block_height: None,
                                                                inscription_id: Some(insc_id),
                                                                inscription_number: None,
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
                "https://wallet-api.unisat.io/v5/address/history?address={}&start={}&limit={}",
                self.config.treasury_address, start, limit
            );

            let res = match self.client.get(&url).send().await {
                Ok(r) if r.status().is_success() => r,
                _ => break,
            };

            let body: Value = match res.json().await {
                Ok(v) => v,
                _ => break,
            };

            let data = match body.get("data") {
                Some(d) => d,
                None => break,
            };

            let details = match data.get("detail").and_then(|v| v.as_array()) {
                Some(d) if !d.is_empty() => d,
                _ => break,
            };

            let total = data.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

            for item in details {
                let txid = match item.get("txid").and_then(|t| t.as_str()) {
                    Some(t) => t,
                    None => continue,
                };

                let direction = item.get("direction").and_then(|d| d.as_str()).unwrap_or_default();
                if direction != "receive" {
                    continue;
                }

                if self.payments.read().contains_key(txid) {
                    continue;
                }

                let height = item.get("height").and_then(|h| h.as_u64()).map(|n| n as u32);

                if let Some(received) = item.get("received").and_then(|r| r.as_array()) {
                    for r in received {
                        let kind = r.get("kind").and_then(|k| k.as_str()).unwrap_or_default();
                        if kind == "runes" {
                            let symbol = r.get("displayName")
                                .or_else(|| r.get("symbol"))
                                .and_then(|s| s.as_str())
                                .unwrap_or("RUNE");
                            let asset_id = r.get("assetId").and_then(|s| s.as_str()).unwrap_or_default();
                            let amt_str = r.get("amount")
                                .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_f64().map(|n| n.to_string())))
                                .unwrap_or_else(|| "0".to_string());

                            if asset_id == "840000:3" || symbol.contains("DOG") {
                                let amt_atoms = amt_str.parse::<f64>().map(|f| (f * 100_000.0).round() as u128).unwrap_or(0);
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
                            let amt_str = r.get("amount")
                                .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_f64().map(|n| n.to_string())))
                                .unwrap_or_else(|| "0".to_string());
                            let dec = if tick == "ORDI" { 18u8 } else { 8u8 };
                            let amt_atoms = amt_str.parse::<f64>().map(|f| (f * 10f64.powi(dec as i32)).round() as u128).unwrap_or(0);

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
                                status: "confirmed".to_string(),
                            };
                            self.payments.write().insert(txid.to_string(), entry);
                            discovered += 1;
                        }
                    }
                }
            }

            start += limit;
            if start >= total as usize || start >= 300 {
                break;
            }
        }
        discovered
    }

    /// 5. Process Pending Mints Queue from disk
    pub async fn process_pending_mints(&self) -> usize {
        let p_path = &self.config.pending_mints_path;
        if !p_path.exists() {
            return 0;
        }

        let content = match std::fs::read_to_string(p_path) {
            Ok(c) => c,
            Err(_) => return 0,
        };

        let mut remaining = Vec::new();
        let mut new_verified = 0;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let txid = if trimmed.starts_with('{') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    parsed.get("txid").and_then(|t| t.as_str()).unwrap_or(trimmed).to_string()
                } else {
                    trimmed.to_string()
                }
            } else {
                trimmed.to_string()
            };

            if self.payments.read().contains_key(&txid) {
                continue;
            }

            if let Some(verified) = self.verify_polymorphic_payment(&txid).await {
                println!("[Rust Sidecar] Verified payment for candidate txid {}: {} {}", txid, verified.amount, verified.asset);
                self.payments.write().insert(txid, verified);
                new_verified += 1;
            } else {
                remaining.push(trimmed.to_string());
            }
        }

        // Rewrite remaining pending mints
        let _ = std::fs::write(p_path, remaining.join("\n"));
        new_verified
    }

    /// 6. Save all evidence and external payments atomically
    pub fn save_files(&self) {
        let payments = self.payments.read().clone();
        let mut dog_ev = self.dog_evidence.write();
        let mut ordi_ev = self.ordi_evidence.write();

        // Sync dog_evidence & ordi_evidence with all payments
        for (txid, p) in &payments {
            if p.asset == "DOG" {
                if !dog_ev.items.iter().any(|i| &i.txid == txid) {
                    dog_ev.items.push(EvidenceItem {
                        txid: txid.clone(),
                        block_height: p.block_height,
                        amount_atoms: p.amount.to_string(),
                        amount_dog: p.amount_display.clone().or_else(|| Some((p.amount as f64 / 100_000.0).to_string())),
                        amount_ordi: None,
                        rate: p.rate.or(Some(47913)),
                        destination_vout: Some(1),
                        seal_vout: Some(2),
                        inscription_id: None,
                        note: Some("Verified by Rust native UniSat sidecar".to_string()),
                    });
                }
            } else if p.asset == "ORDI" {
                if !ordi_ev.items.iter().any(|i| &i.txid == txid) {
                    ordi_ev.items.push(EvidenceItem {
                        txid: txid.clone(),
                        block_height: p.block_height,
                        amount_atoms: p.amount.to_string(),
                        amount_dog: None,
                        amount_ordi: p.amount_display.clone().or_else(|| Some((p.amount as f64 / 1e18).to_string())),
                        rate: None,
                        destination_vout: Some(1),
                        seal_vout: Some(2),
                        inscription_id: p.inscription_id.clone(),
                        note: Some("Verified by Rust native UniSat sidecar".to_string()),
                    });
                }
            }
        }
        dog_ev.count = dog_ev.items.len();
        ordi_ev.count = ordi_ev.items.len();

        // Save atomically
        let _ = atomic_save_json(&self.config.target_output_file, &payments);
        let _ = atomic_save_json(&self.config.dog_evidence_file, &*dog_ev);
        let _ = atomic_save_json(&self.config.ordi_evidence_file, &*ordi_ev);
    }

    /// Single execution cycle
    pub async fn run_cycle(&self) -> usize {
        let h_disc = self.scan_treasury_history().await;
        let q_disc = self.process_pending_mints().await;
        let total = h_disc + q_disc;
        if total > 0 || !self.config.target_output_file.exists() {
            self.save_files();
        }
        total
    }

    /// Continuous background loop
    pub async fn run_loop(&self) {
        println!("=== Precop Native Rust Sidecar Started ===");
        println!("  Target File: {:?}", self.config.target_output_file);
        println!("  Treasury:    {}", self.config.treasury_address);
        println!("  Interval:    {}s", self.config.poll_interval_secs);

        loop {
            let discovered = self.run_cycle().await;
            if discovered > 0 {
                println!("[Rust Sidecar] Discovered and saved {} new payments.", discovered);
            }
            tokio::time::sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
        }
    }
}

fn atomic_save_json<T: serde::Serialize>(path: &Path, data: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = format!("{}.tmp", path.to_string_lossy());
    let serialized = serde_json::to_string_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(&tmp_path, serialized)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}
