use solana_account::AccountSharedData;
use solana_account::ReadableAccount;
use solana_pubkey::Pubkey;
use solana_sysvar::clock::Clock;
use solana_sysvar_id::SysvarId;

use super::accounts_db::AccountsDB;

impl AccountsDB {
    pub(crate) fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if let Some(account) = self.get_temporary_account(pubkey) {
            Some(account.to_owned())
        } else if let Some(permanent_account) = self.get_permanent_account(pubkey) {
            Some(permanent_account.to_owned())
        } else if let Some(program) = self.get_program(pubkey) {
            Some(program)
        } else {
            if pubkey.eq(&Clock::id()) {
                self.update_clock();
            }
            self.get_sysvar_account(pubkey)
        }
    }
    pub(crate) fn update_clock(&self) {
        let mut clock: Clock =
            bincode::deserialize(self.sysvars.get(&Clock::id()).unwrap().data()).unwrap();

        #[allow(mutable_transmutes)]
        let mutable_db = unsafe { std::mem::transmute::<&AccountsDB, &mut AccountsDB>(self) };
        mutable_db.sysvar_tracker.refresh_with_clock(&mut clock);
        mutable_db.set_sysvar::<Clock>(&clock);
    }

    #[allow(dead_code)]
    pub(crate) fn forward_in_time(&mut self, seconds: i64) {
        let mut clock: Clock = self.get_sysvar();
        clock.unix_timestamp = clock.unix_timestamp.saturating_add(seconds);
        self.set_sysvar(&clock);
    }
    #[allow(dead_code)]
    pub(crate) fn warp_to_timestamp(&mut self, timestamp: i64) {
        let mut clock: Clock = self.get_sysvar();
        clock.unix_timestamp = timestamp;
        self.set_sysvar(&clock);
    }
}
