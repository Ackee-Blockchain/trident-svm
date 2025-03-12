use solana_sdk::account::AccountSharedData;
use solana_sdk::account::WritableAccount;
use solana_sdk::bpf_loader_upgradeable;
use solana_sdk::bpf_loader_upgradeable::UpgradeableLoaderState;

use solana_sdk::native_loader;

use solana_sdk::rent::Rent;

use solana_program_runtime::loaded_programs::ProgramCacheEntry;

use crate::trident_svm::TridentSVM;
use crate::types::trident_entrypoint::TridentEntrypoint;
use crate::types::trident_program::TridentProgram;

impl TridentSVM {
    pub fn deploy_binary_program(&mut self, program: &TridentProgram) {
        let rent = Rent::default();

        let program_account = &program.program_id;

        let program_data_account =
            bpf_loader_upgradeable::get_program_data_address(program_account);

        let state = UpgradeableLoaderState::Program {
            programdata_address: program_data_account,
        };

        let buffer = bincode::serialize(&state).unwrap();
        let account_data = AccountSharedData::create(
            rent.minimum_balance(buffer.len()),
            buffer,
            bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        self.accounts.set_program(program_account, &account_data);

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

        let mut buffer: Vec<u8> = program.data.to_vec();
        header.append(&mut complement);
        header.append(&mut buffer);

        let account_data = AccountSharedData::create(
            rent.minimum_balance(header.len()),
            header,
            bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        self.accounts
            .set_program(&program_data_account, &account_data);
    }

    pub fn deploy_entrypoint_program(&mut self, program: &TridentEntrypoint) {
        let entry = match program.entry {
            Some(entry) => entry,
            None => panic!("Native programs have to have entry specified"),
        };

        self.accounts.set_program(
            &program.program_id,
            &native_loader::create_loadable_account_for_test("program-name"),
        );

        let program_data_account =
            bpf_loader_upgradeable::get_program_data_address(&program.program_id);

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
            bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        self.accounts
            .set_program(&program_data_account, &account_data);

        self.processor.add_builtin(
            self,
            program.program_id,
            "program-name",
            ProgramCacheEntry::new_builtin(0, "program-name".len(), entry),
        );
    }
}
