use crate::state::RejectReason;

pub trait TusmTxEnv {
    fn num_inputs(&self) -> u32;
    fn num_outputs(&self) -> u32;
    fn output_value(&self, index: u32) -> Result<u64, RejectReason>;
    fn output_script_pubkey(&self, index: u32) -> Result<&[u8], RejectReason>;
    fn input_sequence(&self, index: u32) -> Result<u32, RejectReason>;
    fn input_utxos_hash(&self) -> [u8; 32];
}

#[derive(Debug, Clone)]
pub struct MockTxEnv {
    pub num_inputs: u32,
    pub num_outputs: u32,
    pub output_values: Vec<u64>,
    pub output_scripts: Vec<Vec<u8>>,
    pub input_sequences: Vec<u32>,
    pub utxos_hash: [u8; 32],
}

impl TusmTxEnv for MockTxEnv {
    fn num_inputs(&self) -> u32 {
        self.num_inputs
    }

    fn num_outputs(&self) -> u32 {
        self.num_outputs
    }

    fn output_value(&self, index: u32) -> Result<u64, RejectReason> {
        self.output_values
            .get(index as usize)
            .copied()
            .ok_or(RejectReason::InvalidTopology)
    }

    fn output_script_pubkey(&self, index: u32) -> Result<&[u8], RejectReason> {
        self.output_scripts
            .get(index as usize)
            .map(|s| s.as_slice())
            .ok_or(RejectReason::InvalidTopology)
    }

    fn input_sequence(&self, index: u32) -> Result<u32, RejectReason> {
        self.input_sequences
            .get(index as usize)
            .copied()
            .ok_or(RejectReason::InvalidInputContext)
    }

    fn input_utxos_hash(&self) -> [u8; 32] {
        self.utxos_hash
    }
}
