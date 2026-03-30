use std::collections::HashSet;

use solana_clock::Clock;
use solana_compute_budget::compute_budget_limits::ComputeBudgetLimits;
use solana_fee_structure::FeeDetails;

use solana_transaction::sanitized::SanitizedTransaction;
use solana_transaction::Transaction;

use solana_svm::account_loader::CheckedTransactionDetails;
use solana_svm::transaction_processing_result::TransactionProcessingResultExtensions;
use solana_svm::transaction_processor::ExecutionRecordingConfig;
use solana_svm::transaction_processor::TransactionProcessingConfig;
use solana_svm::transaction_processor::TransactionProcessingEnvironment;

use crate::trident_svm::TridentSVM;
use crate::types::transaction_result::TridentTransactionResult;

const LOG_MESSAGES_BYTES_LIMIT: usize = 100 * 1000;

fn recording_config() -> ExecutionRecordingConfig {
    ExecutionRecordingConfig {
        enable_cpi_recording: true,
        enable_log_recording: true,
        enable_return_data_recording: true,
        enable_transaction_balance_recording: false,
    }
}

impl TridentSVM {
    pub fn process_transaction(&mut self, transaction: Transaction) -> TridentTransactionResult {
        let tx_processing_environment = TransactionProcessingEnvironment::<'_> {
            feature_set: *self.feature_set,
            ..Default::default()
        };

        let tx_processing_config = TransactionProcessingConfig {
            log_messages_bytes_limit: Some(LOG_MESSAGES_BYTES_LIMIT),
            recording_config: recording_config(),
            ..Default::default()
        };

        self.processor.reset_sysvar_cache();
        self.processor.fill_missing_sysvar_cache_entries(self);

        let sanitized_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())
                .expect("Failed to create sanitized transaction");

        let transaction_timestamp = self.accounts.deserialize_sysvar::<Clock>().unix_timestamp;

        let output = self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitized_tx],
            get_transaction_check_results(1),
            &tx_processing_environment,
            &tx_processing_config,
        );

        self.accounts.update_clock();

        TridentTransactionResult::from_svm_output(output, transaction_timestamp)
    }

    pub fn process_transaction_with_settle(
        &mut self,
        transaction: Transaction,
    ) -> TridentTransactionResult {
        let tx_processing_environment = TransactionProcessingEnvironment::<'_> {
            feature_set: *self.feature_set,
            ..Default::default()
        };

        let tx_processing_config = TransactionProcessingConfig {
            log_messages_bytes_limit: Some(LOG_MESSAGES_BYTES_LIMIT),
            recording_config: recording_config(),
            ..Default::default()
        };

        self.processor.reset_sysvar_cache();
        self.processor.fill_missing_sysvar_cache_entries(self);

        let sanitized_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())
                .expect("Failed to create sanitized transaction");

        let transaction_timestamp = self.accounts.deserialize_sysvar::<Clock>().unix_timestamp;

        let output = self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitized_tx],
            get_transaction_check_results(1),
            &tx_processing_environment,
            &tx_processing_config,
        );

        self.accounts.update_clock();

        let processed_transaction = output.processing_results[0]
            .processed_transaction()
            .expect("Transaction was not processed");

        match &processed_transaction {
            solana_svm::transaction_processing_result::ProcessedTransaction::Executed(
                executed_tx,
            ) => {
                if executed_tx.execution_details.status.is_ok() {
                    self.settle_accounts(&executed_tx.loaded_transaction.accounts);
                }
            }
            solana_svm::transaction_processing_result::ProcessedTransaction::FeesOnly(_) => {}
        }

        TridentTransactionResult::from_svm_output(output, transaction_timestamp)
    }
}

pub(crate) fn get_transaction_check_results(
    len: usize,
) -> Vec<solana_transaction_error::TransactionResult<CheckedTransactionDetails>> {
    let compute_budget_limit = ComputeBudgetLimits {
        updated_heap_bytes: 256 * 1024,
        ..ComputeBudgetLimits::default()
    };
    vec![
        solana_transaction_error::TransactionResult::Ok(CheckedTransactionDetails::new(
            None,
            Ok(compute_budget_limit.get_compute_budget_and_limits(
                compute_budget_limit.loaded_accounts_bytes,
                FeeDetails::default()
            )),
        ));
        len
    ]
}
