pub trait TxEnv {
    fn get_output_value(&self, idx: u32) -> Option<u64>;
    fn get_script_pubkey_hash(&self, idx: u32) -> Option<[u8; 32]>;
    fn get_current_sequence(&self, idx: u32) -> Option<u32>;
    fn jet_compute_merkle_root(&self, leaf: [u8; 32], proof: &[u8]) -> Option<[u8; 32]>;
}
