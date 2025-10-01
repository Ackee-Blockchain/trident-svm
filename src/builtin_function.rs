#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::HashMap;
use std::sync::Once;

use solana_sdk::{program_error::PrintProgramError, transaction_context::IndexOfAccount};

use solana_bpf_loader_program::serialization::serialize_parameters;
use trident_syscall_stubs_v2::TridentSyscallStubs;
use solana_program_runtime::invoke_context::InvokeContext;
use solana_sbpf::aligned_memory::AlignedMemory;
use solana_sbpf::ebpf::HOST_ALIGN;
// use trident_syscall_stubs_v1::set_invoke_context as set_invoke_context_v1;
use trident_syscall_stubs_v2::set_invoke_context as set_invoke_context_v2;
use std::panic;
use solana_sdk::sysvar::{

    rent::Rent,
    epoch_schedule::EpochSchedule,
    fees::Fees,
    slot_hashes::SlotHashes,
    slot_history::SlotHistory,
    stake_history::StakeHistory,
    Sysvar, // for defaults if needed
};
use solana_program_runtime::sysvar_cache::SysvarCache; // ok to keep, even if unused directly
use solana_sdk::{sysvar::{instructions::Instructions, SysvarId}};
use solana_sdk::clock::Clock;
  // This imports the module from sysvar_bridge.rs

use solana_program::{example_mocks::solana_sdk::sysvar, program_stubs::SyscallStubs};
static ONCE: Once = Once::new();

#[macro_export]
macro_rules! processor {
    ($builtin_function:path) => {{
        #[allow(non_snake_case)]
        #[inline(always)]
        fn __trident_entry_shim<'a>(
            vm: *mut $crate::processor::solana_sbpf::vm::EbpfVm<
                'a,
                $crate::processor::solana_program_runtime::invoke_context::InvokeContext<'static>
            >,
            _arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64, _arg4: u64
        ) {
            eprintln!("[processor] ENTER");

            // Rebase VM pointer (sbpf runtime env layout)
            let vm: &mut $crate::processor::solana_sbpf::vm::EbpfVm<
                'a,
                $crate::processor::solana_program_runtime::invoke_context::InvokeContext<'static>
            > = unsafe {
                let base = vm as *mut u64;
                let key = $crate::processor::solana_sbpf::vm::get_runtime_environment_key() as isize;
                let rebased = base.offset(-key) as *mut $crate::processor::solana_sbpf::vm::EbpfVm<
                    'a,
                    $crate::processor::solana_program_runtime::invoke_context::InvokeContext<'static>
                >;
                eprintln!("[processor] Rebasing VM ptr by -{}", key);
                &mut *rebased
            };

            // --- Prior Invocation ---
            eprintln!("[processor] pre_invocation()");
            let (mut parameter_bytes, deduplicated_indices) =
                match $crate::processor::pre_invocation(vm.context_object_pointer) {
                    Ok(pair) => {
                        eprintln!("[processor] pre_invocation OK");
                        pair
                    }
                    Err(err) => {
                        eprintln!("[processor] pre_invocation ERROR: {:?}", err);
                        vm.program_result = Err(err)
                            .map_err(|err| {
                                $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                            })
                            .into();
                        return;
                    }
                };

            let log_collector = vm.context_object_pointer.get_log_collector();

            // Deserialize call params
            eprintln!("[processor] deserialize()");
            let (program_id_, account_infos, data) = unsafe {
                $crate::processor::deserialize(&mut parameter_bytes.as_slice_mut()[0] as *mut u8)
            };
            eprintln!(
                "[processor] deserialize OK: accounts={}, data={}",
                account_infos.len(),
                data.len()
            );

            // Same-crate types (no transmutes)
            let program_id: &$crate::processor::Pubkey = &program_id_;
            let account_infos: &[$crate::processor::account_info::AccountInfo<'_>] = &account_infos;

            $crate::processor::stable_log::program_invoke(
                &log_collector,
                &program_id_,
                vm.context_object_pointer.get_stack_height(),
            );

            eprintln!("[processor] call user entry");
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                $builtin_function(program_id, account_infos, data)
            })) {
                Ok(Ok(_)) => {
                    eprintln!("[processor] entry OK");
                    $crate::processor::stable_log::program_success(&log_collector, &program_id_);
                    vm.program_result = Ok(0).into();
                }
                Ok(Err(program_error)) => {
                    eprintln!("[processor] entry ProgramError: {:?}", program_error);
                    let err = $crate::processor::InstructionError::from(u64::from(program_error));
                    $crate::processor::stable_log::program_failure(&log_collector, &program_id_, &err);
                    let err: Box<dyn std::error::Error> = Box::new(err);
                    vm.program_result = Err(err)
                        .map_err(|err| {
                            $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                        })
                        .into();
                    return;
                }
                Err(_) => {
                    eprintln!("[processor] entry PANICKED");
                    let err = $crate::processor::InstructionError::ProgramFailedToComplete;
                    $crate::processor::stable_log::program_failure(&log_collector, &program_id_, &err);
                    let err: Box<dyn std::error::Error> = Box::new(err);
                    vm.program_result = Err(err)
                        .map_err(|err| {
                            $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                        })
                        .into();
                    return;
                }
            }

            eprintln!("[processor] post_invocation()");
            let account_infos: &[$crate::processor::account_info::AccountInfo<'_>] = &account_infos;
            if let Err(err) = $crate::processor::post_invocation(
                vm.context_object_pointer,
                &account_infos,
                &deduplicated_indices,
            ) {
                eprintln!("[processor] post_invocation ERROR: {:?}", err);
                vm.program_result = Err(err)
                    .map_err(|err| {
                        $crate::processor::solana_sbpf::error::EbpfError::SyscallError(err)
                    })
                    .into();
                return;
            }

            eprintln!("[processor] EXIT OK");
        }

        let __ptr: $crate::processor::solana_program_runtime::invoke_context::BuiltinFunctionWithContext
            = __trident_entry_shim;
        Some(__ptr)
    }};
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
    //let sysvar_stub = TridentSyscallStubs {};
    let mut clock = Clock::default();
