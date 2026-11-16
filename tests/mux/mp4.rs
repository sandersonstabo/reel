#[path = "../support/boxes.rs"]
mod boxes;

use reel::mux::mp4::{AudioTrack, Sample, VideoTrack, durations, mux};

const SECOND: i64 = 10_000_000;

fn sample(time: i64, duration: i64, keyframe: bool, fill: u8, size: usize) -> Sample {
    Sample {
        time,
        duration,
        keyframe,
        data: vec![fill; size],
    }
}

fn video(samples: Vec<Sample>) -> VideoTrack {
    VideoTrack {
        width: 1280,
        height: 720,
        entry: *b"avc1",
        config: vec![0, 0, 0, 8, b'a', b'v', b'c', b'C'],
        samples,
    }
}

fn audio(samples: Vec<Sample>) -> AudioTrack {
    AudioTrack {
        rate: 48_000,
        channels: 2,
        config: [0x11, 0x90],
        bitrate: 128_000,
        samples,
    }
}

fn frames(count: i64) -> Vec<Sample> {
    (0..count)
        .map(|n| sample(n * SECOND / 60, SECOND / 60, n % 60 == 0, n as u8, 100 + n as usize))
        .collect()
}

fn aac(count: i64) -> Vec<Sample> {
    (0..count)
        .map(|n| sample(n * 1024 * SECOND / 48_000, 1024 * SECOND / 48_000, true, 200, 10))
        .collect()
}

#[test]
fn file_starts_with_ftyp_then_moov_then_mdat() {
    let file = mux(&video(frames(10)), &audio(aac(5)));
    let order: Vec<[u8; 4]> = boxes::children(&file).iter().map(|(kind, _, _)| *kind).collect();
    assert_eq!(order, vec![*b"ftyp", *b"moov", *b"mdat"]);
}

#[test]
fn chunk_offsets_point_at_each_sample() {
    let video_samples = frames(120);
    let audio_samples = aac(50);
    let file = mux(&video(video_samples.clone()), &audio(audio_samples.clone()));
    let tracks = boxes::find(&file, &[b"moov", b"trak"]);
    assert_eq!(tracks.len(), 2);
    [video_samples, audio_samples].iter().zip(tracks).for_each(|(samples, track)| {
        let stco = boxes::find(track, &[b"mdia", b"minf", b"stbl", b"stco"])[0];
        boxes::table(stco, 1).iter().zip(samples).for_each(|(row, sample)| {
            let offset = row[0] as usize;
            assert_eq!(&file[offset..offset + sample.data.len()], sample.data.as_slice());
        });
    });
}

#[test]
fn constant_frame_rate_collapses_into_one_time_to_sample_run() {
    let file = mux(&video(frames(120)), &audio(aac(5)));
    let track = boxes::find(&file, &[b"moov", b"trak"])[0];
    let stts = boxes::find(track, &[b"mdia", b"minf", b"stbl", b"stts"])[0];
    assert_eq!(boxes::table(stts, 2), vec![vec![120, 1500]]);
}

#[test]
fn sync_samples_list_the_keyframes() {
    let file = mux(&video(frames(150)), &audio(aac(5)));
    let track = boxes::find(&file, &[b"moov", b"trak"])[0];
    let stss = boxes::find(track, &[b"mdia", b"minf", b"stbl", b"stss"])[0];
    assert_eq!(boxes::table(stss, 1), vec![vec![1], vec![61], vec![121]]);
}

#[test]
fn variable_timing_keeps_total_duration() {
    let samples = vec![
        sample(0, SECOND / 60, true, 0, 1),
        sample(SECOND / 60, SECOND / 60, false, 0, 1),
        sample(SECOND / 20, SECOND / 60, false, 0, 1),
    ];
    let deltas = durations(&samples, 90_000);
    assert_eq!(deltas, vec![1500, 3000, 1500]);
}

#[test]
fn track_without_audio_is_omitted() {
    let file = mux(&video(frames(10)), &audio(vec![]));
    assert_eq!(boxes::find(&file, &[b"moov", b"trak"]).len(), 1);
}
