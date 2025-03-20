use solana_sdk::clock::Clock;

use crate::utils::get_current_timestamp;

#[derive(Default)]
pub struct SysvarTracker {
    pub last_clock_update: u64, // unix timestamp as seconds
}

impl SysvarTracker {
    pub fn refresh(&mut self) {
        self.last_clock_update = get_current_timestamp();
    }
    pub fn refresh_with_clock(&mut self, clock: &mut Clock) {
        let current_timestamp = get_current_timestamp();

        let time_since_last_update = current_timestamp.saturating_sub(self.last_clock_update);

        clock.unix_timestamp = clock
            .unix_timestamp
            .saturating_add(time_since_last_update as i64);

        self.last_clock_update = current_timestamp;
    }
}

#[cfg(test)]
mod tests {
    use std::thread::sleep;
    use std::time::Duration;

    use solana_sdk::clock::Clock;

    use crate::accounts_database::accounts_db::AccountsDB;

    #[test]
    fn test_clock_update() {
        let mut db = AccountsDB::default();

        // Set initial clock
        let initial_clock = Clock::default();
        db.set_sysvar(&initial_clock);
        let initial_timestamp = db.get_sysvar::<Clock>().unix_timestamp;

        // Sleep for 2 seconds
        sleep(Duration::from_secs(2));
        let updated_clock: Clock = db.get_sysvar();
        assert!(
            updated_clock.unix_timestamp > initial_timestamp,
            "Clock timestamp should have increased"
        );
        let diff = (updated_clock.unix_timestamp - initial_timestamp) as u64;
        assert!(
            (1..=3).contains(&diff),
            "Clock update difference should be ~2 seconds, got {}",
            diff
        );
    }

    #[test]
    fn test_sysvar_tracker_updates() {
        let mut db = AccountsDB::default();

        // Set initial clock and get tracker time
        db.set_sysvar(&Clock::default());
        let initial_tracker_time = db.sysvar_tracker.last_clock_update;
        sleep(Duration::from_secs(1));

        // Force clock update
        let _: Clock = db.get_sysvar();
        assert!(
            db.sysvar_tracker.last_clock_update > initial_tracker_time,
            "SysvarTracker should have been updated"
        );
    }

    #[test]
    fn test_multiple_clock_updates() {
        let mut db = AccountsDB::default();

        // Set initial clock
        let initial_clock = Clock::default();
        db.set_sysvar(&initial_clock);

        // First update
        sleep(Duration::from_secs(1));
        let first_update: Clock = db.get_sysvar();
        let first_diff = (first_update.unix_timestamp - initial_clock.unix_timestamp) as u64;
        assert!(
            (1..=2).contains(&first_diff),
            "First update difference should be ~1 second"
        );

        // Second update
        sleep(Duration::from_secs(1));
        let second_update: Clock = db.get_sysvar();
        let second_diff = (second_update.unix_timestamp - first_update.unix_timestamp) as u64;
        assert!(
            (1..=2).contains(&second_diff),
            "Second update difference should be ~1 second"
        );

        // Verify total elapsed time
        let total_diff = (second_update.unix_timestamp - initial_clock.unix_timestamp) as u64;
        assert!(
            (2..=3).contains(&total_diff),
            "Total time difference should be ~2 seconds"
        );
    }

    #[test]
    fn test_time_manipulation() {
        let mut db = AccountsDB::default();

        // Set initial clock
        let initial_clock = Clock::default();
        db.set_sysvar(&initial_clock);

        // Get initial time
        let mut clock: Clock = db.get_sysvar();
        let initial_time = clock.unix_timestamp;

        // Forward 600 seconds
        db.forward_in_time(600);
        clock = db.get_sysvar();
        assert_eq!(
            clock.unix_timestamp,
            initial_time + 600,
            "Clock should advance 600 seconds"
        );

        // Warp to specific timestamp
        db.warp_to_timestamp(500);
        clock = db.get_sysvar();
        assert_eq!(
            clock.unix_timestamp, 500,
            "Clock should warp to timestamp 500"
        );

        // Test negative time forwarding
        db.forward_in_time(-300);
        clock = db.get_sysvar();
        assert_eq!(
            clock.unix_timestamp, 200,
            "Clock should go back 300 seconds from 500"
        );
    }
}
