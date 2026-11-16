const LOW_COMPLEXITY: u8 = 2;
const RATES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000, 7_350,
];

pub fn audio_specific_config(rate: u32, channels: u16) -> Option<[u8; 2]> {
    let index = RATES.iter().position(|candidate| *candidate == rate)? as u8;
    let channels = u8::try_from(channels).ok().filter(|count| *count <= 7)?;
    Some([
        (LOW_COMPLEXITY << 3) | (index >> 1),
        ((index & 1) << 7) | (channels << 3),
    ])
}
