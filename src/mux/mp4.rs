pub const MOVIE_TIMESCALE: u32 = 1000;
pub const VIDEO_TIMESCALE: u32 = 90_000;
pub const SOURCE_TICKS: i64 = 10_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub time: i64,
    pub duration: i64,
    pub keyframe: bool,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoTrack {
    pub width: u32,
    pub height: u32,
    pub entry: [u8; 4],
    pub config: Vec<u8>,
    pub samples: Vec<Sample>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioTrack {
    pub rate: u32,
    pub channels: u16,
    pub config: [u8; 2],
    pub bitrate: u32,
    pub samples: Vec<Sample>,
}

pub fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let size = (body.len() + 8) as u32;
    [&size.to_be_bytes()[..], kind, body].concat()
}

fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let header = (u32::from(version) << 24) | (flags & 0x00ff_ffff);
    boxed(kind, &[&header.to_be_bytes()[..], body].concat())
}

fn container(kind: &[u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    boxed(kind, &children.concat())
}

fn u16s(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_be_bytes()).collect()
}

fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_be_bytes()).collect()
}

const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

fn scale(ticks: i64, timescale: u32) -> u64 {
    ((i128::from(ticks.max(0)) * i128::from(timescale) + i128::from(SOURCE_TICKS) / 2) / i128::from(SOURCE_TICKS)) as u64
}

pub fn durations(samples: &[Sample], timescale: u32) -> Vec<u32> {
    let starts: Vec<u64> = samples.iter().map(|sample| scale(sample.time, timescale)).collect();
    samples
        .iter()
        .enumerate()
        .map(|(index, sample)| match starts.get(index + 1) {
            Some(next) => next.saturating_sub(starts[index]) as u32,
            None => scale(sample.duration, timescale).max(1) as u32,
        })
        .collect()
}

fn run_lengths(values: &[u32]) -> Vec<(u32, u32)> {
    values.iter().fold(Vec::new(), |mut runs: Vec<(u32, u32)>, value| {
        match runs.last_mut() {
            Some((count, last)) if last == value => *count += 1,
            _ => runs.push((1, *value)),
        }
        runs
    })
}

fn stts(samples: &[Sample], timescale: u32) -> Vec<u8> {
    let runs = run_lengths(&durations(samples, timescale));
    let entries: Vec<u32> = runs.iter().flat_map(|(count, delta)| [*count, *delta]).collect();
    full(b"stts", 0, 0, &[&(runs.len() as u32).to_be_bytes()[..], &u32s(&entries)].concat())
}

fn stsz(samples: &[Sample]) -> Vec<u8> {
    let sizes: Vec<u32> = samples.iter().map(|sample| sample.data.len() as u32).collect();
    full(b"stsz", 0, 0, &[&0u32.to_be_bytes()[..], &(sizes.len() as u32).to_be_bytes(), &u32s(&sizes)].concat())
}

fn stsc() -> Vec<u8> {
    full(b"stsc", 0, 0, &u32s(&[1, 1, 1, 1]))
}

fn stco(offsets: &[u32]) -> Vec<u8> {
    full(b"stco", 0, 0, &[&(offsets.len() as u32).to_be_bytes()[..], &u32s(offsets)].concat())
}

fn stss(samples: &[Sample]) -> Vec<u8> {
    let keys: Vec<u32> = samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| sample.keyframe)
        .map(|(index, _)| index as u32 + 1)
        .collect();
    full(b"stss", 0, 0, &[&(keys.len() as u32).to_be_bytes()[..], &u32s(&keys)].concat())
}

fn total(samples: &[Sample], timescale: u32) -> u64 {
    durations(samples, timescale).iter().map(|duration| u64::from(*duration)).sum()
}

fn mvhd(duration: u64) -> Vec<u8> {
    let body = [
        u32s(&[0, 0, MOVIE_TIMESCALE, duration as u32, 0x0001_0000]),
        u16s(&[0x0100, 0]),
        u32s(&[0, 0]),
        u32s(&MATRIX),
        u32s(&[0; 6]),
        u32s(&[3]),
    ]
    .concat();
    full(b"mvhd", 0, 0, &body)
}

fn tkhd(id: u32, duration: u64, volume: u16, width: u32, height: u32) -> Vec<u8> {
    let body = [
        u32s(&[0, 0, id, 0, duration as u32, 0, 0]),
        u16s(&[0, 0, volume, 0]),
        u32s(&MATRIX),
        u32s(&[width << 16, height << 16]),
    ]
    .concat();
    full(b"tkhd", 0, 3, &body)
}

fn mdhd(timescale: u32, duration: u64) -> Vec<u8> {
    full(b"mdhd", 0, 0, &[u32s(&[0, 0, timescale, duration as u32]), u16s(&[0x55c4, 0])].concat())
}

fn hdlr(handler: &[u8; 4], name: &str) -> Vec<u8> {
    full(
        b"hdlr",
        0,
        0,
        &[&0u32.to_be_bytes()[..], handler, &u32s(&[0, 0, 0]), name.as_bytes(), &[0]].concat(),
    )
}

fn dinf() -> Vec<u8> {
    container(b"dinf", &[full(b"dref", 0, 0, &[&1u32.to_be_bytes()[..], &full(b"url ", 0, 1, &[])].concat())])
}

