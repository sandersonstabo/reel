use reel::settings::{
    ClipKey, ClipLength, Codec, FrameRate, ResolutionCap, Settings, SizeTarget,
    codec::{parse, serialize},
};

#[test]
fn round_trips_every_field() {
    let settings = Settings {
        enabled: false,
        clip_length: ClipLength::Minutes2,
        size_target: SizeTarget::Megabytes(10),
        resolution: ResolutionCap::P1080,
        frame_rate: FrameRate::Fps30,
        codec: Codec::Av1,
        clip_key: ClipKey::F10,
    };
    assert_eq!(parse(&serialize(&settings)), settings);
}

#[test]
fn empty_file_gives_defaults() {
    assert_eq!(parse(""), Settings::default());
}

#[test]
fn invalid_values_fall_back_per_field() {
    let parsed = parse("clip_seconds=17\nsize_target_mb=20\ncodec=vp9\nfps=144\nresolution=native\nclip_key=F7\n");
    assert_eq!(
        parsed,
        Settings {
            size_target: SizeTarget::Megabytes(20),
            clip_key: ClipKey::F7,
            ..Settings::default()
        }
    );
}

#[test]
fn ignores_noise_and_whitespace() {
    let parsed = parse("garbage\n  codec = hevc \nunknown=1\n");
    assert_eq!(parsed.codec, Codec::Hevc);
}
