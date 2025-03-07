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
use solana_sdk::sysvar::Sysvar;
use solana_sdk::sysvar::SysvarId;
use solana_sdk::transaction;
use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::transaction::Transaction;
use solana_sdk::transaction::TransactionError;

use solana_svm::account_loader::CheckedTransactionDetails;
use solana_svm::transaction_processor::ExecutionRecordingConfig;
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;
use solana_svm::transaction_processor::TransactionProcessingConfig;
use solana_svm::transaction_processor::TransactionProcessingEnvironment;

use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program_runtime::loaded_programs::ProgramCacheEntry;

use crate::log::setup_solana_logging;
use crate::log::turn_off_solana_logging;
use crate::native::BUILTINS;
use crate::trident_fork_graphs::TridentForkGraph;
use crate::trident_svm::TridentSVM;
use crate::utils::get_current_timestamp;
use crate::utils::ProgramEntrypoint;
use crate::utils::SBFTarget;
use crate::utils::TridentAccountSharedData;

use trident_syscall_stubs_v1::set_stubs_v1;
use trident_syscall_stubs_v2::set_stubs_v2;

impl TridentSVM {
    pub fn new(
        program_entries: &[ProgramEntrypoint],
        sbf_programs: &[SBFTarget],
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
    }
    pub fn new_with_syscalls(
        program_entries: &[ProgramEntrypoint],
        sbf_programs: &[SBFTarget],
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
            .with_syscalls_v1()
            .with_syscalls_v2()
    }
    fn with_syscalls_v1(self) -> Self {
        set_stubs_v1();
        self
    }
    fn with_syscalls_v2(self) -> Self {
        set_stubs_v2();

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

        self
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
    fn with_sbf_programs(mut self, sbf_programs: &[SBFTarget]) -> Self {
        sbf_programs.iter().for_each(|sbf_target| {
            self.add_program(
                &sbf_target.program_id,
                &sbf_target.data,
                sbf_target.authority,
            );
        });

        self
    }
    fn with_permanent_accounts(mut self, permanent_accounts: &[TridentAccountSharedData]) -> Self {
        permanent_accounts.iter().for_each(|account| {
            self.accounts
                .add_permanent_account(&account.address, &account.account);
        });

        self
    }

    fn with_solana_program_library(mut self) -> Self {
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

    fn with_native_programs(mut self, native_programs: &[ProgramEntrypoint]) -> Self {
        native_programs.iter().for_each(|native| {
            self.add_native_program(native);
        });

        self
    }
    fn with_builtins(mut self) -> Self {
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

impl TridentSVM {
    pub fn set_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        self.accounts.add_sysvar(sysvar);
    }

    pub fn get_sysvar<T: Sysvar>(&self) -> T {
        self.accounts.get_sysvar()
    }
    pub fn add_temp_account(&mut self, address: &Pubkey, account: &AccountSharedData) {
        self.accounts.add_account(address, account);
    }
    pub fn get_account(&self, address: &Pubkey) -> Option<AccountSharedData> {
        self.accounts.get_account(address)
    }
    pub fn get_payer(&self) -> Keypair {
        self.payer.insecure_clone()
    }
    pub fn deploy_sbf_program(&mut self, sbf_program: SBFTarget) {
        self.add_program(
            &sbf_program.program_id,
            &sbf_program.data,
            sbf_program.authority,
        );
    }
    pub fn deploy_native_program(&mut self, native_program: ProgramEntrypoint) {
        self.add_native_program(&native_program);
    }
    fn add_program(&mut self, address: &Pubkey, data: &[u8], authority: Option<Pubkey>) {
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
    fn add_native_program(&mut self, native_program: &ProgramEntrypoint) {
        let entry = match native_program.entry {
            Some(entry) => entry,
            None => panic!("Native programs have to have entry specified"),
        };

        self.accounts.add_program(
            &native_program.program_id,
            &native_loader::create_loadable_account_for_test("program-name"),
        );

        let program_data_account =
            bpf_loader_upgradeable::get_program_data_address(&native_program.program_id);

        let state = UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: native_program.authority,
        };
        let mut header = bincode::serialize(&state).unwrap();

        let mut complement = vec![
            0;
            std::cmp::max(
                0,
                UpgradeableLoaderState::size_of_programdata_metadata().saturating_sub(header.len())
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
            self,
            native_program.program_id,
            "program-name",
            ProgramCacheEntry::new_builtin(0, "program-name".len(), entry),
        );
    }
}

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
            epoch_total_stake: None,
            epoch_vote_accounts: None,
            feature_set: self.feature_set.clone(),
            fee_structure: None,
            lamports_per_signature,
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
        let execution_result = if result.execution_results.len() != 1 {
            return Err(TransactionError::ProgramCacheHitMaxLimit);
        } else {
            &result.execution_results[0]
        };

        match &execution_result {
            solana_svm::transaction_results::TransactionExecutionResult::Executed {
                details,
                ..
            } => {
                details
                    .status
                    .as_ref()
                    .map_err(|transaction_error| transaction_error.clone())?;

                match &result.loaded_transactions[0] {
                    Ok(loaded_transaction) => {
                        self.settle_accounts(&loaded_transaction.accounts);
                        Ok(())
                    }
                    Err(transaction_error) => Err(transaction_error.clone()),
                }
            }
            solana_svm::transaction_results::TransactionExecutionResult::NotExecuted(
                transaction_error,
            ) => Err(transaction_error.clone()),
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
    fn settle_accounts(&mut self, accounts: &[(Pubkey, AccountSharedData)]) {
        for account in accounts {
            if !account.1.executable() && account.1.owner() != &solana_sdk::sysvar::id() {
                // Update permanent account if it should be updated
                if self.accounts.get_permanent_account(&account.0).is_some() {
                    self.accounts.add_permanent_account(&account.0, &account.1);
                } else {
                    // Otherwise, add it to the temp accounts
                    self.accounts.add_account(&account.0, &account.1);
                }
            }
        }
    }
}
