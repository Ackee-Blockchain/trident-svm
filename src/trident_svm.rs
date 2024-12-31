use std::sync::{Arc, RwLock};

use solana_sdk::{
    account::{AccountSharedData, ReadableAccount, WritableAccount},
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::{Clock, Slot},
    epoch_rewards::EpochRewards,
    epoch_schedule::EpochSchedule,
    feature_set::FeatureSet,
    fee::FeeStructure,
    hash::Hash,
    native_loader,
    native_token::LAMPORTS_PER_SOL,
    pubkey,
    pubkey::Pubkey,
    rent::Rent,
    signature::Keypair,
    signer::Signer,
    slot_hashes::SlotHashes,
    slot_history::SlotHistory,
    stake_history::StakeHistory,
    sysvar::{Sysvar, SysvarId},
};

#[allow(deprecated)]
use solana_sdk::sysvar::fees::Fees;
#[allow(deprecated)]
use solana_sdk::sysvar::recent_blockhashes::{IterItem, RecentBlockhashes};

use solana_svm::{
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_processor::{
        ExecutionRecordingConfig, LoadAndExecuteSanitizedTransactionsOutput,
        TransactionBatchProcessor, TransactionProcessingConfig, TransactionProcessingEnvironment,
    },
};

use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program_runtime::loaded_programs::{BlockRelation, ForkGraph, ProgramCacheEntry};

// use crate::{config::Config, fuzz_client::FuzzingProgram};

use std::collections::HashSet;

use super::log::{setup_solana_logging, turn_off_solana_logging};
use crate::{accounts_db::AccountsDB, native::BUILTINS};

pub struct TridentForkGraph {}

impl ForkGraph for TridentForkGraph {
    fn relationship(&self, _a: Slot, _b: Slot) -> BlockRelation {
        BlockRelation::Unknown
    }
}

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

