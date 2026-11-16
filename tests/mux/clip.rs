#[path = "../support/boxes.rs"]
mod boxes;

use std::sync::Arc;

use reel::{
    mux::{
        clip::{ClipError, Format, build, regularize},
        mp4::Sample,
    },
    record::ring::{Clip, Packet},
    settings::Codec,
};

const SPS: &[u8] = &[0x67, 0x64, 0x00, 0x2a, 0xac];
const PPS: &[u8] = &[0x68, 0xee, 0x3c, 0x80];

fn packet(time: i64, keyframe: bool, data: Vec<u8>) -> Packet {
    Packet {
        time,
        duration: 166_666,
        keyframe,
        data: Arc::from(data),
    }
}

fn annexb(units: &[&[u8]]) -> Vec<u8> {
    units.iter().flat_map(|unit| [&[0, 0, 0, 1][..], unit].concat()).collect()
}

fn format(sequence_header: Option<Vec<u8>>) -> Format {
    Format {
        codec: Codec::H264,
        fps: 60,
        width: 1920,
        height: 1080,
        rate: 48_000,
        channels: 2,
        audio_bitrate: 192_000,
        sequence_header,
    }
}

#[test]
fn h264_clip_uses_in_band_parameter_sets() {
    let clip = Clip {
        video: vec![
            packet(0, true, annexb(&[SPS, PPS, &[0x65, 1, 2]])),
            packet(166_666, false, annexb(&[&[0x41, 3]])),
        ],
        audio: vec![packet(0, true, vec![0x21, 0x10])],
    };
    let file = build(&clip, &format(None)).unwrap();
    let stsd = boxes::find(&file, &[b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stsd"]);
    assert_eq!(&stsd[0][12..16], b"avc1");
    assert_eq!(&stsd[1][12..16], b"mp4a");
}

#[test]
fn falls_back_to_the_encoder_sequence_header() {
    let clip = Clip {
        video: vec![packet(0, true, annexb(&[&[0x65, 1]]))],
        audio: vec![],
    };
    assert!(build(&clip, &format(Some(annexb(&[SPS, PPS])))).is_ok());
    assert_eq!(build(&clip, &format(None)), Err(ClipError::MissingParameterSets));
}

#[test]
fn empty_clip_is_an_error() {
    let clip = Clip {
        video: vec![],
        audio: vec![],
    };
    assert_eq!(build(&clip, &format(None)), Err(ClipError::Empty));
}

fn at(time: i64) -> Sample {
    Sample {
        time,
        duration: 1,
        keyframe: false,
        data: vec![],
    }
}

#[test]
fn jittery_capture_times_snap_to_the_frame_grid() {
    let interval = 10_000_000 / 60;
    let times: Vec<i64> = regularize(
        (0..120).map(|n| at(n * interval + [40_000, -50_000, 10_000][n as usize % 3])).collect(),
        60,
    )
    .iter()
    .map(|sample| sample.time)
    .collect();
    let expected: Vec<i64> = (0..120).map(|n| n * 10_000_000 / 60).collect();
    assert_eq!(times, expected);
}

#[test]
fn snapping_never_merges_or_reorders_frames() {
    let snapped = regularize(vec![at(0), at(10_000), at(20_000), at(5_000_000)], 60);
    let times: Vec<i64> = snapped.iter().map(|sample| sample.time).collect();
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]), "{times:?}");
    assert_eq!(times[3], 30 * 10_000_000 / 60);
}
