use super::stats::FuzzStats;
use serde_json::json;
use std::fs::OpenOptions;
use std::sync::atomic::Ordering;

impl FuzzStats {
    pub fn save_to_file(&self, signal: Option<i32>) {
        // Get the output file path from the struct
        let path = unsafe {
            std::str::from_utf8(&*self.output_path.get())
                .unwrap_or("")
                .trim_matches(char::from(0))
        };

        // Initialize metrics storage and counters
        let mut metrics = serde_json::Map::new();
        let mut total = 0u64;
        let mut total_successful = 0u64;

        // Process each transaction slot
        for slot in &self.slots {
            if slot.is_active.load(Ordering::Relaxed) == 1 {
                let total_count = slot.total_count.load(Ordering::Relaxed);
                let success_count = slot.success_count.load(Ordering::Relaxed);

                if total_count > 0 {
                    // Get transaction name
                    let name = unsafe {
                        std::str::from_utf8(&*slot.name.get())
                            .unwrap_or("")
                            .trim_matches(char::from(0))
                            .to_string()
                    };

                    // Skip empty transaction names
                    if name.is_empty() {
                        continue;
                    }

                    // Collect error statistics for this transaction
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

                    // Update total counters
                    total += total_count;
                    total_successful += success_count;

                    // Create transaction statistics
                    let transaction_fields = json!({
                        "total_executions": total_count,
                        "successful_executions": success_count,
                        "failed_executions": total_count - success_count,
                        "errors": errors
                            .iter()
                            .map(|(msg, count)| {
                                json!({
                                    "message": msg,
                                    "count": count
                                })
                            })
                            .collect::<Vec<_>>()
                    });

                    // Add transaction to metrics map using name as key
                    metrics.insert(name, transaction_fields);
                }
            }
        }

        // Create the final JSON structure
        let json_output = json!({
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
            }
        });

        // Write the JSON to file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap_or_else(|_| panic!("Failed to open stats file"));

        serde_json::to_writer_pretty(&file, &json_output)
            .unwrap_or_else(|_| eprintln!("Failed to write stats"));
    }
}
