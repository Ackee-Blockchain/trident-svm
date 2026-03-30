use solana_account::AccountSharedData;
use solana_instruction::error::InstructionError;
use solana_message::inner_instruction::InnerInstructionsList;
use solana_pubkey::Pubkey;
use solana_svm::transaction_processing_result::ProcessedTransaction;
use solana_svm::transaction_processing_result::TransactionProcessingResultExtensions;
use solana_svm::transaction_processor::LoadAndExecuteSanitizedTransactionsOutput;
use solana_transaction_context::TransactionReturnData;
use solana_transaction_error::TransactionError;

/// A resolved CPI (cross-program invocation) with actual pubkeys instead of
/// raw index references into the transaction's account key table.
#[derive(Debug, Clone)]
pub struct ResolvedInnerInstruction {
    pub program_id: Pubkey,
    pub accounts: Vec<Pubkey>,
    pub data: Vec<u8>,
    pub stack_height: u8,
}

/// Per top-level instruction, the list of CPIs it made.
pub type ResolvedInnerInstructions = Vec<ResolvedInnerInstruction>;

/// One entry per top-level instruction in the transaction.
pub type ResolvedInnerInstructionsList = Vec<ResolvedInnerInstructions>;

fn resolve_inner_instructions(
    raw: &InnerInstructionsList,
    account_keys: &[(Pubkey, AccountSharedData)],
) -> ResolvedInnerInstructionsList {
    raw.iter()
        .map(|outer| {
            outer
                .iter()
                .map(|inner| {
                    let program_id = account_keys
                        .get(inner.instruction.program_id_index as usize)
                        .map(|(key, _)| *key)
                        .unwrap_or_default();

                    let accounts = inner
                        .instruction
                        .accounts
                        .iter()
                        .map(|&idx| {
                            account_keys
                                .get(idx as usize)
                                .map(|(key, _)| *key)
                                .unwrap_or_default()
                        })
                        .collect();

                    ResolvedInnerInstruction {
                        program_id,
                        accounts,
                        data: inner.instruction.data.clone(),
                        stack_height: inner.stack_height,
                    }
                })
                .collect()
        })
        .collect()
}

pub struct TridentTransactionResult {
    pub(crate) status: Result<(), TransactionError>,
    pub(crate) logs: Vec<String>,
    pub(crate) compute_units_consumed: u64,
    pub(crate) inner_instructions: Option<ResolvedInnerInstructionsList>,
    pub(crate) return_data: Option<TransactionReturnData>,
    pub(crate) transaction_timestamp: i64,
}

impl TridentTransactionResult {
    pub(crate) fn from_svm_output(
        output: LoadAndExecuteSanitizedTransactionsOutput,
        transaction_timestamp: i64,
    ) -> Self {
        let processing_result = &output.processing_results[0];

        match processing_result.processed_transaction() {
            Some(ProcessedTransaction::Executed(executed_tx)) => {
                let details = &executed_tx.execution_details;
                let accounts = &executed_tx.loaded_transaction.accounts;

                let inner_instructions = details
                    .inner_instructions
                    .as_ref()
                    .map(|raw| resolve_inner_instructions(raw, accounts));

                Self {
                    status: details.status.clone(),
                    logs: details.log_messages.clone().unwrap_or_default(),
                    compute_units_consumed: details.executed_units,
                    inner_instructions,
                    return_data: details.return_data.clone(),
                    transaction_timestamp,
                }
            }
            Some(ProcessedTransaction::FeesOnly(fees_only)) => Self {
                status: Err(TransactionError::clone(&fees_only.load_error)),
                logs: Vec::new(),
                compute_units_consumed: 0,
                inner_instructions: None,
                return_data: None,
                transaction_timestamp,
            },
            None => Self {
                status: processing_result.flattened_result(),
                logs: Vec::new(),
                compute_units_consumed: 0,
                inner_instructions: None,
                return_data: None,
                transaction_timestamp,
            },
        }
    }

    pub fn status(&self) -> &Result<(), TransactionError> {
        &self.status
    }

    pub fn is_success(&self) -> bool {
        self.status.is_ok()
    }

    pub fn logs(&self) -> String {
        format!("{:#?}", self.logs)
    }

    pub fn compute_units_consumed(&self) -> u64 {
        self.compute_units_consumed
    }

    pub fn inner_instructions(&self) -> Option<&ResolvedInnerInstructionsList> {
        self.inner_instructions.as_ref()
    }

    pub fn return_data(&self) -> Option<&TransactionReturnData> {
        self.return_data.as_ref()
    }

    pub fn transaction_timestamp(&self) -> i64 {
        self.transaction_timestamp
    }

    pub fn is_program_failed_to_complete(&self) -> bool {
        self.status.is_err()
            && matches!(
                self.status.as_ref().unwrap_err(),
                TransactionError::InstructionError(_, InstructionError::ProgramFailedToComplete)
            )
    }
}
