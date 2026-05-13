use solana_sysvar::Sysvar;
use solana_sysvar_id::SysvarId;

use solana_account::AccountSharedData;
use solana_account::ReadableAccount;

use solana_keypair::Keypair;
use solana_pubkey::Pubkey;

use crate::trident_svm::TridentSVM;

impl TridentSVM {
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts.get_account(pubkey, true)
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
                // Always settle into the temporary overlay.
                //
                // Permanent accounts are treated as an immutable base snapshot (genesis + forks).
                // This allows `clear_accounts()` (which resets temp) to restore the initial state
                // for the next fuzz iteration.
                self.accounts.set_temporary_account(&account.0, &account.1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_account::WritableAccount;
    use solana_pubkey::Pubkey;

    #[test]
    fn permanent_account_is_restored_after_clear_accounts() {
        let mut svm = TridentSVM::default();
        let key = Pubkey::new_unique();

        let mut base = AccountSharedData::new(123, 0, &solana_sdk_ids::system_program::id());
        base.set_lamports(123);
        svm.accounts.set_permanent_account(&key, &base);

        let mut updated = base.clone();
        updated.set_lamports(999);
        svm.settle_accounts(&[(key, updated.clone())]);

        // Updated value should be visible (temp shadows permanent).
        assert_eq!(svm.get_account(&key).unwrap().lamports(), 999);

        // Clearing temp should restore the permanent base snapshot.
        svm.clear_accounts();
        assert_eq!(svm.get_account(&key).unwrap().lamports(), 123);
    }
}
