use solana_program_runtime::loaded_programs::BlockRelation;
use solana_program_runtime::loaded_programs::ForkGraph;

use solana_sdk::clock::Slot;

pub struct TridentForkGraph {}

impl ForkGraph for TridentForkGraph {
    fn relationship(&self, _a: Slot, _b: Slot) -> BlockRelation {
        BlockRelation::Unknown
    }
}
