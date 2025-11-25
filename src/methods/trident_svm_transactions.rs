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
use crate::types::transaction_result::TridentTransactionProcessingResult;

impl TridentSVM {
    pub fn process_transaction(
        &mut self,
        transaction: Transaction,
    ) -> TridentTransactionProcessingResult {
        let tx_processing_environment = TransactionProcessingEnvironment::<'_> {
            feature_set: *self.feature_set,
            ..Default::default()
        };

        let tx_processing_config = TransactionProcessingConfig::default();

        // reset sysvar cache
        self.processor.reset_sysvar_cache();

        // replenish sysvar cache with sysvars from the accounts db
        self.processor.fill_missing_sysvar_cache_entries(self);

        // create sanitized transaction
        let sanitezed_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())
                .unwrap();

        // get current transaction timestamp
        let transaction_timestamp =
            self.accounts.deserialize_sysvar::<Clock>().unix_timestamp as u64;

        // execute transaction
        let res = self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1),
            &tx_processing_environment,
            &tx_processing_config,
        );

        // update clock
        self.accounts.update_clock();

        // return transaction processing result
        TridentTransactionProcessingResult::new(res, transaction_timestamp)
    }
    pub fn process_transaction_with_settle(
        &mut self,
        transaction: Transaction,
    ) -> TridentTransactionProcessingResult {
        let tx_processing_environment = TransactionProcessingEnvironment::<'_> {
            feature_set: *self.feature_set,
            ..Default::default()
        };

        let tx_processing_config = TransactionProcessingConfig {
            log_messages_bytes_limit: Some(20 * 1000),
            recording_config: ExecutionRecordingConfig::new_single_setting(true),
            ..Default::default()
        };

        // reset sysvar cache
        self.processor.reset_sysvar_cache();

        // replenish sysvar cache with sysvars from the accounts db
        self.processor.fill_missing_sysvar_cache_entries(self);

        // create sanitized transaction
        let sanitezed_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())
                .expect("Trident SVM is not able to create sanitized transaction");

        // get current transaction timestamp
        let transaction_timestamp =
            self.accounts.deserialize_sysvar::<Clock>().unix_timestamp as u64;

        // execute transaction
        let result = self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1),
            &tx_processing_environment,
            &tx_processing_config,
        );

        // update clock
        self.accounts.update_clock();

        let processed_transaction = result.processing_results[0]
            .processed_transaction()
            .expect("Transaction was not processed");

        match &processed_transaction {
            solana_svm::transaction_processing_result::ProcessedTransaction::Executed(
                executed_tx,
            ) => match &executed_tx.execution_details.status {
                Ok(()) => {
                    self.settle_accounts(&executed_tx.loaded_transaction.accounts);
                }
                Err(_transaction_error) => {
                    // in case of transaction error, we don't need to do anything
                }
            },
            solana_svm::transaction_processing_result::ProcessedTransaction::FeesOnly(
                _transaction_error,
            ) => {
                // in case of transaction error, we don't need to do anything
            }
        }
        TridentTransactionProcessingResult::new(result, transaction_timestamp)
    }
}

/// This function is also a mock. In the Agave validator, the bank pre-checks
/// transactions before providing them to the SVM API. We mock this step in
/// PayTube, since we don't need to perform such pre-checks.
pub(crate) fn get_transaction_check_results(
    len: usize,
) -> Vec<solana_transaction_error::TransactionResult<CheckedTransactionDetails>> {
    let compute_budget_limit = ComputeBudgetLimits::default();
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
