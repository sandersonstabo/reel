use reel::record::level::{decibels, describe, peak};

fn pcm(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|sample| sample.to_le_bytes()).collect()
}

#[test]
fn peak_is_the_largest_magnitude_including_negative_samples() {
    assert_eq!(peak(&pcm(&[3, -900, 400])), 900);
    assert_eq!(peak(&pcm(&[i16::MIN])), 32_768);
}

#[test]
fn full_scale_is_zero_decibels() {
    assert!(decibels(i16::MAX as u16).unwrap().abs() < 1e-9);
    assert!((decibels(16_384).unwrap() + 6.02).abs() < 0.01);
}

#[test]
fn digital_silence_is_reported_as_silent() {
    assert_eq!(decibels(0), None);
    assert_eq!(describe(peak(&pcm(&[0, 0, 0]))), "silent");
}
