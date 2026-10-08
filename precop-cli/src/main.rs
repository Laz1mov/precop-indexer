use clap::{Args, Parser, Subcommand};
use bitcoin::consensus::encode::deserialize;
use bitcoin::Transaction;
use std::sync::Arc;
use serde_json::json;

#[derive(Parser, Debug)]
#[command(
    name = "precop",
    author = "Laz1m0v and Sovereign Bitcoin Contributors",
    version = "1.1.0-rc3",
    about = "Institutional CLI for Precop Sovereign Bitcoin Layer 1 Consensus & Indexer",
    long_about = "Autonomous Layer 1 consensus validator, Taproot covenant evaluator, and state indexer for Bitcoin."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Zero-trust offline transaction consensus verification
    VerifyTx(VerifyTxArgs),

    /// Parse and decode CRC-20 / CRC-21 / Tap3 speech payload from raw tx or script hex
    ParseSpeech(ParseSpeechArgs),

    /// Bonding curve mathematical quotes and spot price calculations
    Curve(CurveArgs),

    /// Query a live Precop indexer node via REST API
    Query(QueryArgs),

    /// Offline inspection and management of local Sled state database
    Db(DbArgs),

    /// Start the full autonomous Layer 1 indexer daemon
    Daemon(DaemonArgs),
}

#[derive(Args, Debug)]
struct VerifyTxArgs {
    /// Raw Bitcoin transaction hex
    #[arg(short, long)]
    tx_hex: String,

    /// Optional prevouts script hex or json
    #[arg(short, long, default_value = "")]
    prevouts_hex: String,

    /// Optional Sigma state JSON (defaults to empty supply)
    #[arg(short, long)]
    state_json: Option<String>,

    /// Optional CRC-21 parent/item UTXO mapping JSON
    #[arg(long)]
    crc21_utxos: Option<String>,
}

#[derive(Args, Debug)]
struct ParseSpeechArgs {
    /// Raw Bitcoin transaction hex OR OP_RETURN scriptPubKey hex
    #[arg(short, long)]
    input: String,
}

#[derive(Args, Debug)]
struct CurveArgs {
    #[command(subcommand)]
    action: CurveSubcommand,
}

#[derive(Subcommand, Debug)]
enum CurveSubcommand {
    /// Calculate exact mint quote for satoshis deposited into bonding curve
    Quote {
        /// Token ticker (e.g., BONS, CRC, LEAF)
        #[arg(short, long)]
        ticker: String,

        /// Deposited satoshis or payment units
        #[arg(short, long)]
        deposit_sats: u64,

        /// Current circulating supply in token atoms (optional, defaults to 0)
        #[arg(short, long, default_value = "0")]
        current_supply: u128,

        /// Consumed USD points / points limit (for BONS/CRC)
        #[arg(long, default_value = "0")]
        consumed_points: u128,
    },
    /// Calculate instantaneous spot price at current supply
    SpotPrice {
        /// Token ticker (e.g., BONS)
        #[arg(short, long)]
        ticker: String,

        /// Consumed USD points (for BONS, target: 3,500,000,000,000)
        #[arg(long, default_value = "0")]
        consumed_usd_atoms: u64,
    },
}

#[derive(Args, Debug)]
struct QueryArgs {
    /// Node REST API base URL
    #[arg(short, long, default_value = "http://127.0.0.1:8088")]
    node_url: String,

    #[command(subcommand)]
    action: QuerySubcommand,
}

#[derive(Subcommand, Debug)]
enum QuerySubcommand {
    /// Check indexer node health status
    Health,

    /// Query token state and total supply
    Token {
        /// Token ticker (e.g., LEAF, BONS, CRC)
        #[arg(short, long)]
        ticker: String,
    },

    /// List all indexed tokens
    Tokens,

    /// Query balance of an address for a specific token
    Balance {
        /// Token ticker
        #[arg(short, long)]
        ticker: String,

        /// Bitcoin address
        #[arg(short, long)]
        address: String,
    },

    /// Get top token holders
    Holders {
        /// Token ticker
        #[arg(short, long)]
        ticker: String,

        /// Maximum number of holders to return
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Query CRC-21 collection metadata
    Collection {
        /// Collection ID (e.g., txidi0)
        #[arg(short, long)]
        id: String,
    },
}

#[derive(Args, Debug)]
struct DbArgs {
    /// Path to local Sled database
    #[arg(short, long, default_value = "./data/tusm_indexer.db")]
    db_path: String,

