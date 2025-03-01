use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::feature_set::FeatureSet;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::TransactionBatchProcessor;

use crate::accounts_db::AccountsDB;
use crate::trident_fork_graphs::TridentForkGraph;

pub struct TridentSVM {
    pub(crate) accounts: AccountsDB,
    pub(crate) payer: Keypair,
    pub(crate) feature_set: Arc<FeatureSet>,
    pub(crate) processor: TransactionBatchProcessor<TridentForkGraph>,
    pub(crate) fork_graph: Arc<RwLock<TridentForkGraph>>,
}

impl TransactionProcessingCallback for TridentSVM {
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

impl Default for TridentSVM {
    fn default() -> Self {
        let payer = Keypair::new();

        let mut client = Self {
            accounts: Default::default(),
            payer: payer.insecure_clone(),
            feature_set: Arc::new(FeatureSet::all_enabled()),
            processor: TransactionBatchProcessor::<TridentForkGraph>::new(1, 1, HashSet::default()),
            fork_graph: Arc::new(RwLock::new(TridentForkGraph {})),
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
