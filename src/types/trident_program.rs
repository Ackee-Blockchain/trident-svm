use solana_pubkey::Pubkey;

pub struct TridentProgram {
    pub(crate) program_id: Pubkey,
    pub(crate) authority: Option<Pubkey>,
    pub(crate) data: Vec<u8>,
}
impl TridentProgram {
    pub fn new(program_id: Pubkey, authority: Option<Pubkey>, data: Vec<u8>) -> Self {
        Self {
            program_id,
            authority,
            data,
        }
    }
}
