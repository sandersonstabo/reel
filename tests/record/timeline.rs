use reel::record::timeline::{Alignment, Timeline};

const SECOND: i64 = 10_000_000;

fn timeline() -> Timeline {
    Timeline::new(48_000, SECOND, SECOND / 50)
}

#[test]
fn packet_close_to_the_write_position_is_contiguous() {
    let mut clock = timeline();
    clock.advance(48_000);
    assert_eq!(clock.align(SECOND + SECOND / 100), Alignment::Contiguous);
}

#[test]
fn late_packet_is_preceded_by_silence() {
    let clock = timeline();
    assert_eq!(clock.align(SECOND / 2), Alignment::Silence(24_000));
}

#[test]
fn early_packet_has_its_overlap_skipped() {
    let mut clock = timeline();
    clock.advance(48_000);
    assert_eq!(clock.align(SECOND / 2), Alignment::Skip(24_000));
}

#[test]
fn idle_fill_reaches_the_requested_time() {
    let mut clock = timeline();
    let gap = clock.silence_until(SECOND * 2);
    clock.advance(gap);
    assert_eq!(clock.position(), SECOND * 2);
    assert_eq!(clock.silence_until(SECOND * 2), 0);
}

#[test]
fn durations_are_exact_at_48k() {
    assert_eq!(timeline().duration(480), SECOND / 100);
}
