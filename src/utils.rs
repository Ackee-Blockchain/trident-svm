use std::time::UNIX_EPOCH;

pub(crate) fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards!")
        .as_secs()
}
