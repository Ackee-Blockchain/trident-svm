use std::cell::UnsafeCell;
use std::fs::OpenOptions;
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C)]
pub struct ErrorEntry {
    count: AtomicU64,
    message: UnsafeCell<[u8; 128]>, // Larger buffer for error messages
    is_active: AtomicU64,
}

#[repr(C)]
pub struct TransactionSlot {
    total_count: AtomicU64,
    success_count: AtomicU64,
    name: UnsafeCell<[u8; 64]>,
    errors: [ErrorEntry; 32], // Up to 32 different error types per transaction
    next_error_slot: AtomicU64,
    is_active: AtomicU64,
}

#[repr(C)]
pub struct FuzzStats {
    slots: [TransactionSlot; 512],
    next_slot: AtomicU64,
    output_path: UnsafeCell<[u8; 256]>,
}

// Required to share FuzzStats between threads
unsafe impl Sync for FuzzStats {}

impl FuzzStats {
    pub fn new(output_path: String) -> Self {
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

    pub fn increment_executions(&self, transaction: &str) {
        let slot = self.find_or_create_slot(transaction);
        self.slots[slot].total_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_successful_executions(&self, transaction: &str) {
        let slot = self.find_or_create_slot(transaction);
        self.slots[slot]
            .success_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self, transaction: &str, error_msg: &str) {
        let slot = self.find_or_create_slot(transaction);
        let tx_slot = &self.slots[slot];

        // Try to find existing error entry
        for i in 0..tx_slot.errors.len() {
            if tx_slot.errors[i].is_active.load(Ordering::Relaxed) == 1 {
                let msg = unsafe {
                    std::str::from_utf8(&*tx_slot.errors[i].message.get())
                        .unwrap_or("")
                        .trim_matches(char::from(0))
                };
                if msg == error_msg {
                    tx_slot.errors[i].count.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }
        }

        // Create new error entry if not found
        let new_error_slot = (tx_slot.next_error_slot.fetch_add(1, Ordering::Relaxed) as usize)
            % tx_slot.errors.len();
        let error_entry = &tx_slot.errors[new_error_slot];

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

    pub fn save_to_file(&self, signal: Option<i32>) {
        let path = unsafe {
            std::str::from_utf8(&*self.output_path.get())
                .unwrap_or("")
                .trim_matches(char::from(0))
        };

        let mut metrics = Vec::new();
        let mut total = 0u64;
        let mut total_successful = 0u64;

        for slot in &self.slots {
            if slot.is_active.load(Ordering::Relaxed) == 1 {
                let total_count = slot.total_count.load(Ordering::Relaxed);
                let success_count = slot.success_count.load(Ordering::Relaxed);

                if total_count > 0 {
                    let name = unsafe {
                        std::str::from_utf8(&*slot.name.get())
                            .unwrap_or("")
                            .trim_matches(char::from(0))
                            .to_string()
                    };

                    // Collect error statistics
                    let mut errors = Vec::new();
                    for error in &slot.errors {
                        if error.is_active.load(Ordering::Relaxed) == 1 {
                            let count = error.count.load(Ordering::Relaxed);
                            if count > 0 {
                                let msg = unsafe {
                                    std::str::from_utf8(&*error.message.get())
                                        .unwrap_or("")
                                        .trim_matches(char::from(0))
                                        .to_string()
                                };
                                errors.push((msg, count));
                            }
                        }
                    }

                    if !name.is_empty() {
                        total += total_count;
                        total_successful += success_count;
                        metrics.push(serde_json::json!({
                            "name": name,
                            "total_executions": total_count,
                            "successful_executions": success_count,
                            "failed_executions": total_count - success_count,
                            "success_rate": if total_count > 0 {
                                (success_count as f64 / total_count as f64) * 100.0
                            } else {
                                0.0
                            },
                            "errors": errors.iter().map(|(msg, count)| {
                                serde_json::json!({
                                    "message": msg,
                                    "count": count,
                                    "percentage": if total_count > 0 {
                                        (*count as f64 / total_count as f64) * 100.0
                                    } else {
                                        0.0
                                    }
                                })
                            }).collect::<Vec<_>>()
                        }));
                    }
                }
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap_or_else(|_| panic!("Failed to open stats file"));

        let stats_json = serde_json::json!({
            "signal_info": match signal {
                Some(2) => "SIGINT (Ctrl+C)".to_string(),
                Some(15) => "SIGTERM".to_string(),
                Some(sig) => format!("Signal {}", sig),
                None => "Normal Termination".to_string(),
            },
            "statistics": {
                "transactions": metrics,
                "total_executions": total,
                "total_successful_executions": total_successful,
                "total_failed_executions": total - total_successful,
                "overall_success_rate": if total > 0 {
                    (total_successful as f64 / total as f64) * 100.0
                } else {
                    0.0
                }
            }
        });

        serde_json::to_writer_pretty(&file, &stats_json)
            .unwrap_or_else(|_| eprintln!("Failed to write stats"));
    }
}
