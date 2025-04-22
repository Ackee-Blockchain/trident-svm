#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::HashMap;
use std::sync::Once;

use solana_sdk::transaction_context::IndexOfAccount;

use solana_bpf_loader_program::serialization::serialize_parameters;

use solana_program_runtime::invoke_context::InvokeContext;

// use solana_rbpf::aligned_memory::AlignedMemory;
// use solana_rbpf::ebpf::HOST_ALIGN;

use solana_sbpf::aligned_memory::AlignedMemory;
use solana_sbpf::ebpf::HOST_ALIGN;

// use trident_syscall_stubs_v1::set_invoke_context as set_invoke_context_v1;
use trident_syscall_stubs_v2::set_invoke_context as set_invoke_context_v2;

static ONCE: Once = Once::new();

#[macro_export]
macro_rules! processor {
    ($builtin_function:expr) => {
        Some(|vm, _arg0, _arg1, _arg2, _arg3, _arg4| {
            let vm = unsafe {
                &mut *((vm as *mut u64).offset(
                    -($crate::processor::solana_sbpf::vm::get_runtime_environment_key() as isize),
                )
                    as *mut $crate::processor::solana_sbpf::vm::EbpfVm<
                        $crate::processor::solana_program_runtime::invoke_context::InvokeContext,
                    >)
            };

            ///Prior Invocation
            let (mut parameter_bytes, deduplicated_indices) =
                match $crate::processor::pre_invocation(vm.context_object_pointer) {
                    Ok(parameter_bytes) => parameter_bytes,
                    Err(err) => {
                        vm.program_result = Err(err)
                            .map_err(|err| {
                                $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                            })
                            .into();
                        return;
                    }
                };

            ///Get log collector
            let log_collector = vm.context_object_pointer.get_log_collector();

            ///Deserialize parameter bytes
            let (program_id_, account_infos, data) = unsafe {
                $crate::processor::deserialize(&mut parameter_bytes.as_slice_mut()[0] as *mut u8)
            };

            ///Convert program_id_ to Pubkey of correct solana program version
            /// The type is inferred by the compiler/
            let program_id = unsafe {
                std::mem::transmute::<
                    &$crate::processor::Pubkey,
                    &_,
                >(&program_id_)
            };
            ///Convert account_infos to Vec<AccountInfo> of correct solana program version
            /// The type is inferred by the compiler/
            let account_infos = unsafe {
                std::mem::transmute::<
                    &[$crate::processor::account_info::AccountInfo<'_>],
                    &_,
                >(&account_infos)
            };
            ///Log program invoke
            $crate::processor::stable_log::program_invoke(
                &log_collector,
                &program_id_,
                vm.context_object_pointer.get_stack_height(),
            );
            ///Invoke builtin function
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                $builtin_function(program_id, account_infos, data)
            })) {
                Ok(program_result) => match program_result {
                    ///In case of success, set program result to Ok(0), log success and continue
                    Ok(_) => {
                        ///Log program success
                        $crate::processor::stable_log::program_success(&log_collector, &program_id_);

                        ///Set program result to Ok(0)
                        {
                            vm.program_result = Ok(0).into();
                        }
                    }
                    ///In case of error, set program result to error, log failure and return
                    Err(program_error) => {
                        let err =
                            $crate::processor::InstructionError::from(u64::from(program_error));
                        $crate::processor::stable_log::program_failure(
                            &log_collector,
                            &program_id_,
                            &err,
                        );
                        let err: Box<dyn std::error::Error> = Box::new(err);
                        ///Set program result to error
                        {
                            vm.program_result = Err(err)
                                .map_err(|err| {
                                    $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                                }).into();
                        }
                        return;
                    }
                },
                Err(_panic_error) => {
                    ///In case of panic, set program result to ProgramFailedToComplete, log failure and return
                    let err = $crate::processor::InstructionError::ProgramFailedToComplete;
                    $crate::processor::stable_log::program_failure(
                        &log_collector,
                        &program_id_,
                        &err,
                    );
                    let err: Box<dyn std::error::Error> = Box::new(err);

                    ///Set program result to error
                    {
                        vm.program_result = Err(err)
                            .map_err(|err| {
                                $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                            })
                            .into();
                    }
                    return;
                }
            };

            ///Post invocation
            /// The type is inferred by the compiler
            let account_infos = unsafe {
                std::mem::transmute::<
                    & _,
                    &[$crate::processor::account_info::AccountInfo<'_>],
                >(account_infos)
            };

            ///Post invocation
            match $crate::processor::post_invocation(
                vm.context_object_pointer,
                &account_infos,
                &deduplicated_indices,
            ) {
                Ok(_) => (),
                Err(err) => {
                    vm.program_result = Err(err)
                        .map_err(|err| {
                            $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                        })
                        .into();
                    return;
                }
            }
        })
    };
}

pub fn pre_invocation(
    invoke_context: &mut InvokeContext,
) -> Result<
    (
        AlignedMemory<HOST_ALIGN>,
        std::collections::HashSet<IndexOfAccount>,
    ),
    Box<dyn std::error::Error>,
> {
    ONCE.call_once(|| {
        if std::env::var("TRIDENT_LOG").is_ok() {
            solana_logger::setup_with_default(
                "solana_rbpf::vm=debug,\
            solana_runtime::message_processor=debug,\
            solana_runtime::system_instruction_processor=trace",
            );
        } else {
            solana_logger::setup_with_default("off");
        }
    });

    // set_invoke_context_v1(invoke_context);
    set_invoke_context_v2(invoke_context);

    let transaction_context = &invoke_context.transaction_context;
    let instruction_context = transaction_context.get_current_instruction_context()?;
    let instruction_account_indices = 0..instruction_context.get_number_of_instruction_accounts();

    invoke_context.consume_checked(1)?;

    let deduplicated_indices: std::collections::HashSet<IndexOfAccount> =
        instruction_account_indices.collect();

    let (parameter_bytes, _regions, _account_lengths) =
        serialize_parameters(transaction_context, instruction_context, true)?;

    Ok((parameter_bytes, deduplicated_indices))
}

pub fn post_invocation(
    invoke_context: &mut solana_program_runtime::invoke_context::InvokeContext,
    account_infos: &[crate::processor::account_info::AccountInfo<'_>],
    deduplicated_indices: &std::collections::HashSet<IndexOfAccount>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Re-fetch the instruction context. The previous reference may have been
    // invalidated due to the `set_invoke_context` in a CPI.
    let transaction_context = &invoke_context.transaction_context;

    let instruction_context = transaction_context.get_current_instruction_context()?;

    let account_info_map: HashMap<_, _> = account_infos.iter().map(|a| (a.key, a)).collect();

    // Commit AccountInfo changes back into KeyedAccounts
    for i in deduplicated_indices.iter() {
        let mut borrowed_account =
            instruction_context.try_borrow_instruction_account(transaction_context, *i)?;
        if borrowed_account.is_writable() {
            if let Some(account_info) = account_info_map.get(borrowed_account.get_key()) {
                if borrowed_account.get_lamports() != account_info.lamports() {
                    borrowed_account.set_lamports(account_info.lamports())?;
                }

                // eprintln!("Before Setting data from Slice");
                if borrowed_account
                    .can_data_be_resized(account_info.data_len())
                    .is_ok()
                    && borrowed_account.can_data_be_changed().is_ok()
                {
                    borrowed_account.set_data_from_slice(&account_info.data.borrow())?;
                }
                if borrowed_account.get_owner() != account_info.owner {
                    borrowed_account.set_owner(account_info.owner.as_ref())?;
                }
            }
        }
    }
    Ok(())
}
