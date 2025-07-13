mod accounts_database;
mod builder;
mod methods;
mod native;
mod trident_fork_graphs;
pub mod trident_svm_log;
mod utils;

#[cfg(any(feature = "syscall-v1", feature = "syscall-v2"))]
pub mod builtin_function;
pub mod trident_svm;
pub mod types;

pub mod processor {
    #[cfg(any(feature = "syscall-v1", feature = "syscall-v2"))]
    pub use crate::builtin_function::post_invocation;
    #[cfg(any(feature = "syscall-v1", feature = "syscall-v2"))]
    pub use crate::builtin_function::pre_invocation;

    pub use solana_program_runtime;
    pub use solana_program_runtime::stable_log;
    #[cfg(any(feature = "syscall-v1", feature = "syscall-v2"))]
    pub use solana_rbpf;
    pub use solana_sdk::account_info;
    pub use solana_sdk::entrypoint::deserialize;
    pub use solana_sdk::instruction::InstructionError;
    pub use solana_sdk::pubkey::Pubkey;
}

pub mod prelude {
    pub use super::trident_svm_log;
    pub use log::Level;
    pub use solana_svm;
}
