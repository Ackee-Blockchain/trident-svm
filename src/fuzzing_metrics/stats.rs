use super::error_entry::ErrorEntry;
use super::transaction_slot::TransactionSlot;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C)]
pub struct FuzzStats {
    pub(crate) slots: [TransactionSlot; 512],
    pub(crate) next_slot: AtomicU64,
    pub(crate) output_path: UnsafeCell<[u8; 256]>,
}

unsafe impl Sync for FuzzStats {}

impl FuzzStats {
    pub(crate) fn new(output_path: String) -> Self {
        let stats = Self {
            slots: std::array::from_fn(|_| TransactionSlot {
                total_count: AtomicU64::new(0),
                success_count: AtomicU64::new(0),
                name: UnsafeCell::new([0; 64]),
                errors: std::array::from_fn(|_| ErrorEntry {
                    count: AtomicU64::new(0),
                    message: UnsafeCell::new([0; 128]),
                    is_active: AtomicU64::new(0),
                }),
                next_error_slot: AtomicU64::new(0),
                is_active: AtomicU64::new(0),
            }),
            next_slot: AtomicU64::new(0),
            output_path: UnsafeCell::new([0; 256]),
        };

        // Copy output path to fixed buffer
        let path_bytes = output_path.as_bytes();
        let len = path_bytes.len().min(255);
        unsafe {
            let output_buffer = &mut *stats.output_path.get();
            output_buffer[..len].copy_from_slice(&path_bytes[..len]);
            output_buffer[len] = 0; // Null terminate
        }

        stats
    }

    pub(crate) fn increment_executions(&self, transaction: &str) {
        let slot = self.find_or_create_slot(transaction);
        self.slots[slot].total_count.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn increment_successful_executions(&self, transaction: &str) {
        let slot = self.find_or_create_slot(transaction);
        self.slots[slot]
            .success_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_error(&self, transaction: &str, error_msg: &str) {
        let slot = self.find_or_create_slot(transaction);
        self.slots[slot].record_error(error_msg);
    }

    fn find_or_create_slot(&self, transaction: &str) -> usize {
        // First try to find existing slot
        for i in 0..self.slots.len() {
            if self.slots[i].is_active.load(Ordering::Relaxed) == 1 {
                let name = unsafe {
                    std::str::from_utf8(&*self.slots[i].name.get())
                        .unwrap_or("")
                        .trim_matches(char::from(0))
                };
                if name == transaction {
                    return i;
                }
            }
        }

        // Create new slot if not found
        let new_slot = (self.next_slot.fetch_add(1, Ordering::Relaxed) as usize) % self.slots.len();

        // Initialize the slot if it's not active
        if self.slots[new_slot]
            .is_active
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            unsafe {
                let name_buffer = &mut *self.slots[new_slot].name.get();
                let bytes = transaction.as_bytes();
                let len = bytes.len().min(63);
                name_buffer[..len].copy_from_slice(&bytes[..len]);
                name_buffer[len] = 0; // Null terminate
            }
        }

        new_slot
    }
}
