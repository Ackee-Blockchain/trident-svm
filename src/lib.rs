mod accounts_database;
mod builder;
mod methods;
mod trident_fork_graphs;
pub mod trident_svm_log;
mod utils;

#[cfg(feature = "syscall-v2")]
pub mod builtin_function;
pub mod trident_svm;
pub mod types;

pub mod processor {
    #[cfg(feature = "syscall-v2")]
    pub use crate::builtin_function::post_invocation;
    #[cfg(feature = "syscall-v2")]
    pub use crate::builtin_function::pre_invocation;

    #[cfg(feature = "syscall-v2")]
    pub use solana_program_entrypoint::deserialize;

    pub use solana_account_info as account_info;
    pub use solana_instruction::error::InstructionError;
    pub use solana_program_runtime;
    pub use solana_program_runtime::stable_log;
    pub use solana_pubkey::Pubkey;
    #[cfg(feature = "syscall-v2")]
    pub use solana_sbpf;
}

pub mod prelude {
    pub use super::trident_svm_log;
    pub use crate::types::transaction_result::{
        ExecutionTraces, TracedTransactionResult, TridentTransactionProcessingResult,
    };
    // Coverage utilities for AFL integration
    pub use crate::types::coverage::{
        count_edges, count_trace_entries, reset_coverage_map, traces_to_coverage,
        traces_to_coverage_map, CoverageMap, COVERAGE_MAP_SIZE,
        // Enhanced versions
        traces_to_coverage_map_context,
        traces_to_coverage_map_dataflow,
        traces_to_coverage_map_enhanced,
    };
    pub use log::Level;
    pub use solana_svm;
}
