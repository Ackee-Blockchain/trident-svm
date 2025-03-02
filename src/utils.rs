use std::time::UNIX_EPOCH;

use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;

use solana_sdk::account::AccountSharedData;
use solana_sdk::pubkey::Pubkey;

pub struct ProgramEntrypoint {
    pub(crate) program_id: Pubkey,
    pub(crate) authority: Option<Pubkey>,
    pub(crate) entry: Option<BuiltinFunctionWithContext>,
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

pub struct SBFTarget {
    pub(crate) program_id: Pubkey,
    pub(crate) authority: Option<Pubkey>,
    pub(crate) data: Vec<u8>,
}
impl SBFTarget {
    pub fn new(program_id: Pubkey, authority: Option<Pubkey>, data: Vec<u8>) -> Self {
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

pub(crate) fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards!")
        .as_secs()
}
