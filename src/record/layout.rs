#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

fn even(value: u64) -> i32 {
    (value & !1) as i32
}

pub fn fit(source: (u32, u32), target: (u32, u32)) -> Rect {
    let (sw, sh) = (u64::from(source.0.max(1)), u64::from(source.1.max(1)));
    let (tw, th) = (u64::from(target.0), u64::from(target.1));
    let (w, h) = match sw * th >= tw * sh {
        true => (tw, tw * sh / sw),
        false => (th * sw / sh, th),
    };
    let left = even((tw - w) / 2);
    let top = even((th - h) / 2);
    Rect {
        left,
        top,
        right: left + even(w),
        bottom: top + even(h),
    }
}
