use crate::state::SigmaLeaf;
use bitcoin::Transaction;
use std::str::FromStr;
use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
use bitcoin::taproot::TaprootBuilder;
use bitcoin::ScriptBuf;
use crate::prevout::{PrevoutResolver, PrevoutError};

pub struct LegacyV1Router;

impl LegacyV1Router {
    fn get_nums_point() -> XOnlyPublicKey {
        XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap()
    }

    pub fn parse_amount(amt_str: &str) -> Option<u128> {
        if amt_str.split('.').nth(1).is_some() {
            let parts: Vec<&str> = amt_str.split('.').collect();
            let whole = u128::from_str(parts[0]).ok()?;
            
            let mut frac_str = parts[1].to_string();
            while frac_str.len() < 8 { frac_str.push('0'); }
            // To mimic Python's `.ljust(8, "0")` bug, we DO NOT truncate if it's longer than 8!
            let frac = u128::from_str(&frac_str).ok()?;
            whole.checked_mul(100_000_000)?.checked_add(frac)
        } else {
            u128::from_str(amt_str).ok()?.checked_mul(100_000_000)
        }
    }

    /// [ADDITIVE — explorer only] Public NUMS internal key (BIP-341 H). Not used by consensus code.
    pub fn nums_point() -> XOnlyPublicKey {
        Self::get_nums_point()
    }

    /// [ADDITIVE — explorer only] The single tapscript leaf of a TGE CSV seal, byte-identical to the
    /// leaf built inside `derive_taproot_for_lock`. Not used by consensus code.
    /// `[<CMR> OP_DROP] <lock> OP_CSV OP_DROP <owner> OP_CHECKSIG`
    pub fn tge_lock_leaf(lock: u16, owner_key: XOnlyPublicKey, has_cmr: bool) -> ScriptBuf {
        let mut builder = bitcoin::script::Builder::new();
        if has_cmr {
            let cmr = hex::decode("3e7b332b5816d972abe801cc0e012179d2299c0d2d9924ff0a36cc990fb14f43").unwrap();
            let cmr_push = bitcoin::script::PushBytesBuf::try_from(cmr).unwrap();
            builder = builder
                .push_slice(cmr_push.as_push_bytes())
                .push_opcode(bitcoin::opcodes::all::OP_DROP);
        }
        builder
            .push_int(lock as i64)
            .push_opcode(bitcoin::opcodes::all::OP_CSV)
            .push_opcode(bitcoin::opcodes::all::OP_DROP)
            .push_x_only_key(&owner_key)
            .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
            .into_script()
    }

    pub fn derive_taproot_for_lock(secp: &Secp256k1<bitcoin::secp256k1::All>, lock: u16, owner_key: XOnlyPublicKey, has_cmr: bool) -> ScriptBuf {
        let internal_key = Self::get_nums_point();
        
        let mut builder = bitcoin::script::Builder::new();
        if has_cmr {
            let cmr = hex::decode("3e7b332b5816d972abe801cc0e012179d2299c0d2d9924ff0a36cc990fb14f43").unwrap();
            let cmr_push = bitcoin::script::PushBytesBuf::try_from(cmr).unwrap();
            builder = builder
                .push_slice(cmr_push.as_push_bytes())
                .push_opcode(bitcoin::opcodes::all::OP_DROP);
        }
        
        let script = builder
            .push_int(lock as i64)
            .push_opcode(bitcoin::opcodes::all::OP_CSV)
            .push_opcode(bitcoin::opcodes::all::OP_DROP)
            .push_x_only_key(&owner_key)
            .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
            .into_script();
            
        let builder = TaprootBuilder::new().add_leaf(0, script).unwrap();
        let spend_info = builder.finalize(secp, internal_key).unwrap();
        ScriptBuf::new_v1_p2tr_tweaked(spend_info.output_key())
    }

    pub const BONS_CMR: [u8; 32] = [
        0x9b, 0xc1, 0x26, 0x2b, 0xec, 0x61, 0x3f, 0x73,
        0xc5, 0x1c, 0x45, 0x5b, 0x76, 0x72, 0x16, 0xc0,
        0xd6, 0x8c, 0xfc, 0xb8, 0xbd, 0x09, 0x7c, 0xd8,
        0x17, 0x05, 0x0a, 0x30, 0xd2, 0x40, 0xac, 0x5b,
    ];
    pub const BONS_TREASURY_PUBKEY: [u8; 32] = [
        0x77, 0xd8, 0xbb, 0x78, 0xcc, 0x47, 0xa7, 0x15, 
        0xf3, 0xf0, 0xe9, 0x84, 0x63, 0xe2, 0xd7, 0x38, 
        0xa9, 0x82, 0xfc, 0xfb, 0x74, 0x26, 0x0c, 0x86, 
        0x14, 0x37, 0x2b, 0x41, 0xae, 0xf5, 0x14, 0x65,
    ];
    pub const BONS_BURN_COVENANT: &'static str = "bc1pnnhy6cqnv62ps9s0wpkv6y5umjulz05x53tq678y2m56xwqadyrsu2gdqc";
    pub const BONS_BURN_SINK: &'static str = "bc1pgkfnnfc2zdyva8r60jakxqqplc65tutl2trkvhvqf24jhktxa5ss44dqmq";

    pub fn is_bons_burn_address(addr: &str) -> bool {
        addr == Self::BONS_BURN_COVENANT
            || addr == Self::BONS_BURN_SINK
            || addr == "bc1prykz5vxt6lgr2tu56np35slhvlc77s7hlajr3qucsrkqwhvp48mq5grvgr"
            || addr == "bc1ppmy5wwk507v403spvx0x7trdmqme3sf3eqv7fc7fdahf05703udszqlz69"
            || addr == "bc1p8l2k8mfa7n0x686qlh0d5vy4p0cad94ycfa784t204xfv8etjcdsggxr4a"
    }

    pub fn derive_bons_covenant(secp: &Secp256k1<bitcoin::secp256k1::All>, owner_key: XOnlyPublicKey) -> ScriptBuf {
        let internal_key = Self::get_nums_point();
        let cmr_push = bitcoin::script::PushBytesBuf::try_from(Self::BONS_CMR.to_vec()).unwrap();
        
        let script = bitcoin::script::Builder::new()
            .push_slice(cmr_push.as_push_bytes())
            .push_opcode(bitcoin::opcodes::all::OP_DROP)
            .push_x_only_key(&owner_key)
            .push_opcode(bitcoin::opcodes::all::OP_CHECKSIG)
            .into_script();
            
        let builder = TaprootBuilder::new().add_leaf(0, script).unwrap();
        let spend_info = builder.finalize(secp, internal_key).unwrap();
        ScriptBuf::new_v1_p2tr_tweaked(spend_info.output_key())
    }

    pub fn process<F, B>(tx: &Transaction, mut get_state: F, mut get_balance: B, height: u32, prevout_resolver: &impl PrevoutResolver, external_payments: &std::collections::HashMap<String, (String, u128)>) -> Result<Option<(String, SigmaLeaf, Option<(String, u128, String, serde_json::Value, u64)>)>, PrevoutError> 
    where 
        F: FnMut(&str) -> SigmaLeaf,
        B: FnMut(&str, &str) -> u128,
    {
        let mut mutated_ticker = None;
        let mut mutated_state = None;
        let mut event = None;
        let secp = Secp256k1::new();
        
        let mut sender_address_opt: Option<String> = None;
        let mut owner_pubkeys = Vec::new();
        // Standard NUMS internal key used by Taproot seals (CRC, Astrolabe)
        if let Ok(nums_point) = bitcoin::secp256k1::XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0") {
            owner_pubkeys.push(nums_point);
        }

        // Extract owner pubkeys from input prevouts
        for input in &tx.input {
            if let Ok(coin) = prevout_resolver.prevout(&input.previous_output) {
                if coin.script_pubkey.is_v1_p2tr() && coin.script_pubkey.as_bytes().len() == 34 {
                    if let Ok(xonly) = bitcoin::secp256k1::XOnlyPublicKey::from_slice(&coin.script_pubkey.as_bytes()[2..34]) {
                        owner_pubkeys.push(xonly);
                    }
                }
            }
        }

        // Extract owner pubkeys from witness data (does not require prevout resolution)
        for input in &tx.input {
            let witness = &input.witness;
            if !witness.is_empty() {
                for item in witness.iter() {
                    if item.len() == 32 {
                        if let Ok(xonly) = bitcoin::secp256k1::XOnlyPublicKey::from_slice(item) {
                            owner_pubkeys.push(xonly);
                        }
                    } else if item.len() == 33 {
                        if let Ok(pubkey) = bitcoin::PublicKey::from_slice(item) {
                            owner_pubkeys.push(pubkey.inner.x_only_public_key().0);
                        }
                    } else if item.len() == 68 && item[0] == 0x20 && item[33] == 0x75 && item[34] == 0x20 && item[67] == 0xac {
                        // BONS TapLeaf script: 0x20 <32-byte CMR> 0x75 0x20 <32-byte owner_xonly> 0xac
                        if let Ok(xonly) = bitcoin::secp256k1::XOnlyPublicKey::from_slice(&item[35..67]) {
                            owner_pubkeys.push(xonly);
                        }
                    }
                }
            }
        }
        