impl TridentSVM<'_> {
    pub fn new() -> Self {
        TridentSVM::default()
            .with_processor()
            .with_sysvars()
            // .with_native_programs(program)
            // .with_sbf_programs(config)
            // .with_permanent_accounts(config)
            .with_builtins()
            .with_solana_program_library()
            .with_logging()
    }
    pub fn with_logging(mut self) -> Self {
        if std::env::var("TRIDENT_LOG").is_ok() {
            setup_solana_logging();
            self.tx_processing_config
                .recording_config
                .enable_log_recording = true;
        } else {
            turn_off_solana_logging();
            self.tx_processing_config
                .recording_config
                .enable_log_recording = false;
        }
        self
    }
    pub fn with_sysvars(mut self) -> Self {
        self.set_sysvar(&Clock::default());
        self.set_sysvar(&EpochRewards::default());
        self.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        let fees = Fees::default();
        self.set_sysvar(&fees);
        // self.set_sysvar(&LastRestartSlot::default());
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

        // self.set_sysvar(&Clock::default());
        // self.set_sysvar(&Rent::default());
        // #[allow(deprecated)]
        // let fees = Fees::default();
        // self.set_sysvar(&fees);

        self.processor.fill_missing_sysvar_cache_entries(&self);

        self
    }

    pub fn with_processor(self) -> Self {
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
    // pub fn with_sbf_programs(mut self, config: &Config) -> Self {
    //     config.fuzz.programs.iter().for_each(|sbf_target| {
    //         self.add_program(&sbf_target.address, &sbf_target.data, sbf_target.authority);
    //     });

    //     self
    // }
    // pub fn with_permanent_accounts(mut self, config: &Config) -> Self {
    //     config.fuzz.accounts.iter().for_each(|account| {
    //         self.accounts
    //             .add_permanent_account(&account.pubkey, &account.account);
    //     });

    //     self
    // }
    pub fn add_program(&mut self, address: &Pubkey, data: &[u8], authority: Option<Pubkey>) {
        let rent = Rent::default();

        let program_account = address;

        let program_data_account =
            bpf_loader_upgradeable::get_program_data_address(program_account);

        let state = UpgradeableLoaderState::Program {
            programdata_address: program_data_account,
        };

        let buffer = bincode::serialize(&state).unwrap();
        let account_data = AccountSharedData::create(
            rent.minimum_balance(buffer.len()),
            buffer,
            bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        self.accounts.add_program(program_account, &account_data);

        let state = UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: authority,
        };
        let mut header = bincode::serialize(&state).unwrap();

        let mut complement = vec![
            0;
            std::cmp::max(
                0,
                UpgradeableLoaderState::size_of_programdata_metadata().saturating_sub(header.len())
            )
        ];

        let mut buffer: Vec<u8> = data.to_vec();
        header.append(&mut complement);
        header.append(&mut buffer);

        let account_data = AccountSharedData::create(
            rent.minimum_balance(header.len()),
            header,
            bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        self.accounts
            .add_program(&program_data_account, &account_data);
    }
    pub fn with_solana_program_library(mut self) -> Self {
        self.add_program(
            &pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            include_bytes!("solana-program-library/spl-token-mainnet.so"),
            None,
        );
        self.add_program(
            &pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
            include_bytes!("solana-program-library/associated-token-program-mainnet.so"),
            None,
        );
        self
    }
    pub fn set_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        let account = AccountSharedData::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();
        self.accounts.add_sysvar(&T::id(), &account);
    }

    // #[allow(dead_code)]
    // #[doc = "Executing programs as native is currently not supported"]
    // #[doc = "thus programs can be included only as SBF binaries"]
    // pub fn with_native_programs(mut self, native_programs: &[FuzzingProgram]) -> Self {
    //     native_programs.iter().for_each(|native| {
    //         let entry = match native.entry {
    //             Some(entry) => entry,
    //             None => panic!("Native programs have to have entry specified"),
    //         };

    //         self.accounts.add_program(
    //             &native.program_id,
    //             &native_loader::create_loadable_account_for_test(&native.program_name),
    //         );

    //         let program_data_account =
    //             bpf_loader_upgradeable::get_program_data_address(&native.program_id);

    //         let state = UpgradeableLoaderState::ProgramData {
    //             slot: 0,
    //             upgrade_authority_address: native.authority,
    //         };
    //         let mut header = bincode::serialize(&state).unwrap();

    //         let mut complement = vec![
    //             0;
    //             std::cmp::max(
    //                 0,
    //                 UpgradeableLoaderState::size_of_programdata_metadata()
    //                     .saturating_sub(header.len())
    //             )
    //         ];

    //         let mut buffer: Vec<u8> = vec![];
    //         header.append(&mut complement);
    //         header.append(&mut buffer);

    //         let rent = Rent::default();

    //         let account_data = AccountSharedData::create(
    //             rent.minimum_balance(header.len()),
    //             header,
    //             bpf_loader_upgradeable::id(),
    //             true,
    //             Default::default(),
    //         );

    //         self.accounts
    //             .add_program(&program_data_account, &account_data);

    //         self.processor.add_builtin(
    //             &self,
    //             native.program_id,
    //             &native.program_name,
    //             ProgramCacheEntry::new_builtin(0, native.program_name.len(), entry),
    //         );
    //     });

    //     self
    // }
    pub fn with_builtins(mut self) -> Self {
        BUILTINS.iter().for_each(|builtint| {
            self.accounts.add_program(
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
    pub fn settle(&mut self, output: &LoadAndExecuteSanitizedTransactionsOutput) {
        for x in output.loaded_transactions.iter().flatten() {
            for account in &x.accounts {
                if !account.1.executable() && account.1.owner() != &solana_sdk::sysvar::id() {
                    self.accounts.add_account(&account.0, &account.1);
                }
            }
        }
    }
    pub fn clear_accounts(&mut self) {
        self.accounts.reset_temp();
        let payer_account = AccountSharedData::new(
            500_000_000 * LAMPORTS_PER_SOL,
            0,
            &solana_sdk::system_program::ID,
        );
        self.accounts
            .add_account(&self.payer.pubkey(), &payer_account);
    }
}
