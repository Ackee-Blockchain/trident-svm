use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;

use solana_sdk::account::AccountSharedData;
use solana_sdk::pubkey::Pubkey;

pub struct ProgramEntrypoint {
    pub program_id: Pubkey,
    pub authority: Option<Pubkey>,
    pub entry: Option<BuiltinFunctionWithContext>,
}
impl ProgramEntrypoint {
    pub fn new(
        program_id: Pubkey,
        authority: Option<Pubkey>,
        entry_fn: Option<BuiltinFunctionWithContext>,
    ) -> ProgramEntrypoint {
        Self {
            program_id,
            authority,
            entry: entry_fn,
        }
    }
}

pub struct SBFTargets {
    pub program_id: Pubkey,
    pub authority: Option<Pubkey>,
    pub data: Vec<u8>,
}
impl SBFTargets {
    pub fn new(program_id: Pubkey, authority: Option<Pubkey>, data: Vec<u8>) -> SBFTargets {
        Self {
            program_id,
            authority,
            data,
        }
    }
}

pub struct TridentAccountSharedData {
    pub address: Pubkey,
    pub account: AccountSharedData,
}
impl TridentAccountSharedData {
    pub fn new(address: Pubkey, account: AccountSharedData) -> TridentAccountSharedData {
        Self { address, account }
    }
}
