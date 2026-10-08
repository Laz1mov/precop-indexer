//! CRC-21 Vault v2 (OpenGarden "Merkel Koala" layout) — pure, deterministic derivation.
//!
//! Proof chain implemented here (every step is recomputable by any third party):
//!
//! ```text
//! S            = 220-byte vault state carried in the ord envelope metadata (tag 5, CBOR key
//!                "crc21_vault_v1" | "crc21_vault_v2")
//! state_root   = H_tag("crc-21/VaultState/v2", S)
//! C            = H_tag("crc-21/VaultCommitment/v2", S[1..42] || CMR || state_root)
//! claim leaf   = PUSH32(C) DROP <csv_claim>  CSV DROP SHA256 PUSH32(h_A) EQUALVERIFY PUSH32(T) CHECKSIG
//! owner leaf   = PUSH32(C) DROP <csv_owner>  CSV DROP PUSH32(O) CHECKSIG
//! public leaf  = PUSH32(C) DROP <csv_public> CSV
//! MAST         = [claim (depth 1), [owner, public] (depth 2)] on the BIP-341 NUMS key
//! Q            = NUMS + H_TapTweak(NUMS || merkle_root)·G  ->  P2TR(Q) must equal the vault output
//! H_tag(x)     = sha256(sha256(tag) || sha256(tag) || x)   (BIP-340 tagged hash)
//! ```
//!
//! S layout (offsets in bytes). Only fields used by the proof chain are interpreted; every
//! other byte range is exposed raw (`opaque_78_154`) — never guessed.
//!
//! | range    | field                                  |
//! |----------|----------------------------------------|
//! | 0        | version (must be 0x02)                 |
//! | 1        | network (0x00 = mainnet)               |
//! | 2..34    | collection txid (internal byte order)  |
//! | 34..38   | collection vout (LE)                   |
//! | 38..42   | item number (LE)                       |
//! | 42..44   | csv_claim (LE)                         |
//! | 44..46   | csv_public (LE)                        |
//! | 46..78   | T  (x-only claim key)                  |
//! | 78..154  | opaque (raw)                           |
//! | 154..186 | h_A (sha256 claim-secret hash)         |
//! | 186..218 | O  (x-only owner key)                  |
//! | 218..220 | csv_owner (LE)                         |
//!
//! The claim-program CMR is NOT carried in S nor in the envelope: it is a protocol constant,
//! enumerated in [`KNOWN_CLAIM_CMRS`]. A vault is accepted only if one known CMR reproduces the
//! exact on-chain scriptPubKey.

use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
use bitcoin::taproot::{LeafVersion, TapLeafHash, TapTweakHash, TaprootBuilder};
use bitcoin::ScriptBuf;
use std::str::FromStr;

pub const VAULT_STATE_LEN: usize = 220;
pub const VAULT_STATE_VERSION: u8 = 0x02;
pub const NETWORK_MAINNET: u8 = 0x00;

pub const TAG_VAULT_STATE: &str = "crc-21/VaultState/v2";
pub const TAG_VAULT_COMMITMENT: &str = "crc-21/VaultCommitment/v2";

/// BIP-341 NUMS point (H = lift_x(sha256(uncompressed G))).
pub const NUMS_HEX: &str = "50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0";

