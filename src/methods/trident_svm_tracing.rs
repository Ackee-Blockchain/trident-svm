//! Custom execution path for trace extraction.
//!
//! This module provides a way to execute transactions and extract sBPF VM traces
//! that can be used for coverage-guided fuzzing (e.g., AFL feedback).
//!
//! The standard SVM API doesn't expose traces because it drops InvokeContext
//! after execution. This module reimplements the execution path to capture traces.

use std::collections::HashSet;
use std::rc::Rc;

use solana_account::AccountSharedData;
use solana_account::ReadableAccount;
use solana_account::WritableAccount;
use solana_clock::Clock;
use solana_hash::Hash;
use solana_log_collector::LogCollector;
use solana_program_runtime::execution_budget::SVMTransactionExecutionBudget;
use solana_program_runtime::execution_budget::SVMTransactionExecutionCost;
use solana_program_runtime::invoke_context::EnvironmentConfig;
use solana_program_runtime::invoke_context::InvokeContext;
use solana_program_runtime::loaded_programs::ProgramCacheForTxBatch;
use solana_program_runtime::loaded_programs::ProgramCacheMatchCriteria;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_sdk_ids::native_loader;
use solana_svm::program_loader::load_program_with_pubkey;
use solana_svm_callback::TransactionProcessingCallback;
use solana_svm_transaction::svm_message::SVMMessage;
use solana_timings::ExecuteTimings;
use solana_transaction::sanitized::SanitizedTransaction;
use solana_transaction::Transaction;
use solana_transaction_context::ExecutionRecord;
use solana_transaction_context::IndexOfAccount;
use solana_transaction_context::InstructionAccount;
use solana_transaction_context::TransactionContext;
use solana_transaction_error::TransactionError;
use std::sync::Arc;

use crate::trident_svm::TridentSVM;
use crate::types::transaction_result::ExecutionTraces;
use crate::types::transaction_result::TracedTransactionResult;

impl TridentSVM {
    /// Execute a transaction and extract sBPF VM traces.
    ///
    /// This method creates its own execution path to get access to InvokeContext
    /// and extract traces before they're lost. The result includes both the
    /// standard transaction processing output and the execution traces.
    ///
    /// The traces can be used for coverage-guided fuzzing (e.g., AFL feedback)
    /// without requiring LLVM instrumentation when compiling Solana programs.
    ///
    /// Use `result.get_traces()` to access the traces after execution.
    ///
    /// **Important:** This uses `enable_instruction_tracing` which must be enabled
    /// in the runtime environment. Make sure `debugging_features = true` when
    /// creating the program runtime environment.
    pub fn execute_transaction_with_traces(
        &mut self,
        transaction: Transaction,
    ) -> TracedTransactionResult {
        // Reset and fill sysvar cache
        self.processor.reset_sysvar_cache();
        self.processor.fill_missing_sysvar_cache_entries(self);

        // Get current transaction timestamp
        let transaction_timestamp =
            self.accounts.deserialize_sysvar::<Clock>().unix_timestamp as u64;

        // Sanitize transaction
        let sanitized_tx =
            match SanitizedTransaction::try_from_legacy_transaction(transaction, &HashSet::new()) {
                Ok(tx) => tx,
                Err(_e) => {
                    return TracedTransactionResult::new(
                        Err(TransactionError::SanitizeFailure),
                        transaction_timestamp,
                        Vec::new(),
                        Vec::new(),
                    );
                }
            };

        // Load accounts
        let (accounts, program_indices) = match self.load_transaction_accounts(&sanitized_tx) {
            Ok(result) => result,
            Err(e) => {
                return TracedTransactionResult::new(
                    Err(e),
                    transaction_timestamp,
                    Vec::new(),
                    Vec::new(),
                );
            }
        };

        // Execute with trace extraction
        let (result, traces, _execute_timings, updated_accounts, logs) =
            self.execute_with_traces_internal(&sanitized_tx, accounts, program_indices);

        // Pass traces to collector if available
        if let Some(trace_collector) = &mut self.trace_collector {
            trace_collector.trace(&traces);
        }

        // On success, settle accounts (like the standard implementation)
        if result.is_ok() {
            self.settle_accounts(&updated_accounts);
        }

        // Update clock
        self.accounts.update_clock();

        TracedTransactionResult::new(result, transaction_timestamp, logs, traces)
    }