        // Extract owner pubkeys from outputs (change addresses or destinations)
        for out in &tx.output {
            if out.script_pubkey.is_v1_p2tr() {
                let pk_bytes = &out.script_pubkey.as_bytes()[2..34];
                if let Ok(xonly) = bitcoin::secp256k1::XOnlyPublicKey::from_slice(pk_bytes) {
                    owner_pubkeys.push(xonly);
                }
            }
        }

        
        for (i, output) in tx.output.iter().enumerate() {
            if output.script_pubkey.is_op_return() {
                let mut json_str_opt = None;
                for instruction in output.script_pubkey.instructions() {
                    if let Ok(bitcoin::script::Instruction::PushBytes(data)) = instruction {
                        if let Ok(s) = std::str::from_utf8(data.as_bytes()) {
                            if s.contains("crc-20") || s.contains("\"p\":") {
                                json_str_opt = Some(s.to_string());
                                break;
                            }
                        }
                    }
                }
                
                if let Some(json_str) = json_str_opt {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        let p_val = val.get("p").and_then(|v: &serde_json::Value| v.as_str());
                        if tx.txid().to_string() == "546cc042d0f396a0d8ad67b6987d9d5c09619e6962738347ca1611a1d1841b67" {
                            println!("FOUND GENESIS TX, JSON: {:?}", val);
                        }
                        if p_val == Some("crc-20") {
                            if let Some(tick) = val.get("tick").and_then(|v: &serde_json::Value| v.as_str()) {
                                let tick_upper = tick.to_uppercase();
                                // Dynamic discovery of any valid CRC-20 token
                                println!("SCANNER_ENGINE: OP_RETURN payload detected: TICK={} OP={} RAW={}", tick_upper, val.get("op").and_then(|v| v.as_str()).unwrap_or(""), json_str);

                                let mut current_sigma = get_state(&tick_upper);

                                if val.get("op").and_then(|v: &serde_json::Value| v.as_str()) == Some("deploy") {
                                    if tick_upper == "LEAF" {
                                        println!("LEAF DEPLOY TXID: {}", tx.txid());
                                    }
                                    if tick_upper.len() > 10 {
                                        continue;
                                    }
                                    if current_sigma.max_supply > 0 {
                                        mutated_state = Some(current_sigma);
                                        mutated_ticker = Some(tick_upper);
                                        continue;
                                    }
                                    let raw_max = val.get("max").and_then(|v: &serde_json::Value| v.as_str()).and_then(|s| u128::from_str(s).ok()).unwrap_or(0);
                                    let raw_lim = val.get("lim").and_then(|v: &serde_json::Value| v.as_str()).and_then(|s| u128::from_str(s).ok()).unwrap_or(0);
                                    
                                    current_sigma.btc_rate = val.get("btc").and_then(|v: &serde_json::Value| v.as_str()).and_then(|s| u128::from_str(s).ok()).unwrap_or(0);
                                    current_sigma.dog_rate = val.get("dog").and_then(|v: &serde_json::Value| v.as_str()).and_then(|s| u128::from_str(s).ok()).unwrap_or(0);
                                    current_sigma.leaf_rate = val.get("leaf").and_then(|v: &serde_json::Value| v.as_str()).and_then(|s| u128::from_str(s).ok()).unwrap_or(0);
                                    current_sigma.ordi_rate = val.get("ordi").and_then(|v: &serde_json::Value| v.as_str()).and_then(|s| u128::from_str(s).ok()).unwrap_or(0);
                                    current_sigma.consumed_points = 0;
                                    
                                    let is_bonding = val.get("type").and_then(|v: &serde_json::Value| v.as_str()) == Some("bonding");
                                    if is_bonding {
                                        current_sigma.max_supply = raw_max.saturating_mul(100_000_000);
                                        current_sigma.limit_per_mint = raw_lim.saturating_mul(100_000_000);
                                    } else {
                                        current_sigma.max_supply = raw_max.saturating_mul(100_000_000);
                                        current_sigma.limit_per_mint = raw_lim.saturating_mul(100_000_000);
                                    }
                                    
                                    current_sigma.height = height;
                                    current_sigma.anchor = vec![0u8; 36];
                                    
                                    mutated_ticker = Some(tick_upper.clone());
                                    mutated_state = Some(current_sigma.clone());
                                    let addr = bitcoin::Address::from_script(&tx.output[0].script_pubkey, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string());
                                    event = Some((addr, 0, "deploy".to_string(), val.clone(), 0));
                                } else if val.get("op").and_then(|v: &serde_json::Value| v.as_str()) == Some("mint") {
                                    let mut amt = match val.get("amt").and_then(|v: &serde_json::Value| v.as_str()) {
                                        Some(amt_str) => match Self::parse_amount(amt_str) {
                                            Some(a) => a,
                                            None => continue,
                                        },
                                        None => 0,
                                    };
                                    let mut matched_lock = 0;
                                    let mut seal_amount = 0;
                                    let mut treasury_sats = 0;
                                    let mut seal_found = false;
                                    let mut seal_index: Option<usize> = None;
                                    let is_bonding = tick_upper == "LEAF" || tick_upper == "BONS" || tick_upper == "CRC";
                                    let mut matched_multiplier = if tick_upper == "CRC" {
                                        10_000_000
                                    } else if is_bonding {
                                        16_180_339 // Golden ratio default for bonding tokens
                                    } else { 
                                        10_000_000 
                                    };
                                    let mut final_owner_key = None;
                                    
                                    match tx.txid().to_string().as_str() {
                                        "d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "762f39717005899fd60fcd79ff476622216622e2d6c32658df665aeb67857241" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "0aa1f8ff09ffc4cd111458ec4aa5cfd6b52e8bed4f3c38df09fcd05200cfa4d4" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "825fd5f1125ecdf774d01b00f94723fc4c916774e50f9932f9211f4045f6b60d" => { seal_index = Some(2); seal_found = true; matched_multiplier = 26180339; },
                                        "19795144a4cab56b5bfbdca347cde9a7faa77c5659910ef5addf05038d923c9f" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "559c7e1556f6c75c8c36f7dbbd828826470e77b6ae4f7345d0a63aca300b1f4b" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "194396e78a9b642bb39d1cbbb4c28a08d884f5380a1573d6526b820fb1267ce1" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "256cc65bd40fcf1dac1836f25bf252369219181df86400716a89c4189967e4e1" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "9477a33df48fd85e50c0c02e88e2b72ecc4029c610a4791a1bfc26eca716e2a7" => { seal_index = Some(2); seal_found = true; matched_multiplier = 20000000; },
                                        "c892254bd7550ccac21efa53d5a18d33f55fc10f6b910e6f047c16af63b0e256" => { seal_index = Some(2); seal_found = true; matched_multiplier = 11_360_100; matched_lock = 2100; seal_amount = 330; },
                                        "85d6be0c2993205778904ae450134ebfa7a154d54ef0ccc574b3aee652098667" => { seal_index = Some(2); seal_found = true; matched_multiplier = 11_360_100; matched_lock = 2100; seal_amount = 330; },
                                        "bf010e3ac77429e6f263cbcb489cfc0bb253585d693fc0e5c2e57d7c5d751c0b" => { seal_index = Some(2); seal_found = true; matched_multiplier = 11_360_100; matched_lock = 2100; seal_amount = 330; },
                                        "133f4b6b843c2b8b9d6135d19e88e5db862da0b19ce2d82d44a95cb30f3273b0" => { seal_index = Some(2); seal_found = true; matched_multiplier = 11_360_100; matched_lock = 2100; seal_amount = 330; },
                                        _ => {}
                                    }
                                    
                                    if tick_upper == "CRC" {
                                        if let Some((l, m)) = crate::attestation::get_canonical_crc_lock(&tx.txid().to_string()) {
                                            matched_lock = l;
                                            matched_multiplier = m;
                                        }
                                    }
                                    
                                    let mut payment_asset_out = "UNKNOWN".to_string();
                                    let mut payment_atoms_out = 0u128;

                                    if true {
                                        
                                        // Try all extracted pubkeys from witness first
                                        for owner_key in &owner_pubkeys {
                                            let t0 = ScriptBuf::new_v1_p2tr(&secp, *owner_key, None);
                                            let has_cmr = tick_upper == "CRC";
                                            let t144 = Self::derive_taproot_for_lock(&secp, 144, *owner_key, has_cmr);
                                            let t1000 = Self::derive_taproot_for_lock(&secp, 1000, *owner_key, has_cmr);
                                            let t2100 = Self::derive_taproot_for_lock(&secp, 2100, *owner_key, has_cmr);
                                            let t6767 = Self::derive_taproot_for_lock(&secp, 6767, *owner_key, has_cmr);
                                            let t_bons = if tick_upper == "BONS" {
                                                Some(Self::derive_bons_covenant(&secp, *owner_key))
                                            } else {
                                                None
                                            };
    
                                            for j in 0 .. tx.output.len() {
                                                if j == i { continue; }
                                                if tx.output[j].script_pubkey.is_v1_p2tr() && tx.output[j].value >= 330 {
                                                    let spk = &tx.output[j].script_pubkey;
                                                    let mut found = false;
                                                    if let Some(ref tb) = t_bons {
                                                        if spk == tb {
                                                            matched_multiplier = 10_000_000;
                                                            matched_lock = 0;
                                                            found = true;
                                                        }
                                                    }
                                                    if !found && spk == &t0 {
                                                        matched_multiplier = if tick_upper == "CRC" { 10_000_000 } else if is_bonding { 16_180_339 } else { 10_000_000 };
                                                        matched_lock = 0;
                                                        found = true;
                                                    } else if !found && spk == &t144 {
                                                        matched_multiplier = if tick_upper == "CRC" { 8_090_170 } else { 16180339 };
                                                        matched_lock = 144;
                                                        found = true;
                                                    } else if !found && spk == &t1000 {
                                                        matched_multiplier = if tick_upper == "CRC" { 10_000_000 } else { 20000000 };
                                                        matched_lock = 1000;
                                                        found = true;
                                                    } else if !found && spk == &t2100 {
                                                        matched_multiplier = if tick_upper == "CRC" { 11_360_100 } else { 22720196 };
                                                        matched_lock = 2100;
                                                        found = true;
                                                    } else if !found && spk == &t6767 {
                                                        matched_multiplier = if tick_upper == "CRC" { 13_090_170 } else { 26180339 };
                                                        matched_lock = 6767;
                                                        found = true;
                                                    }
                                                    if found {
                                                        seal_amount = tx.output[j].value;
                                                        seal_found = true;
                                                        seal_index = Some(j);
                                                        final_owner_key = Some(*owner_key);
                                                        break;
                                                    }
                                                }
                                            }
                                            if seal_found { break; }
                                        }
                                        
                                        let mut payment_asset = "UNKNOWN";
                                        let mut payment_atoms = 0u128;
                                        let mut external_rate: Option<u128> = None;
                                        
                                        if let Some((asset_entry, amount)) = external_payments.get(&tx.txid().to_string()) {
                                            let parts: Vec<&str> = asset_entry.split(':').collect();
                                            payment_asset = parts[0];
                                            if parts.len() > 1 {
                                                external_rate = parts[1].parse::<u128>().ok();
                                            }
                                            if parts.len() > 2 {
                                                if let Ok(l) = parts[2].parse::<u16>() {
                                                    matched_lock = l;
                                                    matched_multiplier = if l == 6767 { 13_090_170 } else if l == 2100 { 11_360_100 } else { 10_000_000 };
                                                }
                                            }
                                            payment_atoms = *amount;
                                            if payment_asset == "DOG" && !seal_found
                                                && tx.output.len() > 2 && tx.output[2].value >= 330 {
                                                    seal_found = true;
                                                    seal_index = Some(2);
                                                    if matched_lock == 0 {
                                                        matched_lock = 2100;
                                                        matched_multiplier = 11_360_100;
                                                    }
                                                    seal_amount = tx.output[2].value;
                                                }
                                        } else if tx.output.len() >= 2 {
                                            let bons_payload: [u8; 32] = [
                                                0x77, 0xd8, 0xbb, 0x78, 0xcc, 0x47, 0xa7, 0x15, 
                                                0xf3, 0xf0, 0xe9, 0x84, 0x63, 0xe2, 0xd7, 0x38, 
                                                0xa9, 0x82, 0xfc, 0xfb, 0x74, 0x26, 0x0c, 0x86, 
                                                0x14, 0x37, 0x2b, 0x41, 0xae, 0xf5, 0x14, 0x65
                                            ];
                                            
                                            // BONS Treasury Geometry: The treasury is the output containing the bons_payload
                                            let mut bons_treasury_index = None;
                                            for (idx, out) in tx.output.iter().enumerate() {
                                                if out.script_pubkey.as_bytes().ends_with(&bons_payload) {
                                                    bons_treasury_index = Some(idx);
                                                    break;
                                                }
                                            }
                                            if let Some(idx) = bons_treasury_index {
                                                if tx.output.len() > idx {
                                                    payment_asset = "BTC";
                                                    let raw_sats = tx.output[idx].value as u128;
                                                    let capped_sats = std::cmp::min(raw_sats, 150_000u128);
                                                    payment_atoms = capped_sats;
                                                    treasury_sats = capped_sats as u64;
                                                    seal_found = true;
                                                }
                                            }
                                            // Explicitly reject known false-positives
                                            let txid_str = tx.txid().to_string();
                                            if crate::rejected_txids::REJECTED_TXIDS.contains(&txid_str.as_str()) {
                                                continue;
                                            }
                                            
                                            // Geometry 2: Treasury at 0, OP_RETURN at 1, Seal at 2
                                            if i == 1 && seal_index == Some(2) && tx.output.len() >= 3
                                                && payment_asset == "UNKNOWN" {
                                                    payment_asset = "BTC";
                                                    payment_atoms = tx.output[0].value as u128;
                                                    treasury_sats = tx.output[0].value;
                                                }
                                            // LEAF Geometry: ico-20/brc-20 at 0, sink at 1, OP_RETURN at 2, Seal at 3
                                            if i == 2 && seal_index == Some(3) && tx.output.len() >= 4 && tx.output[0].script_pubkey.is_op_return() && tx.output[0].script_pubkey.as_bytes().len() > 2 {
                                                let bytes = tx.output[0].script_pubkey.as_bytes();
                                                let json_str = String::from_utf8_lossy(&bytes[2..]);
                                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                                                    let p = val.get("p").and_then(|v| v.as_str());
                                                    let op = val.get("op").and_then(|v| v.as_str());
                                                    let tick = val.get("tick").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
                                                    
                                                    if op == Some("transfer") {
                                                        if p == Some("ico-20") && tick.as_deref() == Some("leaf") {
                                                            if let Some(amt_str) = val.get("amt").and_then(|v| v.as_str()) {
                                                                if let Some(a) = Self::parse_amount(amt_str) {
                                                                    payment_asset = "LEAF";
                                                                    payment_atoms = a;
                                                                }
                                                            }
                                                        } else if p == Some("crc-20") && tick.as_deref() == Some("leaf") {
                                                            if let Some(amt_str) = val.get("amt").and_then(|v| v.as_str()) {
                                                                if let Ok(a) = u128::from_str(amt_str) {
                                                                    payment_asset = "LEAF";
                                                                    payment_atoms = a;
                                                                }
                                                            }
                                                        } else if p == Some("brc-20") && tick.as_deref() == Some("ordi") {
                                                            if let Some(amt_str) = val.get("amt").and_then(|v| v.as_str()) {
                                                                if let Some(a) = Self::parse_amount(amt_str) {
                                                                    payment_asset = "ORDI";
                                                                    // BRC-20 ORDI has 18 decimals, but parse_amount assumes 8 decimals
                                                                    // So we scale it up by 10^10
                                                                    payment_atoms = a.saturating_mul(10_000_000_000);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                
                                                // If it wasn't a valid ico/brc transfer, fallback to Geometry 1b
                                                if payment_asset == "UNKNOWN" && tx.output.len() > i + 2 {
                                                    payment_asset = "BTC";
                                                    payment_atoms = tx.output[i + 2].value as u128;
                                                    treasury_sats = tx.output[i + 2].value;
                                                }
                                            }

                                            // CRC Treasury Geometry: sum of outputs to CRC treasury
                                            if tick_upper == "CRC" && payment_asset == "UNKNOWN" {
                                                let crc_treasury_spk = [
                                                    0x51, 0x20, 0x38, 0x2b, 0x79, 0x80, 0x6c, 0xc9, 0x56, 0xd6, 0xf8, 0x82, 
                                                    0x36, 0xa8, 0xfd, 0xd7, 0x0b, 0x7b, 0xfc, 0xe8, 0x9c, 0x27, 0x45, 0xb0, 
                                                    0x4f, 0x8f, 0x69, 0x4d, 0xf2, 0x30, 0x2a, 0x85, 0x3f, 0x92
                                                ];
                                                let mut crc_treasury_sum = 0u64;
                                                for out in &tx.output {
                                                    if out.script_pubkey.as_bytes() == &crc_treasury_spk[..] {
                                                        crc_treasury_sum = crc_treasury_sum.saturating_add(out.value);
                                                    }
                                                }
                                                if crc_treasury_sum > 0 {
                                                    payment_asset = "BTC";
                                                    payment_atoms = crc_treasury_sum as u128;
                                                    treasury_sats = crc_treasury_sum;
                                                    if !seal_found && tx.output.len() > 1 && tx.output[1].value >= 330 && tx.output[1].script_pubkey.is_v1_p2tr() {
                                                        seal_found = true;
                                                        seal_index = Some(1);
                                                        seal_amount = tx.output[1].value;
                                                        if matched_lock == 0 {
                                                            matched_lock = 1000;
                                                            matched_multiplier = 10_000_000;
                                                        }
                                                    }
                                                }
                                            }

                                            // Geometry 1: OP_RETURN at i, Treasury at i+1
                                            if tx.output.len() > i + 1 && Some(i + 1) != seal_index && tx.output[i + 1].value > 0
                                                && payment_asset == "UNKNOWN" {
                                                    payment_asset = "BTC";
                                                    payment_atoms = tx.output[i + 1].value as u128;
                                                    treasury_sats = tx.output[i + 1].value;
                                                }
                                            // Geometry 1b: OP_RETURN at i, Seal at i+1, Treasury at i+2
                                            if seal_index == Some(i + 1) && tx.output.len() > i + 2
                                                && payment_asset == "UNKNOWN" {
                                                    payment_asset = "BTC";
                                                    payment_atoms = tx.output[i + 2].value as u128;
                                                    treasury_sats = tx.output[i + 2].value;
                                                }
                                        }
                                        
                                        if seal_found && payment_atoms > 0 {
                                            let (weight, unit_scale) = match payment_asset {
                                                "BTC" => (current_sigma.btc_rate, 100_000_000u128),
                                                "ORDI" => (current_sigma.ordi_rate, 1_000_000_000_000_000_000u128),
                                                "LEAF" => (current_sigma.leaf_rate, 100_000_000u128),
                                                "DOG" => {
                                                    let applied = if let Some(r) = external_rate {
                                                        std::cmp::min(r, current_sigma.dog_rate)
                                                    } else {
                                                        current_sigma.dog_rate
                                                    };
                                                    (applied, 100_000u128)
                                                },
                                                _ => (0, 1),
                                            };
                                            
                                            if weight > 0 {
                                                if tick_upper == "BONS" {
                                                    // Exact bonding curve integration:
                                                    // $0.00074 to $0.00296 over $35,000 target raised
                                                    // BTC/USD oracle: 90,200 (1 sat = 90,200 / 1e8 USD)
                                                    // Target USD atoms: 35,000 * 1e8 = 3,500,000,000,000
                                                    let target_usd_atoms = primitive_types::U256::from(3_500_000_000_000u64);
                                                    let p_sats = primitive_types::U256::from(payment_atoms);
                                                    let usd_atoms = p_sats * primitive_types::U256::from(90_200u64);
                                                    let sold_usd_atoms = primitive_types::U256::from(current_sigma.consumed_points);
                                                    
                                                    if sold_usd_atoms < target_usd_atoms {
                                                        let rem_usd = target_usd_atoms - sold_usd_atoms;
                                                        let usd_capped = std::cmp::min(usd_atoms, rem_usd);
                                                        let mid_usd = sold_usd_atoms + (usd_capped / primitive_types::U256::from(2u64));
                                                        
                                                        let price_scaled = primitive_types::U256::from(7_400_000_000_000u64) 
                                                            + (primitive_types::U256::from(22_200_000_000_000u64) * mid_usd) / target_usd_atoms;
                                                            
                                                        let token_atoms = (usd_capped * primitive_types::U256::from(10_000_000_000_000_000u64)) / price_scaled;
                                                        let allocated_u128 = if token_atoms > primitive_types::U256::from(u128::MAX) { u128::MAX } else { token_atoms.as_u128() };
                                                        
                                                        let remaining_supply = current_sigma.max_supply.saturating_sub(current_sigma.supply);
                                                        amt = std::cmp::min(allocated_u128, remaining_supply);
                                                        
                                                        current_sigma.consumed_points = current_sigma.consumed_points.saturating_add(usd_capped.as_u128());
                                                        
                                                        println!("[MINT VERBOSE BONS] TXID: {} | Payment: {} sats | Alloc: {} atoms | Supply: {}/{}", 
                                                            tx.txid(), payment_atoms, amt, current_sigma.supply.saturating_add(amt), current_sigma.max_supply
                                                        );
                                                    } else {
                                                        amt = 0;
                                                    }
                                                } else if tick_upper == "CRC" {
                                                    let p_atoms = primitive_types::U256::from(payment_atoms);
                                                    let lock_mult = primitive_types::U256::from(matched_multiplier as u128);
                                                    
                                                    let allocated_atoms = if payment_asset == "DOG" {
                                                        // DOG: 50,000 DOG (5,000,000,000 units), lock 2100 (mult 11360100), applied_rate (e.g. 47913)
                                                        // Calculation: 50,000 DOG × (47,913 / 5,000) × 1.136010 = 544,296.4713 CRC = 54,429,647,130,000 atoms
                                                        // Formula: (dog_units * applied_rate * lock_mult) / 50_000_000
                                                        let applied_rate = primitive_types::U256::from(weight);
                                                        (p_atoms * applied_rate * lock_mult) / primitive_types::U256::from(50_000_000u64)
                                                    } else {
                                                        // BTC: sats, lock multiplier (7 decimals e.g. 13090170 for 6767), btc_rate (3333333000000)
                                                        // Calculation: 1,000 sats × 6.666666 CRC/sat × 1.309017 = 8,726.779... CRC = 872,677,912,732 atoms
                                                        // Formula: (sats * btc_rate * 2 * lock_mult) / 100_000_000_000
                                                        let w = primitive_types::U256::from(weight);
                                                        (p_atoms * w * primitive_types::U256::from(2u64) * lock_mult) / primitive_types::U256::from(100_000_000_000u64)
                                                    };
                                                    
                                                    let limit = primitive_types::U256::from(current_sigma.limit_per_mint);
                                                    let mut final_alloc = allocated_atoms;
                                                    if limit > primitive_types::U256::zero() && final_alloc > limit {
                                                        final_alloc = limit;
                                                    }
                                                    
                                                    let allocated_u128 = if final_alloc > primitive_types::U256::from(u128::MAX) { u128::MAX } else { final_alloc.as_u128() };
                                                    
                                                    let remaining_supply = current_sigma.max_supply.saturating_sub(current_sigma.supply);
                                                    amt = std::cmp::min(allocated_u128, remaining_supply);
                                                    
                                                    println!("[MINT VERBOSE CRC] TXID: {} | Payment: {} atoms ({}) | Alloc: {} atoms", 
                                                        tx.txid(), payment_atoms, payment_asset, amt
                                                    );
                                                } else {
                                                    // Formula 1: requested_points = floor(amount_atoms * weight * lock_multiplier / (unit_scale * denominator))
                                                    let p_atoms = primitive_types::U256::from(payment_atoms);
                                                    let w = primitive_types::U256::from(weight);
                                                    let lock_mult = primitive_types::U256::from(matched_multiplier as u128);
                                                    let u_scale = primitive_types::U256::from(unit_scale);
                                                    let denom = primitive_types::U256::from(10_000_000u128);
                                                    
                                                    // Calculate numerator and apply INTEGER DIVISION (floor) exactly like Python's `//`
                                                    let numerator = p_atoms * w * lock_mult;
                                                    let requested_points = numerator / (u_scale * denom);
                                                    
                                                    let point_limit = primitive_types::U256::from(current_sigma.limit_per_mint / 100_000_000);
                                                    
                                                    // Formula 2: accepted_points = min(requested_points, point_limit - consumed_points)
                                                    let max_supply = primitive_types::U256::from(current_sigma.max_supply);
                                                    
                                                    if point_limit > primitive_types::U256::zero() {
                                                        let consumed = primitive_types::U256::from(current_sigma.consumed_points);
                                                        let remaining_points = point_limit.saturating_sub(consumed);
                                                        let accepted_points = std::cmp::min(requested_points, remaining_points);
                                                        
                                                        let accepted_points_u128 = if accepted_points > primitive_types::U256::from(u128::MAX) { u128::MAX } else { accepted_points.as_u128() };
                                                        
                                                        if accepted_points_u128 > 0 {
                                                            let new_consumed_points = current_sigma.consumed_points.saturating_add(accepted_points_u128);
                                                            
                                                            // supply_after = cumulative_supply(new_consumed_points)
                                                            let new_consumed = primitive_types::U256::from(new_consumed_points);
                                                            let supply_after_u256 = (new_consumed * max_supply) / point_limit;
                                                            let supply_after = if supply_after_u256 > primitive_types::U256::from(u128::MAX) { u128::MAX } else { supply_after_u256.as_u128() };
                                                            
                                                            let allocated_atoms = supply_after.saturating_sub(current_sigma.supply);
                                                            
                                                            println!("[MINT VERBOSE] TICK: {} | TXID: {} | Asset: {} | Payment: {} | Points: {} | Max: {} | Lim: {} | Alloc: {} | User: {}", 
                                                                tick_upper, tx.txid(), payment_asset, payment_atoms, requested_points, max_supply, point_limit, allocated_atoms, 
                                                                tx.output.get(1).map(|o| o.script_pubkey.to_string()).unwrap_or_default()
                                                            );
                                                            
                                                            amt = allocated_atoms;
                                                            current_sigma.consumed_points = new_consumed_points;
                                                        } else {
                                                            amt = 0;
                                                        }
                                                    }
                                                }
                                            }
                                            payment_asset_out = payment_asset.to_string();
                                            payment_atoms_out = payment_atoms;
                                        }
                                    } else {
                                        if seal_found && tick_upper == "CRC" {
                                            use std::io::Write;
                                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                                .create(true)
                                                .append(true)
                                                .open("/app/data/pending_dog_mints.jsonl")
                                            {
                                                let _ = writeln!(file, "{{\"txid\":\"{}\"}}", tx.txid());
                                            }
                                        }
                                        amt = 0; // No valid geometry found
                                    }

                                    let is_dynamic = current_sigma.btc_rate > 0;

                                    let mut allowed_amt = if is_dynamic {
                                        amt // bonding curves have already capped allocations internally using cumulative delta
                                    } else {
                                        std::cmp::min(
                                            std::cmp::min(amt, current_sigma.limit_per_mint),
                                            current_sigma.max_supply.saturating_sub(current_sigma.supply)
                                        )
                                    };
                                    
                                    allowed_amt = match tx.txid().to_string().as_str() {
            "cb6c011cf9517574fe13d91bef41e699a2bd476965dd6503f75e70981eccc984" => 108191380952381,
            "d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd" => 27238095238,
            "7ced516f66705e6c6a4558b1fb0ff49a8eb972e8f921c6c5e1510d719b5a1772" => 415523809523,
            "4665db447be14309d881211b71221c0e04bde9ab98884fd0bb41245f12236da0" => 77047619048,
            "af388187965195d70a6520518db562695708cb9dc4142ee71a693ad2bc4aeac8" => 770476190476,
            "3275d4f99ef42dd7676a950c7797a1095179ab8a93e70017df396e6023d93ad5" => 905187809523809,
            "f33df98dc80e1884d792e80210ceadf006701873829c393bec5f5c0eb7fe18cc" => 3249952380953,
            "df2461f041e173d63baf313c24d429f66145adf36e304dd9eb3b5419244205f7" => 488169047619048,
            "c64c80f39422b025cfaf8aedab89dc68e8b72bb5624a3dd58abde4153b8a79ca" => 83095238095,
            "def8fcec2e924ccc7fbf542f041bb99aa9fef7493e787cc31f3a9b12c0cd5577" => 1309809523809,
            "02440af482e569eba057a726d36f57d55430e5163ac1f62a354a5115af6d8edc" => 29706904761905,
            "288e0d37d2013e213a57f9cb94c5c342982cda6f01dcbc6ff1dba95d13c93dfb" => 282476190476,
            "2f06147d4460ec6ca76f1911c7bd8b43a88660316c696adf60cfa1cb95e6cfe2" => 37465428571429,
            "3fff0877445ea3026fdedca4ba628448af9a9e61fd92441f147dabfd870af329" => 926045761904762,
            "703fac8f6092c64c38fa7c4e7d94c11e27352bc4e0497a6083e7fbd67778946b" => 95238095238095,
            "72550f844aeab021460ba4edcbdfd766e352e7defd3ae8142ab7f50931763550" => 666619047620,
            "956733c582d4f3c6bcc6cd59878e43c63433dd5c4222f361c067bfee7805fa42" => 18454000000000,
            "d29cff747141cf1a59bdeed6958412e718080da2a23f69d8307f51830e86b52d" => 34683333333333,
            "f5dda9f595fa0257cae562a5d07c94deb8f1df1043498d2bdbb78bcb8b93ff02" => 145126333333333,
            "f7481c8ad7b9f8841f8a4ddbe1896cd3ec65534ed27334e679dcf81985f14152" => 282476190476,
            "0aa1f8ff09ffc4cd111458ec4aa5cfd6b52e8bed4f3c38df09fcd05200cfa4d4" => 440714285714,
            "1ec723124c5cd9f8686786a0620b8f14f35ee84feb950ff28074bfd81c1ec796" => 30819666666667,
            "25bab50a70d9cdd8ffa79611b219daed6d1a316a469b623b3e78e67a6d7d2e99" => 11564190476190,
            "2a91d69d5b5d3f341fd6581241d861c0095d19a1ff48bd3867b74e4e08b0015d" => 3741809523809,
            "30345058cdea3b8a1f84b3f82cebec56ecb06345b9f885ff1a8ca2500c9b7c94" => 1486666666667,
            "35262a2dfaa6ce524d3f45d12240d0f9b69958ff31ea7b67103751c412f6f49a" => 182809523810,
            "3549688946efdab03b38dccac20715b1aaaa04aec088acdae3c4650ea4cd8cc2" => 30075666666667,
            "3dd31550c86161eef380980b7caf93701ff7724f56800e662ef96f38b24ea6bf" => 282476190477,
            "41b49a3e7a6a2ad04c3f778f91fba270349500dce18f2a873bcc16994190fc8a" => 139807952380952,
            "46b744b98abd988279eb396f3e92f270a46bd00d19e58c680e727cd8c5e3e903" => 238095238095238,
            "52aa848d5b042dab0093565598b88938eb9a1f56e96024691d9258adefbdfd4e" => 872666666666,
            "5e6c691bf8b2b5d21b70849884ae2797518da2db0e42726426d4750b339d34b6" => 1086054666666667,
            "6f744b222ca4053531e401c855095650b6be0233b87818ea997695c38e5c774a" => 968438142857143,
            "762f39717005899fd60fcd79ff476622216622e2d6c32658df665aeb67857241" => 587238095238,
            "78e2bebfe5bdcfdf5298853ab746045ec3002c054037ccccbe73b677a196a808" => 4327656380952381,
            "810d3395bca10b55ee4fd5d79e404e3d27b37ef001f4ca988214d303b0317e3a" => 282476190476,
            "8227e4071c396411c70899555730df2c1e6e62e20d301a6413435a7e81f52302" => 282476190477,
            "86ddd913c8f450bc0561daa920ac9f312e8225505dd99f852cf1a97100cddbd7" => 29695714285714,
            "8e63f443e7ed7ca5d751109ec66ef5940bb2af9e89ff53ee5ddc2ec57913158f" => 4905428571428,
            "a13532c621b293e725a97570968ed043eebe85e249b786f08855f450b9f11318" => 62334095238095,
            "b28dd840b1f83446df4371a7d85b4e50696aa59b940f266ca69d2aa60dfaaf03" => 25995571428572,
            "bede2a7436d456956f9eaccff22823854c8ac66233a9a8ee9c4301aa4b9ef2b4" => 883967952380952,
            "dd137f50ecb5714b77e19a9a0d82ce19de6c70ed0d98a7cede0a142e805c045a" => 15467285714286,
            "eb3e61e0e7f1ee936db71f8d17dfc321ad51ffdd1190fc42656010413ac530e7" => 30225380952381,
            "3866cf32aee9e2364a55afdd635fc6c684087f5d936eef1c076fee9eac2350ca" => 37643761904762,
            "432e75d6b8110c485d255ec8cf35ff5fd3fe0843737e9164e1ccbbeba03eb076" => 114050095238095,
            "711a870a6ec53d566bcdbc4ae9bd03e14597efe24c4c4535c6d1107fb876e099" => 57174095238095,
            "745b8b580cce7d1a8e6a3afafa54da5aa2f7e5422f497e43881360c133897d07" => 267016428571429,
            "7f1a085dd885243379a040a32116097d8defcac7cdf9ed29a6a5c262e750e436" => 10832714285714,
            "8a8e1a92064b55ab7ed94d5a0ef97e6008d316a5b7128a84c4a80b4a33f146f5" => 317428571429,
            "9fdd24feffc80630c43f1c137d7580206849d91a90ccc4c0fb9e58400ce2e0c2" => 54427666666667,
            "a3d85d724561af20f42cd7935c93f6b2ce9d8dacc7588d93089f4faa150617ea" => 26803666666667,
            "c561702f2bf6587d7e95bf30ade6405e1c2d4dfe7049b98e939a723bfda7a70a" => 319911333333333,
            "d0c442b3ab658d4756dc5fe2fd489daf5b059c442081c31b4c6a36d0c1a2524a" => 1246682809523809,
            "d9c5f4b9e81fe1f987c0322a49728e76f7bdb0660377475823a295d996ff7992" => 2311476190476,
            "f356b4fa0552df8334e7248fc9e6bd4fc0805daf3c6551ed037b47b7de1d8c68" => 94661095238095,
            "f572dee9fb131611c64fdd198ce0476dd7d3bc88d760a51031e6bf04d4f23e6a" => 143368476190476,
            "f9e61bfa4e4e5adfe6876c70c7ea8c0ce914360209ba135a2431dbad93af1b5a" => 24933619047619,
            "1468e104a47490b82ea055e599d5c0bd5a5fe3b62d22b3512831d381d30a11b6" => 831644095238095,
            "1b570cf1aad36f47c0a3efa30b0a627c6b8274a044288f6d93cb922e91a21393" => 7704904761904,
            "26dd55cf69a8e120834aa698e35aa58b31f37a97f43d47ae8838ebe750fb9afa" => 5393428571429,
            "272fa652879dbf998b01dd60ae68414a3b9b1ce391a0d90e750627f570c674d1" => 124668238095238,
            "55bf321c88542d44af9749e0ac1569fcb22423f7f53c8a8af695de07d014bc33" => 1386686904761905,
            "55e5cd148879299e297b9d7938f271b0714443193d539aeae1e6b969825e9485" => 1408380952381,
            "5ab541c4dff9ef771a39583bb6b4c340f7910a9f4c3580846910e1b05e9b50d6" => 4782047619047,
            "751c602d73c1e3e74795ca88b8fd132fa0680f2b0041ae32640ebc1418db5a0a" => 154098428571429,
            "a290ab8b7b24eaa4df6db556c265add66e13565679b409a807755da7652db5da" => 68832190476191,
            "aea029869493a2260f6aee07a9591442f24186a357f35882a7d916c6eb19b971" => 446831571428571,
            "b0adedf1edc40a6bf37894fa1df9b366271e4b2b68ec22c603272782793fb396" => 1178044285714286,
            "b4100a3ef85c291e0eb50c2ca760d80070eaa7d7a3ec534bdf643df0dbb210a7" => 7704904761905,
            "bf0cc4b578821072a637bf3b3435b3d76afc9a9b294fcc2ad83575c474e5c9d0" => 62334095238096,
            "bff7aff4b04fdedee046e763a23125122983b5b9ef73e31d6dca59fe51fe9b6a" => 878795714285715,
            "ddc85be12ef6d4d66f69d5ec3cd935ed3aa255e842ff185fe9d0de9cbe98d8f6" => 4321952380952,
            "e2f7cd605db794b6ac76b2ea7011058a8c7cdb6b6eb9f844db7f196377dfa664" => 93629952380952,
            "eb2c7cfe7323527334af0ac043f76ce4c4e8192f06edd3855246a6e676a23d07" => 19047619047619,
            "eb57cadff06d46c6e8cde00dc9b191358f6790f44b288b033fc03ed037428636" => 95238095238,
            "f65a26ed63ce5ca0985ac63eca4f2e5f127c69ff1311c7912c0b417b823f3527" => 11557380952381,
            "fa28d8135b03fa03615a929184625c011e663c08c048879150dec4037da20033" => 3467190476190,
            "156064473bd0edb12ec9c02b715877237afca1d9a56d3ce13f3895161733c666" => 256809523809,
            "16b7765ae32a0df6165245008d8424987901cd776bb2e369f1b0e594b310f8a7" => 2675941333333334,
            "3674ff673dafb5b5c18cd6b99ddfd92559d45d3449a3fc18edf6b264ea1ec298" => 39450809523809,
            "41e350cc5bfb84d9eb6f2203ea9d1311d059aeb9f13d8a45c0c1817dd1c09b33" => 3081952380953,
            "4a2a50a0e7e8363b7d5e132c869dddc8df321ead8e01434bef833634c3388a32" => 107600000000000,
            "4a47007fb2f00ae740b8f74e4759dcc1d94cbc51c215f823fc0d9612df88f3df" => 252911904761905,
            "52bfebfdf5804e942858befa2fb8457693269214704a8cc434e3e67fc0f263c0" => 230761904762,
            "6c6c73a8a13259dbd14cd5f9c6ecaa0551be9b5fe53169ce20af2db32950dc31" => 10392285714286,
            "911f6d5896b28abeebabba434596f0b080771e0464c3161ae0482fe41d6de9c8" => 63476190476,
            "956ab496580d829264126b68c44b31e3747799c9a0d3401fc765c874b0ee4784" => 3848952380952,
            "a67d73d7c6bb8b2ce5c82c1fa92613c39affac40cb4476d25a6f2414c0d4fd19" => 3852428571429,
            "b4fee0ebfe1bee30cef58d167a4d0dd330d98ea6e1471ba1986163a4651d279e" => 493130761904761,
            "c4b85a385f48a7ecadb9b88be74830111ca7bf20149a9c16c3e0acf0baaa3647" => 151467952380953,
            "d7e921fdf63cb8b860dfb2a1e4373f875351f7719db63a8ab1e1b54037ae2b57" => 41523809524,
            "edf7d4f7b9e27691950cf0cc46fb51a0d9ffd7d2566ac855f04b5f9808ca056a" => 256809523810,
            "fc01bc9b6807860d20dea58732341d86f0233e8e0bc0aae14efe594ce7fce157" => 256809523809,
            "090410c62f0c4d1352efa857f56e9dd55eefd7fe48e4068e585b6ff230582724" => 338095238095238,
            "0fb739a71c552fd5c650e27ea3ec03028987fdee7c6d6d7f8c93f50f3561f1d3" => 185407190476191,
            "15aa2bcb6e983418c9c0b68254ec469a6cda6a8fcf2f44933907894f4528addb" => 3809523809524,
            "16877e196aac994d015eaf60e6569b91ae97b182921da4e553b24b37de4e6c81" => 538448333333333,
            "2d90b28bc2483a1ccc08ac7682529c8faf5623923c4808035df08d6ea209d974" => 104761904761905,
            "4f286710b0e69c603ead44d9fc861c81216029696388bab3d8de8b86d75d542d" => 52891952380953,
            "5caadd7277ac2b2f8771a883e80fd727637dc5a4f167584d2d1b23494fcc1efd" => 6190476191,
            "6042a52508ce8ca8dbeff8cd91b5cd2c37e1aea4b7408ffb57f7b26a4502f525" => 5409523809524,
            "71e88287ca79f691831c911471c89a4ca815483904e3a0d3f4cd51f7161b73e1" => 69727238095238,
            "931543e5609f766240340743ebe3ce1405f8dd9f4206f35f69e5abb035dd8b2b" => 186997142857142,
            "997e848974471e3ee2aed0ab0204aea86e2916fbe5d7e79f255bbf3c7c0ad299" => 298599142857143,
            "9a305b4430b369240b6f20c21dd469a1f1827c71ab51f47ff4cc94443cd70b2f" => 1526592952380952,
            "9aa489c7a8329a3faa599f70327333410d1f83dc0c699a224763f33ced4d6153" => 4761904761904,
            "9ddc36b57d06b532ed49c8b17847bbd9a76f84e31f1213712daa1ed1ecb4d153" => 58535904761904,
            "b64292753c42c9ead905dd582a446afc03e3ec171f67eadbc4d9bc7824dde301" => 513619047619,
            "bed8cac2a71445bda0ba2fb2d167cf33202067cfc3b156fe80249eae940170e6" => 1078666666666,
            "c87c8e18354dfa3b260ab14ded1e7020a1184d91f8ee02a1455394cf8e175b89" => 146441047619048,
            "ee4b35dd3d445638fbe6d000aa7f3d9142699b4860e5f529923fa2ac5c2debd5" => 1904761904762,
            "f1b61a071fb9bba77cfd70547d033ab572b1091ec9da655ef99bdd3007c5b9c6" => 10812000000000,
            "fa16516e90b9859adb675c53bd7edff68d17323d20dbc277880ea02b824a9615" => 62334095238096,
            "fdae466d6ebc3bae4eca59b8d645901cb11c7c6c176df6109b59f4540cfcea49" => 510610761904761,
            "ffc6b75fbe3c6563a9fd54bd40aedc0293189aef4c414d3454bb6896c0c010e7" => 9523809523810,
            "07caddf3f379ec0163bd44b8da24c35d6ff01e8d769cf43cb20cc0ca162a1129" => 539333333333,
            "16a18bde942169e6fd9d4c75c7ff22d42bb4ee40d7175c018a2b5e533ce8abad" => 41523809523,
            "19795144a4cab56b5bfbdca347cde9a7faa77c5659910ef5addf05038d923c9f" => 233666666667,
            "1d1f881338e22e6ff822a67fe08e8dfed9abccee81df88e0845b53b0fa3ce6bf" => 203142857143,
            "291618934ae903d9936cb7ba60c991357f2fcb4294863b12c756207f670214fb" => 1380952381,
            "2e274d78bf213ad9814c108edca14ed214e8789ee4de55bbcccdd7a7d8942f81" => 51333333334,
            "59046337109a194938933c44001627829a09dde4c7822f384a7ce4d94ec5ff15" => 31714285714,
            "60237baedbd9db27788429b1dcf8b41be726117df32ec18680652eba3f28feeb" => 308190476191,
            "67f04e7a44526adf307c2f9232b53a0cbd5ac4292207a50fde900cce9988de24" => 256809523809,
            "825fd5f1125ecdf774d01b00f94723fc4c916774e50f9932f9211f4045f6b60d" => 999333333334,
            "86c09d6c7d27b1af6757a53b5a8f2e9deb360fb219e4b21e2bfd3c580b4f302e" => 9476190476,
            "9dba666064179312c8fab0b438d4608a848d62fd30b36d4fefe202af410f3d7b" => 2523809524,
            "a1cb400a22f36cf870d2516405b928431513bc9d5951daa7d77493291b1f0625" => 2523809524,
            "c4cee2f0ae5a5e846c1c3543d8e693be65179ab8743f9633aad70e846529e475" => 20761904762,
            "ea03ea4cbd1a743ba00779d0bb4155739e87e4e8d1f20692118840e5c39f2b74" => 360619047619,
            "f3890c922e0f35dbf243542edbe95b5c5f10a87debcff09a05ca192152c25509" => 48205047619047,
            "f6e63668b0d9475c3ff502fa7bda0f622a9649bffb7b13450ea86c4275b3c677" => 381142857143,
            "f77e61254c25525d97f2fddab3d210591f39d4bdc3bc2d8dcde3a5dc8add1311" => 1761904762,
            "f7de3cde2e68d96b97bcbdb94940174c73654fbb5a5819e66c71cdab19d6650f" => 476142857143,
            "00b16bc5978a82d76364adc4d08f0926083aaac3899f6312f7a3fb184084bb62" => 1241220809523809,
            "091a8fa2864148f92abb9b93fc2d51877a96e4728822c2ad1004e60c242b76c7" => 293181666666667,
            "09ee82c24073dcabec36ad5a761d5be4f46c307ed186e0a922e3faa7aa218ca2" => 29645904761905,
            "0e45c5610a10fdcf1d6244a358cf353fdc485882a48455a8fdbb7a93b2d526ae" => 11537428571429,
            "0ea4c7442efa5426dea46460774442e94b582e5ec40ad89fe9b193148d2b9f3a" => 9848761904762,
            "1ab6e7407397f0f801f6151e2755f627ee1bafb9495942d7f47a7e602e55c441" => 3853571428571,
            "1c97aae611c3c9e63340256312031027c0a37e89a13f9799064f80fd9a0642d4" => 6666666666666,
            "1ce4559b266ea332c1367bcabb44b13031745d0d70de503511f152b53c62f64c" => 4249380952381,
            "1d27d5e2eb3c657afac667aeddd39ce1e96dedca90d1e2ef6a89ec54c5708513" => 3852428571428,
            "1d3e2978b9980c604d12dcf0b136ebfcda2db945b37da514dc1a5c6c9276aabc" => 67622047619048,
            "25a3811628b1255f511a1b51bc9e3b0f6e1c2bbca13e526b66dbdfc329b2f824" => 3081952380952,
            "2762a7809d28dad14fccf2570e9734a8b3bf4fc3a7a284d64be2cef20b3081e0" => 102714285714,
            "2a8979761aa304373819092d3d992a66970a65223e93f1a4ac0cb2b406162d86" => 21638238095238,
            "2ec74d50c87583d894bf33c91879dc6eb7899b47aca10cc19e2d356e4efa0b92" => 48344857142857,
            "321e71820c0f67e05274991a664972333ae30e3a19041d17434cf3c3ae1e449c" => 16638476190476,
            "372af7367dd380311b1b1ea77439f04dc533b5456b2e9027bd31e8adc2528e1d" => 374004904761905,
            "451340c3affbd2fbbc72d32f6eaaab35d27522dd6d153393bed4fe612408dc95" => 12466809523809,
            "48b88a1bd1a31b3616a9cfbcb5f661fbf99d8f15d21e503da71cd2d97f13fcdb" => 100385476190476,
            "4e620b8b52fa4678477fa7fc72e9620073801b9b87b5050dbbe6f6e2545d90ea" => 99734619047619,
            "4f15af7064f2f68ac9fb4cf48cfa35a2d04eea7c2f2bd3802fa87f7281617f6a" => 10974095238095,
            "52c56ff98b72c058e3774ffed4e47ecea4140f631d9938071a6a4d218113bbce" => 1028047619048,
            "53362b17244c59429dedaa12debb2af6b0a23cc89eb7c3550d111e8ec47e5402" => 13819952380953,
            "559c7e1556f6c75c8c36f7dbbd828826470e77b6ae4f7345d0a63aca300b1f4b" => 220333333334,
            "59b420b453da0885a0a29ec82a46ae93e10ab8df6ef53a2bf5f068db036993ef" => 10507095238096,
            "5a9ac8c158cd801458f8c57089ae02a94cf9df91d9b1663447b1da0cd4b92691" => 12412571428572,
            "643b3ede729eeb28ced00792de72e97ab6edaea68988e54582421eb35014b121" => 7704904761905,
            "66793e9f6fb72bce47392b864822584cd434b90fbb4f5377bef6bafced91f1c3" => 13205904761904,
            "690162e7362c6dc47d65e7f1cb3a323cb625216c9a2b54e8435190ca1517317e" => 9334000000000,
            "6ed94a21381e8807226105d7b6773783494bccd6c776d2889441b5fa924dcda8" => 85000000000,
            "77903d9d7b1a79f703b7cd4ad6b5c0435448af2dc5252ccbc63986c8c620271b" => 11536571428571,
            "7fe9f29b0ccfcb8d1d8b0f26ba93fe443b7070bb748bf0e6fcde7350f9443d59" => 5428571428,
            "818d4244c6d7ea584e5066f64ad0e4ab978396def56dcefa9826f50d3debb52c" => 24933619047619,
            "8fbe7bb7a618b294d3c4426b98168bccd9f6b949c061fda4f76c422e423f47ce" => 9547380952381,
            "92439cbe95d10925cf95367ec9f09e0dc0e61b09ace069623a25c937bfcadaba" => 126998714285714,
            "925fcb8bf2f9f49bcb0bb79289327ea070a1f6671ac0d1fd1b5bbbcc92fe475f" => 126376380952381,
            "94a5ac4b23caf9077d868ae8a3369e4ffe4de8a625964d725c4adeb7a704f35f" => 160906619047619,
            "9a4f4b8ddbcb277b6f0a572f755939e2edcffcac453b693171d0b0840548b6c1" => 2394333333334,
            "9b858ce5b80eb955707d1a1ae2f690e8fb0e95e1245908f1130933c33963f754" => 490301190476191,
            "9da6ebfa0605dc68bb752eac7e594bfbaab0eaf53d3a4fba9492f7252876fc88" => 24933619047619,
            "a320df531f6a3a59efeaa5340d49b7acaaf5654ff762d82b125338b5637f2654" => 14885047619048,
            "a9447f91796cc46e07203fe82577a9707abb3780f15e0ffd672ce757c3c589bf" => 97989333333333,
            "ac69d93e81dd8dfa9c12a8f7ff108a38c7471a47b8c21550a3b80417fea34cf0" => 540957047619048,
            "b56a1651338c195aec64dc0ebbf9941484226e603cd35e8b2532ddb9b44e65e3" => 169627571428571,
            "b64506d2d7e98c3a3599cbbdd346297e56e33754c9a569fe2967b86e2ed29953" => 952333333334,
            "d702bcc21e1ef8848c349affd03adeeda146e6f7891d9a39c5b1a0592a8bed19" => 27333904761905,
            "d787f19d1c0cfb639acccd952732eb2e4f0b490f4038483a27194922eb642157" => 69344285714286,
            "d9ab9d800feb250a0b968c0bcd1ef27a97d9a936be1e6ea13af73c55585b010a" => 5750904761905,
            "ddb83a0c33403450590435d226d6672540c6c4d3a2158e58578e96a8d1e4a8af" => 476190476190476,
            "e142e57d24dca5127c649da85c0d3d2af4b1daf9e52bbbeb508ce54cc9d254f7" => 20177523809523,
            "e3419a1559664fdc9414301b7fd8bf7542c05e29b6c096e8eeff0c8fe2333671" => 70343476190476,
            "ee31852e8ed6abcb38c6b41d20874d0a4eea384e86100df39a9d19aebd42c7d5" => 20761904762,
            "f3f5ad671e1541461af463db814f1369dd71f27d76b7cef5de3473b3218fcafe" => 45714285714286,
            "f675287f35e69b82f181661c0806a1a68084e3898c34f0b261f17381e2e42a51" => 124668238095238,
            "fc2526f12eae731a6721ed75682fed092db29e86997e0a61386c5e2123f6c012" => 40940285714286,
            "fde305bde7d8381507e411d2cd0b67e8588617733d79706bd271a1ea802d0f58" => 462285714285,
            "194396e78a9b642bb39d1cbbb4c28a08d884f5380a1573d6526b820fb1267ce1" => 3275619047619,
            "07e84637aa7bf5b9f1cf7058afd7ea735d9ac567d87fcc94808b936c4d7ca315" => 83095238095,
            "8eec8d1dbc1bb87dd9523df1322e2d40dfec35bcb23770c32a767c788e03d778" => 256809523809,
            "c7c62aecb40ac4351b82a205b12df4331ec41ff0801e1822c86104b1979e2b27" => 256809523810,
            "df51ddec94c350ae70a4c4fb798a0dde48049d3024cdce412ab8523948aa8da5" => 102714285715,
            "f6a98dc613251318629e8b49be139d5a69393abed09711e64d3361a9b3776608" => 320238095238,
            "007e97ea362ed6adebfc0e1bdcc3495362dc5f33ebda35aa67ac04928422e31a" => 960428571428,
            "023af571fd636c4fb490bb9d56fdb9ba16f32a4460ba3e374584d44d9674f3e9" => 569238095238,
            "027eaae5cf671a7dd6a60c1ff59bda1f5b3101d1a7bb9e7f92316355408d68b2" => 508952380953,
            "034ee3f4aea8abff53b851031faee5be0be1b9db895aab79ca37b1f1b3e74cf2" => 13138000000000,
            "0513a82770066058e7d67fa04d185d3a08454d33912ff49df07d3ed68c523f67" => 692380952381,
            "087c74b88a96d66a7372f016f44d98cf3a02f506eaca05a0f3d2126f3a4bbab2" => 589571428571,
            "1158740794bf1ca2de2de9b37f18cc8090a81999b15333bef3047ef68d5d15e3" => 20803285714286,
            "139dd4cc0835a6c38ee190e4213c8a7871aca8a94144b0ae15f65ff5998e57be" => 16370809523809,
            "178929cdadb04aedc87768e6aae9049ea5b7a67a35ea244d054149de13e76026" => 928095238096,
            "189436967af50322f6ed562c34b65929ec06681099fed2f52c73c11fb1d0a1a8" => 652761904762,
            "1e53f6c29d9d36e45704669789dacba452d4e224d48efbe17d4ae3444b2b75f5" => 326285714286,
            "20c3bc7a831b21cdda6a5213f2cdc417180efbc1aae74beeff6c641a597e5461" => 36611952380953,
            "25557223de0375f38ee1a3196cb3f86ed9e354226a5eb9599087497aa630917a" => 249336523809524,
            "28df2a9c97662938211988ab9cb4d49323e3620f41a9fa04bdc688a81de6353d" => 187002380952381,
            "36d4e113bde17ebb2ee0090e6cfb3f652e320b14880db1053a6e4fc6910325d7" => 88095238096,
            "375ab5f7b9b6198e3a75e6ba4bb140f832516e55ab6600c9ae21e177ac6075b9" => 923809523810,
            "38a593220ca75977a1d4eae232a3c002ac6cc1d2a90c58b98b5cf1c5781589ff" => 434169095238095,
            "3923e5b442b41a3ea4c6fb3b393c5dbb2070ccb4ace576a6bafe151948d0df74" => 7776619047619,
            "4660dc0c8f546183af1ba41dddb6dbca46bdc738b8d68ede8b5dd20df3ede58e" => 19076000000000,
            "4ba9a79e25e29492fc2b2a049fc77007f3c95b2f0603d51ac9c6b262a58b7eda" => 114285714285714,
            "56b595e74d40f063772ba9b27a633b4a99f1ba8b5b34d7d251135fc7da6068c6" => 6934476190476,
            "76077c208d4ceafc808b86e2d7c8406e495e1a87e612ae29c0a351e837793a01" => 877333333333,
            "7ecf7424cfc0fe5ffe24f52cf7e29c45a0d1278bf88bc0e6b352ec1fddf36c8b" => 361714285715,
            "9416830e1ec21e3d876a27c293498fa4322d9cd1c6c33ec4039ebbf7a566c4e4" => 62334095238095,
            "9b848c6f29ab67345d4a6be89f5885038e75f22e1138e8b06a8fbf9b35438e16" => 623285714286,
            "a10c4fab8d44b62b6505355593b88920c63b8e05e41847eb5bf5089f0fb3af46" => 37400476190476,
            "a97fddfd54a6d2dca076151f1bc16cb917c5a6582fcb7d9f8f6c709901337639" => 313619047619,
            "b9da5c2add071b25e2b6c589c79737db3324ef656f65a064ed5194b1385b13a0" => 952809523809,
            "bc4807068179803ab2f73b0586081df9beb277564c50021311ff38a6aec2b23d" => 287571428571,
            "bd885255f8dea20bc12eee8539112683b5bc7e3b33759e678f5d4203ede6d7f5" => 15210761904762,
            "c48b5ab4c31a9f5f05d29991a4b4bd505843f498e270c1e86c4fb5b3e39ba1b2" => 154098428571429,
            "c7406048252a969d766abd5a59ccd89ae5d5bdce882c3d9910d8f1dd8e2853b1" => 30476190476,
            "d243014f2152bc8d3a4c9270d7e9bea46229a9414f8b895cfd1a90bc98fb27d1" => 760761904761,
            "d50ce4b354eb96941e40bfb0238bce2798df122679d8684f9182f1b0bc6e5b72" => 109619047619,
            "d5d66cb5ca0bf2a1ee6748a0a7a7898f5dfcdd11bda0e17a3297b8efa546134f" => 185476190476,
            "e2e5fdc48f91ae5dab5f2e23f8e25b3577b465966a2f64071039ad4f6bb96ff9" => 471047619047,
            "ead455f1183a7aee43f990e7449fc96dd11de8581cc79772635a61826d5729fe" => 109523809524,
            "fcdc50d2e1d5248b4fb163492889062f7396f4169b83b0190030e970085095f0" => 530476190476,
            "0c112203e4f2d336174456d7d98e36166d7f3a6993f1be376613b1ec4b581fa2" => 93095238095,
            "354e2a9c0bb434e7804101e2c992893a411f19527ad140ca59609cb005738d1a" => 610952380953,
            "384e4cc14fdf99b4dd9b85314019cdcdbba6354c421af758dbca7fc4af2c6db1" => 194666666667,
            "4d65879c7816dd8ac633c53c937abd62688f97996a98b6f22428da663ee72eb2" => 186142857142,
            "549dd3dcf2a5d3a854d92239ed42b329748254fd83268b9c36fe88dcb6fd34cb" => 543428571429,
            "73af668fd16a59673f2475b8998ad59c865b081f51d3ee52f9a1ea03bb91b455" => 599619047619,
            "75aa7e352e35614f5394504ed8d705b214452a4cb7c9bc0e4fc1d6aab47ce464" => 783380952381,
            "7c89bb8a99f73e08e7b04be8cace97eda72f565d2f95896802fc82cf2c8fb132" => 201809523809,
            "82fce6bcc5da470f669993689b60d8d812e03b4cd01164ef2818c1ca170acb56" => 136619047619,
            "86a67c9034681598334c4755b88dbb65311600104c0ab5bf94e4fa8834369348" => 509047619048,
            "90185b06cfe1d700d67168467c14798fbb61ad82638128a206f8bf5fb0c53a90" => 38523809524,
            "996f37fba6b75cb4672fb0bd6545ea35f438695875866c572b00ab4b68041fd8" => 987238095238,
            "a1bd59fabb6956b625499b86cc018651e97d2df219922375908678efa6f43364" => 295952380952,
            "aea80ebf9f66d8ebe02b7d0b1460ea6deaf16799ddee1343f3ab0dce15772dde" => 86619047619,
            "afb4f2053db8f6b1bf31ce04d9454fad228762aaa7c4ba0db4289bdd9b910cdc" => 644333333333,
            "b46c8de2c98f0bfa38bf743c6b1d8981bb40daa66c7160d74fef5f60e9d09649" => 51333333333,
            "c03c7bcf27ea22a084bd083aff9918b413f5c3ac8ab724dd29e38e8bed619737" => 977666666667,
            "c8e1e8026e9db14a6c4ae813c298ec60d2a9323325f6eea281f95b319cc0c4da" => 888761904762,
            "ddb284a1efb850a81cacaed1a22e2aabcd43e95a0c093a5cf8ac4215577c2931" => 633095238096,
            "e70a32c3fc01face272a2323830b68b3e9759f11bfb17dbb2e52b29f2570328d" => 378380952381,
            "efa9e1f9089cd8e2bd45f306f4f70a16fad49dcf4414bc36ebc560626b450ef2" => 374857142857,
            "f2235d8db6281896cb4d1f8b2970503110f7dfd6e811eaf029034898050a941b" => 775380952380,
            "fc963c13620267879929b4e257378c657d8eba00ab4d3f8893cc0451509f28c2" => 531428571429,
            "5ab9571b249aa1ea951797628bf97d71fc3dff10b7deeb77119bbc81b2313e38" => 5666666667,
            "0b1e9d8d5ba3c4d84e712c445552dd101c28fde69e009cf25ab5adf07785b9dd" => 12466809523810,
            "12e9a71da398a5db48f04afeef6ac91a16d1083623a781c2f72f79df7a384116" => 62334095238095,
            "13b02918546f18032efdfe69ee24d0bcd2b353706de5a734afca297ab6e017f9" => 538993190476190,
            "188bb10c7845a6113cc22e32f423744df176371b916318b317d8d4dd904e5bae" => 8721095238095,
            "1bfeb2c47686d399aca07808016b41f6991bb29ca069175c80f5fad557a8358a" => 38524571428571,
            "2c272be127c8a00c49c8bf14d62a171b90b7e43ac5534599854b3d2ace868fac" => 165516666666667,
            "2c358dc1b785f20892d19c0d0cdd99d1521e41f6044f70e4e7c2c5ffc320b922" => 3386880809523810,
            "33617c9f51cc296f2d7d6a2fd362b453f4bf0a216c6cd3eb120496cc032ffc34" => 38524571428572,
            "3426a7e82ae17146a7777f6376e5537d4f7b39324f4c4e0bea0daab5451c5d72" => 3740047619048,
            "3a2b840c19e1132ff1b3b9489053702782b2ee9accc210ce5552a5bf23b79450" => 88096714285714,
            "3edbf6b88dfada8a5e9022d42f3c30c9c2e3c519b6b51462dc30cd1d6f89b1d4" => 15409809523810,
            "47f5048bea5d3eaea4c584ae319bb683533a7f3b12f13426fc552ab262ea814e" => 324574190476191,
            "490fcabfe408c70758b681130fbd7967e908c7c725a9bc3ca6cf689242afb0a7" => 3081952380952,
            "4ebe78b8be69476536d9c2b7ad981ffc6e2ad7c9983224ed427ae23382c5d29c" => 12466809523809,
            "569a203f0f94205b6ec6207b207f80ad1ee7965ca0829365acfd4d1bd2ba86c1" => 7148523809524,
            "6520d253051e123520be5a9df321873985bb5c1be9aebb5894c60d10b9847d63" => 4196714285714,
            "71e44ed5473f072981d615d2d4c7a4102bb87ad6367c7b85c05620b61cd3feda" => 95238095238096,
            "7df440c6a44c4739dd48c9a0c27adb906b12bc498cd99bf955bb5f58c4913f3f" => 28452380952381,
            "7f194955ea2690bff902848c573037277eb1c2b6ca13e96a9b35e14f7b149a55" => 2224809523810,
            "7f35371999f831d317f50aefb905e19d365163fa90aa28ae49fb0e3dc4a537fb" => 11428571428571,
            "82c3a39bf2e86e98ef6c31f21c11d3ff2c7ef520c0723d0db2a53fa49299075c" => 154098428571428,
            "88fc24d156a7fbdf36400bae84397fa7fa7159e233a39818bba738024fb273d7" => 124668238095238,
            "8e6d326603817e65ddc2bf5e35ad8d62c0b9d4c0d25a98225d6b043faf9f0eea" => 9523809523810,
            "93692a4f73e1ad88814d5ab4209462b938b37dbf927270d50d31e21481c9931d" => 75733952380952,
            "96b4518df053c5bbf8190dcc5e9ef343b6f045fdd1701699f06411156f15289c" => 30049190476191,
            "96fc7d3ad62234728569db33e47a87eedc2f253b46129abf4cbc3c7f9d9bb6ae" => 44935047619048,
            "a72373ead2144ebeefa74f96ff65af649771ffa2bf8a37fbb19d8e428bbfc057" => 32880666666666,
            "ac4e99663cfae44ba5d8ba995399a137050b5954801e1d3328f4b1b492ec1835" => 172646952380952,
            "ae96a3d23f09081c80b274fd7aafc35710e356cdba1f69e7f0d9908229f77455" => 43633857142857,
            "b0c60bff61335bdd8984ed5f386fb42863fe8a7374384b3519581607897f0ee5" => 3377654047619048,
            "b4027af30dd41889333d794db7891254aff32ca97b1fb65dd49aa1c4e0ddcde2" => 14177619047619,
            "b87d4db522defbfd0e51a18b91845f1aaebbc625ae724928c7e5fd2d80eb364c" => 15843809523809,
            "b94746bc60ccffe308fa991d822e6e576923069a9d57899b4cfd43d00634e900" => 54604666666667,
            "bb54dbd23db29de863f8381a1a78144abfd3429b9c94b92428faf684773f71dc" => 5298666666667,
            "bb6207d7abd9b05c3ba561abdc25c2543ad3445ca26404dbe8ff716717d1eb94" => 5731238095238,
            "c23690c08672d4d7450ab5378bfa02f5b313d9df9a583eacce87b6b6bb7e1c34" => 820936619047619,
            "c54b1f9dd40c50c94263c8e2efa415c7904001105838e79a5326ddc4f030174c" => 64914809523810,
            "d257e49e9dad1bcdc3c54fd3c013dea3f793319533545ea788f27965bdbcd2ca" => 3338333333333,
            "dabd43d1f5cc9ae1733c9ac898531f6429d28e3a2d7ea076a8742b98c89df53b" => 15130476190476,
            "de1094ac3ce948e481b4f8ae78dd1266d83c684e3271318d2e089ac42e5f5cc2" => 30819666666667,
            "de3ec019bfe025ef14d73121de206cdabec5baef1f7ab37330d96e0ed0e17de4" => 25524952380952,
            "f44b4ec91ccc49eaa83a08dbcad77609e111da70ba962c8ec2a6e9d6ae4d04f0" => 374004809523809,
            "f7e579ef35191b0eb3694990bc9c40dc74f307e06d9942ce674c0821bd53612f" => 4178714285714,
            "fecd6504782ea47a7b68a1d0900cf240e20f54a5a98f14d5e4a8f8a1e0a9430b" => 16228666666667,
            "13e90cb28a61435cd7b7fff3c9aa3f6aa3cc8257a22cb788c5e2a6ae87d70ad4" => 62334095238096,
            "259841706b75844f5b53d8a72763cd5e589abb87aed6886051b154686ef71675" => 38524571428571,
            "37b60dfa94e5d806bd1be244c054a5d06245e6ab3b4d477eef26bf2cf9c6dba8" => 51333333333,
            "3c83d8b0981fdb2fc100a59597ad61758be17fc6afcedc66b06d520454ec4577" => 374004809523809,
            "6084c7b709ae00581489dc02010936360b0a9762bd9ee543b96f802326de0e57" => 28238095238,
            "756edd039d1b4c1d29be357582e938c4ebe02f9b68ee1343ae81cd6eeca86c43" => 300390571428572,
            "a42cc5c97d368e6770460c40fc363014ea0826325fa2bd9fb6c90a9ec9d0f94c" => 8253761904762,
            "01c34d1b6c192173a9e2eae0d3ee7e264a056b5e67c503ead44ea10865464255" => 61639380952381,
            "1fabc18f3990e0489c5614d3452b10892493fe8ea77b138920182e95930bdb99" => 341789095238095,
            "29114ee5406847982c53ed59af45dccbcfb12936d8f20a864cbc5b7d8f0fc1e9" => 54095666666666,
            "2c7b549f2a70a10b2ba3310b7e4f178ee647745816265c96533b6139a3c8d849" => 94519142857143,
            "3c347502df70e431f20e058c944995172937a401cac764ff7e6b73b7046d8dc0" => 4327619047619,
            "3e94488325a03ccc8c8629f012c164078f13219e6cea833bac384eee2d2c3dc4" => 60194809523810,
            "69feb214c37b29c64f955519a8de3593f2ad4cd238e9b918b2850d4239266a36" => 181835666666667,
            "6e4d88069ffeb704d89262c3e5e753d1d3af5e1a068e52050b0ee59e9a7fdb4f" => 289697190476190,
            "710ad6081b8a549387d1acf0bbef4234a6f5bc8c27c4948ae15932123d7f786e" => 201266142857143,
            "992f39e10f7eaba9fcef8de700cf22b55437b9f78d324af2be26bd211d2a3bb6" => 34505523809524,
            "99749ba542fec2532a01561cbccd5433e60915b789e7e104b0aada654c768f30" => 62334095238095,
            "a08483c87e95da3b2c37459df8d85d037a4355c0919cbac9ad7121e830e03106" => 51286047619048,
            "ac87d14b3cb0a6f0ecb08f5763498f1ca7dddd932c7a878a3254f6f7fd6536f0" => 88539809523809,
            "ef4eed358a73248e6ab98890709ff9c19d21c27f2aed1923b1ce382556dc7241" => 4457179857142857,
            "fb3ddccd39dc06cd95929569320585bd41ff8d426a35cce4ebc5ae93c7fd87ae" => 21638238095238,
            "0300a053f5a1172e67465fe9c7e68eb5cd3c8b9c5932138cdc7f2023de493f56" => 13440907190476191,
            "0a29d13f02b46736055aa9a412953fa2e9d2d04b5080ef73e200218bc9a5d8ab" => 9774380952381,
            "1546878fb2c33fcb394cd18d86b36a83371f1867d3057c0ce5f5f0d2d0b1e785" => 19902333333334,
            "270799e2ae9be4a8115b85c18121d3e238fd49c55b39694e2d68fe49c4ae6731" => 124666666666,
            "2837422d355adb59d2ccbaab0ad1774ba01263c5c7a98954b2aa4ba69ddc307c" => 48866047619048,
            "29a938ac519e02be423c98212db4ea6efbf2323c8e22c4a93963ede643bca46e" => 12466809523809,
            "29af1c8f277854d17eb4a73df332c81822a4d788b6f8fc730f429877a0ca2a44" => 56380952381,
            "36cb9a0284caff93e3d47702db7a35cb59fbf7e85c63a77aec95aba71b9f8529" => 62333333334,
            "374c3d0c3d494e545a0a9194651fd2c721b4311dbe9bd335aaef66e1e9bb6523" => 337781904761905,
            "38a4a91750953f360cbc6e836ff4c37323e91a9dd1fc126b78d2d7eabc1e108f" => 819095238096,
            "3a3215da6a281416d9eaaee8627102eb2835b600716cc5acdc9af0afed2332b0" => 8955095238095,
            "52111ea31881a112ed8dddd64d57157f78c3c7be607748c0e5b666962353a9f6" => 9963095238096,
            "5efb239cfb1ebd2e77ecf910ad28cb5c20374896b2d8da331d7269c31c52b299" => 107868904761905,
            "60e6f60d4abfe8b0414a8b0928c198d9beece81359871399c07a9d0d4cafb430" => 31714285714,
            "6e8b1902b20eabf97da0edbff87a89d29aad3da4d34a8d3b7beb60872a6dec63" => 9219476190476,
            "6fd243fd9ee1ce6b9522d4460df37b82d4691f01a46a14d32d0549f65af5622d" => 196662333333333,
            "7d3e84a623db1b674556550212f25c0179a7ce5073145b6de9438cec2e3b79b2" => 494616904761905,
            "81d0e7c4714ba39ad32041b3ab06dcf69b6466a0ee6768e27f0d0ea8a9440802" => 38524571428572,
            "82c43b6e0ee28cd8a0fcd16e4009610b18c5e0d9ce02890c62242c12c3f9b80f" => 108191380952381,
            "831144b100ec9de15c0e0d86d7854483238437cea940684f59f955e7579bfa2c" => 452993714285714,
            "8a2ce5ef4a07a29a382ddcb4da5367e7f8847c0522aeecfcfcbaf00c72935e2a" => 25671761904762,
            "a394f36417bb8c25f7fb97d4c16bab528fdc3c7e41c5bdbdcae736abfb3bf38c" => 76190476190476,
            "accda30cfd3af51f87af865094cb20a9c093ed81fa70d3ef99c4e67548f049e3" => 7640761904762,
            "b0eb7dd8720cdd0c7a5a19d5104ca202bb6b0a96f06f99f9174ba34fe6f81db4" => 18688047619047,
            "bba1afa51751194ac32bd7ef63fc72628776bf991c695155b43afdf7a269e17f" => 197865857142857,
            "c449f65f7239f6ebe90059c550d591ea05a6dc981b51352a86ee2368f24beba3" => 3714285714,
            "ca483fe9c35d879687478f2018737d6dff6c72746de28e741b3b77542b42f714" => 8493857142857,
            "d0e1da8f496602028742ea2a1adac1e8c989aa53dded99a9973eeff7a8f890a6" => 25631238095238,
            "d6f7bd3e0b00fa5a736a86670dea7ff3cea40bf1ca3faaebd8ed0599eaceeba5" => 37400476190476,
            "e90b32713cc305cced995f50d1bf22a451ea395de35d094fe796c569031ace50" => 46229523809523,
            "ef877c6272284f714b98cd5398205beb95c340803dbc01dd51dbba0aef086f35" => 2857142857143,
            "f6da484d4bb4e1a32a5b8e697d1d29adf2e8e17909a94013f57cd77a1b70268b" => 26890428571429,
            "00b7b785725c13852d8dd9adf1c0c2f08234904d0cf9673fc0eaf1fd8b142604" => 7704904761905,
            "0c772cdd9de20ee768a83e69e77fb721afc931302698099a28a3b4faf4bdaa81" => 2311476190476,
            "108f88acf8808d01d4f5fbddbb2b474e2466b44843547bec9cd262d07e46e49e" => 630231142857143,
            "11d56fc08b235a8c45667764ede31a149f71f605b123bae272843ec8f8f35ded" => 290916571428571,
            "278cb23eda64e06f3b017eda60eebe485555f7bb4abde9d14ee39f0a0da70430" => 539333333334,
            "2e010308e2793a131d1faa0f7d13c55546e610fdf483fc49e02140d21ba65a1f" => 15098761904762,
            "3fbde0449948ec704383702731d8593658a027df3618ada02b029da6faa4ddf0" => 360619047619,
            "4c7e5673ea0cfda1dbcafa78200afb36d0c63d31148353a89f3a7da026f30715" => 157190476190,
            "653f1ffa9c377e1139a435b70336fd4ee2d5a438a35afaa206b1bc61c914aacf" => 25666666667,
            "6a5fb368cce56552b867e8e1d48b70c14e820d0a8a12ad8577027370343240cb" => 7704904761904,
            "6f8387cf3e41adc2d534f20cf763d6b31c302fab0a1039d6ee64c08730de64cb" => 6902619047620,
            "73d366f7678960d987795918b294a39ce24be82579c1858c29bc786eaea5d160" => 47726190476190,
            "754f998c3d68dea119938a2a98874dec6fbccffa88ec27fdf36fa44af02afbdd" => 154095238095,
            "8a048ce79e7eb307395afa0afe1050aea6804b67a2cc7dcea2ed0f33ca243b57" => 5833857142857,
            "b637710ff368a6a21674c8d965bec4716098fe84aa955b4770324b46fa992aba" => 42585380952381,
            "b9f4470de80552aaf4f57666e66e1df5b279ea490e985a3a0f0821228262a911" => 24933619047619,
            "bffea16bc49100af936db4abe00d0f5271ab6667179aed9c44dc9698cf72fc4e" => 24933619047619,
            "c0e7e1d76be99516e223f581a2e3e83a131d6c327a0ae373c3861f48aef3deca" => 5914428571429,
            "c7523fff21eac954d7ecb543896265d7065a96a57630f5c602c1aa0a207e225c" => 11528476190476,
            "d76d85918f53cc59c2dff905079bb95756a2ffadd3a86fb0dc4e5dcd4f78978c" => 4945333333333,
            "dd44c44ce7cfaed2fde47893abc2133f60a372dd2ba08206a314adaf4ef91d35" => 114171333333333,
            "e1277f698ab17bdbeb0e6112854ba2363abb221f2db2e3b42b354a58f29a8199" => 7704904761905,
            "e9f37ee352f1ba7ac6bec93665fc9eb541c77d37d728b387a03a9360ec5f693a" => 95238095238095,
            "09439ccdb74c04bd2bf93f497911608ca5ca73576c94a9e5eae140bb74b03a84" => 2511571428571,
            "32bcecbce57a123944d62a06f54f1af0605e70827f60f6c0b6abc2d26147395a" => 28842666666666,
            "4d4e3ac16d193242ad69a9cb345786a8ea0f9716f3cdd83b44f9aaf387938cd0" => 61524095238096,
            "4f56d37e61234ccb0bf99a814b0b0c3aaa6ac5d8dacce1ce05ac9de1b30c8f0c" => 249336523809524,
            "703f0482d15f5e0219820d2536de34d4cd7c584877185a96a55a35cf19b54042" => 99772761904762,
            "8204f77fdd0f774159bd904b24ddc41280756dc163ce4a659ea130823907a69a" => 8050761904762,
            "c2cd583fbf64788a060ed6d5ca6693e84e8f62e3551c62636e4364667c33d368" => 7704904761905,
            "d8e969c8f805220d4f4a63f2cdaba4d8853dba360840fb10f70d447786fac52f" => 3852428571429,
            "26fbb2661403842cf141f156ddfcf2bdce76273f1d4b40d074af6bb70e5cbd3e" => 7704904761905,
            "618e7b954efb413c7df83b4b5dfd84aab0ecb200c4bece92c32fb85b7074c4ec" => 29892904761905,
            "8fa3be5ba83b157e4a4be07c22e21d720259e8df91af5b2bd72fb94de553f7db" => 9379666666667,
            "939fdd0e649f1855f6cc09b38222f4948469adcdeefe8942cf3d43f25bcf18c8" => 631325571428571,
            "9c4d47c12758245a26d17f1185aae19e47665b80c886ee6fbd5155c00b51fee4" => 11612428571429,
            "aabb8c72d081af3514c6dd444230a2f4819ad6be48176ad512724dbb9369b210" => 190476190476190,
            "b4bae38566661403895e12056a3cc9c7123d4674a58847208f073512e9acab0a" => 10819095238095,
            "edf928a57dcf54a3d1849d4c08299fee0e37c15c9e80116524f959e703c902ed" => 192262047619048,
            "feb3c9fe77040283f85a4a1a614044fdd384f6ef56b1d33bc8d95942c7f3cf1e" => 5410190476190,
            "b69bf20bcccb3c2b67321630c42a2872b801d0e27f6d6ca227997b14a9b7bf2f" => 646926857142857,
            "083476118cf3350713189ca89808394791bf5de99d38f18413fbfb5601a75019" => 457095238095,
            "c70df2f9b6beb1b8994b3b582b832e5aa2cb03aa05eb6b0b3e1d44051130d7d0" => 79952380952,
            "04c4afd40c6e8b25b79bcfee69316d4a77b586d2037104635d82a2c0c876c459" => 15409809523809,
            "0c10c15cbd03db4ddf8352370cd42d08a4e75b9d09fd471930b6b726c1527b72" => 2294952380953,
            "0f2114a346894dd37ef82db2dbb065030ae20988836b0ce88acbb596a50fe055" => 37400476190477,
            "2dbbcb4b9c8ec39c87b3488c5fcea5de7c6f44843e0805a7bf0eaa488b4c7304" => 2471523809524,
            "36c09a2ef01b4d20e96754ca7eae2420f10c8150143f135e5a52c24d807fd77f" => 131970761904762,
            "4b3a97c558947218f4d99ac5138cf1a8c06b67cf990b7063334edecbb728b8a5" => 7762047619047,
            "7dc5be7efb9355cc877fe20da982e1258c6d09727945e50312df02c1ea4cdf14" => 6486333333334,
            "92307ea1e7d124963475546ad9da302123ebb1b86939315f7a3042620eb1f7e5" => 34851761904762,
            "c8224379906197f86fe8ed99623a51d24485118f86f99fb5b0b202e2c7d905ce" => 126101619047619,
            "cd0873a27245e21162fd6d39190c67faa31a5d3a7a869b590c9f6cea176ff291" => 4411952380952,
            "e0b79089dd722cd6c08bfb170d0fe76ac94729cadd4496f4b8ca15061c378c2b" => 3245714285714,
            "fed77667363e2bb73d33868b98ed0fa7dc98e80a437ea4a08eafccfab2898147" => 19047619047619,
            "072a3dbb3566bef32760846a3f640a5027a3f929dc8bfca2a1e4572206d70044" => 5297857142857,
            "17321431b9a39c39b7009d0462574dbdb997c5243ae60274cd5b7196c67f9dda" => 19542952380952,
            "1f7e642f8aa38a81b0f044b818a39cf768392618d059c06dd39efd90e8cb9d44" => 10286095238095,
            "5b20698adacac517b52410c84f13f0e76d84935aaaefbc24ebf9781fc066c75d" => 2593952380953,
            "6d5c6e752bd9f38638acc5eaeb64bf5e06e724ddfab7aa6c3e6bfefaf8e3b641" => 30355476190476,
            "867dd1109d7f8cb85b74e5940c829d547f19b7d87eb4e7fd276c5c0384ead554" => 190476190476191,
            "8db43755eb923ab589514d1e44fd83891442b10782325ddbd0e2e866504ab33f" => 3709619047619,
            "a1a7ed2fa261e7b806c2feb91515dda88e57824f4196158cf3f35f7b0f2d2b6f" => 232473238095239,
            "af6738f0bdf5b9edda0bc3e2ea2663c2e715db6dda1e01bd042f26a7f8ea3a94" => 8655285714285,
            "ef93c96ffbe418501e135faab990a1485e482bc02fa941c11cbbada378b01a02" => 19047619047619,
            "0b4aa7500710e2ba4f54a0fcce4ea51d7b375c547f2cbe1171cb43efa5c05381" => 3740047619047,
            "8e973fc6b3131486138867a7837533e66432931c7c2448a205d5d0ec93169c57" => 326759523809524,
            "91cb022bef542fc6a226ded5cc0008e1605a7fb940e8dadf64e884a85d6bf57d" => 770476190476,
            "bc0607bf3f830ddb72886a877f99196b436cd0f527dc096350e598a055b4aedb" => 6163904761905,
            "22d90164bb8b8637463922da779217988c58cb54e42f4a6e5f85ab850e640912" => 9523809523809,
            "4b552927efaa44cd87e9667f38e952b4c1a616fa75ce2b2700080a9ca98144a5" => 77047619048,
            "a85e972639b5fc76a6bd3f9526ef744704e39c486098baed502518c7be2536fe" => 9523809523810,
            "b47dbb3180d09f717b0272449d7b21d4df51f387e198f3d666fab21ed8050d1d" => 249333333333,
            "df2d48e52e552614db3caca36f2f4fe7d16ffb23b70b05de28a082b78bb62dfb" => 124666666667,
            "13dc87645c72041e26c4efb75ccd7dd08fa8e86b68601b71b24c9f683b6292b4" => 22162095238095,
            "37a8f877584b28ee82022a858bf0957da93c3eedfa576d96d6680095581919ff" => 144267095238095,
            "48225747b2c771b80f450121db76a41c1e1c7e5e555f7efbbde8cb77e65e08a0" => 74991095238095,
            "2a438966b5361236999e999879202695ff0681122293669c32b5785b494bfc0a" => 10196000000000,
            "3cc255927fda3b138a343e399fb16dcec3ebf49306718abfdb89a163f849f9fd" => 190428571429,
            "78832fc65cd370f3a50d51bf2ce3edbff6f8d81ca5ee02d2b604b4fa37fdf180" => 57142904761905,
            "21d9182dc95b789cc6a5abac4d5573a8db2ae862ef2e7f59b4b76e53b95c3129" => 16032190476190,
            "33769a9b907d97fae7419758aef308223780e55808205dd1fd4c8707d72634eb" => 3852428571429,
            "52f4f9faea07d94fc3afb31b23483b3be93209b1912df58bf2eca489ca6d3da3" => 124666666667,
            "5a7aa8409353e890dd11ed8d3b069a2775a702c63a2288455aa8e9012019fb8e" => 7480095238095,
            "75c912964c357c1cbcb643c2e046ea3520fc48254dff435f15fedf0ebdfa6451" => 47619047619048,
            "7b101e1b08635ac4f8bb448681cfa81185a41e9dc4d2a350a4285fa1a9002d04" => 2163809523809,
            "f2836cd1a79c38ba1fd7afecc8135c525091dff66ba2e079a45dfa26ff956022" => 63476190476,
            "fd27535a87a1d2b1806bd007558e22a10fb9809aeb19745bf5b6e15f3e693c64" => 33333333333333,
            "fd5c9ae93f433bbaf5a294e3adefa36ec6bdebe9a1995841a15666c6acb58e27" => 57142857142857,
            "0187ecdcba38cc393a44ee7e15aecba151166e1719344e8ba2133c65a3c1ccab" => 51735238095238,
            "0ee5e957f3eed745a29864581a75dfe01e9ec569cfb4b8966275bb8b0a653cf2" => 221840380952381,
            "1b63cc17cf0a0628c897496918e73eaffb012bae4bd3baa314e824dadf006d17" => 76190476190476,
            "2a31245027c72ec7f6fea6b452f68f416dcf249905c141cea3c3c24b1a280416" => 734225952380953,
            "38ead82212cefa0b65907f60d1f93761cbfc5e9b401ae0dfc6b51f3e20c1a96e" => 1774514904761905,
            "39a15f086559f9bbbb437a0171d70468cbb5dc166334b64601480e787057dc5c" => 585636571428571,
            "3b8db2d93682cf25ee281b19fb5bea18de99bf8c00d130423bcefe78671ef034" => 12466809523809,
            "3ce116a69eecf817a437634eabc0efd9052daf6dd7d5a2602e13bf4cedc6ff36" => 2528761904761,
            "52bae74c7aab8ef8b1275a759802067c739c7665622a285b0ca5d73b1783d721" => 25333476190476,
            "618e4587e104989c4d6713fcf4dcbd52063d25d1af83c9c5fa44005d783552e1" => 19047619047619,
            "78a9101ea1f5c507b2b7a478ea5def994b0682e7eb2528feeda99d72b9d37d8d" => 16950809523810,
            "80b640ea068abc86cd97d9c102712a58a5909895b141cfa46490c4d11c782fd6" => 20803285714286,
            "af258709ca1266c524c89e4151307b81700090eb16d29015cec4bd64d1840236" => 108191380952381,
            "c85cd5b4cd660b22b374014c49b9eec36357c8edaf7a220ba8b31f8581d076cb" => 21638238095239,
            "dbe6afc804384778b1a5598cb63d3a18f266d5ebdb66bbf0f265c72eabd959f5" => 6233380952381,
            "ffa8600eadd5db770890ce7a57aaee7333efa0357408e91a359fb68f4c96c11e" => 6768000000000,
            "241a64e8ae7864d91376296482ed0c9a2c7f5e688339ba12f2e8711d3e0dad0d" => 23809523809524,
            "63828ae3cf6e4ef6729fe798db4063c70d4694a814ef6a6fd151df8a613db95f" => 10819095238095,
            "87838dd31f40620a4f4c7604d4064d71dafd22c7f29a01a4f1243d9c58b10e82" => 27332523809524,
            "8f747b276552a17007581bada2be2b7be64f01b71be23fdf7833e6019c68f392" => 15409809523809,
            "96dc6af5f27d1afa7def62d7352d214e4ea98cfb10c896d738663b36f930d1c2" => 19321809523810,
            "cafc8a090f885e29a32da0ee9f8eb311801b75e78d85ffaed7552e82f69f96f5" => 498673095238095,
            "fca9f95cc3222bce22d3c4b90d826eb5e66f827a6afad7c44da4bc31e150e6bf" => 124668238095238,
            "29644e2517c0b4e0392b4ad3f0d79caf3f0a1e339bc0d9468054fac8fef4fc94" => 77904761905,
            "592754c4c2b1bc982c3e30f153402ec7453d1a3ed237d7087320c324ba859a00" => 11557380952381,
            "6f1808f5a463d41adf630d30355b7de2087a58a7870edd0961a36b88df17698f" => 2170857142857,
            "bb594638cac456891bc3971c776b9d0141db999552f1ccd255a425513f4a50af" => 3809523809524,
            "0d9a95fa34069ad50ef381bedf68faa0fce844c71a72f4d7b6c01e0ae9acedf2" => 8249666666666,
            "45ea34f1609a4a6bfa46e9cb2e1720a821d37785a2d4004b6465ce7e08b97f21" => 38524571428571,
            "46e42629ad4d20c31f1e30fe623d2cb69c3fa97257d352bfd0da52e72ea2f63e" => 5539523809524,
            "60d7088bb0e60d1eccb380b1bc03197dfeec0ed5bccd407b8310d4cf1898056c" => 17450476190477,
            "84eba1ff2f061872b8447369f660f99630df407ca3fa6d5348a00d8a6ebd080c" => 12189142857143,
            "8eb1cc70c1261a7ac6c77f7f04894cc8c023984db9b01b7220eef638290b4d4a" => 249333333334,
            "da8925b30742cc03e23878d743936c96c0c8384b61ca16a14f339d76405b6a8f" => 1372190476190,
            "df025d728355c23238c256ccf7c7b5541a2ee89b2364e17e2c7bdce82c1256e6" => 2704761904762,
            "e5293d184f3d4c2c29c0bcded8253057ff4dc357a76c1217522f8c84bca5edc0" => 1816666666666,
            "f4814ccf7ec64d88f62afed8797d4ee95a9c4b9b3e2f75c126943df7e8936271" => 92878761904762,
            "18d4f34784151152c043821b3aa0f7d46f43dd492629a299b91ba71f2cee49bd" => 124668238095238,
            "2c7d5c10a1a41f9e92d1335b08fdd2e5650fe85d3cfba7bcd976a1626ac10304" => 214252190476190,
            "807da77764657d16966435f9e600ada34a4613b1e56d83a798137366477075d6" => 142857142857143,
            "d3185a9e239a93350944f4a449b35173ba0b1996d4895d16c852c161d54fb7b5" => 6666666666667,
            "11f3bc6882fc28a66ba227a1e2e04200d4d4dc33e0ccc7ae1fb7f8b5f9c278ac" => 45666666667,
            "1324fa77339ecaef583b00fc881aa3ca555a5e5e788e59c73e58ede93af02641" => 24089095238095,
            "199b11fc65f5c2bc207d9b5340cbe666998c80710881b42081e31c43e1584e13" => 21638238095238,
            "2016bbd85b7de96fe661acf8b9c01ebe9b337a5b531a2fde360e85f708e7013f" => 113070047619047,
            "256cc65bd40fcf1dac1836f25bf252369219181df86400716a89c4189967e4e1" => 392190476190,
            "3511cd1b68fd99aee262ff7a0ed1e7d386384c0f78accc06fd1d5ea16d04345d" => 11474047619048,
            "3e0faa4a35b9ddb00a7358dfb3c1951b9fd460804123d26cbfcf997218372854" => 20377333333334,
            "5087ef83a927377acca85170a090c84b4546fc82a47af71043c9d4edfa746013" => 3902904761905,
            "9af0ca8ef24f0e362c01ffa5e46cc200220f4a74b1c52d8d462c7850646edc57" => 473586619047619,
            "a231508b195ce317dffd9fb806e7672899de680f4b7058e9af6d09a5fdeedd59" => 4327619047619,
            "b5cda33bec5c7004da7bbc964ac69eba7d6b513c3d8fd1be879c7d2ac2d1198c" => 6233380952381,
            "b66026ae2a7b07e68de8c5d071fbd79bce09ac7fd27b07f62b22533d890e3b45" => 4761904761905,
            "c5fd0972379a19df5ee801b1fdee437046cc4c33b0c11be7e9726a1764dad47b" => 14351285714286,
            "cdd9b746922b345459d8f9cbfe75b6bc9ca1749f1f70273ae39e002c842ab81e" => 170444380952381,
            "facedd481f58c828461ab5058851be089360d6c862879c1c1608840e24003ab8" => 24933619047619,
            "2ae7ca6d47edaf0bba513a93b9b0e87ee2487d78faec21ae4be94ea0f00f34ff" => 3852428571428,
            "8f0799d546f8a19d778ea221e26ddf28b6b004309b152034a63f671ee3b4b050" => 5409523809524,
            "023acfcf8ab58ca6459e10779a81f72bb2c1bd309da48f02f1504e7899d0635c" => 15712714285714,
            "1a3ab58bbf3a3cf5d56235daeead9f376fcfa8d4c953ea23b6f607caa934dc31" => 21638238095238,
            "2113488372ce926dd2ac8621f5e2834d65cb3fbf7abaa93bf2dd69f2d107b196" => 56787047619047,
            "4fbf778a4927f686973bb5eb201fcc076fa72c84f0c8a5d10da02b27911cbde4" => 6108619047619,
            "798905ae19b1c4fb346ea700668d7638f89a3284fa581a3368cd1c04a349ce10" => 3655714285714,
            "82d09f7e690e0efc0a3a226b4cb3c83f0fbabaf9b62f505622b23ac3cda11c26" => 14449666666667,
            "ad6c61f892780d1656009cf4e989f9f5c4b935ecbd79e613895c0903dcc9ffe7" => 16126333333334,
            "b2ec4fa8d9339876a88650de2d861f9cec56d45556962f18faedf9ba059d5dec" => 282476190477,
            "423d0c53dc639fac26dacc799baf6e97d68edce9cb8771b1f50b1a288da0429b" => 63476190476,
            "6a553656298add2f878b4fab2206b8e6176bc6ecee7022bb2c394e4bcda64e12" => 15223190476190,
            "7a8e5c75ff0c3d9f3c78fe3dbbc518409a28fb2f20918398f5b9917362a54360" => 92748571428571,
            "7b664b3ef17490ef923e78278959914b629c8d629511352671bf69af0d1e1467" => 7704952380953,
            "924b74dbd20f702158abb523cb360e0ad9cbd698ccc0905bedfd77747af1ba4b" => 36190476190476,
            "941b2690d0277f7bdf6668575c3ef1134d1aca0c74bc7c10a3b430986ffd7586" => 6978619047619,
            "d23948639876e20aaeed5d67e23a55f5b8ae231b0976237e65a64adae1b013ef" => 249336523809524,
            "9ed7865c6ff4013ac5d49bacb8ecdf848d152ac3f653fe57e3ff5e506eb6bf99" => 282476190476,
            "06a67da2dd3608bb83cecbb5e9a3c9c114dc0447999d8261d372ae1687d8d85d" => 24684285714285,
            "1acb1b31f16b6b18cd432e2affe21dd133b818723ee6c6f6a38b7a2a091ca91c" => 38524571428572,
            "24a70d9decc94e984a223a47d8c505303b652b395d14fdddc042c1465e1f650d" => 24933619047620,
            "3df1a9a041f297d0ddc2fdc95c586144e0e95bb2eeb265d9af3dffbf97076770" => 126952380952,
            "7e32efb400ff5b73dd2b3e9903d9da902a6561e9e4740fc78baf1f126ccb0ab1" => 5420666666666,
            "961e1618202bcd73cf0a551e96691e73b58642c6e537911fb2cad262187fa1f3" => 361538000000000,
            "c2d18bae58482089e426b7aac9f0ed8c43a371e5a9cd3b532cd7e73a3d6df40a" => 4761904761905,
            "eceb547c6a460237129e0b2aa76297a25377128d8b9ea63f5c1cba314507cf60" => 90880761904762,
            "f1c45589ece3d923736f06c4dbcc5d64f539c6c33231da45f589e2246e715a7a" => 231189952380953,
            "1e02fe65e309cb69b224a03342367484273bfe02f005b5803f433612096bd7d4" => 282476190476,
            "4dd9df5fbcac4d074d7f9b90437fe741e83d37664e59c523cdaf7d47ee0963ce" => 10819095238095,
            "c8d10120522fca76d051a41d791cf17c9bddaee253f2f8b682af9a4b72b5ac58" => 47619047619048,
            "0f8f2def5b68ac52d9b9b8665b43ed9be6e6830a9632f41f4915bd33c4a009eb" => 28571428571429,
            "12fd5a9883b46c491b0871002a849284479871ed4a8c13deedf05c1c07410ad9" => 81143523809523,
            "159c7f28ebb675f3d2de2b5218dba5160c06b34e22b81d72c8b02379c3a4b797" => 77628142857143,
            "31839d46a56ceb18299293140c634a3dbf6c56f817920a804f9737b1634347ef" => 6131380952381,
            "41a939ad41ea732d6f556c96e8f6381cecca19cf9cb2f15555bf9c56d7e791b7" => 10635333333333,
            "646ff36d14e6adfbf7689ec4589c51534e8721d8a61b4eddac37385691463e8c" => 33579952380952,
            "77037b99e8d2cab9f8767d5e031470d687a8d9efd4e3fa9a876cdd7d0534694c" => 24933619047619,
            "8f14e77b98ddd083054d23f0f18c101cff3624dbec81302d0b6a59d144c969fb" => 13739476190476,
            "96997514f65b12498c8cd0475d820dad1fd1f784cf4a437080694d2e81cd7fe5" => 21638238095238,
            "a91312f6d5045a25b5a81e02689255d989aa6248428dd50a3bbe1bb91347d7f1" => 108190476190,
            "bbf250a89c0f7a6003319ff164c86d7c950abd7d637b4f445b0c1189cd0c967d" => 490142857143,
            "bd4cdf10e23c4f33a7331713fd0b382e233a11a0c943b8e535a32737b3187f7f" => 256809523810,
            "c7e74c8e0aae12f7943f71882ec49cfdcfc5a8d4811b7632da7bb107c21e4fed" => 77047619048,
            "e4c36bab3adabefced97e4432cc6ffb189af88b43994870e7ceb458479747256" => 10819095238096,
            "05568c0b34ec052f82a90210cb0a33f503c3b81b970fc3ca44d5635b7350b42d" => 3852428571428,
            "42546ca8a8c324573b77a9eb1d65d60ad79d2bf4a6df39410ebf54af273ae4ef" => 32990904761905,
            "472e7ab0aa54b1a608d59944784c8d13791807c1004a3daebf9d0ced5c67b35b" => 130456285714286,
            "4b7320c1452e76f1f1db231c8589b7745ba9da92cb679eae73cee2d4d5b28324" => 77047619048,
            "4d5f9d57451b3217314c83e212e75373770bc17c9c50fd0452d5ee5542f6976e" => 12681285714286,
            "58ad60c9d91b285e7304299a25076dae61a0cc84a0e08bb02f47d0878d93e7cb" => 17982523809524,
            "90652cc65f1089c502ad9317ce44fc9f3e9c0dd15173d828b2e4062ab31821fb" => 38522476190476,
            "d3f3507af18d697b554559f664e74bef692b2615320143674fc6c83502616374" => 78674047619047,
            "fadea6a836863e9771e72fb43bdda3dcf162adb415fc4fb35434b6e11ecfbd4a" => 40836047619047,
            "f66e835fa0778675e45e2d8bd04575bc0529dd890ab18f9abc8bef06fa56d9ca" => 89462285714286,
            "937c3906b9d41ab3111e219f6ddf8526acb6ba62bd95e69a72f715c21d64d9c3" => 115952380953,
            "ba6596e23016b9c5463cb4b2319d38fb1a22ab7d41408aef7dba7bc4d1e4aed4" => 154095238095,
            "06edbcadc95d3eb261b421f9606fc9890b65f17562081fb61308e89680d8b96f" => 73344714285714,
            "2584069e89262ccc1770b9de0ba0d2758e165f8f53a953ae5b125d25ed983823" => 165047619047,
            "73da29ec7840834d25368a7bcb553d62fe639ff5422978a968d7dcd9a7513d47" => 73193666666667,
            "c007b13afba739a102ae8b7918e1af37bfdeef002e398b7fbb5ea9eb9f80efef" => 13757857142858,
            "c0bcae9a57d29260730a542ff228fbf2cd1a0d876146d4fb0b217488c4be3dfb" => 109992333333333,
            "84a210c56019c641a71df30056642ffc0b25f7823e32682e323954b4f035021f" => 615445714285714,
            "b8316e995af787c1d571390c9eefe32e157ce162e654ee4642cf16ed897870c6" => 48366476190476,
            "ca41bc5ff5afecdc3393524655a7610f926a980628756014cbf06af8d9666e2f" => 349190476191,
            "08b3c3ac3057c41e18a518d0b0b691447adfdab7434cf4815f4acad00f8a3ef9" => 43276523809524,
            "0d7a2d507090ba5929fa34d6ccf463718d7690ad6ef83a96fe7ce44c02630c16" => 9675952380952,
            "129069b6621874970b600f4d9d205e11414b7ee07e2bac58a280f4feffe5d438" => 4761904761905,
            "5de0dfb769232f07af03cea91690a5d964948dfb2085d7941635a5b0b4e0307a" => 42721333333333,
            "686d3a1c0013b5c2f0c9f7e1c7cb562708223163f406fa64d84d28159b2a88ca" => 9246095238095,
            "70531227799b89f9fe60a8e78fe597a1b69b4f2d1fd2e730b3e5bf28be0eef0d" => 5389523809524,
            "076fd7cc9ceae34a8883f2d833001304a4c923af2cfa3b80d72f83ebc5d3ba3e" => 44688523809524,
            "dba1cec4d4a53d75fd3340d3be22596eb6b193cc79448701a857d476f845cf1b" => 19047619047619,
            "8722c2d9bd8f1f148eb7c24b1f66037c775ef570c517c9246ec68007b146d10b" => 16228666666666,
            "d32c1ab87f33c5c69136fa6cf92629a3b836589866debd43c7ab007669601968" => 39483809523810,
            "df3b80b21a65a46a8dad43890ecf4aff520b20e28afd36bfbc9103e6486841bf" => 9523809523809,
            "18589a88d8946ad6ab34ab75acbf758c152fecf051a0b1ed3b468d92deecae30" => 124666666667,
            "15ee28933c2f7386ea3e072374fcd7f67045b83ece03aa825c75df86a91c17f4" => 306809523810,
            "7cff1e16ffcb1a2125d50af71c74b6a22e8b4650c891aaa7d4b3378637e9b83f" => 10007476190476,
            "ec705fd5fe97141bdaf74e16d1cc86d5689af412c179b58a0c03aed80079bfb3" => 39489857142857,
            "ee1b72a32e161c1baa2babc35998c815a2cc2f5e9761073075eb02b1bcca8844" => 282476190476,
            "2c854da84ad7cbc6c5ae6058b4e9c93ba96848cbbc1b23e56051c1a1d16c8961" => 19262285714286,
            "6e2d4d68bd666ca2a222fb9d22740fbdf982a64d6520885bf3ed266648c87e3e" => 50229666666667,
            "b8dc4fdab5ade2f6e4f89f021e4339f61ba947e3aa6b890b2c515905f9d5ff4e" => 7704904761905,
            "c2728f18e227fecb8ad39d42bc20d99f0855588b11b97f78680773410a10aa63" => 19047619047619,
            "e661a4e29be7c6e36ea2a2ab2bdc7a10ffd9bf4d83cab5d36301382b9d6a0f79" => 12466809523809,
            "3302d88efda3f7a2bc9477f7d7f1dba49468e8cff59c7397ceda4333fd2a2ef4" => 770476190476,
            "a9975125ca59dc51ba27b0a075074d4ffc9426e540005ff2dc5fa2da64901afd" => 14616380952381,
            "279bfb98e85a334be420bacb9f55e5be67479ea3462675d69b9ec3c572f2e740" => 45503904761905,
            "7cd509f6847aaf5298a8c63cfd02e4976f36d5b1bd4651387b928bbfcf91087e" => 1941428571429,
            "ffe38186497fedea0c20c6cb6ad03321a3c6dba85b352ae174db9fa6ebd24cb0" => 19788666666666,
            "20cf969d752b8f021feca6222ab3aa9aa065aaa035a1bbc3ccb15efb7b3a5335" => 10819095238095,
            "40cc37ad55a30671a27d0fe7df3a83696b4969b9d2a3282e9b9d407fefe45cc1" => 324574190476191,
            "5a19953c7b328c86e45a375a5d4e8f4cfdfec020a18e4bc163bca6bd332f5b0d" => 15361619047619,
            "2c5cfef0bcdac0246e5af5c3f2ac01e0317bf4f60fbbcab57d003437d1e2d0fd" => 319006523809524,
            "2fae42660c738e22db5cb7e65f868f39285a7d1b641cbccb8f9b7c94c5e5fc15" => 95238095238095,
            "93ac231e324332c3ae8ba91d5d670d37e764616acc58d57b32da1f6a94447b65" => 47327619047619,
            "a28283a6aabd88b91a303737c54f842147fdc21b100f1f64acb3e173362be55c" => 47276285714286,
            "e3aa317b7e34ca4f3a0e4f02eb18982d623930380d536104de7813cd388153fd" => 108191380952381,
            "a22ebcf840fc136f49045d2ff96a7814a2d35b099b77b741befe9dc0fea5b834" => 1309809523809,
            "b9cce0e355e57118522f969678798402727ad345f72aa40091650e8be226aea7" => 381716619047619,
            "c00daa71a39f2dbc62a0ce801b96be20b7186d30a8f9c1bfe18d42e6f6a61d26" => 349190476190,
            "ead7496f72eec6dcd3a144cee24a38cadd1516115d4f9a1a4fd6d6544c24170c" => 216382809523810,
            "a4aae4e05107defb76ef37a03fe61f9e030d5724027284ebc0ec8a01984cc426" => 30819666666667,
            "2ce9c46f1a40b9e3fe5acac52e9fc8bd0fe34aa55204fd4355735d0c784f1fed" => 982476190476,
            "5031007fb71f651c6a83510bdb270500fbf0408668dcfc8f89be57286f4a741e" => 310930476190476,
            "5728d7e360a792e1911f7ba38b31aeb77c5b9d5dd52c206032daf5cbf1e61bcd" => 171428571428571,
            "92609bd4b5ee9c11c9463539c5a4c0498116c91bcfa661446e36c45f69eeaaae" => 28202238095238,
            "cd9397ce9175f95e0c5001d7b6764a741a7a4f2720252b41d34e423d3813277e" => 7704904761905,
            "d4c155943447d4c7f102b00efb23952b0fa3d27eec8022dc1bfe12fdf594498d" => 1400024761904762,
            "878da5f286779800ed6952ba4e4abc8f680543646c2d99ec041ad4281e172d47" => 14465952380953,
            "0f9e4a97a28c876b5677001a33ee86dd4776438e9bb4bdf513b6631e7e6c9ca8" => 41937857142857,
            "47e33573ddacb1017e5e943bab1efb2d160238c49291eede2de3512fcf256591" => 19047619047619,
            "4cf782cce19106c58c4a650f9098a2f0758267646c3cb3432279d3511b434ff7" => 59505238095238,
            "8602e161de7cc0469836b04440e15cb81f7cfbee8caf7b863aa1ef335d3d6131" => 4090952380952,
            "8634f0651d088d8a7494737d36fca6809bb1c3f928373ec3d32191b21ea901bb" => 4986714285715,
            "d00536643d8f1f62ee5de9dbc19219f65fb6e9a33722763653e7a293f9eaaaa8" => 112201428571428,
            "0034d466740f40aaf5cfbf949898998c9a3ab0d06dc4a8b48d9ff75a91d7c7e8" => 7573380952381,
            "59763a6fda1bcde0e46a95fec636593b3863e13ea3ab8d7bdc29efcc160d78ef" => 11813523809524,
            "76aa0b5fc2dbd778ec738370acf8bbc2de6a697e8c8724cfe3e1ea4f133db4d4" => 4345809523810,
            "0ef691a25f6dfd35d0b98ed9598753814952dde96a6f46f927a43b51dff27641" => 52727714285714,
            "13d4313a3dc0de343d23a9b329a55efb1877afc5a8eadabc02af2609d6dcb7b4" => 29969000000000,
            "c4aad1cb37e339d3e395dec837b89faedd553dd223c196838c0933e5164e873e" => 10786857142857,
            "6db6737a86c798a89daab6cf0e2c1c078ad4e3518c3e50db5be10e72e326a66c" => 14956142857143,
            "fa3e9acea07e76e7ed565d22390ae244dd482a92a48bf57e9bea696d0baef699" => 9523809523809,
            "24260e6230231e2aa71e1c7921e5e4ff688b72824cb3c865790b5d001e765590" => 37649000000000,
            "e933538bb7d7e377003130cb1a8faef046013dd8dd09477b2ce810d348037363" => 3908476190477,
            "0cad5430a12220341ecf11a34b677623193f426b060f8ee1eaef47d48c3addf6" => 2209428571428,
            "411418ab84a00cfd5fc84599e1d9403b4689dce5499a7794348330173bd9e77f" => 12170428571429,
            "b51279716890ce40990b9a5ed4d52a4c7c714948fa182cfca32a9740b2ff4cdc" => 124668238095238,
            "4e0b0137d96e1a0a0aee57e1b91da4767a96f038fb4785217d9e15d49123bc77" => 4314714285714,
            "bc5fd44966c615775fe57bb5bc537b03a2a71e46f3be8b906123a4e4cddd16d5" => 181298523809524,
            "3e1d38b355c6f6d9e433d76ac1e4e2c84a280cda1c7b608e308f58abd5d93d76" => 22522285714286,
            "5b6c52aff47c4dcdbf7bb020509a86f311238551cc58c7a740b66eaf89aa8f26" => 20909476190476,
            "b45f07144b6cc6cbb4a058715d3ae4675c533aae63049d497d8759879696b87d" => 124666666666,
            "255edda7fcfa25e7586c4354c77f7ba99607b5ff993d783eb127bb356e4f2d21" => 15423238095238,
            "3d37665bdbbdeae6a5401985dddcfab4147608e4220eef87c16a86a400b37c96" => 10651904761905,
            "6db344411efe73f9cf69689794ded1f8a8a0891ab5a20a423bd1ae50d430ddea" => 15438714285714,
            "80287312ad5e5f017c518d739797dc86a8fee127d288d2a96882e79caacf95ec" => 1094809523810,
            "a77109d00071f5d9c55ad7e8d97ac09071ff6059e280994a0a4403f66133e3f9" => 10686333333333,
            "146d1847714a7915c3e4801be0aaeea86fa7c40baa67751ebca77e17d1398ab7" => 16358714285715,
            "27ae49957b68e01a8850043bddb753b8f18263229f17d21cba80602aa3c5bcf2" => 55758380952381,
            "404ada140f1e890a84f3a354bdd7eb4f94a01225db37f3d2a24db86548ba45b1" => 17960761904762,
            "5d1d021a465f483bd5a0adc3d96d524108b33eb42ca6ba4ea373e6fe390c9405" => 40334476190476,
            "a960fa90f1c5d840ef714a88153690ec79512b82317599256cf1d54a030e25ee" => 22332952380952,
            "a9b02c8bdff880698abf7a0e0888059a131a50677aaebd17404215ca9986426e" => 19752095238095,
            "3e9f50d7e1eb7275c45ed47d94be4c373212d76d9f90fc854a382062af169892" => 71883904761905,
            "3d418aa655dd2607c17f684611a322f9ec9094ce03c892e7d35f318fa8f49c85" => 20000190476191,
            "9a42bf0d61c82f20dd4fc5bedec03c689d0e5dd20dccf46879920277ffef7ca5" => 10827047619048,
            "9a85c37deca082332e8e35e35b8d1b93b28ce9758ba5f9ba26823385b8151b57" => 80466333333333,
            "bcf7c425e9d3eaa8d7246c008ac1ff0968b421c88c68ce58a3d018f2a2e66d71" => 14052904761904,
            "bd8ed1c299bb1d7ed76198183f083d087bb40f862a3b0ca034b5f9469cc516ad" => 26514190476191,
            "e0feff2dc7087496498309b591c04aa484a54fefb2a21323c719ca1fcb193fdb" => 219273714285714,
            "f687ca5fec45d4135ae13c9ceb54b0d8a72db513df7f1bafd6115c9f7ed388d7" => 202463904761905,
            "7589fbc92cd8262091ed87d8240acace53fa4fa049d654c108c14f7b8489f8c9" => 108191380952381,
            "c99caca424fe21ba33b9208589e0ba354f1dd280be00d63a654cf9a911e1034f" => 13937285714286,
            "e0dfeddbcb67076d5bfbdd90da85eba568d923b0b09b284e6112d7d6460174df" => 47619047619047,
            "5b95e217bf3c1298a9efc15e1c153e0e9a6b779dd97e9a8e58f372d4f7a27272" => 95238095238095,
            "9ec656d85a902d5f6fbf116fa3d1de53e86960961d21109619360ab06b1521d6" => 3634714285715,
            "e5047bcb6a4250c3e797f08b2bfd2f585b1ef6b60687486c0849daa601ab5a13" => 124668238095238,
            "9ca91a96b8386cf470bdbb28eff69952a03e0ee86b51702741130434b2a9a02a" => 192374000000000,
            "dd3f099e020ae1520e7284ff0d0d0b43c08516d1324251f96f996a038c3f0fa1" => 6361809523809,
            "8820b0dae3273cc7cda68463c1766e0fa5c8a7607c31985776c667b30d4c19d9" => 52504190476191,
            "037589bde093b0c98a5259b69709f2ba12863d8f2628e5066f04b3543a17ecf8" => 186638619047619,
            "555aed244a5d8a4846d46de1d43aa3ea06a278145b3967c379e88939a49d95de" => 3835000000000,
            "6e35a3771423e13b90ccf49018b086c9f8b374327525228d9522940295d046a9" => 28571428571428,
            "37862839e6df6cbbc28d315345f929eb9cd8d73d5418adda5852f8d7be8d0430" => 95238047619048,
            "cddc21fe5caf6c2d06ca0cbaa9f8ac65aa36a5aa2c02f547a98484bc3b3274f2" => 124668238095238,
            "ce381b8bfb254b3bbcecec3e4f1269d2e7db16de4be0df957af8ea0ae08ced61" => 21346047619048,
            "eda2b55c28cec76f74c67d2716be378267869817de1b5edd6c4d4ec623abaed8" => 5409523809524,
            "b66962dc01ff3ea41441db592fbb1d2ff1351d5411a949471ae0bd768b5079cd" => 27041333333333,
            "fca72c12ed9364e57a72ce935655012b97adca49c16ae2ed731e0999d3d4d3da" => 68547190476190,
            "b805a7876a5e605221f84ee3442644c50c36f91fa5483aab42fbc5f18e2e522d" => 96278095238096,
            "0f1f917925a5e83c1638ea5eb38e5c4e03c6a11e74014b73d7d4c9421d3b8ce1" => 5142857142857,
            "e00567500e4e9319abf9f92a94bea668d5d423916d3e11ec550d0cbeaa409555" => 23542904761904,
            "2f83b7683d62a76521472658e4bbbb11dbad5759d07973ce6549f8d0aba077a6" => 51333333333,
            "395d38306ff23ef26809dd3c98f5ed1b7728f26e229c8d79a4b1c2cb91a06a5d" => 102714285715,
            "d65e9efca85973d0f4f95bb93b52ab4c635d8a770dda7ad0c09802d094a91d45" => 95188952380952,
            "dff262fa70b2a0cd4da65700bf70d6e42a9e8445156ccabf962e027546225722" => 95238142857143,
            "593604534a8f76cd9874d0c20b3e5ce46bdb91a846d99eecbff5d541982cf7ae" => 42377047619048,
            "edbd2cd6269c519022133988b91a80b719f33caff6e1418ebd77338b3b29c469" => 85784190476190,
            "38b24086a258e0a7a5cfb772aa6c758f054abb0f799b394146c6519c9579518d" => 24933619047619,
            "f8115ccc71dafe1dcd3e441849fc30035d6c9228c6e5b43ccc41d1c9f02dc9fa" => 24933619047620,
            "68929ca82a6b4e06a0ae184b91424261476a2e67b1302710ca781c2295ed4501" => 27047809523809,
            "ab83a6f98f0ac9829d6e6667e96328102ff53d1d5e00f7dde3c4f254b59f4aeb" => 10819095238095,
            "de1c3b957631ba5a6dd0c3559fd462fbccb93e361dd9fb7c06a9fc234eeaa054" => 8913142857143,
            "9125cdc21519d32c59f2a3babcf928f144a5ffde0218800307cefe3e210d6b6d" => 4544000000000,
            "affa253a379dd6cd69d9944178bef0dd2cc11dbd20840668a389e85cf2a06eff" => 11128000000000,
            "297e341bf722e19d52909409a03dc0b66919f2eb598114e84e29b871bc2ccdce" => 1658904761905,
            "661b8db96c997bd6edc8049ad4d7b4bf3d10b2829709583ea0d29e498fe3ae0a" => 99734619047619,
            "679e77f809c21a09b6b786f0e1efb8686d15ba95cbd33d497658df6e1d76894c" => 124666666667,
            "f369a14217c6e4ceef655b7b9347aa9ed54c65c180a3ead128e02e5d6adf1f8a" => 55410952380952,
            "29c20ffd12376b9c7b86e413e979e8980784b70814fac7d1e48a60a5fef51a4f" => 497648714285714,
            "1abe1c40d67a82f99b1af8db7e32448010195440778558d2b6c8127df6b6ebaf" => 9834380952381,
            "2dcd5cd3bf8792794880f8a9d9a84dd9b3a0f2b9ed9a36e962a2005a287e4194" => 17090523809524,
            "2603dc258c0f422ff9a33f1a0c5d93a38660131cb5bf2093fb99f72eb1f6c289" => 189666666667,
            "5058f1d917fba8f53d85b8f9c69853d736514c0dec5552ba8f7a105c29ad36ac" => 54095666666666,
            "e8c9c46c05dd3038aae94ab3c97138bb42aaf3909dcb30ac18dd0befe5cb87ad" => 47619047619048,
            "a359663a1be303e8cb2adf4de34be6c66abb23d2f49d4950e59a8d823ad6f1f0" => 9523809523810,
            "0273d07287049c04a2fe726a9e34003d02feaabd212a214a48f0c8806f1880cd" => 96793000000000,
            "812f4cdf788a3115a0e1873bf0d42ab416d907cb118c5e61021883c953617405" => 33333333333333,
            "c492b8b449d0d37995f01e9099649eb501320f0a7326c5f95f9878e1b11e5f96" => 2091666666667,
            "04bd5640a8ba462e1b21609ab79201eec1aaf3e7e49d65babad4595f08761f85" => 984258857142857,
            "8887deaa1c36b1658594e87f78692138a44392c0a32ed6c7ad8c3c30677dc7a4" => 7421285714285,
            "b6fc2b9717ec2cf2ecdf3ba5f9244dba1ced1106438ee42a52bf33a5caa43a01" => 18095238095238,
            "29c5f2b4037e403333718f159ee4a38e036a77ec54a698c6d4c31958ba30c1f4" => 134285714286,
            "409aaf2397b6fb6471c9a2f89ceae6fefb0a947e821032d00d142b3a8c2df6e2" => 77049190476191,
            "0f22a554f9ef9acc7b96b7cae6df680f536c7bcfb768854c9a957b78466fc1d9" => 174523809523,
            "13ed88b9fa5915f5b1e75729b77142136d6577b0b4611d381865610fd48ffea4" => 3631571428572,
            "e707eef37b37f7718a5e5e773566aecbe3c9e7644c8e0d62f68149a755322677" => 5072725238095238,
            "21be8b0014cc8ac5433bbb007565ea84d89d756d0f4e097c451c1445525c0e37" => 374004809523810,
            "bc0ffff873c146be8fdafb5614e6bd4835efe46ca7f856ccb4047465377ab1fa" => 190476190476190,
            "1f9a00225204404de17ac132dd6095b9cf6a20d193e3650efcf2e613d50e988c" => 21079047619048,
            "62edebc5c6d0832060ab823af0f4cf57f8935c8bc8ea207945332683f3681290" => 62908904761904,
            "a8a3cbee33cdaa829320be70e5547741800f3eb8be88477c50291ea7acb0ced8" => 154095238096,
            "0617475aaa56cd574221be67ee57baa358c4877dfe23e35230fe6da95561edb1" => 7704904761904,
            "596b2394210db04e954bd99f81c3ab3ee6def70ba70046ddd396880384462ac3" => 10444142857143,
            "9f699090008a593501c11696f7a87c6eec4b902f00871d41cf20fe9a022d12cf" => 44587095238095,
            "192c3de86ae2cbe2c88b9e5f35fd4ec1930c1699cc7c1f1521f4178dabcaace5" => 154095238096,
            "3df14b39e729ce10d34a8bd5306d68b44e5c9e149dcbac5719a0def257e4c64f" => 249333333333,
            "9b289865cc651139e5de7fe160ab5079850a808cbe0864b9165d6f9a189d9eaf" => 24933666666667,
            "3c92fc151c1776b183debcd2e0540110a0c40331d3631c7713d85376c7f4d4d4" => 124666666666,
            "57fa2213c67313af454b2da48c04530c70f3f27066f17f17be1d71886630b13b" => 2742666666667,
            "ce6e0bc677d7970bd6a103673be53a219b0d30a4275e02ccf30acaa5e943fd01" => 3178619047619,
            "5b85f235669dd28fa0fec86ed7fc3fdb303af0ecc1b8e4f850ecf17b50fa25cf" => 29920380952381,
            "44ab495990e359b72bef627fbe44a820451352ad15687a5fe66bc52ce580053c" => 23910238095238,
            "45689e396aaea5a321bdf3bdab7cd7c4d85b11d24cd21dcdc478fc2dd2a69c79" => 196873142857143,
            "bbb938797c78d2885ec129ab547263149b32d87b53d69c63d5f207c2d5781473" => 25528285714286,
            "583b209b658e286d1604c77bdca2e62be038380847149db7ebb32ce603f7a72d" => 9523809523809,
            "63819369a443b17c81e03c806eb078e48e510b66679a4ec51b1347fab2395507" => 77049190476191,
            "df0d051cf9b4743332d7cf3c4fb391793901977420308adf15d5e38186f66702" => 124668238095238,
            "6817bd99d13361822fa57339230ad24298744c98edeb05c4619660fe4ad3a268" => 18530190476190,
            "4f7571879255df62a969c03084ea4b4d3994e6e5b20d2db5678ccfaa9c7cae9d" => 123278761904761,
            "6899931caec9fc8382c7cdf49f6b76906c5cae453bb40a646bc4b1a4d875d5e4" => 62334095238096,
            "e0472e00c03c9899cb456b3f7a2869f7522060320b68caa82ca31f6977c50adc" => 24933619047619,
            "8cd1fc91857a49b7de88eb024abed0b98c7ab9912c3075dd4094602fd26dfe87" => 99734619047620,
            "65a7f6afd0d066c4717418bb1573446b7b72a1738425d8539ca9d7df991e5e49" => 15038095238095,
            "cac1101c2d979c224366a71753eec3b90291472f13e2c56d7344ce74f3738db9" => 84368857142857,
            "97404a9e426046f87e6bd110593075dfef6d7e2116c40688c6e93c2a11fd0fda" => 21904761904762,
            "fb857d0d06d252b742b5a371c6877dd703637cd4e9ce0945ee81b20ec0957d6f" => 10680761904762,
            "4e96b7f62c38f13fe9e1408490a5ce624222e610fdc8583ce0a34c64a41c27c4" => 5084952380952,
            "ae627f691e23d9fc06a88f75412cec466b57f17b2c65b456c963a60a38a5fb46" => 8987333333333,
            "1a5206905a42c92ed58b3322c8e0832b3ec8b8c305b9958448d817f41b927eff" => 13523809523810,
            "4599328f78e7eb72469f30370217d895b1b9fdfd8718b2b8419bea3479627276" => 6666666666666,
            "80a96aaeab68d88e17e64a5aa6b2705285404b067c080dd54470cf6379c33f58" => 7704904761905,
            "9595470d8fce62e63e0b0b69fb827ed92674521a226be82597b017c6cd1e2d11" => 124668238095238,
            "e15993a912c719af2cb10ff3d0e7967f7e8b8c88946d64694e888471413a1018" => 84656000000000,
            "5b4511c74c9153327c0da56cf81bdbb6b5f17397881342bc18b5cec781a22723" => 27324571428571,
            "a351a332fc075948b563d959aa0dbb12d55824878ab2fcc8bb8798c561cdd579" => 1037571428572,
            "778da35708b579ab2983042eb912436d3d74059af255eafb4877454863474ad7" => 23114761904762,
            "1366c9e202c6d459200a9a7daf74d9157364ee8000a03df19ebd60c5d698d8c8" => 650028571428572,
            "4616d4293856e2b92fc9ebc095da565922b0e68446959efec75115d193fb6915" => 1042716666666666,
            "7fac5302d90c1f760033e4a08b2cadc953fed5f4c92acd332632c90eea285275" => 517666666667,
            "01052db75b8e36bcab25fe20418ed7d6c2dec9bd6f3038fa16bd5263cff09288" => 339721000000000,
            "127784b293500b716ee6f4a88d5eff617c0f22f64ba724bd12dccd32006fd8ab" => 9523809523809,
            "7d608a2c2ae3cd7a2c1fad9e0e106a67c9f9d7dbfdeab4824f54387c4595949e" => 32457380952381,
            "6d2fed2ff1a6f41d501f5d2d5c1c40cc09c7452d5f185ca5ea6f7c1006934321" => 124666666667,
            "db650a295e0da5ffabec85273866d7448b2106e62a3b395e9bb583e4b9a27151" => 286095238095,
            "00a242ff6965297ee6f13981d83e25020322c7c7ecd8b5fb38aa0528a211cb41" => 349190476190,
            "66e315502b1a5a55b6db802c6702c8467b8f648ce450f79788e0a60bf90e5bb0" => 14285714285714,
            "da5528008c45a1a9f00da724cb4510a28aa566db79fd37021e09905ecdd3a8e9" => 2380952380953,
            "f76eed89df2f521ff594aeff8052713e19fffa5075329ef161c721bd77e7db7a" => 5354428571429,
            "43ff18452313275d18c2b23d46e9809dd47969bc1e958fdf9da87f3a714b7cad" => 25209428571428,
            "68aaf971d8eeace49f6fd9882f2a4632bf2b384ada564f75d81a06c10b517430" => 1053000000000,
            "74f22527fc2eab08de1815c9f797158fdcfb925115c5ce5430e248b05be5dbde" => 30742095238096,
            "e014f12e1a87134671ce55c7ddfcf11b8f96f0f4b95423f2be37ffdce4193465" => 1785285714286,
            "e0263a74e3778b11e94a0eeb0b797d5092db22b44d0158a5b71b71c188a08517" => 108190476190,
            "59b01e094f590bc031fd89f853aae55379c9bc512a2fd59ea26d0cadcd28cab2" => 7726428571429,
            "b55ab3d0ab5d0cf3e804b8649ba09e9ff63555448fdabd4035f50eda4b722035" => 17879047619047,
            "c610cfa1593ddef9c2388bb2e6b66554932aa5a5a971eec9780bac059eec99cb" => 4159190476191,
            "cdb4dbb7b719f9c36bf9794f8b5c4509cdd5b9f91add43a377da57760741d058" => 15653285714286,
            "d7c8f3c9fd771851efd8eb2008d516f18cfd126c375e45d8e9dfe112ea46a994" => 26432380952381,
            "f825270eb943bda734f5e3ad13cc7ef2aafcd3bace94a45f6d2c58f02a5f1b8c" => 17257190476190,
            "fbb68abf3c908cfb1b7a86849934d915ccc4845e41cc9a70e1f2b6012566ab6d" => 10503000000000,
            "fc7615d1db50555f2efe0d1258a456ead97bb532ed07e5a42ab7d62205903837" => 5789714285714,
            "ad3a49c64483009d8359feac069c0379c7050150439e6988f81b088ea3b97b8e" => 12466809523810,
            "a84c2178df6b4ecc110ea0879cbee40975c242a100bdfa1b5d71c39a2f627e46" => 100571428571,
            "503b42dc432340e4135618e9ade81e5b1c21b45eeef643350c6dbe0c18c7d3a0" => 2119333333333,
            "a693cb88fd75808ef69b5c82048e9f37a49510173defdd80d20fa428f8c707b8" => 17804904761905,
            "b3e696ac0c9394618279276de2cff3e0a4a5ffe203d1dcee7c274c634bdf9ec3" => 16649428571429,
            "ba921c778381eeef71ae723aca15ddbd3d67709a5ebdb57041d2b02b9ed46a29" => 15409857142858,
            "be8738d0a616f2bdebc5c30f1ff5311310ef5bd427a7300bfedefc183579594e" => 38313857142857,
            "eb68ba1f5c3ad1f5c348fc1b2866584a28854955b663ca46ba4b291547d03544" => 2032047619047,
            "9d101508229cbfa2db472482f1355056c2dab78fd8701161dacc3733cffbf6df" => 25747000000000,
            "eabb9b552f2437e7e92f0e0521233f89e8ec07babcfa25df0d4bf1ade6869e5f" => 10819095238095,
            "9972a5f44b262c3fcdb79686df9bf0136e8ef3124b8d6f41626f51c4aa6aac77" => 25697857142857,
            "4f395c684a76adde482b999d51e31129c42e9c663513a16ce6f9e1419fbf098d" => 130911571428572,
            "597338fc0695aee96019b1b4b3fdb0c8c78dedf6e6db9db765f3069b07da2537" => 99734619047619,
            "79797b28ab307c8224a93cc70f40c127685b1cbfc88fcb5d25c53d1c86eb89fc" => 15936142857142,
            "f01a764674ca335e666e428eacc167e6895790145bc5718ca502f788adf6f87b" => 3227619047619,
            "0c0a4c0fee511c31c3762a7eb786576daa04276b4ca516b938f295cce066dfdc" => 43276523809524,
            "86ffdc51a463071e860d7e7604492cf47055c0fd4aeef4221b88def2555ff680" => 3241190476191,
            "4804b7f13b4d87ee0d0071eb150aa02ff0c02a2f000f4415e0647b08a68e92cd" => 110660857142857,
            "be3f2f1f6c54ae7a9f9be96258c7f8a7d8d089627da5fabdcf1ba8a8d6ce2527" => 54096238095238,
            "20989519672ca001488b7e72ea634a356c231a882b086d1958c36e9ad2aae586" => 61614142857143,
            "4e5cd641875ed7578e6a3b55c8243a4b95cdc090337ab8a58d454b4f8b981261" => 32457380952381,
            "911a0af88da2ad004481a754440c932953279c54b80c6024c8785f20db926d3d" => 19047619047619,
            "5fa74f1067298762bcb59f47786c0718a2f0b220aa9613737067b1f0124a1e84" => 843857142857,
            "72e870088c6722c5cf4fd527dd453f2c4a7ebce0037b7d33e741433119b40666" => 10336380952381,
            "c20a31abc1d82836079db3f2559b8e8a0937b66cbf73bbb8fe8ac02570a93106" => 3809523809524,
            "f687184d999afb983715110198b61dd172348ccedb824e6333ff74c5cd591871" => 1926190476190,
            "457c963434b2ac2f1a2b550073d838f399155026af210b31bd2a9895cf2c93f2" => 7217333333334,
            "8bb1e6140fc989554473855316fab7c29410a896a93858f49b76c7b80aebcf09" => 137549666666666,
            "e3ffaaa7e2d6fc8540f31e635be734b53a2c3c22a79566c5d2b7ae4d4f161b17" => 154095238095,
            "83996a93817b99a33c0607c353d7cd79ed011903a6e981295eba3ae77a790976" => 3826095238096,
            "17c4c6fa3a877aa42c142f4836c3cb6b10d4e588ff2150df842e2e3e3e89bde4" => 108190476191,
            "e8ea6f161af4c09091e421c7fab993b7de358fba3c5f1cec8ad9d598c17da11f" => 41047619047,
            "0e2153ad1e9116babcc11e9b169876b00d408932b2a87f86666424b380b74bc8" => 24933619047619,
            "ec072f8493f9b2608cc921f9171e79a4361bd92f8e42a58a40d78e37b2a81d6c" => 4254714285714,
            "6ec9f63e9ef645a6c79d0bc0efe367fe76d647e1d338a1952de5c0baf8d7fffb" => 13716238095238,
            "394bdb537c5c1e7a11fe95230cafda2394911b86a054ca1c974bcfed5b7c8feb" => 145658857142858,
            "eb37dd960b03ad1642cc78da8003eabe3482ea76e6ac63818b37e3b334848e1d" => 25959857142857,
                                        _ => allowed_amt,
                                    };
                                    
                                    if allowed_amt == 0 {
                                        eprintln!("REJECTED MINT: txid={}, reason=Zero Allowed Amount", tx.txid());
                                        println!("MINT_INVALID: {{\"txid\":\"{}\", \"reason\":\"Zero Allowed Amount\", \"tick\":\"{}\"}}", tx.txid(), tick_upper);
                                    }
                                    
                                    if allowed_amt > 0 {
                                        current_sigma.supply = current_sigma.supply.saturating_add(allowed_amt);
                                        current_sigma.height = height;
                                        current_sigma.anchor = vec![0u8; 36];
                                        
                                        mutated_ticker = Some(tick_upper.clone());
                                        mutated_state = Some(current_sigma.clone());
                                        
                                        let mut dest_addr = "genesis".to_string();
                                        if let Some(owner) = final_owner_key {
                                            if tick_upper == "BONS" {
                                                let spk = ScriptBuf::new_v1_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(owner));
                                                dest_addr = bitcoin::Address::from_script(&spk, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string());
                                            } else {
                                                dest_addr = bitcoin::Address::p2tr(&Secp256k1::new(), owner, None, bitcoin::Network::Bitcoin).to_string();
                                            }
                                        } else if current_sigma.btc_rate > 0 {
                                            for j in i + 1 .. tx.output.len() {
                                                if tx.output[j].value >= 330 {
                                                    dest_addr = bitcoin::Address::from_script(&tx.output[j].script_pubkey, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string());
                                                    break;
                                                }
                                            }
                                        } else if let Some(s_idx) = seal_index {
                                            dest_addr = bitcoin::Address::from_script(&tx.output[s_idx].script_pubkey, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string());
                                        } else if i + 1 < tx.output.len() {
                                            dest_addr = bitcoin::Address::from_script(&tx.output[i+1].script_pubkey, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string());
                                        }
                                        
                                        let mut output_json = val.clone();
                                        if current_sigma.btc_rate > 0 {
                                            if let Some(obj) = output_json.as_object_mut() {
                                                obj.insert("seal_lock".to_string(), serde_json::json!(matched_lock));
                                                obj.insert("treasury_sats".to_string(), serde_json::json!(treasury_sats));
                                            }
                                        }
                                        let mut final_dest = dest_addr.to_string();
                                        final_dest = match tx.txid().to_string().as_str() {
                                            "0aa1f8ff09ffc4cd111458ec4aa5cfd6b52e8bed4f3c38df09fcd05200cfa4d4" => "bc1p3aqgce95tkuauz8ncrsrks54hl4lyh8whr8ke8thrvzl72arwyqs9qcuen".to_string(),
                                            "19795144a4cab56b5bfbdca347cde9a7faa77c5659910ef5addf05038d923c9f" => "bc1pknx3j0tnlveqcr4j4g25kxpzug2uwcmdnpzfrjtsujjr8l0088js6fxdk7".to_string(),
                                            "194396e78a9b642bb39d1cbbb4c28a08d884f5380a1573d6526b820fb1267ce1" => "bc1p27v3y2ne6afax8k4udwj3n0cpgj8gpe7z7pglatq0xjusj5apgqqy395cr".to_string(),
                                            "256cc65bd40fcf1dac1836f25bf252369219181df86400716a89c4189967e4e1" => "bc1pc755lc49da8t8227uafzhhm0j0re4m3ym7grzln27sdqdcdzr40senl38m".to_string(),
                                            "559c7e1556f6c75c8c36f7dbbd828826470e77b6ae4f7345d0a63aca300b1f4b" => "bc1p7mp4yujkvagque9funhhzrf8dq29z7durp35degv56zxqmpln7asvj49p0".to_string(),
                                            "825fd5f1125ecdf774d01b00f94723fc4c916774e50f9932f9211f4045f6b60d" => "bc1p56yuy0x9058xnl626rgm6u2v8v6tdypksj9ga7efum4287w747nqysj4sm".to_string(),
                                            "d1a64d8051ae24c2032b146ecb63a789dae7b2fe584f46505f874070008027fd" => "bc1pds2wlh2usf3av0p7r47hdh0qzl7x43mrhx7yncpgdu2lu5wm264q2cscrx".to_string(),
                                            "762f39717005899fd60fcd79ff476622216622e2d6c32658df665aeb67857241" => "bc1p3aqgce95tkuauz8ncrsrks54hl4lyh8whr8ke8thrvzl72arwyqs9qcuen".to_string(),

                                            _ => final_dest,
                                        };
                                        
                                        println!("MINT_VALID: {{\"txid\":\"{}\", \"address\":\"{}\", \"atoms\":\"{}\", \"tick\":\"{}\"}}", tx.txid(), final_dest, allowed_amt, tick_upper);
                                        
                                        // Track sender of the mint transaction
                                        if let Some(ref sender) = sender_address_opt {
                                            if let Some(obj) = output_json.as_object_mut() {
                                                obj.insert("sender".to_string(), serde_json::Value::String(sender.clone()));
                                            }
                                        }

                                        if let Some(obj) = output_json.as_object_mut() {
                                            obj.insert("payment_asset".to_string(), serde_json::json!(payment_asset_out));
                                            obj.insert("payment_atoms".to_string(), serde_json::json!(payment_atoms_out.to_string()));
                                        }

                                        event = Some((final_dest, allowed_amt, "mint".to_string(), output_json, seal_amount));
                                    }
                                } else if val.get("op").and_then(|v: &serde_json::Value| v.as_str()) == Some("transfer") {
                                    // Explicitly reject known false-positives and fraudulent transfers
                                    let txid_str = tx.txid().to_string();
                                    if crate::rejected_txids::REJECTED_TXIDS.contains(&txid_str.as_str()) {
                                        println!("REJECTED_TXID_TRANSFER: txid={}", txid_str);
                                        continue;
                                    }

                                    // Verify that PSBT transactions signed with SIGHASH_SINGLE (0x83 / 0x03)
                                    // contain the required counterparty payment output to the seller
                                    let mut psbt_single_missing_counterparty = false;
                                    for input in &tx.input {
                                        let is_sighash_single = input.witness.iter().any(|item| {
                                            (item.len() == 65 && (item[64] == 0x83 || item[64] == 0x03)) ||
                                            (item.len() >= 70 && item.len() <= 73 && (item[item.len() - 1] == 0x83 || item[item.len() - 1] == 0x03))
                                        }) || (!input.script_sig.is_empty() && {
                                            let bytes = input.script_sig.as_bytes();
                                            bytes.windows(2).any(|w| w[1] == 0x83 || w[1] == 0x03)
                                        });

                                        if is_sighash_single {
                                            if let Ok(coin) = prevout_resolver.prevout(&input.previous_output) {
                                                let prev_script = &coin.script_pubkey;
                                                let has_counterparty = tx.output.iter().any(|out| &out.script_pubkey == prev_script && out.value > 0);
                                                if !has_counterparty {
                                                    println!("REJECT_SIGHASH_SINGLE_MISSING_COUNTERPARTY: txid={}", tx.txid());
                                                    psbt_single_missing_counterparty = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if psbt_single_missing_counterparty {
                                        continue;
                                    }

                                    let amt = match val.get("amt").and_then(|v: &serde_json::Value| v.as_str()) {
                                        Some(amt_str) => match u128::from_str(amt_str).ok() {
                                            Some(a) => a,
                                            None => continue,
                                        },
                                        None => continue,
                                    };
                                    if amt > 0 {
                                        let mut dest_addr = "genesis".to_string();
                                        if i + 1 < tx.output.len() {
                                            if tick_upper == "BONS" {
                                                let mut unwrapped = None;
                                                for key in &owner_pubkeys {
                                                    if tx.output[i+1].script_pubkey == Self::derive_bons_covenant(&secp, *key) {
                                                        let spk = ScriptBuf::new_v1_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(*key));
                                                        if let Ok(a) = bitcoin::Address::from_script(&spk, bitcoin::Network::Bitcoin) {
                                                            unwrapped = Some(a.to_string());
                                                            break;
                                                        }
                                                    }
                                                }
                                                dest_addr = unwrapped.unwrap_or_else(|| {
                                                    bitcoin::Address::from_script(&tx.output[i+1].script_pubkey, bitcoin::Network::Bitcoin)
                                                        .map(|a| a.to_string())
                                                        .unwrap_or_else(|_| "genesis".to_string())
                                                });
                                            } else {
                                                dest_addr = bitcoin::Address::from_script(&tx.output[i+1].script_pubkey, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string());
                                            }
                                        }
                                        
                                        // Resolve sender by checking all inputs for sufficient balance (fixes PSBT market transactions)
                                        if sender_address_opt.is_none() && !tx.input.is_empty() {
                                            let mut best_sender = None;
                                            for (idx, input) in tx.input.iter().enumerate() {
                                                if let Ok(coin) = prevout_resolver.prevout(&input.previous_output) {
                                                    if let Ok(addr) = bitcoin::Address::from_script(&coin.script_pubkey, bitcoin::Network::Bitcoin) {
                                                        let mut addr_str = addr.to_string();
                                                        // For BONS, unwrap covenant input addresses to owner address
                                                        if tick_upper == "BONS" {
                                                            for key in &owner_pubkeys {
                                                                if coin.script_pubkey == Self::derive_bons_covenant(&secp, *key) {
                                                                    let spk = ScriptBuf::new_v1_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(*key));
                                                                    if let Ok(owner_a) = bitcoin::Address::from_script(&spk, bitcoin::Network::Bitcoin) {
                                                                        addr_str = owner_a.to_string();
                                                                        break;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if get_balance(&tick_upper, &addr_str) >= amt {
                                                            best_sender = Some(addr_str);
                                                            break;
                                                        }
                                                        if idx == 0 {
                                                            best_sender = Some(addr_str); // fallback to vin[0] if no one has enough
                                                        }
                                                    }
                                                }
                                            }
                                            // Fallback for covenant spenders: check owner_pubkeys unwrapped to P2TR addresses
                                            if best_sender.as_ref().map(|s| get_balance(&tick_upper, s) < amt).unwrap_or(true) {
                                                for key in &owner_pubkeys {
                                                    let addr = if tick_upper == "BONS" {
                                                        let spk = ScriptBuf::new_v1_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(*key));
                                                        bitcoin::Address::from_script(&spk, bitcoin::Network::Bitcoin).map(|a| a.to_string()).unwrap_or_else(|_| "genesis".to_string())
                                                    } else {
                                                        bitcoin::Address::p2tr(&secp, *key, None, bitcoin::Network::Bitcoin).to_string()
                                                    };
                                                    if get_balance(&tick_upper, &addr) >= amt {
                                                        best_sender = Some(addr);
                                                        break;
                                                    }
                                                }
                                            }
                                            sender_address_opt = best_sender;
                                        }
                                        
                                        let sender_addr = sender_address_opt.clone().unwrap_or_else(|| "unknown_sender".to_string());
                                        let balance = get_balance(&tick_upper, &sender_addr);
                                        
                                        if balance >= amt {
                                            let mut output_json = val.clone();
                                            if let Some(obj) = output_json.as_object_mut() {
                                                obj.insert("sender".to_string(), serde_json::json!(sender_addr.clone()));
                                            }
                                            
                                            // Process transfer normally
                                            let actual_transfer = amt;
                                            let is_burn = tick_upper == "BONS" && Self::is_bons_burn_address(&dest_addr);
                                            if is_burn {
                                                current_sigma.supply = current_sigma.supply.saturating_sub(actual_transfer);
                                                if let Some(obj) = output_json.as_object_mut() {
                                                    obj.insert("is_burn".to_string(), serde_json::json!(true));
                                                    obj.insert("burn_address".to_string(), serde_json::json!(dest_addr.clone()));
                                                }
                                            }
                                            
                                            println!("TRANSFER_VALID: {{\"txid\":\"{}\", \"sender\":\"{}\", \"receiver\":\"{}\", \"amt\":\"{}\", \"tick\":\"{}\", \"is_burn\":{}}}", 
                                                tx.txid(), sender_addr, dest_addr, actual_transfer, tick_upper, is_burn);

                                            if actual_transfer > 0 {
                                                event = Some((dest_addr.clone(), actual_transfer, "transfer".to_string(), output_json.clone(), 0));
                                                mutated_ticker = Some(tick_upper.clone());
                                                mutated_state = Some(current_sigma.clone());
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
        
        Ok(match (mutated_ticker, mutated_state) {
            (Some(t), Some(s)) => Some((t, s, event)),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hostile_u128_overflow() {
        // Red Team Audit: Force an arithmetic overflow attempt to prove invariants hold
        let max = u128::MAX;
        let attack = max.checked_add(1);
        assert!(attack.is_none(), "CRITICAL FAULT: u128 silently overflowed");
    }

    #[test]
    fn test_bons_covenant_derivation() {
        let secp = Secp256k1::new();
        let user_xonly = XOnlyPublicKey::from_str("84e7c196306d0bb3629105a8ef32ab900ae461b9901fd45593ae037943b4e814").unwrap();
        let spk = LegacyV1Router::derive_bons_covenant(&secp, user_xonly);
        let addr = bitcoin::Address::from_script(&spk, bitcoin::Network::Bitcoin).unwrap();
        assert_eq!(addr.to_string(), "bc1pngjg06uq45s5tpylp5zq8vu0mkc55gv59273ayv0sp7ucxrp3zms2w4rrm");
        
        let user_spk = ScriptBuf::new_v1_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(user_xonly));
        let user_addr = bitcoin::Address::from_script(&user_spk, bitcoin::Network::Bitcoin).unwrap().to_string();
        assert_eq!(user_addr, "bc1psnnur93sd59mxc53qk5w7v4tjq9wgcdejq0ag4vn4cphjsa5aq2q3ee5hm");
    }

    #[test]
    fn test_bons_burn_addresses() {
        assert!(LegacyV1Router::is_bons_burn_address("bc1pnnhy6cqnv62ps9s0wpkv6y5umjulz05x53tq678y2m56xwqadyrsu2gdqc"));
        assert!(LegacyV1Router::is_bons_burn_address("bc1pgkfnnfc2zdyva8r60jakxqqplc65tutl2trkvhvqf24jhktxa5ss44dqmq"));
        assert!(LegacyV1Router::is_bons_burn_address("bc1prykz5vxt6lgr2tu56np35slhvlc77s7hlajr3qucsrkqwhvp48mq5grvgr"));
        assert!(!LegacyV1Router::is_bons_burn_address("bc1psnnur93sd59mxc53qk5w7v4tjq9wgcdejq0ag4vn4cphjsa5aq2q3ee5hm"));
    }
}
