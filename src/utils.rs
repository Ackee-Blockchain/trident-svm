use std::time::UNIX_EPOCH;

use solana_account::Account;
use solana_account::AccountSharedData;
use solana_account::InheritableAccountFields;
use solana_account::DUMMY_INHERITABLE_ACCOUNT_FIELDS;

pub(crate) fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards!")
        .as_secs()
}

pub(crate) fn create_loadable_account_with_fields(
    name: &str,
    (lamports, rent_epoch): InheritableAccountFields,
) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports,
        owner: solana_sdk_ids::native_loader::id(),
        data: name.as_bytes().to_vec(),
        executable: true,
        rent_epoch,
    })
}

pub(crate) fn create_loadable_account_for_test(name: &str) -> AccountSharedData {
    create_loadable_account_with_fields(name, DUMMY_INHERITABLE_ACCOUNT_FIELDS)
}
