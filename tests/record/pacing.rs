use reel::record::pacing::Pacer;

const SECOND: i64 = 10_000_000;

fn kept(pacer: &mut Pacer, times: impl Iterator<Item = i64>) -> Vec<i64> {
    times.filter(|time| pacer.admit(*time)).collect()
}

#[test]
fn faster_sources_are_thinned_to_the_target_rate() {
    [144, 165, 240, 360].into_iter().for_each(|rate| {
        let mut pacer = Pacer::new(SECOND, 60);
        let count = kept(&mut pacer, (0..rate * 10).map(|n| n * SECOND / rate)).len();
        assert!((599..=601).contains(&count), "{rate} Hz kept {count} frames in 10 s");
    });
}

#[test]
fn jittery_source_at_the_target_rate_keeps_every_frame() {
    let mut pacer = Pacer::new(SECOND, 60);
    let interval = pacer.interval();
    let jitter = |n: i64| match n % 2 {
        0 => interval * 3 / 10,
        _ => -interval * 3 / 10,
    };
    assert_eq!(kept(&mut pacer, (1..=600).map(|n| n * interval + jitter(n))).len(), 600);
}

#[test]
fn late_frame_followed_by_an_on_time_frame_keeps_both() {
    let mut pacer = Pacer::new(SECOND, 60);
    let interval = pacer.interval();
    let times = [0, interval, interval * 2 + interval * 6 / 10, interval * 3, interval * 4];
    assert_eq!(kept(&mut pacer, times.into_iter()).len(), 5);
}

#[test]
fn never_bursts_after_a_pause() {
    let mut pacer = Pacer::new(SECOND, 60);
    pacer.admit(0);
    let after_pause = kept(&mut pacer, (0..240).map(|n| SECOND * 5 + n * SECOND / 240));
    assert!((60..=61).contains(&after_pause.len()));
}

#[test]
fn drops_frames_before_the_recording_started() {
    let mut pacer = Pacer::new(SECOND, 60);
    assert!(!pacer.admit(-1));
    assert!(pacer.admit(0));
}
