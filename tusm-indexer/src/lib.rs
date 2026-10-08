pub mod scanner_engine; pub mod state_db; pub mod legacy_v1; pub mod crc21_engine;
pub mod crc21_cbor;
pub mod crc21_envelope;
pub mod api;
pub mod block_bridge;
pub mod bridge_client;
pub mod vault_watch;

use once_cell::sync::OnceCell;
use tokio::sync::mpsc::Sender;
use bridge_client::BridgePayload;

use tokio::sync::mpsc::UnboundedSender;
pub static BRIDGE_TX: OnceCell<UnboundedSender<BridgePayload>> = OnceCell::new();
