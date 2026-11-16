use reel::record::pacing::Pacer;

const SECOND: i64 = 10_000_000;

#[test]
fn keeps_one_frame_per_slot_from_a_faster_source() {
    let mut pacer = Pacer::new(SECOND, 60);
    let step = SECOND / 240;
    let admitted: Vec<i64> = (0..24).filter_map(|n| pacer.admit(n * step)).collect();
    assert_eq!(admitted.len(), 6);
    assert!(admitted.windows(2).all(|pair| pair[1] - pair[0] == pacer.interval()));
}

#[test]
fn snaps_timestamps_to_the_slot_start() {
    let mut pacer = Pacer::new(SECOND, 60);
    assert_eq!(pacer.admit(pacer.interval() * 3 + 10), Some(pacer.interval() * 3));
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
    assert_eq!(pacer.admit(SECOND), Some(SECOND / 60 * 60));
    assert_eq!(pacer.admit(SECOND + 1), None);
}
