use std::collections::HashMap;

use serde::de::DeserializeOwned;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::clock::Clock;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::sysvar::{Sysvar, SysvarId};
use solana_program;
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
            // keep your existing behavior; assumed to advance clock in storage
            self.update_clock();
        }

        let id = T::id();
        let Some(sysvar_acc) = self.get_sysvar_account(&id) else {
            eprintln!(
                "[sysvar] MISS type={} id={}",
                std::any::type_name::<T>(),
                id
            );
            panic!("The requested sysvar is not available: {}", id);
        };

        bincode::deserialize(sysvar_acc.data())
            .expect("Failed to deserialize sysvar account")
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
