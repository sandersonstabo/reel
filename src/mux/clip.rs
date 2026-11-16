use super::{
    aac, av1, h264, hevc,
    mp4::{self, AudioTrack, Sample, VideoTrack},
};
use crate::{
    record::ring::{Clip, Packet},
    settings::Codec,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Format {
    pub codec: Codec,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub rate: u32,
    pub channels: u16,
    pub audio_bitrate: u32,
    pub sequence_header: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipError {
    Empty,
    MissingParameterSets,
    UnsupportedAudio,
}

impl std::fmt::Display for ClipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            ClipError::Empty => "the buffer has no keyframe yet",
            ClipError::MissingParameterSets => "the encoder produced no parameter sets",
            ClipError::UnsupportedAudio => "unsupported audio format",
        })
    }
}

fn samples(packets: &[Packet], convert: impl Fn(&[u8]) -> Vec<u8>) -> Vec<Sample> {
    packets
        .iter()
        .map(|packet| Sample {
            time: packet.time,
            duration: packet.duration,
            keyframe: packet.keyframe,
            data: convert(&packet.data),
        })
        .collect()
}

pub fn regularize(samples: Vec<Sample>, fps: u32) -> Vec<Sample> {
    let interval = mp4::SOURCE_TICKS / i64::from(fps.max(1));
    let exact = |slot: i64| slot * mp4::SOURCE_TICKS / i64::from(fps.max(1));
    samples
        .into_iter()
        .scan(None, |previous: &mut Option<i64>, sample| {
            let nearest = (sample.time * i64::from(fps.max(1)) + mp4::SOURCE_TICKS / 2).div_euclid(mp4::SOURCE_TICKS);
            let slot = previous.map_or(nearest, |last| nearest.max(last + 1));
            *previous = Some(slot);
            Some(Sample {
                time: exact(slot),
                duration: interval,
                ..sample
            })
        })
        .collect()
}

fn sources<'a>(first: &'a [u8], format: &'a Format) -> impl Iterator<Item = &'a [u8]> {
    std::iter::once(first).chain(format.sequence_header.as_deref())
}

type Description = ([u8; 4], Vec<u8>, fn(&[u8]) -> Vec<u8>);

fn describe(first: &[u8], format: &Format) -> Option<Description> {
    match format.codec {
        Codec::H264 => sources(first, format)
            .find_map(h264::parameter_sets)
            .map(|parameters| (*b"avc1", h264::config_box(&parameters), h264::to_sample as fn(&[u8]) -> Vec<u8>)),
        Codec::Hevc => sources(first, format)
            .find_map(hevc::parameter_sets)
            .and_then(|parameters| hevc::config_box(&parameters))
            .map(|config| (*b"hvc1", config, hevc::to_sample as fn(&[u8]) -> Vec<u8>)),
        Codec::Av1 => sources(first, format)
            .find_map(av1::sequence_header)
            .map(|sequence| (*b"av01", av1::config_box(&sequence), av1::to_sample as fn(&[u8]) -> Vec<u8>)),
    }
}

pub fn is_keyframe(codec: Codec, data: &[u8]) -> bool {
    match codec {
        Codec::H264 => h264::is_keyframe(data),
        Codec::Hevc => hevc::is_keyframe(data),
        Codec::Av1 => av1::sequence_header(data).is_some(),
    }
}

pub fn build(clip: &Clip, format: &Format) -> Result<Vec<u8>, ClipError> {
    let first = clip.video.first().ok_or(ClipError::Empty)?;
    let (entry, config, convert) = describe(&first.data, format).ok_or(ClipError::MissingParameterSets)?;
    let audio_config = aac::audio_specific_config(format.rate, format.channels).ok_or(ClipError::UnsupportedAudio)?;
    let video = VideoTrack {
        width: format.width,
        height: format.height,
        entry,
        config,
        samples: regularize(samples(&clip.video, convert), format.fps),
    };
    let audio = AudioTrack {
        rate: format.rate,
        channels: format.channels,
        config: audio_config,
        bitrate: format.audio_bitrate,
        samples: samples(&clip.audio, <[u8]>::to_vec),
    };
    Ok(mp4::mux(&video, &audio))
}
