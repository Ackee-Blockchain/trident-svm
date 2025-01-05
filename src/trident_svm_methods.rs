use std::collections::HashSet;
use std::sync::Arc;

use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::account::WritableAccount;
use solana_sdk::bpf_loader_upgradeable;
use solana_sdk::bpf_loader_upgradeable::UpgradeableLoaderState;
use solana_sdk::clock::Clock;
use solana_sdk::epoch_rewards::EpochRewards;
use solana_sdk::epoch_schedule::EpochSchedule;
use solana_sdk::fee::FeeStructure;
use solana_sdk::hash::Hash;
use solana_sdk::native_loader;
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::rent::Rent;
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
use solana_sdk::sysvar::Sysvar;
use solana_sdk::sysvar::SysvarId;
use solana_sdk::transaction;
use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::transaction::Transaction;
use solana_svm::account_loader::CheckedTransactionDetails;

use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;

use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program_runtime::loaded_programs::ProgramCacheEntry;

use crate::log::setup_solana_logging;
use crate::log::turn_off_solana_logging;
use crate::native::BUILTINS;
use crate::trident_fork_graphs::TridentForkGraph;
use crate::trident_svm::TridentSVM;
use crate::utils::ProgramEntrypoint;
use crate::utils::SBFTargets;
use crate::utils::TridentAccountSharedData;
use trident_syscall_stubs_v1::set_stubs_v1;
use trident_syscall_stubs_v2::set_stubs_v2;

impl TridentSVM<'_> {
    pub fn new(
        program_entries: &[ProgramEntrypoint],
        sbf_programs: &[SBFTargets],
        permanent_accounts: &[TridentAccountSharedData],
    ) -> Self {
        TridentSVM::default()
            .with_processor()
            .with_sysvars()
            .with_native_programs(program_entries)
            .with_sbf_programs(sbf_programs)
            .with_permanent_accounts(permanent_accounts)
            .with_builtins()
            .with_solana_program_library()
            .with_logging()
    }
    pub fn new_with_syscalls(
        program_entries: &[ProgramEntrypoint],
        sbf_programs: &[SBFTargets],
        permanent_accounts: &[TridentAccountSharedData],
    ) -> Self {
        TridentSVM::default()
            .with_processor()
            .with_sysvars()
            .with_native_programs(program_entries)
            .with_sbf_programs(sbf_programs)
            .with_permanent_accounts(permanent_accounts)
            .with_builtins()
            .with_solana_program_library()
            .with_logging()
            .with_syscalls_v1()
            .with_syscalls_v2()
    }
    pub fn with_syscalls_v1(self) -> Self {
        set_stubs_v1();
        self
    }
    pub fn with_syscalls_v2(self) -> Self {
        set_stubs_v2();

        self
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
    pub fn with_sbf_programs(mut self, sbf_programs: &[SBFTargets]) -> Self {
        sbf_programs.iter().for_each(|sbf_target| {
            self.add_program(
                &sbf_target.program_id,
                &sbf_target.data,
                sbf_target.authority,
            );
        });

        self
    }
    pub fn with_permanent_accounts(
        mut self,
        permanent_accounts: &[TridentAccountSharedData],
    ) -> Self {
        permanent_accounts.iter().for_each(|account| {
            self.accounts
                .add_permanent_account(&account.address, &account.account);
        });

        self
    }
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

    pub fn with_native_programs(mut self, native_programs: &[ProgramEntrypoint]) -> Self {
        native_programs.iter().for_each(|native| {
            let entry = match native.entry {
                Some(entry) => entry,
                None => panic!("Native programs have to have entry specified"),
            };

            self.accounts.add_program(
                &native.program_id,
                &native_loader::create_loadable_account_for_test("program-name"),
            );

            let program_data_account =
                bpf_loader_upgradeable::get_program_data_address(&native.program_id);

            let state = UpgradeableLoaderState::ProgramData {
                slot: 0,
                upgrade_authority_address: native.authority,
            };
            let mut header = bincode::serialize(&state).unwrap();

            let mut complement = vec![
                0;
                std::cmp::max(
                    0,
                    UpgradeableLoaderState::size_of_programdata_metadata()
                        .saturating_sub(header.len())
                )
            ];

            let mut buffer: Vec<u8> = vec![];
            header.append(&mut complement);
            header.append(&mut buffer);

            let rent = Rent::default();

            let account_data = AccountSharedData::create(
                rent.minimum_balance(header.len()),
                header,
                bpf_loader_upgradeable::id(),
                true,
                Default::default(),
            );

            self.accounts
                .add_program(&program_data_account, &account_data);

            self.processor.add_builtin(
                &self,
                native.program_id,
                "program-name",
                ProgramCacheEntry::new_builtin(0, "program-name".len(), entry),
            );
        });

        self
    }
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
    pub fn process_transaction(
        &mut self,
        transaction: Transaction,
    ) -> LoadAndExecuteSanitizedTransactionsOutput {
        let sanitezed_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())
                .unwrap();

        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1, lamports_per_signature),
            &self.tx_processing_environment,
            &self.tx_processing_config,
        )
    }
}

// This function is also a mock. In the Agave validator, the bank pre-checks
// transactions before providing them to the SVM API. We mock this step in
// PayTube, since we don't need to perform such pre-checks.
pub(crate) fn get_transaction_check_results(
    len: usize,
    lamports_per_signature: u64,
) -> Vec<transaction::Result<CheckedTransactionDetails>> {
    vec![
        transaction::Result::Ok(CheckedTransactionDetails {
            nonce: None,
            lamports_per_signature,
        });
        len
    ]
}
