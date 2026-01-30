//! Coverage extraction from sBPF VM execution traces.
//!
//! This module converts execution traces into AFL-compatible coverage bitmaps,
//! enabling coverage-guided fuzzing of Solana programs without LLVM instrumentation.

use super::ExecutionTraces;

/// AFL-compatible coverage bitmap size (64KB).
/// This matches AFL++'s default shared memory size.
pub const COVERAGE_MAP_SIZE: usize = 65536;

/// AFL-style coverage bitmap.
/// Each byte represents a "bucket" for edge hit counts.
pub type CoverageMap = [u8; COVERAGE_MAP_SIZE];

/// Converts sBPF VM execution traces to an AFL-style edge coverage bitmap.
///
/// The algorithm:
/// 1. For each executed instruction, extract the program counter (PC)
/// 2. Compute an "edge" as the transition from previous PC to current PC
/// 3. Hash the edge to a bucket index and increment the count
///
/// This is the same algorithm AFL uses for its compile-time instrumentation,
/// but computed from runtime traces instead.
///
/// # Arguments
/// * `traces` - Execution traces from `execute_transaction_with_traces()`
///
/// # Returns
/// A 64KB coverage bitmap suitable for AFL++ feedback
///
/// # Example
/// ```ignore
/// let result = svm.execute_transaction_with_traces(tx);
/// let coverage = traces_to_coverage(&result.traces);
/// // Write coverage to AFL shared memory...
/// ```
pub fn traces_to_coverage(traces: &ExecutionTraces) -> CoverageMap {
    let mut map = [0u8; COVERAGE_MAP_SIZE];
    let mut prev_location = 0usize;

    for instruction_trace in traces {
        for state in instruction_trace {
            // PC is the 12th element (index 11) in the state array
            // state = [r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, pc]
            let pc = state[11] as usize;

            // Hash the PC to spread values across the bitmap
            // This reduces collisions for sequential instructions
            let cur_location = (pc >> 4) ^ (pc << 8);

            // AFL-style edge: XOR current with previous location
            let idx = (cur_location ^ prev_location) & (COVERAGE_MAP_SIZE - 1);

            // Saturating add prevents overflow (AFL uses this too)
            map[idx] = map[idx].saturating_add(1);

            prev_location = cur_location >> 1;
        }
    }

    map
}

/// Writes coverage data to an existing coverage map (for persistent mode).
///
/// Unlike `traces_to_coverage`, this writes directly to a mutable slice,
/// which is useful when writing to AFL's shared memory region.
///
/// # Arguments
/// * `traces` - Execution traces from transaction execution
/// * `map` - Mutable slice to write coverage into (must be COVERAGE_MAP_SIZE bytes)
///
/// # Panics
/// Panics if `map.len() < COVERAGE_MAP_SIZE`
pub fn traces_to_coverage_map(traces: &ExecutionTraces, map: &mut [u8]) {
    assert!(
        map.len() >= COVERAGE_MAP_SIZE,
        "Coverage map must be at least {COVERAGE_MAP_SIZE} bytes"
    );

    let mut prev_location = 0usize;

    for instruction_trace in traces {
        for state in instruction_trace {
            let pc = state[11] as usize;
            // Hash the PC to spread values across the bitmap
            // This reduces collisions for sequential instructions
            let cur_location = (pc >> 4) ^ (pc << 8);
            // AFL-style edge: XOR current with previous location
            let idx = (cur_location ^ prev_location) & (COVERAGE_MAP_SIZE - 1);
            map[idx] = map[idx].saturating_add(1);
            prev_location = cur_location >> 1;
        }
    }
}

/// Reset a coverage map to zeros (for persistent mode between iterations).
#[inline]
pub fn reset_coverage_map(map: &mut [u8]) {
    map.fill(0);
}

/// Count how many unique edges were hit (non-zero buckets).
/// Useful for debugging and coverage statistics.
pub fn count_edges(map: &CoverageMap) -> usize {
    map.iter().filter(|&&x| x > 0).count()
}

/// Count total trace entries (VM instructions executed).
pub fn count_trace_entries(traces: &ExecutionTraces) -> usize {
    traces.iter().map(|t| t.len()).sum()
}

// ============================================================================
// ENHANCED COVERAGE FUNCTIONS
// ============================================================================

/// Better hash function for sBPF PC values.
/// Uses multiplicative hashing for better distribution.
#[inline]
fn hash_pc(pc: usize) -> usize {
    // Multiplicative hash with golden ratio constant
    // Better distribution than simple shift-xor for sequential values
    let h = pc.wrapping_mul(0x9e3779b9); // 2^32 / golden ratio
    (h >> 16) ^ h
}

/// Enhanced coverage with context sensitivity.
///
/// Improvements over basic version:
/// 1. Better hash function for PC values
/// 2. Context sensitivity (call depth affects coverage)
/// 3. Distinguishes same code reached via different call paths
pub fn traces_to_coverage_map_context(traces: &ExecutionTraces, map: &mut [u8]) {
    assert!(
        map.len() >= COVERAGE_MAP_SIZE,
        "Coverage map must be at least {COVERAGE_MAP_SIZE} bytes"
    );

    let mut prev_location = 0usize;

    for (call_depth, instruction_trace) in traces.iter().enumerate() {
        // Context: same code at different call depths = different coverage
        let context = call_depth.wrapping_mul(31337);

        for state in instruction_trace {
            let pc = state[11] as usize;

            // Better hash + context
            let cur_location = hash_pc(pc) ^ context;

            let idx = (cur_location ^ prev_location) & (COVERAGE_MAP_SIZE - 1);
            map[idx] = map[idx].saturating_add(1);
            prev_location = cur_location >> 1;
        }
    }
}

