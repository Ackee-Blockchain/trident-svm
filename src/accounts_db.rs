use std::collections::HashMap;

use serde::de::DeserializeOwned;

use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::Sysvar;
use solana_sdk::sysvar::SysvarId;

use crate::utils::get_current_timestamp;

#[derive(Default)]
pub struct AccountsDB {
    accounts: HashMap<Pubkey, AccountSharedData>,
    permanent_accounts: HashMap<Pubkey, AccountSharedData>,
    programs: HashMap<Pubkey, AccountSharedData>,
    sysvars: HashMap<Pubkey, AccountSharedData>,
    sysvar_tracker: SysvarTracker,
}

impl AccountsDB {
    pub(crate) fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if let Some(account) = self.get_temp_account(pubkey) {
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
    pub(crate) fn get_sysvar<S: SysvarId + DeserializeOwned>(&self) -> S {
        if S::id() == Clock::id() {
            self.update_clock();
        }
        bincode::deserialize(self.sysvars.get(&S::id()).unwrap().data()).unwrap()
    }
    pub(crate) fn get_program(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.programs.get(pubkey).map(|acc| acc.to_owned())
    }
    pub(crate) fn get_temp_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts.get(pubkey).map(|acc| acc.to_owned())
    }
    pub(crate) fn get_permanent_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.permanent_accounts
            .get(pubkey)
            .map(|acc| acc.to_owned())
    }
    pub(crate) fn get_sysvar_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.sysvars.get(pubkey).map(|acc| acc.to_owned())
    }

    // Setters
    pub(crate) fn add_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self.accounts.insert(pubkey.to_owned(), account.to_owned());
    }
    pub(crate) fn add_permanent_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self
            .permanent_accounts
            .insert(pubkey.to_owned(), account.to_owned());
    }
    pub(crate) fn add_program(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self.programs.insert(pubkey.to_owned(), account.to_owned());
    }
    pub(crate) fn add_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        let account = AccountSharedData::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();
        let _ = self.sysvars.insert(T::id(), account);

        if T::id() == Clock::id() {
            self.sysvar_tracker.refresh_last_clock_update();
        }
    }

    fn update_clock(&self) {
        let mut clock: Clock =
            bincode::deserialize(self.sysvars.get(&Clock::id()).unwrap().data()).unwrap();

        let current_timestamp = get_current_timestamp();

        // current time is always greater than last clock update
        let time_since_last_update =
            current_timestamp.saturating_sub(self.sysvar_tracker.last_clock_update);
        clock.unix_timestamp = clock
            .unix_timestamp
            .saturating_add(time_since_last_update as i64);

        // TODO: remove this once we have a proper way to set sysvars
        #[allow(mutable_transmutes)]
        let mutable_db = unsafe { std::mem::transmute::<&AccountsDB, &mut AccountsDB>(self) };
        mutable_db.add_sysvar::<Clock>(&clock);
        mutable_db.sysvar_tracker.refresh_last_clock_update();
    }

    pub(crate) fn reset_temp(&mut self) {
        self.accounts = Default::default();
    }

    // Helper functions for testing purposes
    pub fn forward_in_time(&mut self, seconds: i64) {
        let mut clock: Clock = self.get_sysvar();
        clock.unix_timestamp = clock.unix_timestamp.saturating_add(seconds);
        self.add_sysvar(&clock);
    }
    pub fn warp_to_timestamp(&mut self, timestamp: i64) {
        let mut clock: Clock = self.get_sysvar();
        clock.unix_timestamp = timestamp;
        self.add_sysvar(&clock);
    }
}

#[derive(Default)]
pub struct SysvarTracker {
    pub last_clock_update: u64, // unix timestamp as seconds
}

impl SysvarTracker {
    pub fn refresh_last_clock_update(&mut self) {
        self.last_clock_update = get_current_timestamp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_clock_update() {
        let mut db = AccountsDB::default();

        // Set initial clock
        let initial_clock = Clock::default();
        db.add_sysvar(&initial_clock);
        let initial_timestamp = db.get_sysvar::<Clock>().unix_timestamp;

        // Sleep for 2 seconds
        sleep(Duration::from_secs(2));
        let updated_clock: Clock = db.get_sysvar();
        assert!(
            updated_clock.unix_timestamp > initial_timestamp,
            "Clock timestamp should have increased"
        );
        let diff = (updated_clock.unix_timestamp - initial_timestamp) as u64;
        assert!(
            (1..=3).contains(&diff),
            "Clock update difference should be ~2 seconds, got {}",
            diff
        );
    }

    #[test]
    fn test_sysvar_tracker_updates() {
        let mut db = AccountsDB::default();

        // Set initial clock and get tracker time
        db.add_sysvar(&Clock::default());
        let initial_tracker_time = db.sysvar_tracker.last_clock_update;
        sleep(Duration::from_secs(1));

        // Force clock update
        let _: Clock = db.get_sysvar();
        assert!(
            db.sysvar_tracker.last_clock_update > initial_tracker_time,
            "SysvarTracker should have been updated"
        );
    }

    #[test]
    fn test_multiple_clock_updates() {
        let mut db = AccountsDB::default();

        // Set initial clock
        let initial_clock = Clock::default();
        db.add_sysvar(&initial_clock);

        // First update
        sleep(Duration::from_secs(1));
        let first_update: Clock = db.get_sysvar();
        let first_diff = (first_update.unix_timestamp - initial_clock.unix_timestamp) as u64;
        assert!(
            (1..=2).contains(&first_diff),
            "First update difference should be ~1 second"
        );

        // Second update
        sleep(Duration::from_secs(1));
        let second_update: Clock = db.get_sysvar();
        let second_diff = (second_update.unix_timestamp - first_update.unix_timestamp) as u64;
        assert!(
            (1..=2).contains(&second_diff),
            "Second update difference should be ~1 second"
        );

        // Verify total elapsed time
        let total_diff = (second_update.unix_timestamp - initial_clock.unix_timestamp) as u64;
        assert!(
            (2..=3).contains(&total_diff),
            "Total time difference should be ~2 seconds"
        );
    }

    #[test]
    fn test_time_manipulation() {
        let mut db = AccountsDB::default();

        // Set initial clock
        let initial_clock = Clock::default();
        db.add_sysvar(&initial_clock);

        // Get initial time
        let mut clock: Clock = db.get_sysvar();
        let initial_time = clock.unix_timestamp;

        // Forward 600 seconds
        db.forward_in_time(600);
        clock = db.get_sysvar();
        assert_eq!(
            clock.unix_timestamp,
            initial_time + 600,
            "Clock should advance 600 seconds"
        );

        // Warp to specific timestamp
        db.warp_to_timestamp(500);
        clock = db.get_sysvar();
        assert_eq!(
            clock.unix_timestamp, 500,
            "Clock should warp to timestamp 500"
        );

        // Test negative time forwarding
        db.forward_in_time(-300);
        clock = db.get_sysvar();
        assert_eq!(
            clock.unix_timestamp, 200,
            "Clock should go back 300 seconds from 500"
        );
    }
}
