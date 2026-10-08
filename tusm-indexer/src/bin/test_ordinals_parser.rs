use bitcoin::Transaction;
use bitcoin::script::Instruction;

async fn fetch_tx(txid: &str) -> Transaction {
    let url = format!("https://mempool.space/api/tx/{}/hex", txid);
    let hex = reqwest::get(&url).await.unwrap().text().await.unwrap();
    let bytes = hex::decode(hex).unwrap();
    bitcoin::consensus::deserialize(&bytes).unwrap()
}

fn parse_ordinals_witness(tx: &Transaction) {
    println!("Analyzing TX: {}", tx.txid());
    for (i, input) in tx.input.iter().enumerate() {
        if input.witness.len() < 2 {
            continue;
        }
        // In P2TR, the script is usually the second to last item
        let script_bytes = input.witness.iter().nth(input.witness.len() - 2).unwrap();
        let script = bitcoin::ScriptBuf::from(script_bytes.to_vec());
        
        let mut in_envelope = false;
        let mut expect_ord = false;

        let mut instructions = script.instructions();
        while let Some(Ok(instruction)) = instructions.next() {
            println!("Instr: {:?}", instruction);
            match instruction {
                Instruction::Op(bitcoin::opcodes::all::OP_PUSHBYTES_0) => {
                    if let Some(Ok(Instruction::Op(bitcoin::opcodes::all::OP_IF))) = instructions.next() {
                        expect_ord = true;
                    }
                }
                Instruction::PushBytes(bytes) if expect_ord => {
                    if bytes.as_bytes() == b"ord" {
                        in_envelope = true;
                        println!("  [+] Found Ordinals Envelope in Input {}", i);
                    }
                    expect_ord = false;
                }
                _ => {}
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let tx1 = fetch_tx("8b7e755ac5b133c904de1604ef402b91ee6fea5e038aafe688e3316b4ef2a259").await;
    parse_ordinals_witness(&tx1);
    
    let tx2 = fetch_tx("116f5736d68aef805656c2fb3b2740ebef9cd9a428bd5f62611588550f27a28a").await;
    parse_ordinals_witness(&tx2);
}
