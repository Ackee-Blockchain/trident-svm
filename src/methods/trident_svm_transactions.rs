use std::collections::HashSet;

use solana_sdk::fee::FeeStructure;
use solana_sdk::hash::Hash;

use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::transaction::Transaction;
use solana_sdk::transaction::TransactionError;

use solana_svm::account_loader::CheckedTransactionDetails;
use solana_svm::transaction_processor::ExecutionRecordingConfig;
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;
use solana_svm::transaction_processor::TransactionProcessingConfig;
use solana_svm::transaction_processor::TransactionProcessingEnvironment;

use solana_compute_budget::compute_budget::ComputeBudget;

use crate::log::setup_solana_logging;
use crate::log::turn_off_solana_logging;
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
            blockhash_lamports_per_signature: 0,
            epoch_total_stake: 0,
            feature_set: self.feature_set.clone(),
            fee_lamports_per_signature: lamports_per_signature,
            rent_collector: None,
        };

        let tx_processing_config = TransactionProcessingConfig {
            compute_budget: Some(ComputeBudget::default()),
            log_messages_bytes_limit: Some(10 * 1000),
            recording_config: ExecutionRecordingConfig {
                enable_cpi_recording: true,
                enable_log_recording: true,
                enable_return_data_recording: true,
            },
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
    ) -> solana_sdk::transaction::Result<()> {
        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        let tx_processing_environment = TransactionProcessingEnvironment {
            blockhash: Hash::default(),
            blockhash_lamports_per_signature: 0,
            epoch_total_stake: 0,
            feature_set: self.feature_set.clone(),
            fee_lamports_per_signature: lamports_per_signature,
            rent_collector: None,
        };

        let mut tx_processing_config = TransactionProcessingConfig {
            compute_budget: Some(ComputeBudget::default()),
            log_messages_bytes_limit: Some(10 * 1000),
            recording_config: ExecutionRecordingConfig {
                enable_cpi_recording: true,
                enable_log_recording: true,
                enable_return_data_recording: true,
            },
            ..Default::default()
        };

        if std::env::var("TRIDENT_LOG").is_ok() {
            setup_solana_logging();
            tx_processing_config.recording_config.enable_log_recording = true;
        } else {
            turn_off_solana_logging();
            tx_processing_config.recording_config.enable_log_recording = false;
        }

        // reset sysvar cache
        self.processor.reset_sysvar_cache();

        // replenish sysvar cache with sysvars from the accounts db
        self.processor.fill_missing_sysvar_cache_entries(self);

        // create sanitized transaction
        let sanitezed_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())?;

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

        // TODO: Check why there is vector of Transaction results
        // We process only one transaction here, so it might possible be always 1 ?
        // TODO: Check if this is correct way to check if transaction was executed, potentially
        // add support to process the whole vector
        let processing_result = if result.processing_results.len() != 1 {
            return Err(TransactionError::ProgramCacheHitMaxLimit);
        } else {
            &result.processing_results[0]
        };

        match processing_result {
            Ok(processed_tx) => match processed_tx.status() {
                Ok(_) => {
                    let loaded_tx = &processed_tx
                        .executed_transaction()
                        .unwrap()
                        .loaded_transaction;
                    self.settle_accounts(&loaded_tx.accounts);
                    Ok(())
                }
                Err(error) => Err(error.clone()),
            },
            Err(error) => Err(error.clone()),
        }
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
        solana_sdk::transaction::Result::Ok(CheckedTransactionDetails::new(
            None,
            lamports_per_signature,
        ));
        len
    ]
}
