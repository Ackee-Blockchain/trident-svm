//! AFL++ Harness Example for TridentSVM
//!
//! This example demonstrates how to integrate TridentSVM's trace-based coverage
//! with AFL++ for fuzzing Solana programs.
//!
//! ## How AFL++ Integration Works
//!
//! 1. AFL++ sets `__AFL_SHM_ID` environment variable pointing to shared memory
//! 2. Your harness attaches to this shared memory (64KB coverage bitmap)
//! 3. For each input:
//!    - Read input from stdin
//!    - Execute transaction with traces
//!    - Convert traces → coverage bitmap
//!    - Write coverage to shared memory
//! 4. AFL++ observes new coverage and saves interesting inputs
//!
//! ## Building for AFL++
//!
//! For a real fuzzing harness, you would create a separate crate with:
//!
//! ```toml
//! [dependencies]
//! trident-svm = { path = "../trident-svm" }
//! libc = "0.2"  # For shared memory access
//! ```
//!
//! Then build with:
//! ```bash
//! cargo install cargo-afl
//! cargo afl build --release
//! cargo afl fuzz -i seeds/ -o output/ target/release/your_harness
//! ```
//!
//! ## Note on Coverage
//!
//! The Solana program itself doesn't need AFL instrumentation because we extract
//! coverage from the sBPF VM execution traces. The harness may optionally be
//! instrumented, but the main coverage comes from `traces_to_coverage_map()`.
//!
//! ## This Example
//!
//! This is a simplified demonstration that works without AFL. It shows the
//! structure of a harness and can be run standalone for testing.

use std::io::{self, Read};

use trident_svm::prelude::*;

/// Example: Deserialize fuzzer input into something your program uses.
fn parse_fuzz_input(input: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() {
        return None;
    }
    Some(input.to_vec())
}

fn main() {
    // In a real AFL harness, you would get AFL's shared memory via __AFL_SHM_ID
    // and the libc::shmat() call. For this demo, we use a local map.
    let mut coverage_map = [0u8; COVERAGE_MAP_SIZE];

    // Read input from stdin (AFL provides input this way)
    let mut input = Vec::new();
    if io::stdin().read_to_end(&mut input).is_err() {
        eprintln!("[-] Failed to read input");
        std::process::exit(1);
    }

    // Parse the fuzzer input
    let instruction_data = match parse_fuzz_input(&input) {
        Some(data) => data,
        None => {
            std::process::exit(0); // Invalid input, skip
        }
    };

    // =========================================================================
    // THIS IS WHERE YOUR ACTUAL FUZZING LOGIC GOES
    // =========================================================================
    //
    // In a real harness, you would:
    //
    // 1. Initialize TridentSVM (once, outside the loop for persistent mode)
    //    let mut svm = TridentSVM::default();
    //    svm.deploy_program(...);
    //
    // 2. Build a transaction from the fuzz input
    //    let tx = Transaction::new_signed_with_payer(...);
    //
    // 3. Execute with traces
    //    let result = svm.execute_transaction_with_traces(tx);
    //
    // 4. Convert traces to coverage and write to AFL's map
    //    traces_to_coverage_map(&result.traces, &mut coverage_map);
    //
    // 5. Check for interesting conditions (optional)
    //
    // =========================================================================

    // For this demo, we simulate coverage based on input bytes
    // In reality, this comes from TridentSVM's execution traces
    simulate_coverage(&instruction_data, &mut coverage_map);

    // Print coverage stats
    let edges = count_edges(&coverage_map);
    eprintln!("[+] Input: {} bytes, Edges: {}", input.len(), edges);

    std::process::exit(0);
}

/// Simulates coverage for demonstration purposes.
/// In a real harness, replace this with `traces_to_coverage_map()`.
fn simulate_coverage(data: &[u8], map: &mut CoverageMap) {
    // Clear the map first (important in persistent mode)
    reset_coverage_map(map);

    // Simulate edge coverage based on input bytes
    // This mimics the AFL edge hashing algorithm
    let mut prev = 0u64;
    for &byte in data {
        let edge = (prev >> 1) ^ (byte as u64);
        let idx = (edge as usize) & (COVERAGE_MAP_SIZE - 1);
        map[idx] = map[idx].saturating_add(1);
        prev = byte as u64;
    }
}

// ============================================================================
// PERSISTENT MODE (Optional, for better performance)
// ============================================================================
//
// AFL++ supports persistent mode where your harness processes multiple inputs
// without restarting. This is MUCH faster.
//
// To use persistent mode, you'd structure your harness like:
//
// ```rust
// fn main() {
//     // One-time initialization
//     let mut svm = TridentSVM::default();
//     svm.deploy_program(...);
//     let afl_map = get_afl_map().expect("Must run under AFL");
//
//     // AFL persistent mode loop
//     // The __AFL_LOOP macro is provided by cargo-afl
//     loop {
//         // Reset coverage map
//         reset_coverage_map(afl_map);
//
//         // Reset SVM state if needed
//         svm.clear_accounts();  // or similar
//
//         // Read and process input
//         let mut input = Vec::new();
//         io::stdin().read_to_end(&mut input).unwrap();
//
//         let tx = build_transaction_from_input(&input);
//         let result = svm.execute_transaction_with_traces(tx);
//         traces_to_coverage_map(&result.traces, afl_map);
//
//         // Signal AFL we're ready for the next input
//         // This is handled by the afl crate's fuzz! macro
//     }
// }
// ```
//
// With cargo-afl, you can use the `afl` crate's `fuzz!` macro for this.
// ============================================================================
