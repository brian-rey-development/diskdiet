pub mod fixtures;
pub mod git;

pub const STALE_DAYS: u64 = 90;
pub const SECS_PER_DAY: u64 = 86_400;
pub const BLOCK_BYTES: u64 = 512;
pub const RWX_MODE: u32 = 0o755;
pub const READ_ONLY_DIR_MODE: u32 = 0o555;
pub const NO_ACCESS_MODE: u32 = 0o000;
