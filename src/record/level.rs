pub fn peak(pcm: &[u8]) -> u16 {
    pcm.chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]).unsigned_abs())
        .max()
        .unwrap_or(0)
}

pub fn decibels(peak: u16) -> Option<f64> {
    (peak > 0).then(|| 20.0 * (f64::from(peak) / f64::from(i16::MAX)).log10())
}

pub fn describe(peak: u16) -> String {
    decibels(peak).map_or_else(|| "silent".to_string(), |level| format!("peak {level:.1} dBFS"))
}
