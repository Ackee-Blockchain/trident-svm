use std::collections::HashMap;

use serde::de::DeserializeOwned;

use solana_sdk::account::AccountSharedData;
use solana_sdk::account::ReadableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::Sysvar;
use solana_sdk::sysvar::SysvarId;
use std::time::UNIX_EPOCH;

#[derive(Default)]
pub struct AccountsDB {
    accounts: HashMap<Pubkey, AccountSharedData>,
    permanent_accounts: HashMap<Pubkey, AccountSharedData>,
    programs: HashMap<Pubkey, AccountSharedData>,
    sysvars: HashMap<Pubkey, AccountSharedData>,
}

impl AccountsDB {
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if let Some(account) = self.accounts.get(pubkey) {
            Some(account.to_owned())
        } else if let Some(permanent_account) = self.permanent_accounts.get(pubkey) {
            Some(permanent_account.to_owned())
        } else if let Some(program) = self.get_program(pubkey) {
            Some(program)
        } else {
            if pubkey.eq(&Clock::id()) {
                self.update_clock();
            }
            self.sysvars.get(pubkey).cloned()
        }
    }
    pub fn get_sysvar<S: SysvarId + DeserializeOwned>(&self) -> S {
        if S::id() == Clock::id() {
            self.update_clock();
        }
        bincode::deserialize(self.sysvars.get(&S::id()).unwrap().data()).unwrap()
    }
    fn update_clock(&self) {
        let mut clock: Clock =
            bincode::deserialize(self.sysvars.get(&Clock::id()).unwrap().data()).unwrap();
        clock.unix_timestamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards!")
            .as_secs() as i64;

        // TODO: remove this once we have a proper way to set sysvars
        #[allow(mutable_transmutes)]
        let mutable_db = unsafe { std::mem::transmute::<&AccountsDB, &mut AccountsDB>(self) };
        mutable_db.set_sysvar::<Clock>(&clock);
    }
    fn get_program(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.programs.get(pubkey).map(|acc| acc.to_owned())
    }
    pub fn add_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self.accounts.insert(pubkey.to_owned(), account.to_owned());
    }
    pub fn add_permanent_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self
            .permanent_accounts
            .insert(pubkey.to_owned(), account.to_owned());
    }
    pub fn add_program(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self.programs.insert(pubkey.to_owned(), account.to_owned());
    }
    pub fn set_sysvar<T>(&mut self, sysvar: &T)
    where
        T: Sysvar + SysvarId,
    {
        let account = AccountSharedData::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();
        self.sysvars.insert(T::id(), account);
    }

    pub fn reset_temp(&mut self) {
        self.accounts = Default::default();
    }
}
