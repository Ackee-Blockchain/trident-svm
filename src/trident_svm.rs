use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use solana_program_runtime::log_collector::log::debug;
use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::feature_set::FeatureSet;
use solana_sdk::fee::FeeStructure;
use solana_sdk::hash::Hash;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

use solana_sdk::transaction::TransactionError;
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::ExecutionRecordingConfig;
use solana_svm::transaction_processor::TransactionBatchProcessor;
use solana_svm::transaction_processor::TransactionProcessingConfig;
use solana_svm::transaction_processor::TransactionProcessingEnvironment;

use solana_compute_budget::compute_budget::ComputeBudget;

use crate::accounts_db::AccountsDB;
use crate::trident_fork_graphs::TridentForkGraph;
use crate::utils::create_hash;

pub struct TridentSVM<'a> {
    pub(crate) accounts: AccountsDB,
    pub(crate) payer: Keypair,
    pub(crate) feature_set: Arc<FeatureSet>,
    pub(crate) processor: TransactionBatchProcessor<TridentForkGraph>,
    pub(crate) fork_graph: Arc<RwLock<TridentForkGraph>>,
    pub(crate) tx_processing_environment: TransactionProcessingEnvironment<'a>,
    pub(crate) tx_processing_config: TransactionProcessingConfig<'a>,
    pub(crate) blockhash_config: BlockhashConfig,
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
            blockhash_config: BlockhashConfig::default(),
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

pub struct BlockhashConfig {
    pub(crate) latest_blockhash: Hash,
    pub(crate) blockhash_check: bool,
    pub(crate) transaction_block: HashSet<Hash>, //hashes of transactions with current blockhash
}

impl Default for BlockhashConfig {
    fn default() -> Self {
        Self {
            latest_blockhash: create_hash(b"genesis"),
            blockhash_check: false,
            transaction_block: HashSet::default(),
        }
    }
}

impl BlockhashConfig {
    pub fn expire_blockhash(&mut self) {
        self.latest_blockhash = create_hash(&self.latest_blockhash.to_bytes());
        self.transaction_block = HashSet::default();
    }

    pub fn block_contains_transaction(&mut self, transaction_hash: &Hash) -> Result<(), TransactionError> {
        if self.transaction_block.contains(transaction_hash) {
            debug!("Transaction hash {} already in block", transaction_hash);
            Err(TransactionError::AlreadyProcessed)
        } else {
            self.transaction_block.insert(*transaction_hash);
            Ok(())
        }
    }
}
