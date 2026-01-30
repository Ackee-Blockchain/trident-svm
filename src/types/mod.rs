pub mod coverage;
pub mod transaction_result;
pub mod trident_account;
#[cfg(feature = "syscall-v2")]
pub mod trident_entrypoint;
pub mod trident_program;

pub use coverage::{
    count_edges, count_trace_entries, reset_coverage_map, traces_to_coverage,
    traces_to_coverage_map, CoverageMap, COVERAGE_MAP_SIZE,
    // Enhanced versions
    traces_to_coverage_map_context,
    traces_to_coverage_map_dataflow,
    traces_to_coverage_map_enhanced,
};
pub use transaction_result::ExecutionTraces;

#[derive(Default)]
pub struct TraceCollector {
    traces: Vec<Vec<[u64; 12]>>,
}

impl TraceCollector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn trace(&mut self, traces: &[Vec<[u64; 12]>]) {
        self.traces.extend(traces.to_vec());
    }
}
