use std::sync::Arc;
use std::sync::RwLock;

use agave_feature_set::FeatureSet;
use shared_memory::ShmemConf;
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

use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::TransactionBatchProcessor;
use trident_syscall_stubs_v2::set_stubs_v2;

use crate::accounts_database::accounts_db::AccountsDB;
use crate::builder::TridentSVMBuilder;
use crate::fuzzing_metrics::stats::FuzzStats;
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
    pub(crate) fuzz_stats: Option<shared_memory::Shmem>,
}

impl TridentSVM {
    #[allow(dead_code)]
    #[deprecated(
        since = "0.4.0",
        note = "In the current TridentSVM version syscall stubs for Solana v1 are not supported anymore, switch to older TridentSVM version"
    )]
    pub(crate) fn initialize_syscalls_v1(&mut self) {
        // set_stubs_v1();
    }

    pub(crate) fn initialize_syscalls_v2(&mut self) {
        set_stubs_v2();
    }
    pub(crate) fn initialize_metrics(&mut self, fuzz_stats_path: String) {
        let shmem_id = format!("fuzzer_stats_{}", std::process::id());

        let shmem = ShmemConf::new()
            .size(std::mem::size_of::<FuzzStats>())
            .os_id(&shmem_id)
            .create()
            .expect("Failed to create shared memory");

        // Get a pointer to the shared memory
        let stats = unsafe { &mut *(shmem.as_ptr() as *mut FuzzStats) };

        // Initialize stats
        *stats = FuzzStats::new(fuzz_stats_path);

        self.fuzz_stats = Some(shmem);
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
            processor: TransactionBatchProcessor::<TridentForkGraph>::new(
                1,
                1,
                Arc::downgrade(&Arc::new(RwLock::new(TridentForkGraph {}))),
                None,
                None,
            ),
            fork_graph: Arc::new(RwLock::new(TridentForkGraph {})),
            fuzz_stats: None,
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

        let associated_token_program = TridentProgram::new(
            pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
            None,
            include_bytes!("solana-program-library/associated-token-program-mainnet.so").to_vec(),
        );

        self.deploy_binary_program(&associated_token_program);

        self
    }

    pub fn clear_accounts(&mut self) {
        self.accounts.reset_temp();
    }
}
