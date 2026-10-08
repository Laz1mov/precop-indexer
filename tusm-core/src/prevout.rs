use bitcoin::{OutPoint, ScriptBuf};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coin {
    pub value: u64,
    pub script_pubkey: ScriptBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrevoutError {
    MissingPrevout(OutPoint),
    ProviderError(String),
}

pub trait PrevoutProvider {
    fn lookup(&self, outpoint: OutPoint) -> Result<Coin, PrevoutError>;
    
    fn batch_lookup(&self, outpoints: &[OutPoint]) -> Result<HashMap<OutPoint, Coin>, PrevoutError> {
        let mut result = HashMap::new();
        for &outpoint in outpoints {
            let coin = self.lookup(outpoint)?;
            result.insert(outpoint, coin);
        }
        Ok(result)
    }
}

pub trait PrevoutResolver {
    fn prevout(&self, outpoint: &OutPoint) -> Result<&Coin, PrevoutError>;
}

impl PrevoutResolver for HashMap<OutPoint, Coin> {
    fn prevout(&self, outpoint: &OutPoint) -> Result<&Coin, PrevoutError> {
        self.get(outpoint).ok_or(PrevoutError::MissingPrevout(*outpoint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Txid;
    use std::str::FromStr;

    #[test]
    fn test_coin_struct() {
        let spk = ScriptBuf::from_hex("512067e6fefb7b9d5af7c4673cb6f3bb0bfa5e789912dfec39635d95a4d259ca393e").unwrap();
        let coin = Coin {
            value: 330,
            script_pubkey: spk.clone(),
        };
        assert_eq!(coin.value, 330);
        assert_eq!(coin.script_pubkey, spk);
    }

    #[test]
    fn test_prevout_error() {
        let txid = Txid::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let outpoint = OutPoint { txid, vout: 0 };
        
        let err = PrevoutError::MissingPrevout(outpoint);
        match err {
            PrevoutError::MissingPrevout(op) => assert_eq!(op, outpoint),
            _ => panic!("Wrong error variant"),
        }
    }
    
    // Mock Provider for future tests
    struct MockPrevoutProvider {
        cache: HashMap<OutPoint, Coin>,
    }
    
    impl PrevoutProvider for MockPrevoutProvider {
        fn lookup(&self, outpoint: OutPoint) -> Result<Coin, PrevoutError> {
            self.cache.get(&outpoint).cloned().ok_or(PrevoutError::MissingPrevout(outpoint))
        }
    }

    #[test]
    fn test_mock_provider() {
        let txid = Txid::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let outpoint = OutPoint { txid, vout: 0 };
        let coin = Coin {
            value: 1000,
            script_pubkey: ScriptBuf::new(),
        };
        
        let mut cache = HashMap::new();
        cache.insert(outpoint, coin.clone());
        let provider = MockPrevoutProvider { cache };
        
        // Test success
        let result = provider.lookup(outpoint).unwrap();
        assert_eq!(result, coin);
        
        // Test missing
        let missing_outpoint = OutPoint { txid, vout: 1 };
        let err = provider.lookup(missing_outpoint).unwrap_err();
        assert_eq!(err, PrevoutError::MissingPrevout(missing_outpoint));
    }
}
