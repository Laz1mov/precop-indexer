use crate::state::RejectReason;
use bitcoin::hashes::{sha256, Hash, HashEngine};
use bitcoin::{Transaction, TxOut};

/// The Physical Transaction Environment
pub struct BitcoinTxEnv<'a> {
    pub tx: &'a Transaction,
    pub input_utxos: &'a [TxOut], // Required for strict input introspection
}

impl<'a> BitcoinTxEnv<'a> {
    pub fn new(tx: &'a Transaction, input_utxos: &'a [TxOut]) -> Self {
        Self { tx, input_utxos }
    }

    pub fn jet_verify_topology(&self) -> Result<(), RejectReason> {
        if self.tx.output.len() < 3 {
            return Err(RejectReason::InvalidTopology);
        }
        if !self.tx.output[0].script_pubkey.is_op_return() {
            return Err(RejectReason::InvalidSpeech);
        }
        if self.tx.output.get(2).map(|o| o.value).unwrap_or(0) != 330 {
            return Err(RejectReason::InvalidAnchor);
        }
        Ok(())
    }

    pub fn jet_output_value(&self, index: usize) -> Result<u64, RejectReason> {
        self.tx
            .output
            .get(index)
            .map(|out| out.value)
            .ok_or(RejectReason::InvalidTopology)
    }

    pub fn jet_output_script_pubkey_hash(&self, index: usize) -> Result<[u8; 32], RejectReason> {
        let script = &self
            .tx
            .output
            .get(index)
            .ok_or(RejectReason::InvalidTopology)?
            .script_pubkey;
        Ok(sha256::Hash::hash(script.as_bytes()).to_byte_array())
    }

    pub fn jet_current_sequence(&self, input_index: usize) -> Result<u32, RejectReason> {
        let input = self
            .tx
            .input
            .get(input_index)
            .ok_or(RejectReason::InvalidInputContext)?;
        let seq = input.sequence.to_consensus_u32();

        // Masks consensus stricts (BIP-68)
        const DISABLE_FLAG: u32 = 1 << 31;
        const TYPE_FLAG: u32 = 1 << 22;
        const VALUE_MASK: u32 = 0x0000FFFF;

        if (seq & DISABLE_FLAG) != 0 || (seq & TYPE_FLAG) != 0 {
            return Err(RejectReason::InvalidCsvCondition);
        }
        Ok(seq & VALUE_MASK)
    }

    pub fn jet_input_witness(&self, input_index: usize, element_index: usize) -> Result<&[u8], RejectReason> {
        let input = self
            .tx
            .input
            .get(input_index)
            .ok_or(RejectReason::InvalidInputContext)?;
        
        let elem = input
            .witness
            .nth(element_index)
            .ok_or(RejectReason::InvalidInputContext)?;
            
        Ok(elem)
    }

    pub fn jet_input_utxos_hash(&self) -> [u8; 32] {
        let mut engine = sha256::HashEngine::default();
        for utxo in self.input_utxos {
            engine.input(&utxo.value.to_le_bytes());
            engine.input(utxo.script_pubkey.as_bytes());
        }
        sha256::Hash::from_engine(engine).to_byte_array()
    }
}