fn visual_entry(track: &VideoTrack) -> Vec<u8> {
    let body = [
        vec![0; 6],
        u16s(&[1]),
        vec![0; 16],
        u16s(&[track.width as u16, track.height as u16]),
        u32s(&[0x0048_0000, 0x0048_0000, 0]),
        u16s(&[1]),
        vec![0; 32],
        u16s(&[0x0018, 0xffff]),
        track.config.clone(),
    ]
    .concat();
    boxed(&track.entry, &body)
}

fn descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    [&[tag, body.len() as u8][..], body].concat()
}

fn mp4a(track: &AudioTrack) -> Vec<u8> {
    let decoder_config = descriptor(
        0x04,
        &[
            &[0x40, 0x15, 0, 0x18, 0][..],
            &track.bitrate.to_be_bytes(),
            &track.bitrate.to_be_bytes(),
            &descriptor(0x05, &track.config),
        ]
        .concat(),
    );
    let es = descriptor(0x03, &[&[0, 2, 0][..], &decoder_config, &descriptor(0x06, &[0x02])].concat());
    let body = [
        vec![0; 6],
        u16s(&[1]),
        vec![0; 8],
        u16s(&[track.channels, 16, 0, 0]),
        u32s(&[track.rate << 16]),
        full(b"esds", 0, 0, &es),
    ]
    .concat();
    boxed(b"mp4a", &body)
}

struct Layout<'a> {
    id: u32,
    timescale: u32,
    samples: &'a [Sample],
    entry: Vec<u8>,
    media_header: Vec<u8>,
    handler: Vec<u8>,
    volume: u16,
    size: (u32, u32),
    keyframes: bool,
}

fn trak(layout: &Layout, offsets: &[u32]) -> Vec<u8> {
    let media_duration = total(layout.samples, layout.timescale);
    let movie_duration = media_duration * u64::from(MOVIE_TIMESCALE) / u64::from(layout.timescale);
    let stsd = full(b"stsd", 0, 0, &[&1u32.to_be_bytes()[..], &layout.entry].concat());
    let tables = [
        vec![stsd, stts(layout.samples, layout.timescale)],
        match layout.keyframes {
            true => vec![stss(layout.samples)],
            false => vec![],
        },
        vec![stsc(), stsz(layout.samples), stco(offsets)],
    ]
    .concat();
    container(
        b"trak",
        &[
            tkhd(layout.id, movie_duration, layout.volume, layout.size.0, layout.size.1),
            container(
                b"mdia",
                &[
                    mdhd(layout.timescale, media_duration),
                    layout.handler.clone(),
                    container(b"minf", &[layout.media_header.clone(), dinf(), container(b"stbl", &tables)]),
                ],
            ),
        ],
    )
}

fn interleave<'a>(video: &'a [Sample], audio: &'a [Sample]) -> Vec<(usize, usize, &'a Sample)> {
    let mut order: Vec<(usize, usize, &Sample)> = video
        .iter()
        .enumerate()
        .map(|(index, sample)| (0, index, sample))
        .chain(audio.iter().enumerate().map(|(index, sample)| (1, index, sample)))
        .collect();
    order.sort_by_key(|(track, index, sample)| (sample.time, *track, *index));
    order
}

pub fn mux(video: &VideoTrack, audio: &AudioTrack) -> Vec<u8> {
    let ftyp = boxed(b"ftyp", &[&b"isom"[..], &512u32.to_be_bytes(), b"isomiso2avc1mp41"].concat());
    let layouts = [
        Layout {
            id: 1,
            timescale: VIDEO_TIMESCALE,
            samples: &video.samples,
            entry: visual_entry(video),
            media_header: full(b"vmhd", 0, 1, &u16s(&[0, 0, 0, 0])),
            handler: hdlr(b"vide", "Reel video"),
            volume: 0,
            size: (video.width, video.height),
            keyframes: true,
        },
        Layout {
            id: 2,
            timescale: audio.rate.max(1),
            samples: &audio.samples,
            entry: mp4a(audio),
            media_header: full(b"smhd", 0, 0, &u16s(&[0, 0])),
            handler: hdlr(b"soun", "Reel audio"),
            volume: 0x0100,
            size: (0, 0),
            keyframes: false,
        },
    ];
    let order = interleave(&video.samples, &audio.samples);
    let moov = |base: u32| {
        let mut offsets = [vec![0u32; video.samples.len()], vec![0u32; audio.samples.len()]];
        order.iter().fold(base, |position, (track, index, sample)| {
            offsets[*track][*index] = position;
            position + sample.data.len() as u32
        });
        let duration = layouts
            .iter()
            .map(|layout| total(layout.samples, layout.timescale) * u64::from(MOVIE_TIMESCALE) / u64::from(layout.timescale))
            .max()
            .unwrap_or(0);
        let tracks: Vec<Vec<u8>> = layouts
            .iter()
            .zip(offsets.iter())
            .filter(|(layout, _)| !layout.samples.is_empty())
            .map(|(layout, offsets)| trak(layout, offsets))
            .collect();
        container(b"moov", &[vec![mvhd(duration)], tracks].concat())
    };
    let header = ftyp.len() as u32 + moov(0).len() as u32 + 8;
    let payload: Vec<u8> = order.iter().flat_map(|(_, _, sample)| sample.data.iter().copied()).collect();
    [ftyp, moov(header), boxed(b"mdat", &payload)].concat()
}
