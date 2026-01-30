use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;
use solana_transaction_error::TransactionError;

/// Execution traces from sBPF VM.
/// Each inner Vec represents traces from one instruction/CPI invocation.
/// Each [u64; 12] contains the VM state: registers r0-r10 and program counter.
pub type ExecutionTraces = Vec<Vec<[u64; 12]>>;

/// Standard transaction processing result (wraps full SVM output).
pub struct TridentTransactionProcessingResult {
    result: LoadAndExecuteSanitizedTransactionsOutput,
    transaction_timestamp: u64,
}

impl TridentTransactionProcessingResult {
    pub fn new(
        result: LoadAndExecuteSanitizedTransactionsOutput,
        transaction_timestamp: u64,
    ) -> Self {
        Self {
            result,
            transaction_timestamp,
        }
    }

    pub fn get_result(&self) -> &LoadAndExecuteSanitizedTransactionsOutput {
        &self.result
    }

    pub fn get_transaction_timestamp(&self) -> u64 {
        self.transaction_timestamp
    }
}

/// Simplified result for traced transaction execution.
/// Contains only what's needed for fuzzing: status, logs, traces, and timestamp.
pub struct TracedTransactionResult {
    /// Execution result: Ok(()) on success, Err(e) on failure
    pub result: Result<(), TransactionError>,
    /// Transaction timestamp (clock value)
    pub transaction_timestamp: u64,
    /// Log messages from execution
    pub logs: Vec<String>,
    /// Execution traces from the sBPF VM for AFL feedback
    pub traces: ExecutionTraces,
}

impl TracedTransactionResult {
    pub fn new(
        result: Result<(), TransactionError>,
        transaction_timestamp: u64,
        logs: Vec<String>,
        traces: ExecutionTraces,
    ) -> Self {
        Self {
            result,
            transaction_timestamp,
            logs,
            traces,
        }
    }

    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Get the error if execution failed
    pub fn get_error(&self) -> Option<&TransactionError> {
        self.result.as_ref().err()
    }
}
