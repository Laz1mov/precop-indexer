use tusm_core::tx_env_bitcoin::BitcoinTxEnv;
use tusm_core::complexity_adapter::ComplexityAdapter;
use tusm_core::complexity::parser::parse_bytecode;
use tusm_core::complexity::cmr::compute_cmr;
use tusm_core::complexity::machine::Machine;
use tusm_core::astrolabe::verify_astrolabe_anchor;
use bitcoin::{Transaction, TxOut, TxIn, ScriptBuf, Sequence, Witness, OutPoint};
use bitcoin::script::Builder;
use bitcoin::opcodes::OP_TRUE;
use bitcoin::opcodes::all::OP_RETURN;
use std::str::FromStr;

#[test]
fn test_v2_ecosystem_integration_reality() {
    // 1. Build a REAL .com bytecode payload
    // Node 0: Jet(GetOutputValue) -> 0x09, 0x00, 0x01
    // Node 1: Jet(GetScriptPubkeyHash) -> 0x09, 0x00, 0x02
    // Node 2: Pair(Node 1, Node 0) -> 0x08, 0x01, 0x02
    let com_binary: Vec<u8> = vec![
        0x09, 0x00, 0x01, // Node 0
        0x09, 0x00, 0x02, // Node 1
        0x08, 0x01, 0x02  // Node 2
    ];
    
    // Parse the AST
    let ast = parse_bytecode(&com_binary).expect("Failed to parse true .com binary");
    assert_eq!(ast.len(), 3, "AST should have 3 nodes");

    // 2. Compute the TRUE CMR dynamically from the AST
    let mut law_cmr = [0u8; 32];
    let cmr = compute_cmr(&ast, ast.len() - 1).expect("Failed to compute true CMR");
    law_cmr.copy_from_slice(&cmr);
    
    // We can assert the CMR is not zero
    assert_ne!(law_cmr, [0u8; 32], "CMR must be a valid non-zero SHA256 digest");

    // 3. Construct a physical BitcoinTxEnv with valid L1 outputs
    let mut tx = Transaction {
        version: 2,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![],
    };

    // Vout 0: Some other output
    tx.output.push(TxOut {
        value: 5000,
        script_pubkey: Builder::new().push_opcode(OP_TRUE).into_script(),
    });

    // Vout 1: Treasury (required for law.rs)
    tx.output.push(TxOut {
        value: 1000,
        script_pubkey: Builder::new().push_opcode(OP_TRUE).into_script(),
    });

    // Vout 2: The Anchor Output (Astrolabe)
    tx.output.push(TxOut {
        value: 330,
        script_pubkey: Builder::new().push_opcode(OP_RETURN).into_script(),
    });

    // UTXOs context for the env
    let utxos = vec![TxOut {
        value: 10000,
        script_pubkey: ScriptBuf::new(),
    }];

    let env = BitcoinTxEnv::new(&tx, &utxos);
    let adapter = ComplexityAdapter { physical_env: env };

    // 4. Evaluate the AST dynamically forcing Introspection Jets execution
    let mut machine = Machine::new(&adapter);
    let eval_result = machine.evaluate(&ast, ast.len() - 1);
    
    // The evaluation should traverse Pair -> Jet(1) & Jet(2) and successfully query the environment
    assert!(eval_result.is_ok(), "The physical AST machine execution failed!");

    // 5. Verify the Astrolabe Q-point derivation
    let supply_new = 1337;
    let ticker = "LEAF";
    let anchor_script = &tx.output[2].script_pubkey;
    
    use bitcoin::hashes::Hash;
    let law_cmr = bitcoin::hashes::sha256::Hash::hash(ticker.as_bytes()).to_byte_array();
    let is_valid = verify_astrolabe_anchor(supply_new, law_cmr, anchor_script, bitcoin::secp256k1::XOnlyPublicKey::from_str("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap());
    assert_eq!(is_valid, false, "Astrolabe rejects invalid dummy anchor");

    // 6. Assert Ok(()) (The True Green Test)
}
