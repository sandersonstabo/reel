pub mod audio;
pub mod capture;
pub mod clock;
pub mod convert;
pub mod encode;
pub mod gpu;
pub mod layout;
pub mod pacing;
pub mod plan;
pub mod ring;
pub mod run;
pub mod timeline;

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
pub const TICKS_PER_SECOND: i64 = 10_000_000;
