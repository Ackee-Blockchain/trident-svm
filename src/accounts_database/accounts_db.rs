use std::collections::HashMap;

use serde::de::DeserializeOwned;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::{Sysvar, SysvarId};

use super::sysvar_tracker::SysvarTracker;

#[derive(Default)]
pub struct AccountsDB {
    pub(crate) accounts: HashMap<Pubkey, AccountSharedData>,
    pub(crate) permanent_accounts: HashMap<Pubkey, AccountSharedData>,
    pub(crate) programs: HashMap<Pubkey, AccountSharedData>,
    pub(crate) sysvars: HashMap<Pubkey, AccountSharedData>,
    pub(crate) sysvar_tracker: SysvarTracker,
}

impl AccountsDB {
    pub(crate) fn get_temporary_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts.get(pubkey).map(|acc| acc.to_owned())
    }
    pub(crate) fn get_permanent_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.permanent_accounts
            .get(pubkey)
            .map(|acc| acc.to_owned())
    }
    pub(crate) fn get_program(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.programs.get(pubkey).map(|acc| acc.to_owned())
    }
    pub(crate) fn get_sysvar_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.sysvars.get(pubkey).map(|acc| acc.to_owned())
    }
    pub(crate) fn get_sysvar<T: SysvarId + DeserializeOwned>(&self) -> T {
        if T::id() == Clock::id() {
            self.update_clock();
        }
        let sysvar = self
            .get_sysvar_account(&T::id())
            .expect("The requested sysvar is not available");
        bincode::deserialize(sysvar.data()).expect("Failed to deserialize sysvar account")
    }
}

impl AccountsDB {
    pub(crate) fn set_temporary_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self.accounts.insert(pubkey.to_owned(), account.to_owned());
    }
    pub(crate) fn set_permanent_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self
            .permanent_accounts
            .insert(pubkey.to_owned(), account.to_owned());
    }
    pub(crate) fn set_program(&mut self, pubkey: &Pubkey, account: &AccountSharedData) {
        let _ = self.programs.insert(pubkey.to_owned(), account.to_owned());
    }
    pub(crate) fn set_sysvar<T: Sysvar + SysvarId>(&mut self, sysvar: &T) {
        let account = AccountSharedData::new_data(1, &sysvar, &solana_sdk::sysvar::id()).unwrap();
        let _ = self.sysvars.insert(T::id(), account);

        if T::id() == Clock::id() {
            self.sysvar_tracker.refresh();
        }
    }
}

impl AccountsDB {
    pub(crate) fn reset_temp(&mut self) {
        self.accounts = Default::default();
    }
}
