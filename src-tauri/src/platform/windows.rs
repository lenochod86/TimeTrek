//! Windows backend built on Win32 + UI Automation.

use super::{PlatformApi, WindowInfo};
use image::RgbaImage;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use windows::core::{BSTR, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, HGDIOBJ, SRCCOPY,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, FILE_ATTRIBUTE_NORMAL,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationValuePattern,
    TreeScope_Descendants, UIA_ClassNamePropertyId, UIA_ValuePatternId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, DrawIconEx, GetForegroundWindow, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, DI_NORMAL, HICON,
};

pub struct WinPlatform;

pub fn create() -> Box<dyn PlatformApi> {
    Box::new(WinPlatform)
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn hwnd(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

impl PlatformApi for WinPlatform {
    fn foreground_window(&self) -> Option<WindowInfo> {
        unsafe {
            let h = GetForegroundWindow();
            if h.is_invalid() {
                return None;
            }
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(h, &mut buf);
            let title = String::from_utf16_lossy(&buf[..len.max(0) as usize]);

            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let exe = process_exe(pid).unwrap_or_default();
            Some(WindowInfo {
                handle: h.0 as isize,
                pid,
                exe,
                title,
            })
        }
    }

    fn idle_seconds(&self) -> u64 {
        unsafe {
            let mut lii = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut lii).as_bool() {
                (GetTickCount().wrapping_sub(lii.dwTime) / 1000) as u64
            } else {
                0
            }
        }
    }

    fn app_name(&self, exe: &str) -> String {
        file_description(exe)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                std::path::Path::new(exe)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Neznámá aplikace".to_string())
            })
    }

    fn app_icon_png(&self, exe: &str) -> Option<Vec<u8>> {
        if exe.is_empty() {
            return None;
        }
        unsafe {
            let wide = to_wide(exe);
            let mut info = SHFILEINFOW::default();
            let r = SHGetFileInfoW(
                PCWSTR(wide.as_ptr()),
                FILE_ATTRIBUTE_NORMAL,
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if r == 0 || info.hIcon.is_invalid() {
                return None;
            }
            let rgba = hicon_to_rgba(info.hIcon, 32);
            let _ = DestroyIcon(info.hIcon);
            let rgba = rgba?;
            let img = RgbaImage::from_raw(32, 32, rgba)?;
            let mut out = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .ok()?;
            Some(out)
        }
    }

    fn browser_address(&self, window: &WindowInfo) -> Option<String> {
        if !is_chromium_browser(&window.exe) {
            return None;
        }
        UIA.with(|cell| {
            let mut guard = cell.borrow_mut();
            if guard.is_none() {
                *guard = UiaState::new();
            }
            guard.as_mut()?.address_of(window.handle)
        })
    }

    fn capture_window(&self, handle: isize) -> Option<RgbaImage> {
        unsafe { capture(hwnd(handle)) }
    }
}

fn is_chromium_browser(exe: &str) -> bool {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
    matches!(
        name.as_str(),
        "chrome.exe" | "msedge.exe" | "brave.exe" | "vivaldi.exe" | "opera.exe"
    )
}

unsafe fn process_exe(pid: u32) -> Option<String> {
    let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let res = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size);
    let _ = CloseHandle(h);
    res.ok()?;
    Some(String::from_utf16_lossy(&buf[..size as usize]))
}

fn file_description(exe: &str) -> Option<String> {
    unsafe {
        let wide = to_wide(exe);
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()), 0, size, data.as_mut_ptr() as *mut c_void)
            .ok()?;

        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let tr = to_wide("\\VarFileInfo\\Translation");
        let mut candidates: Vec<String> = Vec::new();
        if VerQueryValueW(data.as_ptr() as *const c_void, PCWSTR(tr.as_ptr()), &mut ptr, &mut len)
            .as_bool()
            && len >= 4
        {
            let p = ptr as *const u16;
            candidates.push(format!("{:04x}{:04x}", *p, *p.add(1)));
        }
        candidates.extend(["040904b0".to_string(), "040904e4".to_string(), "000004b0".to_string()]);

        for c in candidates {
            let key = to_wide(&format!("\\StringFileInfo\\{c}\\FileDescription"));
            let mut ptr: *mut c_void = std::ptr::null_mut();
            let mut len = 0u32;
            if VerQueryValueW(data.as_ptr() as *const c_void, PCWSTR(key.as_ptr()), &mut ptr, &mut len)
                .as_bool()
                && len > 1
            {
                let slice = std::slice::from_raw_parts(ptr as *const u16, (len - 1) as usize);
                return Some(String::from_utf16_lossy(slice));
            }
        }
        None
    }
}