    #[command(subcommand)]
    action: DbSubcommand,
}

#[derive(Subcommand, Debug)]
enum DbSubcommand {
    /// Inspect token state in database
    Token {
        #[arg(short, long)]
        ticker: String,
    },

    /// Inspect address balance in database
    Balance {
        #[arg(short, long)]
        ticker: String,
        #[arg(short, long)]
        address: String,
    },

    /// List top holders in database
    Holders {
        #[arg(short, long)]
        ticker: String,
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// List all tokens found in database
    Tokens,
}

#[derive(Args, Debug)]
struct DaemonArgs {
    /// Path to Sled database
    #[arg(long, default_value = "./data/tusm_indexer.db")]
    db_path: String,

    /// HTTP API listening port
    #[arg(long, default_value = "8088")]
    port: u16,

    /// Bitcoin node RPC endpoint
    #[arg(long, default_value = "http://127.0.0.1:8332")]
    rpc_url: String,

    /// Bitcoin RPC username
    #[arg(long, default_value = "")]
    rpc_user: String,

    /// Bitcoin RPC password
    #[arg(long, default_value = "")]
    rpc_pass: String,

    /// Path to Bitcoin Core .cookie file for authentication
    #[arg(long)]
    rpc_cookie: Option<String>,

    /// External payments JSON path
    #[arg(long, default_value = "./data/external_payments.json")]
    external_payments_path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::VerifyTx(args) => {
            let state_json = args.state_json.unwrap_or_else(|| {
                json!({
                    "supply": 0,
                    "max_supply": 10000000000000000u128,
                    "limit_per_mint": 1000000000u128,
                    "btc_rate": 100000000u128,
                    "dog_rate": 100000u128,
                    "leaf_rate": 100000000u128,
                    "ordi_rate": 1000000000000000000u128,
                    "anchor": vec![0u8; 36],
                    "height": 0,
                    "version": 2,
                    "halted": false,
                    "consumed_points": 0
                }).to_string()
            });

            let verdict = tusm_judge::evaluate_transaction(
                &args.tx_hex,
                &args.prevouts_hex,
                &state_json,
                args.crc21_utxos,
            );

            // Pretty-print result
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&verdict) {
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            } else {
                println!("{}", verdict);
            }
        }

