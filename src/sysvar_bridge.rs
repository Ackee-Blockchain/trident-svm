// In a new module baby!! 
use solana_program::sysvar::{rent::Rent, clock::Clock, epoch_schedule::EpochSchedule, Sysvar};
use bincode;

/// Initialize all sysvar environment variables from TridentSVM context
pub fn init_sysvar_bridge() -> Result<(), Box<dyn std::error::Error>> {
    // Get actual sysvars from TridentSVM's context
    
    // Rent
    if let Ok(rent) = Rent::get() {
        println!("[SysvarBridge] Rent sysvar: {:?}", rent);
        set_sysvar_env("RENT_DATA_HEX", &rent)?;
    }
    
    // Clock
    if let Ok(clock) = Clock::get() {
        println!("[SysvarBridge] Clock sysvar: {:?}", clock);
        set_sysvar_env("CLOCK_DATA_HEX", &clock)?;
    }
    
    // EpochSchedule
    if let Ok(epoch_schedule) = EpochSchedule::get() {
        println!("[SysvarBridge] EpochSchedule sysvar: {:?}", epoch_schedule);
        set_sysvar_env("EPOCH_SCHEDULE_DATA_HEX", &epoch_schedule)?;
    }
    
    // Currently adding more in a bit!!!
    
    Ok(())
}

/// Helper to serialize and set sysvar as env var
fn set_sysvar_env<T: serde::Serialize>(env_name: &str, sysvar: &T) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = bincode::serialize(sysvar)?;
    let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    std::env::set_var(env_name, hex);
    eprintln!("[SysvarBridge] Set {} with {} bytes", env_name, bytes.len());
    Ok(())
}