/// Renders an icon to a `size`x`size` RGBA buffer.
unsafe fn hicon_to_rgba(icon: HICON, size: i32) -> Option<Vec<u8>> {
    let screen = GetDC(None);
    let dc = CreateCompatibleDC(screen);
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bmp = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
    let result = match bmp {
        Ok(bmp) if !bits.is_null() => {
            let old = SelectObject(dc, HGDIOBJ(bmp.0));
            let _ = DrawIconEx(dc, 0, 0, icon, size, size, 0, None, DI_NORMAL);
            let src = std::slice::from_raw_parts(bits as *const u8, (size * size * 4) as usize);
            let mut rgba = Vec::with_capacity(src.len());
            let mut any_alpha = false;
            for px in src.chunks_exact(4) {
                if px[3] != 0 {
                    any_alpha = true;
                }
                rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            if !any_alpha {
                // Legacy icon without alpha channel: treat drawn pixels as opaque.
                for px in rgba.chunks_exact_mut(4) {
                    if px[0] != 0 || px[1] != 0 || px[2] != 0 {
                        px[3] = 255;
                    }
                }
            }
            SelectObject(dc, old);
            let _ = DeleteObject(HGDIOBJ(bmp.0));
            Some(rgba)
        }
        _ => None,
    };
    let _ = DeleteDC(dc);
    ReleaseDC(None, screen);
    result
}

unsafe fn capture(h: HWND) -> Option<RgbaImage> {
    if IsIconic(h).as_bool() {
        return None;
    }
    let mut rc = RECT::default();
    GetWindowRect(h, &mut rc).ok()?;
    let (w, ht) = (rc.right - rc.left, rc.bottom - rc.top);
    if w < 50 || ht < 50 || w > 10000 || ht > 10000 {
        return None;
    }

    let screen = GetDC(None);
    let dc = CreateCompatibleDC(screen);
    let bmp: HBITMAP = CreateCompatibleBitmap(screen, w, ht);
    let old = SelectObject(dc, HGDIOBJ(bmp.0));

    // PW_RENDERFULLCONTENT (0x2) also captures GPU rendered windows (Chromium, WPF...).
    let printed = PrintWindow(h, dc, PRINT_WINDOW_FLAGS(2)).as_bool();
    let mut img = read_bitmap(dc, bmp, w, ht);
    if !printed || img.as_ref().map_or(true, is_blank) {
        // Fallback: copy what is visible on screen (works for the foreground window).
        let _ = BitBlt(dc, 0, 0, w, ht, screen, rc.left, rc.top, SRCCOPY);
        img = read_bitmap(dc, bmp, w, ht);
    }

    SelectObject(dc, old);
    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(dc);
    ReleaseDC(None, screen);
    img
}

unsafe fn read_bitmap(dc: windows::Win32::Graphics::Gdi::HDC, bmp: HBITMAP, w: i32, h: i32) -> Option<RgbaImage> {
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let lines = GetDIBits(dc, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut c_void), &mut bmi, DIB_RGB_COLORS);
    if lines == 0 {
        return None;
    }
    for px in buf.chunks_exact_mut(4) {
        px.swap(0, 2); // BGRA -> RGBA
        px[3] = 255;
    }
    RgbaImage::from_raw(w as u32, h as u32, buf)
}

fn is_blank(img: &RgbaImage) -> bool {
    // Sample ~1000 pixels; entirely black means PrintWindow produced nothing.
    let step = (img.as_raw().len() / 4 / 1000).max(1);
    img.pixels()
        .step_by(step)
        .all(|p| p[0] < 3 && p[1] < 3 && p[2] < 3)
}

// ---------------------------------------------------------------------------
// UI Automation: read the address bar of Chromium based browsers.
// Chromium's omnibox always has the UIA class name "OmniboxViewViews",
// independent of the UI language.
// ---------------------------------------------------------------------------

thread_local! {
    static UIA: RefCell<Option<UiaState>> = const { RefCell::new(None) };
}

struct UiaState {
    automation: IUIAutomation,
    omnibox: HashMap<isize, IUIAutomationElement>,
}

impl UiaState {
    fn new() -> Option<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
            Some(Self {
                automation,
                omnibox: HashMap::new(),
            })
        }
    }

    fn address_of(&mut self, handle: isize) -> Option<String> {
        if let Some(el) = self.omnibox.get(&handle) {
            if let Some(v) = read_value(el) {
                return Some(v);
            }
            self.omnibox.remove(&handle);
        }
        let el = self.find_omnibox(handle)?;
        let v = read_value(&el);
        if v.is_some() {
            if self.omnibox.len() > 32 {
                self.omnibox.clear();
            }
            self.omnibox.insert(handle, el);
        }
        v
    }

    fn find_omnibox(&self, handle: isize) -> Option<IUIAutomationElement> {
        unsafe {
            let root = self.automation.ElementFromHandle(hwnd(handle)).ok()?;
            let cond = self
                .automation
                .CreatePropertyCondition(UIA_ClassNamePropertyId, &BSTR::from("OmniboxViewViews").into())
                .ok()?;
            root.FindFirst(TreeScope_Descendants, &cond).ok()
        }
    }
}

fn read_value(el: &IUIAutomationElement) -> Option<String> {
    unsafe {
        let pattern: IUIAutomationValuePattern = el.GetCurrentPatternAs(UIA_ValuePatternId).ok()?;
        let value = pattern.CurrentValue().ok()?;
        let s = value.to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}
