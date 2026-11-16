use super::{ClipKey, ClipLength, Codec, FrameRate, ResolutionCap, Settings, SizeTarget};

fn apply(settings: Settings, line: &str) -> Settings {
    let Some((key, value)) = line.split_once('=') else {
        return settings;
    };
    let value = value.trim();
    match key.trim() {
        "enabled" => match value {
            "true" => Settings { enabled: true, ..settings },
            "false" => Settings { enabled: false, ..settings },
            _ => settings,
        },
        "clip_seconds" => value
            .parse()
            .ok()
            .and_then(ClipLength::from_seconds)
            .map_or(settings, |clip_length| Settings { clip_length, ..settings }),
        "size_target_mb" => value
            .parse()
            .ok()
            .and_then(SizeTarget::from_megabytes)
            .map_or(settings, |size_target| Settings { size_target, ..settings }),
        "resolution" => ResolutionCap::from_key(value)
            .map_or(settings, |resolution| Settings { resolution, ..settings }),
        "fps" => value
            .parse()
            .ok()
            .and_then(FrameRate::from_fps)
            .map_or(settings, |frame_rate| Settings { frame_rate, ..settings }),
        "codec" => Codec::from_key(value).map_or(settings, |codec| Settings { codec, ..settings }),
        "clip_key" => ClipKey::from_label(value)
            .map_or(settings, |clip_key| Settings { clip_key, ..settings }),
        _ => settings,
    }
}

pub fn parse(text: &str) -> Settings {
    text.lines().fold(Settings::default(), apply)
}

pub fn serialize(settings: &Settings) -> String {
    format!(
        "enabled={}\nclip_seconds={}\nsize_target_mb={}\nresolution={}\nfps={}\ncodec={}\nclip_key={}\n",
        settings.enabled,
        settings.clip_length.seconds(),
        settings.size_target.megabytes(),
        settings.resolution.key(),
        settings.frame_rate.fps(),
        settings.codec.key(),
        settings.clip_key.label(),
    )
}
