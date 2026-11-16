use std::sync::Arc;

use reel::record::ring::{Packet, Ring};

const SECOND: i64 = 10_000_000;
const FRAME: i64 = SECOND / 10;

fn packet(time: i64, keyframe: bool) -> Packet {
    Packet {
        time,
        duration: FRAME,
        keyframe,
        data: Arc::from(vec![time as u8]),
    }
}

fn filled(span: i64, seconds: i64) -> Ring {
    let mut ring = Ring::new(span);
    (0..seconds * 10).for_each(|n| {
        ring.push_video(packet(n * FRAME, n % 10 == 0));
        ring.push_audio(packet(n * FRAME + FRAME / 2, true));
    });
    ring
}

#[test]
fn clip_starts_on_a_keyframe_at_zero() {
    let clip = filled(5 * SECOND, 20).clip(3 * SECOND).unwrap();
    assert!(clip.video[0].keyframe);
    assert_eq!(clip.video[0].time, 0);
}

#[test]
fn clip_covers_at_least_the_requested_length() {
    let clip = filled(5 * SECOND, 20).clip(3 * SECOND).unwrap();
    let last = clip.video.last().unwrap();
    assert!(last.time >= 3 * SECOND);
    assert!(last.time < 4 * SECOND + FRAME);
}

#[test]
fn buffer_forgets_video_older_than_its_span() {
    let clip = filled(5 * SECOND, 60).clip(60 * SECOND).unwrap();
    let last = clip.video.last().unwrap();
    assert!(last.time < 6 * SECOND + FRAME, "kept {} ticks", last.time);
}

#[test]
fn audio_is_trimmed_to_the_video_window_and_rebased() {
    let clip = filled(5 * SECOND, 20).clip(3 * SECOND).unwrap();
    assert!(clip.audio.iter().all(|packet| packet.time >= 0));
    assert_eq!(clip.audio[0].time, FRAME / 2);
}

#[test]
fn short_history_clips_from_the_first_keyframe() {
    let clip = filled(30 * SECOND, 2).clip(30 * SECOND).unwrap();
    assert_eq!(clip.video.len(), 20);
}

#[test]
fn no_keyframe_means_no_clip() {
    let mut ring = Ring::new(SECOND);
    ring.push_video(packet(0, false));
    assert!(ring.clip(SECOND).is_none());
}
