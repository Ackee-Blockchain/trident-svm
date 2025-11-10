use bincode::serialize;
use solana_sdk::{
    account::AccountSharedData,
    pubkey::Pubkey,
    sysvar::{
        self, Sysvar,
        clock::{self, Clock},
        epoch_rewards::{self, EpochRewards},
        epoch_schedule::{self, EpochSchedule},
        rent::{self, Rent},
        slot_hashes::{self, SlotHashes},
        stake_history::{self, StakeHistory},
        fees::{self, Fees},

    },
};
use std::ptr;
use solana_program_runtime::sysvar_cache::SysvarCache;
use solana_program_runtime::invoke_context::InvokeContext;

fn to_sysvar_account<T: Sysvar>(value: &T) -> AccountSharedData {
    let data = serialize(value).expect("serialize sysvar");
    let mut acc = AccountSharedData::new(1, data.len(), &sysvar::id());
    acc.set_data_from_slice(&data);
    acc
}

/// Canonical set of sysvars expected by 2.2.x (tested on 2.2.2).
pub fn default_sysvar_accounts_2_2() -> Vec<(Pubkey, AccountSharedData)> {
    let mut out = Vec::with_capacity(8);

    let push_and_log = |out: &mut Vec<(Pubkey, AccountSharedData)>, (k, v): (Pubkey, AccountSharedData)| {
        #[cfg(debug_assertions)]
        if std::env::var("TRIDENT_DEBUG_SYSVAR").is_ok() {
            eprintln!("[seed] sysvar {}", k);
        }
        out.push((k, v));
    };
    push_and_log(&mut out, (rent::id(),           to_sysvar_account(&Rent::default())));
    push_and_log(&mut out, (clock::id(),          to_sysvar_account(&Clock {
        slot: 1, epoch: 0, epoch_start_timestamp: 0, leader_schedule_epoch: 0, unix_timestamp: 1
    })));
    push_and_log(&mut out, (epoch_schedule::id(), to_sysvar_account(&EpochSchedule::default())));
    push_and_log(&mut out, (epoch_rewards::id(),  to_sysvar_account(&EpochRewards::default())));


    out
}


pub fn setup_test_sysvars(invoke_context: &mut InvokeContext) {
    let ts = 1_726_160_000_i64;

    let sysvars = TestSysvars {
        clock: Clock {
            slot: 1,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp: ts,
            epoch_start_timestamp: ts,
        },
        rent: Rent::default(),
        epoch_schedule: EpochSchedule::default(),
        fees: Fees::default(),
        slot_hashes: SlotHashes::default(),
        stake_history: StakeHistory::default(),
    };

    sysvars.apply_to_cache(invoke_context);
}

struct TestSysvars {
    clock: Clock,
    rent: Rent,
    epoch_schedule: EpochSchedule,
    fees: Fees,
    slot_hashes: SlotHashes,
    stake_history: StakeHistory,
}

impl TestSysvars {
    fn apply_to_cache(&self, invoke_context: &mut InvokeContext) {
        // Get mutable cache pointer (test-only hack)
    let cache_ref = invoke_context.get_sysvar_cache();
    
    // More explicit about the UB we're doing (still not great)
    let cache = unsafe {
        &mut *ptr::addr_of!(*cache_ref).cast_mut()
    };

        // Set sysvars and export serialized data
        self.set_sysvar_with_export(cache, &self.clock, "CLOCK_DATA_HEX");
        self.set_sysvar_with_export(cache, &self.rent, "RENT_DATA_HEX");
        self.set_sysvar_with_export(cache, &self.epoch_schedule, "EPOCH_SCHEDULE_DATA_HEX");
        self.set_sysvar_with_export(cache, &self.fees, "FEES_DATA_HEX");

        // Set sysvars without export
        unsafe {
            cache.set_sysvar_for_tests(&self.slot_hashes);
            cache.set_sysvar_for_tests(&self.stake_history);
        }

        eprintln!("[processor] InvokeContext ptr: {:p}", invoke_context as *const _);
        eprintln!("[processor] SysvarCache ptr: {:p}", cache as *const _);
    }

    fn set_sysvar_with_export<T>(&self, cache: &mut SysvarCache, sysvar: &T, env_key: &str)
    where
        T: serde::Serialize + Sysvar, // Fixed: Use Sysvar instead of SysvarId
    {
        
        if let Ok(bytes) = bincode::serialize(sysvar) {
            let hex = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            std::env::set_var(env_key, hex);
        }
    }
}
