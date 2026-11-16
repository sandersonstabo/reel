use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

use super::TICKS_PER_SECOND;

pub fn now() -> i64 {
    let mut counter = 0i64;
    let mut frequency = 1i64;
    unsafe {
        let _ = QueryPerformanceCounter(&mut counter);
        let _ = QueryPerformanceFrequency(&mut frequency);
    }
    (i128::from(counter) * i128::from(TICKS_PER_SECOND) / i128::from(frequency.max(1))) as i64
}
