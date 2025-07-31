use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;

use solana_pubkey::Pubkey;

pub struct TridentEntrypoint {
    pub(crate) program_id: Pubkey,
    pub(crate) authority: Option<Pubkey>,
    pub(crate) entry: Option<BuiltinFunctionWithContext>,
}
impl TridentEntrypoint {
    pub fn new(
        program_id: Pubkey,
        authority: Option<Pubkey>,
        entry_fn: Option<BuiltinFunctionWithContext>,
    ) -> TridentEntrypoint {
        Self {
            program_id,
            authority,
            entry: entry_fn,
        }
    }
}