    /// Load accounts for a transaction.
    fn load_transaction_accounts(
        &self,
        tx: &SanitizedTransaction,
    ) -> Result<(Vec<(Pubkey, AccountSharedData)>, Vec<Vec<IndexOfAccount>>), TransactionError>
    {
        let message = tx.message();
        let account_keys = message.account_keys();

        // Load all accounts
        let mut accounts: Vec<(Pubkey, AccountSharedData)> = Vec::with_capacity(account_keys.len());
        for key in account_keys.iter() {
            let account = if solana_sdk_ids::sysvar::instructions::check_id(key) {
                // Special handling for instructions sysvar
                AccountSharedData::default()
            } else {
                self.get_account_shared_data(key).unwrap_or_else(|| {
                    let mut default = AccountSharedData::default();
                    default.set_rent_epoch(0);
                    default
                })
            };
            accounts.push((*key, account));
        }

        // Build program indices for each instruction
        let builtins_start_index = accounts.len();
        let program_indices: Result<Vec<Vec<IndexOfAccount>>, TransactionError> = message
            .instructions()
            .iter()
            .map(|instruction| {
                let program_index = instruction.program_id_index as usize;
                let (program_id, program_account) = accounts
                    .get(program_index)
                    .ok_or(TransactionError::ProgramAccountNotFound)?;

                if native_loader::check_id(program_id) {
                    return Ok(vec![]);
                }

                if !program_account.executable() {
                    return Err(TransactionError::InvalidProgramForExecution);
                }

                let indices: Vec<IndexOfAccount> = vec![program_index as IndexOfAccount];

                let owner_id = program_account.owner();
                if !native_loader::check_id(owner_id) {
                    // Check if owner is already in accounts
                    let owner_exists = accounts
                        .get(builtins_start_index..)
                        .map(|slice| slice.iter().any(|(k, _)| k == owner_id))
                        .unwrap_or(false);

                    if !owner_exists {
                        if let Some(owner_account) = self.get_account_shared_data(owner_id) {
                            if native_loader::check_id(owner_account.owner())
                                && owner_account.executable()
                            {
                                // We need to add the owner, but we can't mutate here
                                // For simplicity, just use the program index
                            }
                        }
                    }
                }

                Ok(indices)
            })
            .collect();

        Ok((accounts, program_indices?))
    }

    /// Execute transaction with an InvokeContext and extract traces.
    /// Returns (result, traces, timings, updated_accounts, logs)
    fn execute_with_traces_internal(
        &mut self,
        tx: &SanitizedTransaction,
        accounts: Vec<(Pubkey, AccountSharedData)>,
        program_indices: Vec<Vec<IndexOfAccount>>,
    ) -> (
        Result<(), TransactionError>,
        ExecutionTraces,
        ExecuteTimings,
        Vec<(Pubkey, AccountSharedData)>,
        Vec<String>,
    ) {
        let compute_budget = SVMTransactionExecutionBudget::default();
        let rent = Rent::default();

        // Create transaction context
        let mut transaction_context = TransactionContext::new(
            accounts,
            rent,
            compute_budget.max_instruction_stack_depth,
            compute_budget.max_instruction_trace_length,
        );

        // Get builtin program IDs - we need ALL of them in the batch cache
        let builtin_ids: HashSet<Pubkey> =
            self.processor.builtin_program_ids.read().unwrap().clone();

        // Build list of ALL programs needed for this transaction
        // This includes both the programs being invoked AND all builtins (loaders)
        let message = tx.message();
        let mut programs_to_load: Vec<(Pubkey, (ProgramCacheMatchCriteria, u64))> = message
            .program_instructions_iter()
            .filter_map(|(program_id, _)| {
                // Only skip the native_loader itself - all other programs need to be loaded
                if native_loader::check_id(program_id) {
                    None
                } else {
                    Some((*program_id, (ProgramCacheMatchCriteria::NoCriteria, 1)))
                }
            })
            .collect();

        // Add ALL builtins to programs_to_load - this is critical!
        // The loaders (bpf_loader, bpf_loader_upgradeable, etc.) must be in the batch cache
        for builtin_id in &builtin_ids {
            programs_to_load.push((*builtin_id, (ProgramCacheMatchCriteria::NoCriteria, 1)));
        }

        // Load any missing programs from accounts BEFORE acquiring the cache lock
        // This mirrors what replenish_program_cache does in the standard SVM
        let environments = self
            .processor
            .program_cache
            .read()
            .unwrap()
            .get_environments_for_epoch(1);

        let mut loaded_programs: Vec<(
            Pubkey,
            Arc<solana_program_runtime::loaded_programs::ProgramCacheEntry>,
        )> = Vec::new();
        for (program_id, _) in &programs_to_load {
            // Skip builtins - they're already in the cache
            if builtin_ids.contains(program_id) {
                continue;
            }

            // Try to load program from accounts
            if let Some(program_entry) = load_program_with_pubkey(
                self,
                &environments,
                program_id,
                1, // slot
                &mut ExecuteTimings::default(),
                false,
            ) {
                loaded_programs.push((*program_id, program_entry));
            }
        }

        // Now add loaded programs to the main cache (only if not already present)
        {
            let mut program_cache = self.processor.program_cache.write().unwrap();
            for (program_id, program_entry) in &loaded_programs {
                // Check if program is already in cache to avoid "Unexpected replacement" panic
                // This happens in persistent fuzzing mode where the same TridentSVM is reused
                if program_cache
                    .get_flattened_entries(true, true)
                    .iter()
                    .any(|(id, _)| id == program_id)
                {
                    continue; // Already in cache, skip
                }
                program_cache.assign_program(*program_id, Arc::clone(program_entry));
            }
        }

        // Create program cache for this batch and populate it
        let mut program_cache_for_tx_batch = {
            let program_cache = self.processor.program_cache.read().unwrap();
            let mut batch_cache = ProgramCacheForTxBatch::new_from_cache(
                1, // slot
                1, // epoch
                &program_cache,
            );

            // Extract programs from the main cache into the batch cache
            program_cache.extract(&mut programs_to_load, &mut batch_cache, true);

            // Also add freshly loaded programs to batch cache
            for (program_id, program_entry) in loaded_programs {
                if batch_cache.find(&program_id).is_none() {
                    batch_cache.replenish(program_id, program_entry);
                }
            }

            batch_cache
        };

        // Get sysvar cache
        let sysvar_cache = self.processor.sysvar_cache();
        let blockhash = Hash::default();

        // Create log collector for capturing execution logs
        let log_collector = Some(LogCollector::new_ref());

        // Create InvokeContext
        let mut invoke_context = InvokeContext::new(
            &mut transaction_context,
            &mut program_cache_for_tx_batch,
            EnvironmentConfig::new(
                blockhash,
                0, // lamports_per_signature
                self,
                &self.feature_set,
                &sysvar_cache,
            ),
            log_collector.clone(),
            compute_budget,
            SVMTransactionExecutionCost::default(),
        );

        // Execute the message (our own implementation using public APIs)
        let mut total_compute_units = 0u64;
        let mut execute_timings = ExecuteTimings::default();

        let result = self.process_message_internal(
            tx,
            &program_indices,
            &mut invoke_context,
            &mut total_compute_units,
            &mut execute_timings,
        );

        // Extract traces BEFORE dropping invoke_context
        let mut traces: ExecutionTraces = invoke_context.get_traces().clone();

        // Also get traces from any remaining syscall contexts
        for syscall_ctx_opt in &invoke_context.syscall_context {
            if let Some(syscall_ctx) = syscall_ctx_opt {
                if !syscall_ctx.trace_log.is_empty() {
                    traces.push(syscall_ctx.trace_log.clone());
                }
            }
        }

        // Drop invoke_context to release the borrow on transaction_context
        drop(invoke_context);

        // Extract logs from log collector
        let logs: Vec<String> = log_collector
            .and_then(|lc| {
                Rc::try_unwrap(lc)
                    .map(|lc| lc.into_inner().into_messages())
                    .ok()
            })
            .unwrap_or_default();

        // Extract updated accounts from transaction context via ExecutionRecord
        let execution_record: ExecutionRecord = transaction_context.into();
        let updated_accounts = execution_record.accounts;

        (result, traces, execute_timings, updated_accounts, logs)
    }

