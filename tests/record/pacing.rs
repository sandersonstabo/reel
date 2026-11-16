use reel::record::pacing::Pacer;

const SECOND: i64 = 10_000_000;

fn admitted(pacer: &mut Pacer, times: impl Iterator<Item = i64>) -> Vec<(i64, i64)> {
    times.filter_map(|time| pacer.admit(time).map(|stamp| (time, stamp))).collect()
}

#[test]
fn faster_source_is_thinned_to_the_target_rate() {
    let mut pacer = Pacer::new(SECOND, 60);
    let kept = admitted(&mut pacer, (0..240).map(|n| n * SECOND / 240));
    assert!((60..=61).contains(&kept.len()), "kept {}", kept.len());
    assert!(kept.windows(2).all(|pair| pair[1].1 - pair[0].1 == pacer.interval()));
}

#[test]
fn jittery_source_at_the_target_rate_keeps_every_frame() {
    let mut pacer = Pacer::new(SECOND, 60);
    let interval = pacer.interval();
    let jitter = |n: i64| match n % 2 {
        0 => interval * 3 / 10,
        _ => -interval * 3 / 10,
    };
    let kept = admitted(&mut pacer, (1..=120).map(|n| n * interval + jitter(n)));
    assert_eq!(kept.len(), 120);
}

#[test]
fn stamps_stay_within_half_a_frame_of_capture_time() {
    let mut pacer = Pacer::new(SECOND, 60);
    let half = pacer.interval() / 2;
    let kept = admitted(&mut pacer, (0..1000).map(|n| n * SECOND / 144));
    assert!(kept.iter().all(|(time, stamp)| (stamp - time).abs() <= half));
}

#[test]
fn drops_frames_before_the_recording_started() {
    let mut pacer = Pacer::new(SECOND, 60);
    assert_eq!(pacer.admit(-1), None);
    assert_eq!(pacer.admit(0), Some(0));
}

#[test]
fn resumes_after_a_gap_without_backfilling() {
    let mut pacer = Pacer::new(SECOND, 60);
    pacer.admit(0);
    assert_eq!(pacer.admit(SECOND), Some(pacer.interval() * 60));
    assert_eq!(pacer.admit(SECOND + 1), None);
}
