use reel::mux::{aac, annexb, h264, hevc};

fn annexb(units: &[&[u8]]) -> Vec<u8> {
    units.iter().flat_map(|unit| [&[0, 0, 0, 1][..], unit].concat()).collect()
}

const SPS: &[u8] = &[0x67, 0x64, 0x00, 0x2a, 0xac];
const PPS: &[u8] = &[0x68, 0xee, 0x3c, 0x80];
const IDR: &[u8] = &[0x65, 0x88, 0x84, 0x21];
const AUD: &[u8] = &[0x09, 0xf0];

#[test]
fn splits_three_and_four_byte_start_codes() {
    let stream = [&[0, 0, 0, 1, 0x67, 1][..], &[0, 0, 1, 0x68, 2], &[0, 0, 1, 0x65, 3]].concat();
    assert_eq!(annexb::units(&stream), vec![&[0x67, 1][..], &[0x68, 2], &[0x65, 3]]);
}

#[test]
fn removes_emulation_prevention_bytes() {
    assert_eq!(annexb::unescape(&[1, 0, 0, 3, 1, 0, 0, 3, 0]), vec![1, 0, 0, 1, 0, 0, 0]);
}

#[test]
fn h264_extracts_parameter_sets_and_strips_them_from_samples() {
    let stream = annexb(&[AUD, SPS, PPS, IDR]);
    let parameters = h264::parameter_sets(&stream).unwrap();
    assert_eq!((parameters.sps.as_slice(), parameters.pps.as_slice()), (SPS, PPS));
    assert!(h264::is_keyframe(&stream));
    assert_eq!(h264::to_sample(&stream), [&[0, 0, 0, 4][..], IDR].concat());
}

#[test]
fn h264_config_copies_profile_and_level_from_sps() {
    let config = h264::config_box(&h264::ParameterSets { sps: SPS.to_vec(), pps: PPS.to_vec() });
    assert_eq!(&config[4..8], b"avcC");
    assert_eq!(&config[8..14], &[1, 0x64, 0x00, 0x2a, 0xff, 0xe1]);
}

#[test]
fn h264_delta_frames_are_not_keyframes() {
    assert!(!h264::is_keyframe(&annexb(&[&[0x41, 0x9a]])));
}

#[test]
fn hevc_recognises_irap_frames_and_builds_hvcc() {
    let vps: &[u8] = &[0x40, 0x01, 0x0c];
    let sps: &[u8] = &[0x42, 0x01, 0x01, 0x01, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 0x78, 0xa0];
    let pps: &[u8] = &[0x44, 0x01, 0xc1];
    let idr: &[u8] = &[0x26, 0x01, 0xaf];
    let stream = annexb(&[vps, sps, pps, idr]);
    assert!(hevc::is_keyframe(&stream));
    assert!(!hevc::is_keyframe(&annexb(&[&[0x02, 0x01, 0xd0]])));
    let config = hevc::config_box(&hevc::parameter_sets(&stream).unwrap()).unwrap();
    assert_eq!(&config[4..8], b"hvcC");
    assert_eq!(config[8], 1);
    assert_eq!(config[9], 0x01);
    assert_eq!(config[20], 0x78);
    assert_eq!(*config.last().unwrap(), 0xc1);
    assert_eq!(hevc::to_sample(&stream), [&[0, 0, 0, 3][..], idr].concat());
}

#[test]
fn aac_config_matches_known_values() {
    assert_eq!(aac::audio_specific_config(48_000, 2), Some([0x11, 0x90]));
    assert_eq!(aac::audio_specific_config(44_100, 2), Some([0x12, 0x10]));
    assert_eq!(aac::audio_specific_config(12_345, 2), None);
}
