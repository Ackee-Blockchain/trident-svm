use super::error_entry::ErrorEntry;
use std::cell::UnsafeCell;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[repr(C)]
pub struct TransactionSlot {
    pub(crate) name: UnsafeCell<[u8; 64]>,
    pub(crate) total_count: AtomicU64,
    pub(crate) success_count: AtomicU64,
    pub(crate) errors: [ErrorEntry; 32],
    pub(crate) next_error_slot: AtomicU64,
    pub(crate) is_active: AtomicU64,
}

impl TransactionSlot {
    pub(crate) fn record_error(&self, error_msg: &str) {
        // Try to find existing error entry
        for i in 0..self.errors.len() {
            if self.errors[i].is_active.load(Ordering::Relaxed) == 1 {
                let msg = unsafe {
                    std::str::from_utf8(&*self.errors[i].message.get())
                        .unwrap_or("")
                        .trim_matches(char::from(0))
                };
                if msg == error_msg {
                    self.errors[i].count.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }
        }

        // Create new error entry if not found
        let new_error_slot =
            (self.next_error_slot.fetch_add(1, Ordering::Relaxed) as usize) % self.errors.len();
        let error_entry = &self.errors[new_error_slot];

        if error_entry
            .is_active
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            unsafe {
                let msg_buffer = &mut *error_entry.message.get();
                let bytes = error_msg.as_bytes();
                let len = bytes.len().min(127);
                msg_buffer[..len].copy_from_slice(&bytes[..len]);
                msg_buffer[len] = 0; // Null terminate
            }
            error_entry.count.store(1, Ordering::Relaxed);
        } else {
            error_entry.count.fetch_add(1, Ordering::Relaxed);
        }
    }
}
