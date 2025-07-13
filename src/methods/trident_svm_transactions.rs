use std::collections::HashSet;

use solana_sdk::fee::FeeStructure;
use solana_sdk::hash::Hash;

use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::transaction::Transaction;

use solana_svm::account_loader::CheckedTransactionDetails;
use solana_svm::transaction_processor::ExecutionRecordingConfig;
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;
use solana_svm::transaction_processor::TransactionProcessingConfig;
use solana_svm::transaction_processor::TransactionProcessingEnvironment;

use solana_compute_budget::compute_budget::ComputeBudget;

use crate::trident_svm::TridentSVM;

impl TridentSVM {
    pub fn process_transaction(
        &mut self,
        transaction: Transaction,
    ) -> LoadAndExecuteSanitizedTransactionsOutput {
        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        let tx_processing_environment = TransactionProcessingEnvironment {
            blockhash: Hash::default(),
            epoch_total_stake: None,
            epoch_vote_accounts: None,
            feature_set: self.feature_set.clone(),
            fee_structure: None,
            lamports_per_signature,
            rent_collector: None,
        };

        let tx_processing_config = TransactionProcessingConfig {
            compute_budget: Some(ComputeBudget::default()),
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
                .unwrap();

        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        // execute transaction
        self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1, lamports_per_signature),
            &tx_processing_environment,
            &tx_processing_config,
        )
    }
    pub fn process_transaction_with_settle(
        &mut self,
        transaction: Transaction,
    ) -> LoadAndExecuteSanitizedTransactionsOutput {
        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        let tx_processing_environment = TransactionProcessingEnvironment {
            blockhash: Hash::default(),
            epoch_total_stake: None,
            epoch_vote_accounts: None,
            feature_set: self.feature_set.clone(),
            fee_structure: None,
            lamports_per_signature,
            rent_collector: None,
        };

        let tx_processing_config = TransactionProcessingConfig {
            compute_budget: Some(ComputeBudget::default()),
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

        // get fee structure
        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        // execute transaction
        let result = self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1, lamports_per_signature),
            &tx_processing_environment,
            &tx_processing_config,
        );

        match &result.execution_results[0] {
            solana_svm::transaction_results::TransactionExecutionResult::Executed {
                details,
                ..
            } => match &details.status {
                Ok(()) => {
                    if let Ok(loaded_transaction) = &result.loaded_transactions[0] {
                        self.settle_accounts(&loaded_transaction.accounts);
                    }
                }
                Err(_transaction_error) => {
                    // in case of transaction error, we don't need to do anything
                }
            },
            solana_svm::transaction_results::TransactionExecutionResult::NotExecuted(
                _transaction_error,
            ) => {
                // in case of transaction error, we don't need to do anything
            }
        }
        result
    }
}

// This function is also a mock. In the Agave validator, the bank pre-checks
// transactions before providing them to the SVM API. We mock this step in
// PayTube, since we don't need to perform such pre-checks.
pub(crate) fn get_transaction_check_results(
    len: usize,
    lamports_per_signature: u64,
) -> Vec<solana_sdk::transaction::Result<CheckedTransactionDetails>> {
    vec![
        solana_sdk::transaction::Result::Ok(CheckedTransactionDetails {
            nonce: None,
            lamports_per_signature,
        });
        len
    ]
}
