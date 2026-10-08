use bitcoin::{Transaction, Block, OutPoint, ScriptBuf, Txid};
use bitcoin::hashes::Hash;
use std::str::FromStr;
use std::collections::HashMap;
use hex::FromHex;
use precop_tusm_indexer::scanner_engine::{ScannerEngine};
use precop_tusm_indexer::state_db::{StateDb, SigmaLeaf};
use tusm_core::prevout::{Coin, PrevoutError, PrevoutProvider};

struct NoRpcProvider {
    cache: HashMap<OutPoint, Coin>,
}

impl PrevoutProvider for NoRpcProvider {
    fn lookup(&self, outpoint: bitcoin::OutPoint) -> Result<Coin, PrevoutError> {
        self.cache.get(&outpoint).cloned().ok_or(PrevoutError::MissingPrevout(outpoint))
    }
}

#[tokio::main]
async fn main() {
    let _ = std::fs::remove_dir_all("/tmp/tusm_offline_test_db");
    let db = StateDb::new("/tmp/tusm_offline_test_db");
    
    // Inject the state right before df2461
    let state = SigmaLeaf {
        supply: 0, 
        max_supply: 2100000000000000, 
        limit_per_mint: 1000000000000, 
        btc_rate: 3, dog_rate: 0, leaf_rate: 0, ordi_rate: 0,
        anchor: bitcoin::OutPoint::null(), height: 967500, version: 1, halted: false,
        tge_root: [0u8; 32], consumed_points: 0,
    };
    db.commit_block_batch(967500, "0000000000000000000000000000000000000000000000000000000000000000", &[("LEAF".to_string(), state, None)], &[]);
    
    let (tx_chan, _rx_chan) = tokio::sync::mpsc::channel(1);
    
    let mut engine = ScannerEngine {
        db: std::sync::Arc::new(db),
        rpc_url: "".to_string(),
        rpc_user: "".to_string(),
        rpc_pass: "".to_string(),
        external_payments: std::collections::HashMap::new(),
        pg_tx: tx_chan,
        crc21_tx: None,
    };
    
    // df2461... tx
    let tx_hex = "020000000001026980addea00c6c872801724ab095c31abf9097d4330c5488eb1e4c69577812820000000000fdffffffd7f827cdcb23867aaf82288e5fdb718e31c897c7597029589b606a1e5daf73cf0000000000fdffffff060000000000000000466a447b2270223a2269636f2d3230222c226f70223a227472616e73666572222c227469636b223a224c454146222c22616d74223a22333931353734332e38323638333339227d22020000000000001976a91462e907b15cbf27d5425399ebf6f0fb50ebb88f1888ac00000000000000002a6a287b2270223a226372632d3230222c226f70223a226d696e74222c227469636b223a224c454146227d4a01000000000000225120516e37e86d0e6d5434f2ee0bfdde0a18d328ef5faaf6e829ca799d9d1ce962881027000000000000225120bf39cfdfc985043dd4f95dd2403167d3d26874dc06ab15e7e09bf4415791c79ffb1500000000000022512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e01406c44f79e92445b773cc800a555b580c6275745907211c8da7dfb8ef7a515afe9bda25083bea21bb7d0f659e82c974159461aa6ea5c97410c80108e3bf4e092cc014079152454fb95870a6c31729c70d10959a40029f032a339e53daea0428dfa566465c8d5af3ae1024b40acb4134339c69acd44cd2dca700be4d662e7770c6f731800000000";
    let tx: Transaction = bitcoin::consensus::encode::deserialize(&Vec::from_hex(tx_hex).unwrap()).unwrap();
    
    let block = Block {
        header: bitcoin::block::Header {
            version: bitcoin::block::Version::NO_SOFT_FORK_SIGNALLING,
            prev_blockhash: bitcoin::hash_types::BlockHash::all_zeros(),
            merkle_root: bitcoin::hash_types::TxMerkleNode::all_zeros(),
            time: 0,
            bits: bitcoin::CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![tx],
    };
    
    let mut provider = NoRpcProvider { cache: HashMap::new() };
    provider.cache.insert(
        OutPoint { 
            txid: Txid::from_str("82127857694c1eeb88540c33d49790bf1ac395b04a720128876c0ca0dead8069").unwrap(), 
            vout: 0 
        },
        Coin { 
            value: 10000, 
            script_pubkey: ScriptBuf::from_hex("512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e").unwrap() 
        }
    );
    provider.cache.insert(
        OutPoint { 
            txid: Txid::from_str("cf73af5d1e6a609b58297059c797c8318e71db5f8e2882af7a8623cbcd27f8d7").unwrap(), 
            vout: 0 
        },
        Coin { 
            value: 6923, 
            script_pubkey: ScriptBuf::from_hex("512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e").unwrap() 
        }
    );
    
    println!("=== TRACE: df2461 ===");
    println!("block height: 967501");
    println!("txid: df2461f041e173d63baf313c24d429f66145adf36e304dd9eb3b5419244205f7");
    
    engine.process_block(&block, 967501, &provider).await;
    
    if let Some(sigma) = engine.db.get_sigma("LEAF") {
        println!("supply after: {}", sigma.supply);
    }
}
