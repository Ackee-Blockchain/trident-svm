use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::feature_set::FeatureSet;
use solana_sdk::fee::FeeStructure;
use solana_sdk::hash::Hash;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::ExecutionRecordingConfig;
use solana_svm::transaction_processor::TransactionBatchProcessor;
use solana_svm::transaction_processor::TransactionProcessingConfig;
use solana_svm::transaction_processor::TransactionProcessingEnvironment;

use solana_compute_budget::compute_budget::ComputeBudget;

use crate::accounts_db::AccountsDB;
use crate::trident_fork_graphs::TridentForkGraph;

pub struct TridentSVM<'a> {
    pub accounts: AccountsDB,
    pub payer: Keypair,
    pub feature_set: Arc<FeatureSet>,
    pub processor: TransactionBatchProcessor<TridentForkGraph>,
    pub fork_graph: Arc<RwLock<TridentForkGraph>>,
    pub tx_processing_environment: TransactionProcessingEnvironment<'a>,
    pub tx_processing_config: TransactionProcessingConfig<'a>,
}

impl TransactionProcessingCallback for TridentSVM<'_> {
    fn account_matches_owners(&self, account: &Pubkey, owners: &[Pubkey]) -> Option<usize> {
        self.get_account_shared_data(account)
            .and_then(|account| owners.iter().position(|key| account.owner().eq(key)))
    }

    fn get_account_shared_data(
        &self,
        pubkey: &Pubkey,
    ) -> Option<solana_sdk::account::AccountSharedData> {
        self.accounts.get_account(pubkey)
    }
}

impl Default for TridentSVM<'_> {
    fn default() -> Self {
        let payer = Keypair::new();

        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        let mut client = Self {
            accounts: Default::default(),
            payer: payer.insecure_clone(),
            feature_set: Arc::new(FeatureSet::all_enabled()),
            processor: TransactionBatchProcessor::<TridentForkGraph>::new(1, 1, HashSet::default()),
            fork_graph: Arc::new(RwLock::new(TridentForkGraph {})),
            tx_processing_config: TransactionProcessingConfig {
                compute_budget: Some(ComputeBudget::default()),
                log_messages_bytes_limit: Some(10 * 1000),
                recording_config: ExecutionRecordingConfig {
                    enable_cpi_recording: true,
                    enable_log_recording: true,
                    enable_return_data_recording: true,
                },
                ..Default::default()
            },
            tx_processing_environment: TransactionProcessingEnvironment {
                blockhash: Hash::default(),
                epoch_total_stake: None,
                epoch_vote_accounts: None,
                feature_set: Arc::new(FeatureSet::all_enabled()),
                fee_structure: None,
                lamports_per_signature,
                rent_collector: None,
            },
        };

        let payer_account = AccountSharedData::new(
            500_000_000 * LAMPORTS_PER_SOL,
            0,
            &solana_sdk::system_program::ID,
        );
        client.accounts.add_account(&payer.pubkey(), &payer_account);

        client
    }
}
