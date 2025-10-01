mod accounts_database;
mod builder;
mod log;
mod methods;
mod native;
mod trident_fork_graphs;
mod utils;
mod svm_sysvars; // ensure the module is visible to this file
use crate::svm_sysvars::default_sysvar_accounts_2_2;
use crate::svm_sysvars::setup_test_sysvars;

pub mod builtin_function;
pub mod fuzzing_metrics;
pub mod trident_svm;
pub mod types;
pub mod sysvar_bridge;

pub use types::TridentAccountSharedData;
pub mod processor {
    pub use crate::builtin_function::post_invocation;
    pub use crate::builtin_function::pre_invocation;

    pub use solana_program_runtime;
    pub use solana_program_runtime::stable_log;
    pub use solana_sbpf;
    pub use solana_sdk::account_info;
    pub use solana_sdk::entrypoint::deserialize;
    pub use solana_sdk::instruction::InstructionError;
    pub use solana_sdk::pubkey::Pubkey;
}
