use solana_account::AccountSharedData;
use solana_pubkey::Pubkey;

pub struct TridentAccountSharedData {
    pub address: Pubkey,
    pub account: AccountSharedData,
}
impl TridentAccountSharedData {
    pub fn new(address: Pubkey, account: AccountSharedData) -> TridentAccountSharedData {
        Self { address, account }
    }
}
