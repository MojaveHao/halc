use crate::ast::Module;

pub trait Backend {
    fn generate(&self, modules: &[Module]) -> Result<String, String>;
}