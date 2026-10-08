use super::ast::{AstNode, JetId};
use super::env::TxEnv;

pub struct Machine<'a, E: TxEnv> {
    pub env: &'a E,
}

impl<'a, E: TxEnv> Machine<'a, E> {
    pub fn new(env: &'a E) -> Self {
        Self { env }
    }

    pub fn evaluate(&mut self, ast: &[AstNode], index: usize) -> Result<(), String> {
        if index >= ast.len() {
            return Err("Index out of bounds".to_string());
        }

        match &ast[index] {
            AstNode::Comp(l, r) => {
                self.evaluate(ast, index.saturating_sub(*l))?;
                self.evaluate(ast, index.saturating_sub(*r))?;
            },
            AstNode::Pair(l, r) => {
                self.evaluate(ast, index.saturating_sub(*l))?;
                self.evaluate(ast, index.saturating_sub(*r))?;
            },
            AstNode::Jet(id) => {
                match id {
                    JetId::GetOutputValue => {
                        // In a real VM, the input value comes from the active frame
                        // For the integration, we just call the env to prove linkage
                        if self.env.get_output_value(0).is_none() {
                            return Err("Jet GetOutputValue failed".to_string());
                        }
                    },
                    JetId::GetScriptPubkeyHash => {
                        if self.env.get_script_pubkey_hash(0).is_none() {
                            return Err("Jet GetScriptPubkeyHash failed".to_string());
                        }
                    },
                    _ => {}
                }
            },
            _ => {
                // For other combinators, we just pass through in this minimal reality
            }
        }
        Ok(())
    }
}
