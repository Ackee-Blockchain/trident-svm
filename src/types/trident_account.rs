use solana_account::AccountSharedData;
use solana_pubkey::Pubkey;

use solana_account::WritableAccount;
use solana_loader_v3_interface::state::UpgradeableLoaderState;
use solana_sysvar::rent::Rent;

pub struct TridentAccountSharedData {
    pub address: Pubkey,
    pub account: AccountSharedData,
}
impl TridentAccountSharedData {
    pub fn new(address: Pubkey, account: AccountSharedData) -> TridentAccountSharedData {
        Self { address, account }
    }

    pub fn loader_v2_program(address: Pubkey, data: &[u8]) -> TridentAccountSharedData {
        let rent = Rent::default();

        let account_data = AccountSharedData::create(
            rent.minimum_balance(data.len()),
            data.to_vec(),
            solana_sdk_ids::bpf_loader::id(),
            true,
            Default::default(),
        );

        TridentAccountSharedData::new(address, account_data)
    }

    pub fn loader_v3_program(
        address: Pubkey,
        data: &[u8],
        authority: Option<Pubkey>,
    ) -> Vec<TridentAccountSharedData> {
        let mut accounts = Vec::new();

        let rent = Rent::default();

        let program_account = &address;

        let program_data_account =
            solana_loader_v3_interface::get_program_data_address(program_account);

        let state = UpgradeableLoaderState::Program {
            programdata_address: program_data_account,
        };

        let buffer = bincode::serialize(&state).unwrap();
        let account_data = AccountSharedData::create(
            rent.minimum_balance(buffer.len()),
            buffer,
            solana_sdk_ids::bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        accounts.push(TridentAccountSharedData::new(
            *program_account,
            account_data,
        ));

        let state = UpgradeableLoaderState::ProgramData {
            slot: 0,
            upgrade_authority_address: authority,
        };
        let mut header = bincode::serialize(&state).unwrap();

        let mut complement = vec![
            0;
            std::cmp::max(
                0,
                UpgradeableLoaderState::size_of_programdata_metadata().saturating_sub(header.len())
            )
        ];

        let mut buffer: Vec<u8> = data.to_vec();
        header.append(&mut complement);
        header.append(&mut buffer);

        let account_data = AccountSharedData::create(
            rent.minimum_balance(header.len()),
            header,
            solana_sdk_ids::bpf_loader_upgradeable::id(),
            true,
            Default::default(),
        );

        accounts.push(TridentAccountSharedData::new(
            program_data_account,
            account_data,
        ));

        accounts
    }
}
