use super::ast::{AstNode, JetId};

pub fn parse_bytecode(bytes: &[u8]) -> Result<Vec<AstNode>, String> {
    let mut ast = Vec::new();
    let mut i = 0;
    
    while i < bytes.len() {
        let tag = bytes[i];
        i += 1;
        
        let node = match tag {
            0x00 => AstNode::Unit,
            0x01 => AstNode::Iden,
            0x02 => {
                if i >= bytes.len() { return Err("Incomplete Injl".to_string()); }
                let child = bytes[i] as usize; i += 1;
                AstNode::Injl(child)
            },
            0x03 => {
                if i >= bytes.len() { return Err("Incomplete Injr".to_string()); }
                let child = bytes[i] as usize; i += 1;
                AstNode::Injr(child)
            },
            0x04 => {
                if i >= bytes.len() { return Err("Incomplete Take".to_string()); }
                let child = bytes[i] as usize; i += 1;
                AstNode::Take(child)
            },
            0x05 => {
                if i >= bytes.len() { return Err("Incomplete Drop".to_string()); }
                let child = bytes[i] as usize; i += 1;
                AstNode::Drop(child)
            },
            0x06 => {
                if i + 1 >= bytes.len() { return Err("Incomplete Comp".to_string()); }
                let l = bytes[i] as usize; i += 1;
                let r = bytes[i] as usize; i += 1;
                AstNode::Comp(l, r)
            },
            0x07 => {
                if i + 1 >= bytes.len() { return Err("Incomplete Case".to_string()); }
                let l = bytes[i] as usize; i += 1;
                let r = bytes[i] as usize; i += 1;
                AstNode::Case(l, r)
            },
            0x08 => {
                if i + 1 >= bytes.len() { return Err("Incomplete Pair".to_string()); }
                let l = bytes[i] as usize; i += 1;
                let r = bytes[i] as usize; i += 1;
                AstNode::Pair(l, r)
            },
            0x09 => {
                if i + 1 >= bytes.len() { return Err("Incomplete Jet".to_string()); }
                let b1 = bytes[i] as u16; i += 1;
                let b2 = bytes[i] as u16; i += 1;
                let id = (b1 << 8) | b2;
                if let Some(jet_id) = JetId::from_u16(id) {
                    AstNode::Jet(jet_id)
                } else {
                    return Err(format!("Unknown Jet ID: {}", id));
                }
            },
            _ => return Err(format!("Unknown tag: {}", tag)),
        };
        
        ast.push(node);
    }
    
    Ok(ast)
}
