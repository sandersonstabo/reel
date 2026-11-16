#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Ring,
    Disc,
}

pub const GRAY: (u8, u8, u8) = (140, 140, 140);
pub const RED: (u8, u8, u8) = (232, 32, 40);

fn coverage(distance: f32, radius: f32) -> f32 {
    (radius - distance + 0.5).clamp(0.0, 1.0)
}

fn alpha(size: u32, shape: Shape, x: u32, y: u32) -> f32 {
    let center = size as f32 / 2.0;
    let distance = ((x as f32 + 0.5 - center).powi(2) + (y as f32 + 0.5 - center).powi(2)).sqrt();
    let outer = coverage(distance, size as f32 * 0.42);
    match shape {
        Shape::Disc => outer,
        Shape::Ring => outer - coverage(distance, size as f32 * 0.27),
    }
}

pub fn pixels(size: u32, shape: Shape, (red, green, blue): (u8, u8, u8)) -> Vec<u32> {
    (0..size * size)
        .map(|index| {
            let a = (alpha(size, shape, index % size, index / size) * 255.0).round() as u32;
            (a << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
        })
        .collect()
}
