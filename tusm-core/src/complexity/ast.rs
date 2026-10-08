#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JetId {
    GetOutputValue = 1,
    GetScriptPubkeyHash = 2,
    GetCurrentSequence = 3,
    ComputeMerkleRoot = 4,
}

impl JetId {
    pub fn from_u16(val: u16) -> Option<Self> {
        match val {
            1 => Some(JetId::GetOutputValue),
            2 => Some(JetId::GetScriptPubkeyHash),
            3 => Some(JetId::GetCurrentSequence),
            4 => Some(JetId::ComputeMerkleRoot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNode {
    Iden,
    Unit,
    Injl(usize),
    Injr(usize),
    Take(usize),
    Drop(usize),
    Comp(usize, usize),
    Case(usize, usize),
    Pair(usize, usize),
    Jet(JetId),
}
