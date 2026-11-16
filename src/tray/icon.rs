use windows::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
        DeleteObject, HGDIOBJ,
    },
    UI::WindowsAndMessaging::{
        CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
    },
};

use super::glyph::{GRAY, RED, Shape, pixels};

pub struct Icons {
    pub off: HICON,
    pub watching: HICON,
    pub recording: HICON,
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
    let source = pixels(size as u32, shape, color);
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