/// Claim-program CMRs known to the indexer. Order matters only for reporting.
/// Source: OpenGarden Koala mainnet reveal 2409bfb8…7edc (#2305), recomputed end-to-end.
pub const KNOWN_CLAIM_CMRS: &[(&str, [u8; 32])] = &[
    (
        "opengarden-koala-claim-420",
        [
            0xa9, 0x29, 0xf3, 0x1f, 0x02, 0x92, 0x22, 0x0b, 0x70, 0x8f, 0x91, 0x69, 0xd3, 0xd7, 0x5d, 0x55,
            0x3f, 0x53, 0xe6, 0x76, 0xd7, 0xf7, 0xbd, 0x8c, 0x78, 0x8c, 0x82, 0xbf, 0x24, 0xc7, 0x97, 0xaa,
        ],
    ),
    (
        "opengarden-koala-claim-2100",
        [
            0xea, 0xd1, 0xe4, 0xe9, 0xca, 0xee, 0xe9, 0xad, 0x38, 0x45, 0x97, 0x1f, 0xf8, 0x49, 0x68, 0x20,
            0x0b, 0x26, 0x21, 0x41, 0xc2, 0x40, 0x36, 0xd7, 0x4d, 0xbf, 0x81, 0xb9, 0x35, 0x5f, 0xcb, 0x54,
        ],
    ),
    (
        "opengarden-koala-claim-5000",
        [
            0xb9, 0xe8, 0x63, 0xdf, 0xf8, 0xf2, 0xa2, 0x33, 0x4d, 0x1b, 0x0d, 0x9e, 0xf8, 0xf9, 0x57, 0x96,
            0x6a, 0xd8, 0x4a, 0x27, 0xae, 0xd7, 0x66, 0x4d, 0x6f, 0x4e, 0xcb, 0xdb, 0x98, 0xaa, 0x71, 0x71,
        ],
    ),
    (
        "opengarden-koala-claim-v2",
        [
            0xb9, 0xe8, 0x63, 0xdf, 0xf8, 0xf2, 0xa2, 0x33, 0x4d, 0x1b, 0x0d, 0x9e, 0xf8, 0xf9, 0x57, 0x96,
            0x6a, 0xd8, 0x4a, 0x27, 0xae, 0xd7, 0x66, 0x4d, 0x6f, 0x4e, 0xcb, 0xdb, 0x98, 0xaa, 0x71, 0x71,
        ],
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultStateError {
    BadLength(usize),
    BadVersion(u8),
    BadNetwork(u8),
    BadClaimKey,
    BadOwnerKey,
}

impl std::fmt::Display for VaultStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadLength(n) => write!(f, "state_len_{}_ne_220", n),
            Self::BadVersion(v) => write!(f, "state_version_0x{:02x}_unsupported", v),
            Self::BadNetwork(n) => write!(f, "state_network_0x{:02x}_not_mainnet", n),
            Self::BadClaimKey => write!(f, "claim_key_T_not_on_curve"),
            Self::BadOwnerKey => write!(f, "owner_key_O_not_on_curve"),
        }
    }
}

