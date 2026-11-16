use windows::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject, HGDIOBJ,
    },
    UI::WindowsAndMessaging::{
        CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
    },
};

#[derive(Clone, Copy)]
enum Shape {
    Ring,
    Disc,
}

pub struct Icons {
    pub off: HICON,
    pub watching: HICON,
    pub recording: HICON,
}

const GRAY: (u8, u8, u8) = (140, 140, 140);
const RED: (u8, u8, u8) = (232, 32, 40);

fn coverage(distance: f32, radius: f32) -> f32 {
    (radius - distance + 0.5).clamp(0.0, 1.0)
}

fn alpha(size: i32, shape: Shape, x: i32, y: i32) -> f32 {
    let center = size as f32 / 2.0;
    let distance = ((x as f32 + 0.5 - center).powi(2) + (y as f32 + 0.5 - center).powi(2)).sqrt();
    let outer = coverage(distance, size as f32 * 0.42);
    match shape {
        Shape::Disc => outer,
        Shape::Ring => outer - coverage(distance, size as f32 * 0.27),
    }
}

fn pixels(size: i32, shape: Shape, (red, green, blue): (u8, u8, u8)) -> Vec<u32> {
    (0..size * size)
        .map(|index| {
            let a = (alpha(size, shape, index % size, index / size) * 255.0).round() as u32;
            (a << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
        })
        .collect()
}

fn create(size: i32, shape: Shape, color: (u8, u8, u8)) -> HICON {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let Ok(color_bitmap) =
        (unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) })
    else {
        return HICON::default();
    };
    let source = pixels(size, shape, color);
    unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), bits as *mut u32, source.len()) };
    let mask = unsafe { CreateBitmap(size, size, 1, 1, None) };
    let icon = unsafe {
        CreateIconIndirect(&ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color_bitmap,
        })
    }
    .unwrap_or_default();
    unsafe {
        let _ = DeleteObject(HGDIOBJ(color_bitmap.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
    }
    icon
}

impl Icons {
    pub fn new() -> Self {
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
        Self {
            off: create(size, Shape::Ring, GRAY),
            watching: create(size, Shape::Disc, GRAY),
            recording: create(size, Shape::Disc, RED),
        }
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.off);
            let _ = DestroyIcon(self.watching);
            let _ = DestroyIcon(self.recording);
        }
    }
}
