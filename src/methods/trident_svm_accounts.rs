use solana_sysvar::Sysvar;
use solana_sysvar_id::SysvarId;

use solana_account::AccountSharedData;
use solana_account::ReadableAccount;

use solana_keypair::Keypair;
use solana_pubkey::Pubkey;

use crate::trident_svm::TridentSVM;

impl TridentSVM {
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts.get_account(pubkey)
    }

    pub fn set_account(&mut self, pubkey: &Pubkey, account: &AccountSharedData, permanent: bool) {
        if permanent {
            self.accounts.set_permanent_account(pubkey, account);
        } else {
            self.accounts.set_temporary_account(pubkey, account);
        }
    }

    pub fn get_sysvar<T: Sysvar + SysvarId>(&self) -> T {
        self.accounts.get_sysvar()
    }

    pub fn set_sysvar<T: Sysvar + SysvarId>(&mut self, sysvar: &T) {
        self.accounts.set_sysvar(sysvar);
    }
    pub fn get_payer(&self) -> Keypair {
        self.payer.insecure_clone()
    }
    pub(crate) fn settle_accounts(&mut self, accounts: &[(Pubkey, AccountSharedData)]) {
        for account in accounts {
            if !account.1.executable() && account.1.owner() != &solana_sdk_ids::sysvar::id() {
                // Update permanent account if it should be updated
                if self.accounts.get_permanent_account(&account.0).is_some() {
                    self.accounts.set_permanent_account(&account.0, &account.1);
                } else {
                    // Otherwise, add it to the temp accounts
                    self.accounts.set_temporary_account(&account.0, &account.1);
                }
            }
        }
    }
}
