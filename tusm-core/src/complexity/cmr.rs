use super::ast::AstNode;
use bitcoin::hashes::{sha256, Hash, HashEngine};

pub fn compute_cmr(ast: &[AstNode], index: usize) -> Result<[u8; 32], String> {
    if index >= ast.len() {
        return Err(format!("Index out of bounds: {}", index));
    }
    
    let mut engine = sha256::HashEngine::default();
    
    match &ast[index] {
        AstNode::Iden => {
            engine.input(b"Simplicity\x1FCommitment\x1Fiden");
        },
        AstNode::Unit => {
            engine.input(b"Simplicity\x1FCommitment\x1Funit");
        },
        AstNode::Injl(child) => {
            engine.input(b"Simplicity\x1FCommitment\x1Finjl");
            let child_cmr = compute_cmr(ast, index.saturating_sub(*child))?;
            engine.input(&child_cmr);
        },
        AstNode::Injr(child) => {
            engine.input(b"Simplicity\x1FCommitment\x1Finjr");
            let child_cmr = compute_cmr(ast, index.saturating_sub(*child))?;
            engine.input(&child_cmr);
        },
        AstNode::Take(child) => {
            engine.input(b"Simplicity\x1FCommitment\x1Ftake");
            let child_cmr = compute_cmr(ast, index.saturating_sub(*child))?;
            engine.input(&child_cmr);
        },
        AstNode::Drop(child) => {
            engine.input(b"Simplicity\x1FCommitment\x1Fdrop");
            let child_cmr = compute_cmr(ast, index.saturating_sub(*child))?;
            engine.input(&child_cmr);
        },
        AstNode::Comp(l, r) => {
            engine.input(b"Simplicity\x1FCommitment\x1Fcomp");
            let l_cmr = compute_cmr(ast, index.saturating_sub(*l))?;
            let r_cmr = compute_cmr(ast, index.saturating_sub(*r))?;
            engine.input(&l_cmr);
            engine.input(&r_cmr);
        },
        AstNode::Case(l, r) => {
            engine.input(b"Simplicity\x1FCommitment\x1Fcase");
            let l_cmr = compute_cmr(ast, index.saturating_sub(*l))?;
            let r_cmr = compute_cmr(ast, index.saturating_sub(*r))?;
            engine.input(&l_cmr);
            engine.input(&r_cmr);
        },
        AstNode::Pair(l, r) => {
            engine.input(b"Simplicity\x1FCommitment\x1Fpair");
            let l_cmr = compute_cmr(ast, index.saturating_sub(*l))?;
            let r_cmr = compute_cmr(ast, index.saturating_sub(*r))?;
            engine.input(&l_cmr);
            engine.input(&r_cmr);
        },
        AstNode::Jet(id) => {
            engine.input(b"Simplicity\x1FCommitment\x1Fjet");
            engine.input(&[*id as u8]);
        }
    }
    
    Ok(sha256::Hash::from_engine(engine).to_byte_array())
}
