use tusm_core::crc21_v2::{VaultStateV2, match_known_cmr, derive_vault, KNOWN_CLAIM_CMRS};
use bitcoin::ScriptBuf;

fn main() {
    let s: Vec<u8> = vec![
        2, 0, 249, 179, 26, 237, 95, 124, 200, 110, 255, 111, 106, 19, 211, 220, 205, 14, 88, 57, 160, 214, 118, 233, 88, 104, 75, 210, 170, 235, 181, 88, 62, 13, 0, 0, 0, 0, 202, 36, 0, 0, 164, 1, 8, 82, 225, 253, 25, 101, 83, 60, 117, 118, 10, 120, 179, 71, 208, 174, 210, 210, 255, 210, 129, 22, 232, 242, 214, 25, 107, 201, 125, 232, 6, 165, 110, 175, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 34, 2, 0, 0, 0, 0, 0, 0, 22, 0, 20, 132, 20, 171, 14, 209, 15, 127, 35, 13, 234, 37, 112, 112, 79, 99, 167, 25, 7, 174, 241, 91, 132, 65, 237, 153, 148, 200, 39, 19, 32, 169, 59, 241, 142, 251, 78, 140, 122, 100, 126, 143, 41, 2, 195, 33, 188, 220, 247, 165, 192, 94, 15, 112, 104, 133, 102, 66, 241, 230, 199, 48, 214, 103, 13, 53, 199, 11, 113, 98, 94, 103, 11, 12, 35, 38, 58, 145, 114, 18, 45, 217, 110, 202, 60, 16, 39
    ];

    let target_spk_hex = "5120207e453cf1684b70af19612cc620a60931944235c8f690ec10d7c0ed79c55263";
    let target_spk = ScriptBuf::from(hex::decode(target_spk_hex).unwrap());

    println!("Testing VaultStateV2 with 208 bytes...");
    match VaultStateV2::parse(&s) {
        Err(e) => println!("Parse error: {:?}", e),
        Ok(vs) => {
            println!("Parsed successfully: Koala #{}", vs.item_number);
            let matched = match_known_cmr(&vs, &target_spk);
            println!("Matched CMR: {:?}", matched.as_ref().map(|m| &m.cmr_label));
            if let Some(d) = matched {
                println!("Address: {}", d.address);
                println!("Script: {}", hex::encode(d.script_pubkey.as_bytes()));
            }
        }
    }
}