let var_addr = &mut clock as *mut Clock as *mut u8;

// Now call the syscall
let sysvar_stub = TridentSyscallStubs {};
let result = sysvar_stub.sol_get_clock_sysvar(var_addr);

// After the syscall, `clock` contains the actual clock data
println!("Current slot: {}", clock.slot);
println!("RSEULF IS: {}", result);
    //sysvar_stub.sol_get_clock_sysvar(var_addr)
    let stubs = Box::new(TridentSyscallStubs {});
   
    let stubs_ptr = Box::into_raw(stubs) as usize;
    std::env::set_var("TRIDENT_STUBS_PTR", format!("{}", stubs_ptr));

    //use crate::sysvar_bridge::*;
    //init_sysvar_bridge();
unsafe {
    let ctx_ptr = invoke_context as *mut _ as usize;
    std::env::set_var("INVOKE_CTX_PTR", format!("{}", ctx_ptr));
    eprintln!("[processor] Set INVOKE_CTX_PTR={:x}", ctx_ptr);
}

    // 🔹 Seed InvokeContext sysvar cache so stubs won't return UnsupportedSysvar
    {
        crate::svm_sysvars::setup_test_sysvars(invoke_context);
        /*let ts = 1_726_160_000_i64;

        let clock = Clock {
            slot: 1,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: ts,
            epoch_start_timestamp: ts,
        };
        let rent         = Rent::default();
        let epoch        = EpochSchedule::default();
        let fees         = Fees::default();
 
        let slot_hashes  = SlotHashes::default();
        let slot_history = SlotHistory::default();
        let stake_hist   = StakeHistory::default();
        // Get &SysvarCache, then cast to &mut SysvarCache (test-only hack)
        let cache_ptr = invoke_context.get_sysvar_cache() as *const SysvarCache as *mut SysvarCache;
        unsafe {
            (*cache_ptr).set_sysvar_for_tests(&rent);
            let rent_bytes = bincode::serialize(&rent).unwrap();
            let rent_hex: String = rent_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            std::env::set_var("RENT_DATA_HEX", rent_hex);
            (*cache_ptr).set_sysvar_for_tests(&clock);
            let clock_bytes = bincode::serialize(&clock).unwrap();
            let clock_hex: String = clock_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            std::env::set_var("CLOCK_DATA_HEX", clock_hex);
            
            (*cache_ptr).set_sysvar_for_tests(&epoch);
            let epoch_bytes = bincode::serialize(&epoch).unwrap();
            let epoch_hex: String = epoch_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            std::env::set_var("EPOCH_SCHEDULE_DATA_HEX", epoch_hex);


            (*cache_ptr).set_sysvar_for_tests(&fees);
            let fees_bytes = bincode::serialize(&fees).unwrap();
            let fees_hex: String = fees_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            std::env::set_var("FEES_DATA_HEX", fees_hex);
            (*cache_ptr).set_sysvar_for_tests(&slot_hashes);
            (*cache_ptr).set_sysvar_for_tests(&stake_hist);
        }
            eprintln!("[processor] InvokeContext ptr: {:p}", invoke_context as *const _);
            eprintln!("[processor] SysvarCache ptr: {:p}", cache_ptr);
            */
    }


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
