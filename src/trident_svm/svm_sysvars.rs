use bincode::serialize;
use solana_sdk::{
    account::AccountSharedData,
    pubkey::Pubkey,
    sysvar::{
        self, Sysvar, SysvarId,
        clock::{self, Clock},
        epoch_rewards::{self, EpochRewards},
        epoch_schedule::{self, EpochSchedule},
        last_restart_slot::{self, LastRestartSlot},
        rent::{self, Rent},
        slot_hashes::{self, SlotHashes},
        stake_history::{self, StakeHistory},
        instructions, // 👈 add this
    },
};

fn to_sysvar_account<T: Sysvar>(value: &T) -> AccountSharedData {
    let data = serialize(value).expect("serialize sysvar");
    let mut acc = AccountSharedData::new(1, data.len(), &sysvar::id());
    acc.set_data_from_slice(&data);
    acc
}

// Minimal helper for raw sysvar data
fn to_sysvar_raw(data: &[u8]) -> AccountSharedData {
    let mut acc = AccountSharedData::new(1, data.len(), &sysvar::id());
    acc.set_data_from_slice(data);
    acc
}

/// Canonical set of sysvars expected by 2.2.x (tested on 2.2.2).
pub fn default_sysvar_accounts_2_2() -> Vec<(Pubkey, AccountSharedData)> {
    eprintln!("MAUHAHAHAHAHAHAHAHAHA");
    let mut out = Vec::with_capacity(8);

    let push_and_log = |out: &mut Vec<(Pubkey, AccountSharedData)>, (k, v): (Pubkey, AccountSharedData)| {
        #[cfg(debug_assertions)]
        if std::env::var("TRIDENT_DEBUG_SYSVAR").is_ok() {
            eprintln!("[seed] sysvar {}", k);
        }
        out.push((k, v));
    };

    push_and_log(&mut out, (clock::id(),          to_sysvar_account(&Clock {
        slot: 1, epoch: 0, epoch_start_timestamp: 0, leader_schedule_epoch: 0, unix_timestamp: 1
    })));
    push_and_log(&mut out, (epoch_schedule::id(), to_sysvar_account(&EpochSchedule::default())));
    push_and_log(&mut out, (rent::id(),           to_sysvar_account(&Rent::default())));
    push_and_log(&mut out, (stake_history::id(),  to_sysvar_account(&StakeHistory::default())));
    push_and_log(&mut out, (slot_hashes::id(),    to_sysvar_account(&SlotHashes::default())));
    push_and_log(&mut out, (epoch_rewards::id(),  to_sysvar_account(&EpochRewards::default())));
    push_and_log(&mut out, (last_restart_slot::id(), to_sysvar_account(&LastRestartSlot::default())));

    // If you added `instructions`, keep it logged too:
    // push_and_log(&mut out, (instructions::id(),   to_sysvar_raw(&[0u8, 0u8])));

    out
}