        Commands::ParseSpeech(args) => {
            let input = args.input.trim();
            let mut extracted = None;

            // Attempt 0: Direct JSON string
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(input) {
                extracted = Some(json!({
                    "type": "direct_json",
                    "payload": val
                }));
            } else if let Ok(raw_bytes) = hex::decode(input) {
                // Attempt 1: Raw transaction hex
                if let Ok(tx) = deserialize::<Transaction>(&raw_bytes) {
                    for (vout, out) in tx.output.iter().enumerate() {
                        if out.script_pubkey.is_op_return() {
                            let bytes = out.script_pubkey.as_bytes();
                            if let Some(pos) = bytes.iter().position(|&b| b == b'{') {
                                if let Ok(json_str) = std::str::from_utf8(&bytes[pos..]) {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                        extracted = Some(json!({
                                            "type": "transaction_op_return",
                                            "txid": tx.txid().to_string(),
                                            "vout": vout,
                                            "payload": val
                                        }));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                // Attempt 2: Raw script hex or push data
                if extracted.is_none() {
                    if let Some(pos) = raw_bytes.iter().position(|&b| b == b'{') {
                        if let Ok(json_str) = std::str::from_utf8(&raw_bytes[pos..]) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                                extracted = Some(json!({
                                    "type": "script_op_return",
                                    "payload": val
                                }));
                            }
                        }
                    }
                }
            }

            match extracted {
                Some(payload) => println!("{}", serde_json::to_string_pretty(&payload)?),
                None => {
                    eprintln!("[ERROR] No valid Precop/CRC speech payload could be extracted from input.");
                    std::process::exit(1);
                }
            }
        }

        Commands::Curve(curve_args) => match curve_args.action {
            CurveSubcommand::Quote {
                ticker,
                deposit_sats,
                current_supply,
                consumed_points,
            } => {
                let tick_upper = ticker.to_uppercase();
                if tick_upper == "BONS" {
                    let target_usd_atoms = primitive_types::U256::from(3_500_000_000_000u64);
                    let p_sats = primitive_types::U256::from(deposit_sats);
                    let usd_atoms = p_sats * primitive_types::U256::from(90_200u64);
                    let sold_usd_atoms = primitive_types::U256::from(consumed_points);

                    if sold_usd_atoms < target_usd_atoms {
                        let rem_usd = target_usd_atoms - sold_usd_atoms;
                        let usd_capped = std::cmp::min(usd_atoms, rem_usd);
                        let mid_usd = sold_usd_atoms + (usd_capped / primitive_types::U256::from(2u64));

                        let price_scaled = primitive_types::U256::from(7_400_000_000_000u64)
                            + (primitive_types::U256::from(22_200_000_000_000u64) * mid_usd) / target_usd_atoms;

                        let token_atoms = (usd_capped * primitive_types::U256::from(10_000_000_000_000_000u64)) / price_scaled;
                        let allocated_u128 = if token_atoms > primitive_types::U256::from(u128::MAX) { u128::MAX } else { token_atoms.as_u128() };

                        let max_supply: u128 = 21_000_000_000_000_000;
                        let remaining_supply = max_supply.saturating_sub(current_supply);
                        let final_alloc = std::cmp::min(allocated_u128, remaining_supply);

                        println!("{}", serde_json::to_string_pretty(&json!({
                            "ticker": "BONS",
                            "deposit_sats": deposit_sats,
                            "usd_value": (usd_atoms.as_u64() as f64) / 1e8,
                            "allocated_atoms": final_alloc,
                            "allocated_formatted": format!("{:.9}", (final_alloc as f64) / 1e9),
                            "new_consumed_points": consumed_points + usd_capped.as_u128(),
                            "curve_progress_pct": ((consumed_points + usd_capped.as_u128()) as f64 / 3_500_000_000_000.0) * 100.0
                        }))?);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&json!({
                            "ticker": "BONS",
                            "status": "target_reached",
                            "allocated_atoms": 0
                        }))?);
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(&json!({
                        "ticker": tick_upper,
                        "status": "linear_or_fixed",
                        "deposit_sats": deposit_sats
                    }))?);
                }
            }
            CurveSubcommand::SpotPrice {
                ticker,
                consumed_usd_atoms,
            } => {
                let tick_upper = ticker.to_uppercase();
                if tick_upper == "BONS" {
                    let target_usd_atoms = 3_500_000_000_000f64;
                    let consumed = consumed_usd_atoms as f64;
                    let p0 = 0.00074f64;
                    let p1 = 0.00296f64;
                    let spot_usd = p0 + (p1 - p0) * (consumed / target_usd_atoms);
                    let spot_sats = (spot_usd / (90_200.0 / 1e8)) * 1e9; // sats per whole token

                    println!("{}", serde_json::to_string_pretty(&json!({
                        "ticker": "BONS",
                        "spot_price_usd": spot_usd,
                        "spot_price_sats_per_token": spot_sats,
                        "target_raised_usd": 35000.0,
                        "current_raised_usd": consumed / 1e8,
                        "progress_percent": (consumed / target_usd_atoms) * 100.0
                    }))?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&json!({
                        "ticker": tick_upper,
                        "note": "Standard fixed pricing model"
                    }))?);
                }
            }
        },

