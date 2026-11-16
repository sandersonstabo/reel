use reel::record::layout::{Rect, fit};

#[test]
fn same_aspect_fills_the_frame() {
    assert_eq!(fit((2560, 1440), (1920, 1080)), Rect { left: 0, top: 0, right: 1920, bottom: 1080 });
}

#[test]
fn ultrawide_is_letterboxed() {
    assert_eq!(fit((3440, 1440), (1920, 1080)), Rect { left: 0, top: 138, right: 1920, bottom: 940 });
}

#[test]
fn stretched_four_by_three_is_pillarboxed() {
    assert_eq!(fit((1440, 1080), (1920, 1080)), Rect { left: 240, top: 0, right: 1680, bottom: 1080 });
}

#[test]
fn degenerate_source_does_not_divide_by_zero() {
    let rect = fit((0, 0), (1920, 1080));
    assert!(rect.right <= 1920 && rect.bottom <= 1080);
}
