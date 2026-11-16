pub mod codec;
pub mod store;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipLength {
    Seconds15,
    Seconds30,
    Minute1,
    Minutes2,
}

impl ClipLength {
    pub const ALL: [ClipLength; 4] = [
        ClipLength::Seconds15,
        ClipLength::Seconds30,
        ClipLength::Minute1,
        ClipLength::Minutes2,
    ];

    pub fn seconds(self) -> u32 {
        match self {
            ClipLength::Seconds15 => 15,
            ClipLength::Seconds30 => 30,
            ClipLength::Minute1 => 60,
            ClipLength::Minutes2 => 120,
        }
    }

    pub fn from_seconds(seconds: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|length| length.seconds() == seconds)
    }

    pub fn label(self) -> &'static str {
        match self {
            ClipLength::Seconds15 => "15 seconds",
            ClipLength::Seconds30 => "30 seconds",
            ClipLength::Minute1 => "1 minute",
            ClipLength::Minutes2 => "2 minutes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeTarget {
    Unlimited,
    Megabytes(u32),
}

impl SizeTarget {
    pub const ALL: [SizeTarget; 6] = [
        SizeTarget::Unlimited,
        SizeTarget::Megabytes(10),
        SizeTarget::Megabytes(20),
        SizeTarget::Megabytes(30),
        SizeTarget::Megabytes(50),
        SizeTarget::Megabytes(100),
    ];

    pub fn megabytes(self) -> u32 {
        match self {
            SizeTarget::Unlimited => 0,
            SizeTarget::Megabytes(megabytes) => megabytes,
        }
    }

    pub fn from_megabytes(megabytes: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|target| target.megabytes() == megabytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipKey {
    F6,
    F7,
    F9,
    F10,
    F11,
}

impl ClipKey {
    pub const ALL: [ClipKey; 5] = [ClipKey::F6, ClipKey::F7, ClipKey::F9, ClipKey::F10, ClipKey::F11];

    pub fn label(self) -> &'static str {
        match self {
            ClipKey::F6 => "F6",
            ClipKey::F7 => "F7",
            ClipKey::F9 => "F9",
            ClipKey::F10 => "F10",
            ClipKey::F11 => "F11",
        }
    }

    pub fn virtual_key(self) -> u16 {
        match self {
            ClipKey::F6 => 0x75,
            ClipKey::F7 => 0x76,
            ClipKey::F9 => 0x78,
            ClipKey::F10 => 0x79,
            ClipKey::F11 => 0x7A,
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|key| key.label().eq_ignore_ascii_case(label))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Av1,
}

impl Codec {
    pub const ALL: [Codec; 3] = [Codec::H264, Codec::Hevc, Codec::Av1];

    pub fn key(self) -> &'static str {
        match self {
            Codec::H264 => "h264",
            Codec::Hevc => "hevc",
            Codec::Av1 => "av1",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Codec::H264 => "H.264 (plays everywhere)",
            Codec::Hevc => "H.265 / HEVC",
            Codec::Av1 => "AV1",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|codec| codec.key() == key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRate {
    Fps30,
    Fps60,
}

impl FrameRate {
    pub const ALL: [FrameRate; 2] = [FrameRate::Fps30, FrameRate::Fps60];

    pub fn fps(self) -> u32 {
        match self {
            FrameRate::Fps30 => 30,
            FrameRate::Fps60 => 60,
        }
    }

    pub fn from_fps(fps: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|rate| rate.fps() == fps)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionCap {
    Native,
    P2160,
    P1440,
    P1080,
    P720,
}

impl ResolutionCap {
    pub const ALL: [ResolutionCap; 5] = [
        ResolutionCap::Native,
        ResolutionCap::P2160,
        ResolutionCap::P1440,
        ResolutionCap::P1080,
        ResolutionCap::P720,
    ];

    pub fn height(self) -> Option<u32> {
        match self {
            ResolutionCap::Native => None,
            ResolutionCap::P2160 => Some(2160),
            ResolutionCap::P1440 => Some(1440),
            ResolutionCap::P1080 => Some(1080),
            ResolutionCap::P720 => Some(720),
        }
    }

    pub fn key(self) -> String {
        self.height().map_or_else(|| "native".to_string(), |height| height.to_string())
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|cap| cap.key() == key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub clip_length: ClipLength,
    pub size_target: SizeTarget,
    pub resolution: ResolutionCap,
    pub frame_rate: FrameRate,
    pub codec: Codec,
    pub clip_key: ClipKey,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            clip_length: ClipLength::Seconds30,
            size_target: SizeTarget::Unlimited,
            resolution: ResolutionCap::Native,
            frame_rate: FrameRate::Fps60,
            codec: Codec::H264,
            clip_key: ClipKey::F9,
        }
    }
}