/// Enhanced coverage with data-flow sensitivity.
///
/// Uses register values (r0-r3) to distinguish different data patterns
/// executing the same code. Similar to AFL's cmplog feature.
pub fn traces_to_coverage_map_dataflow(traces: &ExecutionTraces, map: &mut [u8]) {
    assert!(
        map.len() >= COVERAGE_MAP_SIZE,
        "Coverage map must be at least {COVERAGE_MAP_SIZE} bytes"
    );

    let mut prev_location = 0usize;

    for (call_depth, instruction_trace) in traces.iter().enumerate() {
        let context = call_depth.wrapping_mul(31337);

        for state in instruction_trace {
            let pc = state[11] as usize;

            // Include data from key registers (r0-r3)
            // These often contain function arguments, return values, comparison operands
            let r0 = state[0] as usize;
            let r1 = state[1] as usize;

            // Use low bits of registers to distinguish data patterns
            // (8 bits from each = 256 different patterns per register)
            let data_tag = ((r0 & 0xFF) ^ ((r1 & 0xFF) << 4)) & 0xFFF;

            let cur_location = hash_pc(pc) ^ context ^ (data_tag << 12);

            let idx = (cur_location ^ prev_location) & (COVERAGE_MAP_SIZE - 1);
            map[idx] = map[idx].saturating_add(1);
            prev_location = cur_location >> 1;
        }
    }
}

/// Full enhanced coverage combining all improvements.
///
/// This version includes:
/// 1. Better PC hashing
/// 2. Context sensitivity (call depth)
/// 3. Data-flow sensitivity (register values)
/// 4. Non-sequential PC detection (branches are more interesting)
pub fn traces_to_coverage_map_enhanced(traces: &ExecutionTraces, map: &mut [u8]) {
    assert!(
        map.len() >= COVERAGE_MAP_SIZE,
        "Coverage map must be at least {COVERAGE_MAP_SIZE} bytes"
    );

    let mut prev_location = 0usize;
    let mut prev_pc = 0usize;

    for (call_depth, instruction_trace) in traces.iter().enumerate() {
        let context = call_depth.wrapping_mul(31337);

        for state in instruction_trace {
            let pc = state[11] as usize;

            // Detect non-sequential execution (branches/jumps)
            // These are more "interesting" from a coverage perspective
            let is_branch = if prev_pc != 0 {
                let expected_next = prev_pc.wrapping_add(8); // sBPF instructions are 8 bytes
                pc != expected_next
            } else {
                false
            };

            // Data from key registers
            let r0 = state[0] as usize;
            let data_tag = (r0 & 0xFF) << 8;

            // Combine everything
            let mut cur_location = hash_pc(pc) ^ context ^ data_tag;

            // Extra bit for branch detection
            if is_branch {
                cur_location ^= 0x8000;
            }

            let idx = (cur_location ^ prev_location) & (COVERAGE_MAP_SIZE - 1);
            map[idx] = map[idx].saturating_add(1);

            prev_location = cur_location >> 1;
            prev_pc = pc;
        }

        // Reset prev_pc between call frames
        prev_pc = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_traces() {
        let traces: ExecutionTraces = vec![];
        let coverage = traces_to_coverage(&traces);
        assert_eq!(count_edges(&coverage), 0);
    }

    #[test]
    fn test_simple_trace() {
        // Simulate a simple trace: PC values 100, 104, 108, 200
        let traces: ExecutionTraces = vec![vec![
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100], // PC = 100
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 104], // PC = 104
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 108], // PC = 108
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200], // PC = 200 (jump)
        ]];

        let coverage = traces_to_coverage(&traces);
        let edges = count_edges(&coverage);

        // Should have 4 edges: 0->100, 100->104, 104->108, 108->200
        assert_eq!(edges, 4);
    }

    #[test]
    fn test_edge_direction_matters() {
        // A->B should be different from B->A
        let traces_ab: ExecutionTraces = vec![vec![
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200],
        ]];

        let traces_ba: ExecutionTraces = vec![vec![
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100],
        ]];

        let cov_ab = traces_to_coverage(&traces_ab);
        let cov_ba = traces_to_coverage(&traces_ba);

        // The coverage maps should be different
        assert_ne!(cov_ab, cov_ba);
    }

    #[test]
    fn test_write_to_existing_map() {
        let traces: ExecutionTraces = vec![vec![
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 200],
        ]];

        let mut map = [0u8; COVERAGE_MAP_SIZE];
        traces_to_coverage_map(&traces, &mut map);

        assert!(count_edges(&map) > 0);

        // Reset and verify
        reset_coverage_map(&mut map);
        assert_eq!(count_edges(&map), 0);
    }
}
