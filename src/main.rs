use anyhow::anyhow;
use anyhow::Result;
use windows::core::HRESULT;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsIconic,
    IsWindowVisible, IsZoomed, SetWindowPos, ShowWindow, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE,
    SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE,
};

unsafe fn get_screen_size() -> (i32, i32) {
    (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
}

fn wide_string_to_string(wide_string: &[u16]) -> Result<String> {
    let string = if let Some(null_pos) = wide_string.iter().position(|pos| *pos == 0) {
        String::from_utf16(&wide_string[..null_pos])?
    } else {
        String::from_utf16(wide_string)?
    };

    Ok(string)
}

fn get_window_text(hwnd: HWND) -> Result<String> {
    let text_length = unsafe { GetWindowTextLengthW(hwnd) };
    let mut wide_buffer = vec![0u16; (text_length + 1) as usize];
    unsafe { GetWindowTextW(hwnd, &mut wide_buffer) };
    wide_string_to_string(&wide_buffer)
        .map_err(|e| anyhow!("Failed to convert wide string to string: {:?}", e))
}

/// Get the display percent of a rect on the screen
fn get_display_percent(rect: RECT, width: i32, height: i32) -> f32 {
    let x_min = rect.left.max(0);
    let y_min = rect.top.max(0);
    let x_max = rect.right.min(width);
    let y_max = rect.bottom.min(height);
    if x_min >= x_max || y_min >= y_max {
        return 0.0;
    }

    let display_width = (x_max - x_min) as f32;
    let display_height = (y_max - y_min) as f32;

    let original_width = (rect.right - rect.left) as f32;
    let original_height = (rect.bottom - rect.top) as f32;

    if original_height <= 0.0 || original_width <= 0.0 {
        return 0.0;
    }

    (display_width * display_height) / (original_width * original_height)
}

trait RectCalc {
    fn left_top(&self) -> bool;
}

impl RectCalc for RECT {
    fn left_top(&self) -> bool {
        (self.left >= 0 && self.left <= TOP_LEFT_BOUND)
            && (self.top >= 0 && self.top <= TOP_LEFT_BOUND)
    }
}

unsafe extern "system" fn enum_window_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let is_visible = IsWindowVisible(hwnd).as_bool();
    if !is_visible {
        return BOOL(1);
    }

    let is_minimize = IsIconic(hwnd).as_bool();
    if is_minimize {
        return BOOL(1);
    }

    let window_text = match get_window_text(hwnd) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Get window text failed for {:?}: {:?}", hwnd, e);
            return BOOL(0);
        }
    };

    if window_text.is_empty() {
        return BOOL(1);
    }

    let mut rect = RECT::default();
    match GetWindowRect(hwnd, &mut rect) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("GetWindowRect failed for {:?}: {:?}", hwnd, e);
            return BOOL(0);
        }
    }

    if rect.left_top() {
        return BOOL(1);
    }

    let (width, height) = get_screen_size();
    let display_percent = get_display_percent(rect, width, height);
    if display_percent > 0.5 {
        return BOOL(1);
    }

    let is_maximize = IsZoomed(hwnd).as_bool();
    if is_maximize {
        match ShowWindow(hwnd, SW_RESTORE).ok() {
            Ok(_) => (),
            Err(e) => {
                eprintln!("ShowWindow failed for {:?}: {:?}", hwnd, e);
                return BOOL(0);
            }
        }
    }

    println!(
        "Title: {window_text:?} Percent: {:.2}% {hwnd:?} {rect:?}",
        display_percent * 100.0
    );

    match SetWindowPos(
        hwnd,
        None,
        0,
        0,
        0,
        0,
        SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE,
    ) {
        Ok(_) => BOOL(1),
        Err(e) => {
            eprintln!("SetWindowPos failed for {:?}: {:?}", hwnd, e);
            return BOOL(0);
        }
    }
}

const E_ACCESS_DENIED: HRESULT = HRESULT::from_win32(0x80070005);
const TOP_LEFT_BOUND: i32 = 100;

fn main() {
    match unsafe { EnumWindows(Some(enum_window_callback), LPARAM(0)) } {
        Ok(_) => (),
        Err(e) => match e.code() {
            E_ACCESS_DENIED => eprintln!("Tip: Try running as administrator."),
            _ => (),
        },
    }
}
