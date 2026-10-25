//! The unread count as an overlay on the taskbar button: a red disc with
//! the number in white, drawn here pixel by pixel so no font or resource
//! is needed.

use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, GetDC,
    HBITMAP, ReleaseDC,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, EnumWindows, GW_OWNER, GetSystemMetrics, GetWindow, GetWindowThreadProcessId,
    HICON, ICONINFO, IsWindowVisible, SM_CXSMICON,
};
use windows_core::{BOOL, HSTRING};

use super::super::badge_text;

/// Discord's red for unread badges.
const RED: [u8; 3] = [0xf2, 0x3f, 0x43];

#[derive(Default)]
pub struct Badge {
    taskbar: Option<ITaskbarList3>,
    /// The icon on the button now, destroyed when replaced.
    icon: Option<HICON>,
    shown: Option<String>,
}

impl Badge {
    pub fn set(&mut self, count: u32) {
        let text = badge_text(count);
        if text == self.shown {
            return;
        }
        let Some(window) = main_window() else { return };
        let Some(taskbar) = self.taskbar() else { return };
        let icon = text.as_deref().and_then(|text| draw(text).map_err(|e| tracing::warn!("badge: {e}")).ok());
        let description = HSTRING::from(text.as_deref().unwrap_or_default());
        // SAFETY: a live window of this process and an icon or none.
        if let Err(e) = unsafe { taskbar.SetOverlayIcon(window, icon.unwrap_or_default(), &description) } {
            tracing::warn!("badge: {e}");
            if let Some(icon) = icon {
                // SAFETY: created above and not handed over.
                unsafe { DestroyIcon(icon).ok() };
            }
            return;
        }
        if let Some(old) = std::mem::replace(&mut self.icon, icon) {
            // SAFETY: the taskbar keeps its own copy of the old overlay.
            unsafe { DestroyIcon(old).ok() };
        }
        self.shown = text;
    }

    fn taskbar(&mut self) -> Option<&ITaskbarList3> {
        if self.taskbar.is_none() {
            // gpui's thread is already in an apartment; this only makes sure.
            // SAFETY: plain COM setup, balanced by the process's end.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().ok() };
            // SAFETY: the shell's own taskbar object.
            let taskbar: windows_core::Result<ITaskbarList3> =
                unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) };
            match taskbar.and_then(|t| unsafe { t.HrInit() }.map(|()| t)) {
                Ok(taskbar) => self.taskbar = Some(taskbar),
                Err(e) => tracing::warn!("badge: no taskbar: {e}"),
            }
        }
        self.taskbar.as_ref()
    }
}

/// This process's first visible top-level window that no other owns: the
/// one with the taskbar button.
fn main_window() -> Option<HWND> {
    unsafe extern "system" fn visit(window: HWND, found: LPARAM) -> BOOL {
        let mut pid = 0;
        // SAFETY: Windows hands a valid window; `found` points at the
        // caller's local, alive for the whole enumeration.
        unsafe {
            GetWindowThreadProcessId(window, Some(&mut pid));
            let owned = GetWindow(window, GW_OWNER).is_ok_and(|owner| !owner.is_invalid());
            if pid == GetCurrentProcessId() && IsWindowVisible(window).as_bool() && !owned {
                *(found.0 as *mut HWND) = window;
                return false.into();
            }
        }
        TRUE
    }
    let mut found = HWND::default();
    // SAFETY: `visit` only writes through the pointer to `found`. The
    // enumeration reports an error when the callback stops it early.
    unsafe { EnumWindows(Some(visit), LPARAM(&mut found as *mut HWND as isize)).ok() };
    (!found.is_invalid()).then_some(found)
}

/// The overlay icon for `text`.
fn draw(text: &str) -> windows_core::Result<HICON> {
    // SAFETY: a plain metric query.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as usize;
    let pixels = render(text, size);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            // Negative: rows run top to bottom, as `render` writes them.
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    // SAFETY: the screen DC is released at once; `bits` receives the
    // section's memory, `size * size` BGRA pixels, which is filled below.
    let color: HBITMAP = unsafe {
        let screen = GetDC(None);
        let color = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, screen);
        color?
    };
    // SAFETY: as above, the section holds exactly `pixels.len()` bytes.
    unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len()) };
    // SAFETY: a one-bit mask the alpha channel overrides.
    let mask = unsafe { CreateBitmap(size as i32, size as i32, 1, 1, None) };
    let icon = ICONINFO {
        fIcon: TRUE,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: color,
    };
    // SAFETY: both bitmaps are valid; the icon copies them, so they are
    // freed right after.
    let made = unsafe { CreateIconIndirect(&icon) };
    unsafe {
        DeleteObject(color.into()).ok().ok();
        DeleteObject(mask.into()).ok().ok();
    }
    made
}

/// Glyphs three pixels wide and five high, one row per byte's low bits.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        _ => [0; 5],
    }
}

/// `size` by `size` BGRA pixels, straight alpha: the disc, smoothed at its
/// edge, with `text` centered in white.
fn render(text: &str, size: usize) -> Vec<u8> {
    let mut pixels = vec![0u8; size * size * 4];
    let radius = size as f32 / 2.;
    // Four by four samples per pixel for a smooth edge.
    for y in 0..size {
        for x in 0..size {
            let mut inside = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let dx = x as f32 + (sx as f32 + 0.5) / 4. - radius;
                    let dy = y as f32 + (sy as f32 + 0.5) / 4. - radius;
                    if dx * dx + dy * dy <= radius * radius {
                        inside += 1;
                    }
                }
            }
            let at = (y * size + x) * 4;
            pixels[at..at + 4].copy_from_slice(&[RED[2], RED[1], RED[0], (inside * 255 / 16) as u8]);
        }
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return pixels;
    }
    let columns = chars.len() * 4 - 1;
    let scale = ((size * 8 / 10) / columns).min(size * 65 / 100 / 5).max(1);
    let left = size.saturating_sub(columns * scale) / 2;
    let top = size.saturating_sub(5 * scale) / 2;
    for (ix, c) in chars.iter().enumerate() {
        for (row, bits) in glyph(*c).iter().enumerate() {
            for column in 0..3 {
                if bits & (0b100 >> column) == 0 {
                    continue;
                }
                for y in 0..scale {
                    for x in 0..scale {
                        let px = left + (ix * 4 + column) * scale + x;
                        let py = top + row * scale + y;
                        if px < size && py < size {
                            let at = (py * size + px) * 4;
                            pixels[at..at + 4].copy_from_slice(&[255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_sits_white_on_a_red_disc() {
        let size = 16;
        let pixels = render("7", size);
        let at = |x: usize, y: usize| &pixels[(y * size + x) * 4..(y * size + x) * 4 + 4];
        // Corners stay clear, the edge is red.
        assert_eq!(at(0, 0)[3], 0);
        assert_eq!(at(8, 1), [RED[2], RED[1], RED[0], 255]);
        // Some white is drawn, all of it inside the disc.
        let white = (0..size * size).filter(|ix| pixels[ix * 4..ix * 4 + 4] == [255; 4]).count();
        assert!(white > 0);
        assert_eq!(render("", size).len(), size * size * 4);
    }
}
