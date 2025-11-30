#[cfg(feature = "syscall-v2")]
use crate::types::trident_entrypoint::TridentEntrypoint;
#[cfg(feature = "syscall-v2")]
use solana_program_runtime::loaded_programs::ProgramCacheEntry;

use crate::trident_svm::TridentSVM;

impl TridentSVM {
    #[cfg(feature = "syscall-v2")]
    pub fn deploy_entrypoint_program(&mut self, program: &TridentEntrypoint) {
        use solana_account::{AccountSharedData, WritableAccount};
        use solana_loader_v3_interface::state::UpgradeableLoaderState;
        use solana_rent::Rent;

        use crate::utils::create_loadable_account_for_test;

        let entry = match program.entry {
            Some(entry) => entry,
            None => panic!("Native programs have to have entry specified"),
        };

        self.accounts.set_permanent_account(
            &program.program_id,
            &create_loadable_account_for_test("program-name"),
        );

        let program_data_account =
            solana_loader_v3_interface::get_program_data_address(&program.program_id);

        let state = UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: program.authority,
        };
        let mut header = bincode::serialize(&state).unwrap();

        let mut complement = vec![
            0;
            std::cmp::max(
                0,
                UpgradeableLoaderState::size_of_programdata_metadata().saturating_sub(header.len())
            )
        ];

        let mut buffer: Vec<u8> = vec![];
        header.append(&mut complement);
        header.append(&mut buffer);

        let rent = Rent::default();

        let account_data = AccountSharedData::create(
            rent.minimum_balance(header.len()),
            header,
            solana_sdk_ids::bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        self.accounts
            .set_permanent_account(&program_data_account, &account_data);

        self.processor.add_builtin(
            self,
            program.program_id,
            "program-name",
            ProgramCacheEntry::new_builtin(0, "program-name".len(), entry),
        );
    }
}
