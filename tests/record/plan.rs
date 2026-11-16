use reel::{
    record::plan::{RateControl, Resolution, label, native, plan},
    settings::{ClipLength, Codec, FrameRate, ResolutionCap, Settings, SizeTarget},
};

const QHD: Resolution = Resolution { width: 2560, height: 1440 };

fn settings(size_target: SizeTarget) -> Settings {
    Settings {
        size_target,
        ..Settings::default()
    }
}

#[test]
fn no_target_records_native_resolution_at_quality_rate() {
    let planned = plan(QHD, &settings(SizeTarget::Unlimited));
    assert_eq!(label(&planned), "1440p60");
    assert_eq!(planned.rate_control, RateControl::Quality);
    assert!(planned.video_bitrate > 20_000_000);
}

#[test]
fn ten_megabytes_over_thirty_seconds_drops_to_720p() {
    let planned = plan(QHD, &settings(SizeTarget::Megabytes(10)));
    assert_eq!(planned.resolution, Resolution { width: 1280, height: 720 });
    assert_eq!(planned.rate_control, RateControl::Constant);
}

#[test]
fn target_bitrate_fits_the_file_size() {
    [10, 20, 30, 50, 100].into_iter().for_each(|megabytes| {
        let target = Settings {
            clip_length: ClipLength::Minute1,
            ..settings(SizeTarget::Megabytes(megabytes))
        };
        let planned = plan(QHD, &target);
        let bytes = f64::from(planned.video_bitrate + planned.audio_bitrate) / 8.0 * 61.0;
        assert!(bytes < f64::from(megabytes) * 1e6, "{megabytes} MB plan writes {bytes} bytes");
    });
}

#[test]
fn larger_targets_never_pick_lower_resolutions() {
    let heights: Vec<u32> = [10, 20, 30, 50, 100]
        .into_iter()
        .map(|megabytes| plan(QHD, &settings(SizeTarget::Megabytes(megabytes))).resolution.height)
        .collect();
    assert!(heights.windows(2).all(|pair| pair[0] <= pair[1]), "{heights:?}");
}

#[test]
fn efficient_codecs_keep_more_resolution_for_the_same_size() {
    let height = |codec| plan(QHD, &Settings { codec, ..settings(SizeTarget::Megabytes(10)) }).resolution.height;
    assert!(height(Codec::Av1) >= height(Codec::H264));
}

#[test]
fn resolution_cap_limits_but_never_upscales() {
    let capped = Settings {
        resolution: ResolutionCap::P1080,
        ..settings(SizeTarget::Unlimited)
    };
    assert_eq!(plan(QHD, &capped).resolution, Resolution { width: 1920, height: 1080 });
    let above = Settings {
        resolution: ResolutionCap::P2160,
        ..settings(SizeTarget::Unlimited)
    };
    assert_eq!(plan(QHD, &above).resolution, QHD);
}

#[test]
fn ultrawide_keeps_its_aspect_ratio() {
    let ultrawide = native(3440, 1440);
    let capped = Settings {
        resolution: ResolutionCap::P1080,
        ..settings(SizeTarget::Unlimited)
    };
    assert_eq!(plan(ultrawide, &capped).resolution, Resolution { width: 2580, height: 1080 });
}

#[test]
fn frame_rate_setting_is_used() {
    let slow = Settings {
        frame_rate: FrameRate::Fps30,
        ..settings(SizeTarget::Unlimited)
    };
    assert_eq!(label(&plan(QHD, &slow)), "1440p30");
}

#[test]
fn odd_dimensions_are_made_even() {
    assert_eq!(native(1365, 767), Resolution { width: 1364, height: 766 });
}