/// BIP-340 tagged hash.
pub fn tagged_hash(tag: &str, msg: &[u8]) -> [u8; 32] {
    let t = sha256::Hash::hash(tag.as_bytes());
    let mut e = sha256::Hash::engine();
    e.input(t.as_byte_array());
    e.input(t.as_byte_array());
    e.input(msg);
    sha256::Hash::from_engine(e).to_byte_array()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultStateV2 {
    pub raw: Vec<u8>,
    pub version: u8,
    pub network: u8,
    /// Display-order (RPC) txid hex of the collection inscription.
    pub collection_txid: String,
    pub collection_vout: u32,
    pub item_number: u32,
    pub csv_claim: u16,
    pub csv_public: u16,
    pub csv_owner: u16,
    pub claim_key_t: XOnlyPublicKey,
    pub owner_key_o: XOnlyPublicKey,
    pub claim_hash_h_a: [u8; 32],
    pub dest_script_len: usize,
}

fn le16(b: &[u8]) -> u16 { u16::from_le_bytes([b[0], b[1]]) }
fn le32(b: &[u8]) -> u32 { u32::from_le_bytes([b[0], b[1], b[2], b[3]]) }

impl VaultStateV2 {
    pub fn parse(s: &[u8]) -> Result<Self, VaultStateError> {
        if s.len() < 120 {
            return Err(VaultStateError::BadLength(s.len()));
        }
        if s[0] != VAULT_STATE_VERSION {
            return Err(VaultStateError::BadVersion(s[0]));
        }
        if s[1] != NETWORK_MAINNET {
            return Err(VaultStateError::BadNetwork(s[1]));
        }
        let m = s[119] as usize;
        let expected_len = 120 + m + if s[0] == 2 { 66 } else { 0 };
        if s.len() != expected_len {
            return Err(VaultStateError::BadLength(s.len()));
        }
        let e_idx = 120 + m;
        let mut txid = s[2..34].to_vec();
        txid.reverse();
        let claim_key_t = XOnlyPublicKey::from_slice(&s[46..78]).map_err(|_| VaultStateError::BadClaimKey)?;
        let mut h_a = [0u8; 32];
        h_a.copy_from_slice(&s[e_idx..e_idx + 32]);
        let owner_key_o = XOnlyPublicKey::from_slice(&s[e_idx + 32..e_idx + 64]).map_err(|_| VaultStateError::BadOwnerKey)?;
        let csv_owner = le16(&s[e_idx + 64..e_idx + 66]);

        Ok(Self {
            raw: s.to_vec(),
            version: s[0],
            network: s[1],
            collection_txid: hex::encode(txid),
            collection_vout: le32(&s[34..38]),
            item_number: le32(&s[38..42]),
            csv_claim: le16(&s[42..44]),
            csv_public: le16(&s[44..46]),
            csv_owner,
            claim_key_t,
            owner_key_o,
            claim_hash_h_a: h_a,
            dest_script_len: m,
        })
    }

    /// Ordinals inscription id of the collection (`<txid>i<vout>`).
    pub fn collection_id(&self) -> String {
        format!("{}i{}", self.collection_txid, self.collection_vout)
    }

    pub fn opaque_78_154(&self) -> &[u8] {
        let end = 120 + self.dest_script_len;
        if self.raw.len() >= end && end >= 78 {
            &self.raw[78..end]
        } else {
            &[]
        }
    }

    pub fn state_root(&self) -> [u8; 32] {
        tagged_hash(TAG_VAULT_STATE, &self.raw)
    }

    pub fn commitment(&self, cmr: &[u8; 32]) -> [u8; 32] {
        let root = self.state_root();
        let mut m = Vec::with_capacity(41 + 32 + 32);
        m.extend_from_slice(&self.raw[1..42]);
        m.extend_from_slice(cmr);
        m.extend_from_slice(&root);
        tagged_hash(TAG_VAULT_COMMITMENT, &m)
    }
}

fn push32(b: &[u8; 32]) -> bitcoin::script::PushBytesBuf {
    bitcoin::script::PushBytesBuf::from(*b)
}

/// claim = PUSH32(C) DROP <csv> CSV DROP SHA256 PUSH32(h_A) EQUALVERIFY PUSH32(T) CHECKSIG
pub fn claim_leaf(c: &[u8; 32], csv: u16, h_a: &[u8; 32], t: &XOnlyPublicKey) -> ScriptBuf {
    use bitcoin::opcodes::all::*;
    bitcoin::script::Builder::new()
        .push_slice(push32(c))
        .push_opcode(OP_DROP)
        .push_int(csv as i64)
        .push_opcode(OP_CSV)
        .push_opcode(OP_DROP)
        .push_opcode(OP_SHA256)
        .push_slice(push32(h_a))
        .push_opcode(OP_EQUALVERIFY)
        .push_x_only_key(t)
        .push_opcode(OP_CHECKSIG)
        .into_script()
}

/// owner = PUSH32(C) DROP <csv> CSV DROP PUSH32(O) CHECKSIG
pub fn owner_leaf(c: &[u8; 32], csv: u16, o: &XOnlyPublicKey) -> ScriptBuf {
    use bitcoin::opcodes::all::*;
    bitcoin::script::Builder::new()
        .push_slice(push32(c))
        .push_opcode(OP_DROP)
        .push_int(csv as i64)
        .push_opcode(OP_CSV)
        .push_opcode(OP_DROP)
        .push_x_only_key(o)
        .push_opcode(OP_CHECKSIG)
        .into_script()
}

/// public = PUSH32(C) DROP <csv> CSV
pub fn public_leaf(c: &[u8; 32], csv: u16) -> ScriptBuf {
    use bitcoin::opcodes::all::*;
    bitcoin::script::Builder::new()
        .push_slice(push32(c))
        .push_opcode(OP_DROP)
        .push_int(csv as i64)
        .push_opcode(OP_CSV)
        .into_script()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedLeaf {
    pub label: String,
    pub depth: u8,
    pub csv: u16,
    pub script: ScriptBuf,
    pub leaf_hash: [u8; 32],
    /// Serialized BIP-341 control block (inclusion proof of the leaf in Q).
    pub control_block: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedVault {
    pub cmr_label: String,
    pub cmr: [u8; 32],
    pub state_root: [u8; 32],
    pub commitment: [u8; 32],
    pub leaves: Vec<DerivedLeaf>, // [claim, owner, public]
    pub merkle_root: [u8; 32],
    pub tweak: [u8; 32],
    pub internal_key: XOnlyPublicKey,
    pub output_key: XOnlyPublicKey,
    pub output_key_parity_odd: bool,
    pub script_pubkey: ScriptBuf,
    pub address: String,
}

pub fn nums() -> XOnlyPublicKey {
    XOnlyPublicKey::from_str(NUMS_HEX).expect("NUMS constant")
}

pub fn derive_vault(state: &VaultStateV2, cmr_label: &str, cmr: &[u8; 32]) -> DerivedVault {
    let secp = Secp256k1::verification_only();
    let internal = nums();
    let state_root = state.state_root();
    let c = state.commitment(cmr);

    let claim = claim_leaf(&c, state.csv_claim, &state.claim_hash_h_a, &state.claim_key_t);
    let owner = owner_leaf(&c, state.csv_owner, &state.owner_key_o);
    let public = public_leaf(&c, state.csv_public);

    let spend = TaprootBuilder::new()
        .add_leaf(1, claim.clone()).expect("depth 1")
        .add_leaf(2, owner.clone()).expect("depth 2")
        .add_leaf(2, public.clone()).expect("depth 2")
        .finalize(&secp, internal)
        .expect("complete tree");
    let merkle_root = spend.merkle_root().expect("script tree");
    let tweak = TapTweakHash::from_key_and_tweak(internal, Some(merkle_root));
    let (tweaked, parity) = internal.tap_tweak(&secp, Some(merkle_root));
    debug_assert_eq!(tweaked.to_inner(), spend.output_key().to_inner());
    let spk = ScriptBuf::new_v1_p2tr_tweaked(spend.output_key());
    let address = bitcoin::Address::p2tr_tweaked(spend.output_key(), bitcoin::Network::Bitcoin).to_string();

    let mk = |label: String, depth: u8, csv: u16, s: ScriptBuf| {
        let cb = spend
            .control_block(&(s.clone(), LeafVersion::TapScript))
            .expect("leaf in tree")
            .serialize();
        DerivedLeaf {
            label,
            depth,
            csv,
            leaf_hash: TapLeafHash::from_script(&s, LeafVersion::TapScript).to_byte_array(),
            script: s,
            control_block: cb,
        }
    };
    let leaves = vec![
        mk(format!("CLAIM_{}", state.csv_claim), 1, state.csv_claim, claim),
        mk(format!("OWNER_RESCUE_{}", state.csv_owner), 2, state.csv_owner, owner),
        mk(format!("PUBLIC_RECOVERY_{}", state.csv_public), 2, state.csv_public, public),
    ];

    DerivedVault {
        cmr_label: cmr_label.to_string(),
        cmr: *cmr,
        state_root,
        commitment: c,
        leaves,
        merkle_root: merkle_root.to_byte_array(),
        tweak: tweak.to_byte_array(),
        internal_key: internal,
        output_key: spend.output_key().to_inner(),
        output_key_parity_odd: parity == bitcoin::secp256k1::Parity::Odd,
        script_pubkey: spk,
        address,
    }
}

/// Try every known CMR; return the derivation whose scriptPubKey equals `expected_spk`.
pub fn match_known_cmr(state: &VaultStateV2, expected_spk: &bitcoin::Script) -> Option<DerivedVault> {
    KNOWN_CLAIM_CMRS
        .iter()
        .map(|(label, cmr)| derive_vault(state, label, cmr))
        .find(|d| d.script_pubkey.as_script() == expected_spk)
}

impl DerivedVault {
    /// Canonical JSON proof object (hex everywhere, display-order txids).
    pub fn proof_json(&self, state: &VaultStateV2) -> serde_json::Value {
        serde_json::json!({
            "scheme": "crc21-vault-v2",
            "tags": { "state": TAG_VAULT_STATE, "commitment": TAG_VAULT_COMMITMENT },
            "state_hex": hex::encode(&state.raw),
            "state": {
                "version": state.version,
                "network": state.network,
                "collection_id": state.collection_id(),
                "item_number": state.item_number,
                "csv_claim": state.csv_claim,
                "csv_owner": state.csv_owner,
                "csv_public": state.csv_public,
                "claim_key_t": state.claim_key_t.to_string(),
                "owner_key_o": state.owner_key_o.to_string(),
                "claim_hash_h_a": hex::encode(state.claim_hash_h_a),
                "opaque_78_154": hex::encode(state.opaque_78_154()),
            },
            "cmr_label": self.cmr_label,
            "cmr": hex::encode(self.cmr),
            "state_root": hex::encode(self.state_root),
            "commitment": hex::encode(self.commitment),
            "internal_key": self.internal_key.to_string(),
            "leaves": self.leaves.iter().map(|l| serde_json::json!({
                "label": l.label,
                "depth": l.depth,
                "csv": l.csv,
                "script_hex": hex::encode(l.script.as_bytes()),
                "leaf_hash": hex::encode(l.leaf_hash),
                "control_block": hex::encode(&l.control_block),
            })).collect::<Vec<_>>(),
            "merkle_root": hex::encode(self.merkle_root),
            "tweak": hex::encode(self.tweak),
            "output_key": self.output_key.to_string(),
            "output_key_parity_odd": self.output_key_parity_odd,
            "script_pubkey": hex::encode(self.script_pubkey.as_bytes()),
            "address": self.address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Merkel Koala #2305 — reveal 2409bfb802486c17b46a8324e94fa243740dbbc8cf485bfcd0fb05d3495f7edc, height 969160.
    pub const KOALA_2305_S: &str = "0200f9b31aed5f7cc86eff6f6a13d3dccd0e5839a0d676e958684bd2aaebb5583e0d0000000001090000881308527b20f23162982bc0e7888bfd060135ee53bb93bffec051a5d96fe46cf38dcf760100000000000000000000000000000000000000000000000000000000000000002202000000000000225120f702b8764e9fcd751ea6eb2468bbe24d34f5396c35d2c61fdb08fee3112d3d0792609d0f51e36b24c74d34fec65976e20f1a497a267df317d871c669f1f2be11f702b8764e9fcd751ea6eb2468bbe24d34f5396c35d2c61fdb08fee3112d3d071027";
    const KOALA_ADDR: &str = "bc1pmx7s6ty3th2jxa0t7yn5wn8mhrp6cg0pj4kt7fh235k7z3a4tm6qp3yqr9";

    fn s() -> VaultStateV2 { VaultStateV2::parse(&hex::decode(KOALA_2305_S).unwrap()).unwrap() }

    #[test]
    fn parse_fields() {
        let st = s();
        assert_eq!(st.item_number, 2305);
        assert_eq!(st.csv_claim, 5000);
        assert_eq!(st.csv_public, 21000);
        assert_eq!(st.csv_owner, 10000);
        assert_eq!(st.collection_vout, 0);
        assert!(st.collection_txid.starts_with("0d3e58b5"));
        assert!(st.collection_txid.ends_with("b3f9"));
        assert!(st.claim_key_t.to_string().starts_with("7b20f231"));
        assert!(st.owner_key_o.to_string().starts_with("f702b876"));
        assert!(hex::encode(st.claim_hash_h_a).starts_with("92609d0f"));
    }

    #[test]
    fn golden_state_root_and_commitment() {
        let st = s();
        let cmr_5000 = &KNOWN_CLAIM_CMRS.iter().find(|(name, _)| *name == "opengarden-koala-claim-5000").unwrap().1;
        assert_eq!(hex::encode(st.state_root()), "57f949eafd88ae7ecc116cf72cedfa978ce94712a853d9cadcc4aef9d3edb142");
        assert_eq!(hex::encode(st.commitment(cmr_5000)), "4a660567a7e8eb16138ca1dafa81c719b21e19a345e928bed53e12622c1a0f6a");
    }

    #[test]
    fn golden_tree_and_address() {
        let st = s();
        let cmr_entry = KNOWN_CLAIM_CMRS.iter().find(|(name, _)| *name == "opengarden-koala-claim-5000").unwrap();
        let d = derive_vault(&st, cmr_entry.0, &cmr_entry.1);
        assert!(hex::encode(d.leaves[0].leaf_hash).starts_with("2fb45a91"));
        assert!(hex::encode(d.leaves[1].leaf_hash).starts_with("7b0bedf6"));
        assert!(hex::encode(d.leaves[2].leaf_hash).starts_with("00992fa5"));
        assert!(hex::encode(d.merkle_root).starts_with("463facd8"));
        assert!(hex::encode(d.tweak).starts_with("598d468f"));
        assert!(d.output_key.to_string().starts_with("d9bd0d2c"));
        assert_eq!(d.address, KOALA_ADDR);
        // minimal CScriptNum: 5000 -> 02 8813
        assert!(hex::encode(d.leaves[0].script.as_bytes()).contains("75028813b275"));
        assert_eq!(d.leaves[0].label, "CLAIM_5000");
        assert_eq!(d.leaves[1].label, "OWNER_RESCUE_10000");
        assert_eq!(d.leaves[2].label, "PUBLIC_RECOVERY_21000");
        // control block: depth-1 leaf carries 1 sibling hash, depth-2 leaves carry 2.
        assert_eq!(d.leaves[0].control_block.len(), 33 + 32);
        assert_eq!(d.leaves[1].control_block.len(), 33 + 64);
        let spk = bitcoin::Address::from_str(KOALA_ADDR).unwrap().assume_checked().script_pubkey();
        assert!(match_known_cmr(&st, &spk).is_some());
    }

    #[test]
    fn rejects_tampering() {
        let mut raw = hex::decode(KOALA_2305_S).unwrap();
        let spk = bitcoin::Address::from_str(KOALA_ADDR).unwrap().assume_checked().script_pubkey();
        raw[100] ^= 1; // opaque byte: still part of state_root -> address must change
        let st = VaultStateV2::parse(&raw).unwrap();
        assert!(match_known_cmr(&st, &spk).is_none());

        assert_eq!(VaultStateV2::parse(&raw[..219]), Err(VaultStateError::BadLength(219)));
        let mut v = hex::decode(KOALA_2305_S).unwrap(); v[0] = 1;
        assert_eq!(VaultStateV2::parse(&v), Err(VaultStateError::BadVersion(1)));
        let mut n = hex::decode(KOALA_2305_S).unwrap(); n[1] = 1;
        assert_eq!(VaultStateV2::parse(&n), Err(VaultStateError::BadNetwork(1)));
    }

    #[test]
    fn tagged_hash_vector() {
        // BIP-340 style: independent of any vault data.
        let a = tagged_hash("TapLeaf", b"");
        let t = sha256::Hash::hash(b"TapLeaf");
        let mut m = t.to_byte_array().to_vec(); m.extend_from_slice(t.as_byte_array());
        assert_eq!(a, sha256::Hash::hash(&m).to_byte_array());
    }
}
