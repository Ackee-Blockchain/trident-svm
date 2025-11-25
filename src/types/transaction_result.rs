use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;

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