        Commands::Query(query_args) => {
            let client = reqwest::Client::new();
            let base = query_args.node_url.trim_end_matches('/');

            match query_args.action {
                QuerySubcommand::Health => {
                    let url = format!("{}/v1/health", base);
                    let resp = client.get(&url).send().await?.json::<serde_json::Value>().await?;
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
                QuerySubcommand::Token { ticker } => {
                    let url = format!("{}/v1/crc20/tokens/{}", base, ticker.to_uppercase());
                    let resp = client.get(&url).send().await?.json::<serde_json::Value>().await?;
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
                QuerySubcommand::Tokens => {
                    let url = format!("{}/v1/crc20/tokens", base);
                    let resp = client.get(&url).send().await?.json::<serde_json::Value>().await?;
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
                QuerySubcommand::Balance { ticker, address } => {
                    let token_tick = ticker.to_uppercase();
                    let url = format!("{}/v1/crc20/balance/{}/{}", base, token_tick, address);
                    let resp = client.get(&url).send().await?.json::<serde_json::Value>().await?;
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
                QuerySubcommand::Holders { ticker, limit } => {
                    let url = format!("{}/v1/crc20/holders/{}?limit={}", base, ticker.to_uppercase(), limit);
                    let resp = client.get(&url).send().await?.json::<serde_json::Value>().await?;
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
                QuerySubcommand::Collection { id } => {
                    let url = format!("{}/v1/crc21/collections/{}", base, id);
                    let resp = client.get(&url).send().await?.json::<serde_json::Value>().await?;
                    println!("{}", serde_json::to_string_pretty(&resp)?);
                }
            }
        }

        Commands::Db(db_args) => {
            let db = precop_tusm_indexer::state_db::StateDb::new(&db_args.db_path);

            match db_args.action {
                DbSubcommand::Token { ticker } => {
                    let tick_upper = ticker.to_uppercase();
                    if let Some(sigma) = db.get_sigma(&tick_upper) {
                        println!("{}", serde_json::to_string_pretty(&json!({
                            "ticker": tick_upper,
                            "supply": sigma.supply.to_string(),
                            "max_supply": sigma.max_supply.to_string(),
                            "limit_per_mint": sigma.limit_per_mint.to_string(),
                            "height": sigma.height,
                            "consumed_points": sigma.consumed_points.to_string()
                        }))?);
                    } else {
                        eprintln!("[ERROR] Token '{}' not found in database.", tick_upper);
                    }
                }
                DbSubcommand::Balance { ticker, address } => {
                    let tick_upper = ticker.to_uppercase();
                    let bal = db.get_balance(&tick_upper, &address);
                    println!("{}", serde_json::to_string_pretty(&json!({
                        "ticker": tick_upper,
                        "address": address,
                        "balance": bal.to_string()
                    }))?);
                }
                DbSubcommand::Holders { ticker, limit } => {
                    let tick_upper = ticker.to_uppercase();
                    let mut holders = db.get_holders(&tick_upper);
                    holders.sort_by(|a, b| b.1.cmp(&a.1));
                    let truncated: Vec<_> = holders.into_iter().take(limit).map(|(addr, bal)| json!({
                        "address": addr,
                        "balance": bal.to_string()
                    })).collect();
                    println!("{}", serde_json::to_string_pretty(&truncated)?);
                }
                DbSubcommand::Tokens => {
                    let tokens = db.get_all_tokens();
                    let mapped: Vec<_> = tokens.into_iter().map(|(tick, sigma)| json!({
                        "ticker": tick,
                        "supply": sigma.supply.to_string(),
                        "max_supply": sigma.max_supply.to_string(),
                        "height": sigma.height,
                        "consumed_points": sigma.consumed_points.to_string()
                    })).collect();
                    println!("{}", serde_json::to_string_pretty(&mapped)?);
                }
            }
        }

        Commands::Daemon(daemon_args) => {
            std::env::set_var("TUSM_DB_PATH", &daemon_args.db_path);
            std::env::set_var("BITCOIN_RPC_URL", &daemon_args.rpc_url);
            std::env::set_var("TUSM_EXTERNAL_PAYMENTS_PATH", &daemon_args.external_payments_path);
            if let Some(cookie) = daemon_args.rpc_cookie {
                std::env::set_var("BITCOIN_RPC_COOKIE_FILE", cookie);
            }
            if !daemon_args.rpc_user.is_empty() {
                std::env::set_var("BITCOIN_RPC_USER", daemon_args.rpc_user);
            }
            if !daemon_args.rpc_pass.is_empty() {
                std::env::set_var("BITCOIN_RPC_PASS", daemon_args.rpc_pass);
            }

            println!("============================================================");
            println!("  Precop Sovereign Indexer Daemon — Autonomous Consensus L1 ");
            println!("============================================================");
            println!("DB Path:       {}", daemon_args.db_path);
            println!("RPC URL:       {}", daemon_args.rpc_url);
            println!("API Port:      {}", daemon_args.port);
            println!("============================================================");

            // Open State DB and launch Axum server
            let state_db = Arc::new(precop_tusm_indexer::state_db::StateDb::new(&daemon_args.db_path));
            let app = precop_tusm_indexer::api::router(precop_tusm_indexer::api::AppState { db: state_db });

            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], daemon_args.port));
            println!("[INFO] REST API listening on http://{}", addr);
            axum::Server::bind(&addr)
                .serve(app.into_make_service())
                .await?;
        }
    }

    Ok(())
}
