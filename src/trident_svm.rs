use std::collections::HashSet;
use std::sync::Arc;
use std::sync::RwLock;

use solana_program_runtime::loaded_programs::ProgramCacheEntry;
use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::epoch_rewards::EpochRewards;
use solana_sdk::epoch_schedule::EpochSchedule;
use solana_sdk::hash::Hash;
use solana_sdk::native_loader;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::rent::Rent;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::slot_hashes::SlotHashes;
use solana_sdk::slot_history::SlotHistory;
use solana_sdk::stake_history::StakeHistory;
#[allow(deprecated)]
use solana_sdk::sysvar::fees::Fees;
#[allow(deprecated)]
use solana_sdk::sysvar::recent_blockhashes::IterItem;
#[allow(deprecated)]
use solana_sdk::sysvar::recent_blockhashes::RecentBlockhashes;

use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_sdk::feature_set::FeatureSet;

use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::TransactionBatchProcessor;
use trident_syscall_stubs_v1::set_stubs_v1;
use trident_syscall_stubs_v2::set_stubs_v2;

use crate::accounts_database::accounts_db::AccountsDB;
use crate::builder::TridentSVMBuilder;
use crate::native::BUILTINS;
use crate::trident_fork_graphs::TridentForkGraph;

use crate::types::trident_program::TridentProgram;
use crate::utils::get_current_timestamp;

pub struct TridentSVM {
    pub(crate) accounts: AccountsDB,
    pub(crate) payer: Keypair,
    pub(crate) feature_set: Arc<FeatureSet>,
    pub(crate) processor: TransactionBatchProcessor<TridentForkGraph>,
    pub(crate) fork_graph: Arc<RwLock<TridentForkGraph>>,
}

impl TridentSVM {
    pub(crate) fn initialize_syscalls_v1(&mut self) {
        set_stubs_v1();
    }

    pub(crate) fn initialize_syscalls_v2(&mut self) {
        set_stubs_v2();
    }
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
        client
            .accounts
            .set_permanent_account(&payer.pubkey(), &payer_account);

        client
            .with_processor()
            .with_sysvars()
            .with_builtins()
            .with_solana_program_library()
    }
}

impl TridentSVM {
    pub fn builder() -> TridentSVMBuilder {
        TridentSVMBuilder::new()
    }
    fn with_processor(self) -> Self {
        {
            let compute_budget = ComputeBudget::default();

            let mut cache: std::sync::RwLockWriteGuard<
                '_,
                solana_program_runtime::loaded_programs::ProgramCache<TridentForkGraph>,
            > = self.processor.program_cache.write().unwrap();

            cache.fork_graph = Some(Arc::downgrade(&self.fork_graph));

            cache.environments.program_runtime_v1 = Arc::new(
                create_program_runtime_environment_v1(
                    &self.feature_set,
                    &compute_budget,
                    true,
                    true,
                )
                .unwrap(),
            );
            // cache.environments.program_runtime_v2 =
            //     Arc::new(create_program_runtime_environment_v2(&compute_budget, true));
        }

        self
    }

    fn with_sysvars(mut self) -> Self {
        let clock = Clock {
            unix_timestamp: get_current_timestamp() as i64,
            ..Default::default()
        };
        self.set_sysvar(&clock);
        self.set_sysvar(&EpochRewards::default());
        self.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        let fees = Fees::default();
        self.set_sysvar(&fees);
        let latest_blockhash = Hash::default();
        #[allow(deprecated)]
        self.set_sysvar(&RecentBlockhashes::from_iter([IterItem(
            0,
            &latest_blockhash,
            fees.fee_calculator.lamports_per_signature,
        )]));
        self.set_sysvar(&Rent::default());
        self.set_sysvar(&SlotHashes::new(&[(0, latest_blockhash)]));
        self.set_sysvar(&SlotHistory::default());
        self.set_sysvar(&StakeHistory::default());

        self
    }
    fn with_builtins(mut self) -> Self {
        BUILTINS.iter().for_each(|builtint| {
            self.accounts.set_program(
                &builtint.program_id,
                &native_loader::create_loadable_account_for_test(builtint.name),
            );

            self.processor.add_builtin(
                &self,
                builtint.program_id,
                builtint.name,
                ProgramCacheEntry::new_builtin(0, builtint.name.len(), builtint.entrypoint),
            );
        });

        self
    }
    fn with_solana_program_library(mut self) -> Self {
        let spl_token = TridentProgram::new(
            pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            None,
            include_bytes!("solana-program-library/spl-token-mainnet.so").to_vec(),
        );

        self.deploy_binary_program(&spl_token);
        
        // SPL Token 2022 added for new Token 2022 Trident features
        let spl_token_2022 = TridentProgram::new(
            pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
            None,
            include_bytes!("solana-program-library/spl-2022-token-mainnet.so").to_vec(),
        );

        self.deploy_binary_program(&spl_token_2022);

        let associated_token_program = TridentProgram::new(
            pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
            None,
            include_bytes!("solana-program-library/associated-token-program-mainnet.so").to_vec(),
        );

        self.deploy_binary_program(&associated_token_program);

        let metaplex_token_metadata = TridentProgram::new(
            pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"),
            None,
            include_bytes!("solana-program-library/metaplex-token-metadata.so").to_vec(),
        );

        self.deploy_binary_program(&metaplex_token_metadata);

        // Interesting to have an Oracle program for testing programs with Price feed manipulation
        // Another good program would be Pyth Oracle, which is good for cross-chain price feeds 
        let chainlink_oracle = TridentProgram::new(
            pubkey!("HEvSKofvBgfaexv23kMabbYqxasxU3mQ4ibBMEmJWHny"),
            None,
            include_bytes!("solana-program-library/chainlink-oracle.so").to_vec(),
        );

        self.deploy_binary_program(&chainlink_oracle);

        // Native Stake Pool (SPL Stake Pool):
        // This program is used for managing stake pools, which can be useful for testing programs that interact with staking
        let spl_stake_pool = TridentProgram::new(
            pubkey!("SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy"),
            None,
            include_bytes!("solana-program-library/spl-stake-pool.so").to_vec(),
        );
        self.deploy_binary_program(&spl_stake_pool);

        // Could be interesting to have candy machine for testing programs that interact with it Minting NFTs
        let metaplex_candy_machine_v3 = TridentProgram::new(
            pubkey!("CndyV3LdqHUfDLmE5naZjVN8rBZz4tqhdefbAnjHG3JR"),
            None,
            include_bytes!("solana-program-library/metaplex-candy-machine-v3.so").to_vec(),
        );
        self.deploy_binary_program(&metaplex_candy_machine_v3);

        self
    }

    pub fn clear_accounts(&mut self) {
        self.accounts.reset_temp();
    }
}