    /// Process message using public InvokeContext APIs.
    /// This is our reimplementation of the private `process_message` function.
    fn process_message_internal(
        &self,
        tx: &SanitizedTransaction,
        program_indices: &[Vec<IndexOfAccount>],
        invoke_context: &mut InvokeContext,
        accumulated_consumed_units: &mut u64,
        execute_timings: &mut ExecuteTimings,
    ) -> Result<(), TransactionError> {
        let message = tx.message();

        for (instruction_index, ((program_id, instruction), prog_indices)) in message
            .program_instructions_iter()
            .zip(program_indices.iter())
            .enumerate()
        {
            // Build instruction accounts
            let mut instruction_accounts = Vec::with_capacity(instruction.accounts.len());
            for (account_index, index_in_transaction) in instruction.accounts.iter().enumerate() {
                let index_in_callee = instruction
                    .accounts
                    .get(0..account_index)
                    .ok_or(TransactionError::InvalidAccountIndex)?
                    .iter()
                    .position(|idx| idx == index_in_transaction)
                    .unwrap_or(account_index)
                    as IndexOfAccount;

                let index_in_tx = *index_in_transaction as usize;
                instruction_accounts.push(InstructionAccount {
                    index_in_transaction: index_in_tx as IndexOfAccount,
                    index_in_caller: index_in_tx as IndexOfAccount,
                    index_in_callee,
                    is_signer: message.is_signer(index_in_tx),
                    is_writable: message.is_writable(index_in_tx),
                });
            }

            // Execute the instruction
            let mut compute_units_consumed = 0u64;

            let result = if invoke_context.is_precompile(program_id) {
                invoke_context.process_precompile(
                    program_id,
                    &instruction.data,
                    &instruction_accounts,
                    prog_indices,
                    message.instructions_iter().map(|ix| ix.data),
                )
            } else {
                invoke_context.process_instruction(
                    &instruction.data,
                    &instruction_accounts,
                    prog_indices,
                    &mut compute_units_consumed,
                    execute_timings,
                )
            };

            *accumulated_consumed_units =
                accumulated_consumed_units.saturating_add(compute_units_consumed);

            result
                .map_err(|err| TransactionError::InstructionError(instruction_index as u8, err))?;
        }

        Ok(())
    }
}
