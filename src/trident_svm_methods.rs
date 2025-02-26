use std::collections::HashSet;
use std::sync::Arc;

use solana_program_runtime::log_collector::log::debug;
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
use solana_sdk::sysvar::recent_blockhashes::Entry;
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
    fn with_syscalls_v1(self) -> Self {
        set_stubs_v1();
        self
    }
    fn with_syscalls_v2(self) -> Self {
        set_stubs_v2();

        self
    }
    fn with_logging(mut self) -> Self {
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
    fn with_sysvars(mut self) -> Self {
        self.set_sysvar(&Clock::default());
        self.set_sysvar(&EpochRewards::default());
        self.set_sysvar(&EpochSchedule::default());
        #[allow(deprecated)]
        let fees = Fees::default();
        self.set_sysvar(&fees);
        // self.set_sysvar(&LastRestartSlot::default());
        let latest_blockhash = self.blockhash_config.latest_blockhash;
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
    fn with_sbf_programs(mut self, sbf_programs: &[SBFTargets]) -> Self {
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

impl TridentSVM<'_> {
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
    pub fn get_latest_blockhash(&self) -> Hash {
        self.blockhash_config.latest_blockhash
    }
    #[allow(deprecated)]
    pub fn expire_blockhash(&mut self) {
        const MAX_RECENT_BLOCKHASHES: usize = 1;
        self.blockhash_config.expire_blockhash();

        // Get existing blockhashes or create new vec if None
        let mut recent_hashes = self
            .get_sysvar::<RecentBlockhashes>()
            .iter()
            .map(|item| Entry::new(&item.blockhash, item.fee_calculator.lamports_per_signature))
            .collect::<Vec<_>>();
        recent_hashes.insert(
            0,
            Entry::new(
                &self.blockhash_config.latest_blockhash,
                FeeStructure::default().lamports_per_signature,
            ),
        );

        recent_hashes.truncate(MAX_RECENT_BLOCKHASHES);

        self.set_sysvar(&RecentBlockhashes::from_iter(recent_hashes.iter().map(
            |entry| {
                IterItem(
                    0,
                    &entry.blockhash,
                    entry.fee_calculator.lamports_per_signature,
                )
            },
        )));
    }
    pub fn set_blockhash_check(&mut self, check: bool) {
        self.blockhash_config.blockhash_check = check;
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
}

impl TridentSVM<'_> {
    pub fn process_transaction(
        &mut self,
        transaction: Transaction,
    ) -> LoadAndExecuteSanitizedTransactionsOutput {
        // reset sysvar cache
        self.processor.reset_sysvar_cache();

        // replenish sysvar cache with sysvars from the accounts db
        self.processor.fill_missing_sysvar_cache_entries(self);

        // create sanitized transaction
        let sanitezed_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())
                .unwrap();

        if let Err(err) = self.check_transaction_validity(&sanitezed_tx) {
            return LoadAndExecuteSanitizedTransactionsOutput {
                loaded_transactions: vec![Err(err)],
                execution_results: vec![],
                error_metrics: Default::default(),
                execute_timings: Default::default(),
            };
        }

        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        // execute transaction
        self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1, lamports_per_signature),
            &self.tx_processing_environment,
            &self.tx_processing_config,
        )
    }
    pub fn process_transaction_with_settle(
        &mut self,
        transaction: Transaction,
    ) -> solana_sdk::transaction::Result<()> {
        // reset sysvar cache
        self.processor.reset_sysvar_cache();

        // replenish sysvar cache with sysvars from the accounts db
        self.processor.fill_missing_sysvar_cache_entries(self);

        // create sanitized transaction
        let sanitezed_tx =
            SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new())?;

        self.check_transaction_validity(&sanitezed_tx)?;

        // get fee structure
        let fee_structure = FeeStructure::default();
        let lamports_per_signature = fee_structure.lamports_per_signature;

        // execute transaction
        let result = self.processor.load_and_execute_sanitized_transactions(
            self,
            &[sanitezed_tx],
            get_transaction_check_results(1, lamports_per_signature),
            &self.tx_processing_environment,
            &self.tx_processing_config,
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
    pub fn settle_accounts(&mut self, accounts: &[(Pubkey, AccountSharedData)]) {
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
    fn check_transaction_validity(&mut self, sanitized_tx: &SanitizedTransaction) -> Result<(), TransactionError> {
        if self.blockhash_config.blockhash_check {
            self.check_blockhash_validity(sanitized_tx)?;
            self.is_transaction_hash_valid(sanitized_tx)?;
        }
        Ok(())
    }
    fn check_blockhash_validity(
        &self,
        sanitized_tx: &SanitizedTransaction,
    ) -> Result<(), TransactionError> {
            let recent_blockhash = sanitized_tx.message().recent_blockhash();

            // Check if blockhash is in recent blockhashes list
            #[allow(deprecated)]
            let is_blockhash_valid = self
                .get_sysvar::<RecentBlockhashes>()
                .iter()
                .any(|item| &item.blockhash == recent_blockhash);

            if is_blockhash_valid {
                Ok(())
            } else {
                debug!(
                    "Blockhash {} not found in recent blockhashes",
                    recent_blockhash
                );
                Err(TransactionError::BlockhashNotFound)
            }
    }
    fn is_transaction_hash_valid(&mut self, sanitized_tx: &SanitizedTransaction) -> Result<(), TransactionError> {
        let transaction_hash = sanitized_tx.message_hash();
        self.blockhash_config.block_contains_transaction(transaction_hash)
    }
}
