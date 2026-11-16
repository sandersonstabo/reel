use crate::settings::{Codec, Settings, SizeTarget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateControl {
    Quality,
    Constant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub codec: Codec,
    pub resolution: Resolution,
    pub fps: u32,
    pub video_bitrate: u32,
    pub audio_bitrate: u32,
    pub rate_control: RateControl,
}

pub const KEYFRAME_SECONDS: u32 = 1;
const FULL_AUDIO_BITRATE: u32 = 192_000;
const TARGET_AUDIO_BITRATE: u32 = 128_000;
const CONTAINER_HEADROOM: f64 = 0.92;
const LADDER: [u32; 8] = [2160, 1440, 1080, 900, 720, 540, 480, 360];

fn bits_per_pixel(codec: Codec) -> (f64, f64) {
    match codec {
        Codec::H264 => (0.12, 0.04),
        Codec::Hevc => (0.08, 0.027),
        Codec::Av1 => (0.07, 0.023),
    }
}

fn even(value: u64) -> u32 {
    (value.max(2) & !1) as u32
}

fn scaled(native: Resolution, height: u32) -> Resolution {
    Resolution {
        width: even(u64::from(native.width) * u64::from(height) / u64::from(native.height.max(1))),
        height: even(u64::from(height)),
    }
}

fn pixel_rate(resolution: Resolution, fps: u32) -> f64 {
    f64::from(resolution.width) * f64::from(resolution.height) * f64::from(fps)
}

pub fn native(width: u32, height: u32) -> Resolution {
    Resolution {
        width: even(u64::from(width)),
        height: even(u64::from(height)),
    }
}

fn capped(native: Resolution, settings: &Settings) -> Resolution {
    match settings.resolution.height() {
        Some(height) if height < native.height => scaled(native, height),
        _ => native,
    }
}

pub fn plan(native: Resolution, settings: &Settings) -> Plan {
    let codec = settings.codec;
    let fps = settings.frame_rate.fps();
    let (quality, minimum) = bits_per_pixel(codec);
    let ceiling = capped(native, settings);
    let quality_bitrate = |resolution: Resolution| (pixel_rate(resolution, fps) * quality) as u32;
    match settings.size_target {
        SizeTarget::Unlimited => Plan {
            codec,
            resolution: ceiling,
            fps,
            video_bitrate: quality_bitrate(ceiling),
            audio_bitrate: FULL_AUDIO_BITRATE,
            rate_control: RateControl::Quality,
        },
        SizeTarget::Megabytes(megabytes) => {
            let seconds = f64::from(settings.clip_length.seconds() + KEYFRAME_SECONDS);
            let budget = f64::from(megabytes) * 1_000_000.0 * 8.0 * CONTAINER_HEADROOM / seconds;
            let video = (budget - f64::from(TARGET_AUDIO_BITRATE)).max(100_000.0);
            let smaller: Vec<Resolution> = LADDER
                .into_iter()
                .filter(|height| *height < ceiling.height)
                .map(|height| scaled(ceiling, height))
                .collect();
            let resolution = std::iter::once(ceiling)
                .chain(smaller.iter().copied())
                .find(|resolution| video >= pixel_rate(*resolution, fps) * minimum)
                .or(smaller.last().copied())
                .unwrap_or(ceiling);
            Plan {
                codec,
                resolution,
                fps,
                video_bitrate: (video as u32).min(quality_bitrate(resolution)),
                audio_bitrate: TARGET_AUDIO_BITRATE,
                rate_control: RateControl::Constant,
            }
        }
    }
}

pub fn label(plan: &Plan) -> String {
    format!("{}p{}", plan.resolution.height, plan.fps)
}
