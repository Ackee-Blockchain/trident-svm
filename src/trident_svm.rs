use std::sync::Arc;
use std::sync::RwLock;

use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v2;

use solana_program_runtime::loaded_programs::ProgramCacheEntry;

use solana_account::AccountSharedData;
use solana_account::ReadableAccount;
use solana_clock::Clock;
use solana_epoch_rewards::EpochRewards;
use solana_epoch_schedule::EpochSchedule;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_pubkey::pubkey;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_signer::Signer;
use solana_slot_hashes::SlotHashes;
use solana_svm_feature_set::SVMFeatureSet;
#[allow(deprecated)]
use solana_sysvar::fees::Fees;
#[allow(deprecated)]
use solana_sysvar::recent_blockhashes::IterItem;
#[allow(deprecated)]
use solana_sysvar::recent_blockhashes::RecentBlockhashes;

use solana_slot_history::SlotHistory;

use solana_stake_interface::stake_history::StakeHistory;

use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::TransactionBatchProcessor;

use solana_svm_callback::InvokeContextCallback;
#[cfg(feature = "syscall-v2")]
use trident_syscall_stubs_v2::set_stubs_v2;

use crate::accounts_database::accounts_db::AccountsDB;
use crate::builder::TridentSVMBuilder;

use crate::trident_fork_graphs::TridentForkGraph;
use crate::utils;
use solana_builtins::BUILTINS;

use solana_program_runtime::execution_budget::SVMTransactionExecutionBudget;

use crate::types::trident_program::TridentProgram;
use crate::utils::get_current_timestamp;

pub struct TridentSVM {
    pub(crate) accounts: AccountsDB,
    pub(crate) payer: Keypair,
    pub(crate) feature_set: Arc<SVMFeatureSet>,
    pub(crate) processor: TransactionBatchProcessor<TridentForkGraph>,
    pub(crate) fork_graph: Arc<RwLock<TridentForkGraph>>,
}

impl TridentSVM {
    #[cfg(feature = "syscall-v2")]
    pub(crate) fn initialize_syscalls_v2(&mut self) {
        set_stubs_v2();
    }
}

impl InvokeContextCallback for TridentSVM {}

impl TransactionProcessingCallback for TridentSVM {
    fn account_matches_owners(&self, account: &Pubkey, owners: &[Pubkey]) -> Option<usize> {
        self.get_account_shared_data(account)
            .and_then(|account| owners.iter().position(|key| account.owner().eq(key)))
    }

    fn get_account_shared_data(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts.get_account(pubkey, false)
    }
}

impl Default for TridentSVM {
    fn default() -> Self {
        let payer = Keypair::new();

        let mut client = Self {
            accounts: Default::default(),
            payer: payer.insecure_clone(),
            feature_set: Arc::new(SVMFeatureSet::default()),
            processor: TransactionBatchProcessor::<TridentForkGraph>::new(
                1,
                1,
                Arc::downgrade(&Arc::new(RwLock::new(TridentForkGraph {}))),
                None,
                None,
            ),
            fork_graph: Arc::new(RwLock::new(TridentForkGraph {})),
        };

        let payer_account = AccountSharedData::new(
            500_000_000 * 1_000_000_000,
            0,
            &solana_sdk_ids::system_program::id(),
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
            let compute_budget = SVMTransactionExecutionBudget::default();

            let mut cache: std::sync::RwLockWriteGuard<
                '_,
                solana_program_runtime::loaded_programs::ProgramCache<TridentForkGraph>,
            > = self
                .processor
                .program_cache
                .write()
                .expect("Failed to write to program cache");

            cache.fork_graph = Some(Arc::downgrade(&self.fork_graph));

            cache.environments.program_runtime_v1 = Arc::new(
                create_program_runtime_environment_v1(
                    &self.feature_set,
                    &compute_budget,
                    false,
                    false,
                )
                .expect("Failed to create program runtime environment"),
            );
            cache.environments.program_runtime_v2 =
                Arc::new(create_program_runtime_environment_v2(&compute_budget, true));
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
                &utils::create_loadable_account_for_test(builtint.name),
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
