use super::unified_ir::UnifiedIR;

pub trait SkpBackend {
    fn name(&self) -> &'static str;
    fn import(&self, path: &str) -> Result<UnifiedIR, String>;
}

