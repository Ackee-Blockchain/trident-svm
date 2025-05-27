use crate::fuzzing_metrics::stats::FuzzStats;
use crate::trident_svm::TridentSVM;

impl TridentSVM {
    pub fn increment_transaction_execution(&mut self, transaction: String) {
        if let Some(shmem) = &mut self.fuzz_stats {
            let stats = unsafe { &mut *(shmem.as_ptr() as *mut FuzzStats) };
            stats.increment_executions(&transaction);
        }
    }
    pub fn increment_transaction_success(&mut self, transaction: String) {
        if let Some(shmem) = &self.fuzz_stats {
            let stats = unsafe { &*(shmem.as_ptr() as *const FuzzStats) };
            stats.increment_successful_executions(&transaction);
        }
    }
    pub fn record_transaction_error(&mut self, transaction: String, error_msg: String) {
        if let Some(shmem) = &self.fuzz_stats {
            let stats = unsafe { &mut *(shmem.as_ptr() as *mut FuzzStats) };
            stats.record_error(&transaction, &error_msg);
        }
    }
}
