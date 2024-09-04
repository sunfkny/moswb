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

fn wide_string_to_string(wide_string: &[u16]) -> Result<String, std::string::FromUtf16Error> {
    let string = if let Some(null_pos) = wide_string.iter().position(|pos| *pos == 0) {
        String::from_utf16(&wide_string[..null_pos])?
    } else {
        String::from_utf16(wide_string)?
    };

    Ok(string)
}

unsafe fn get_window_text(hwnd: HWND) -> String {
    let text_length = GetWindowTextLengthW(hwnd);
    let mut wide_buffer = vec![0u16; (text_length + 1) as usize];
    GetWindowTextW(hwnd, &mut wide_buffer);
    match wide_string_to_string(&wide_buffer) {
        Ok(s) => s,
        Err(e) => panic!("Failed to convert wide string to string: {:?}", e),
    }
}

/// Get the display percent of a rect on the screen
fn get_display_percent(rect: RECT, width: i32, height: i32) -> f32 {
    assert!(width > 0 && height > 0);
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

    (display_width * display_height) / (original_width * original_height)
}

trait RectCalc {
    fn left_top(&self) -> bool;
}

impl RectCalc for RECT {
    fn left_top(&self) -> bool {
        (self.left >= 0 && self.left <= 100) && (self.top >= 0 && self.top <= 100)
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

    let window_text = get_window_text(hwnd);
    if window_text.is_empty() {
        return BOOL(1);
    }

    let mut rect = RECT::default();
    match GetWindowRect(hwnd, &mut rect) {
        Ok(_) => {}
        Err(e) => panic!("GetWindowRect failed for {:?}: {:?}", hwnd, e),
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
        ShowWindow(hwnd, SW_RESTORE).expect(&format!("ShowWindow failed for {:?}", hwnd));
    }

    println!(
        "\
Title: {window_text:?}
Window Handle: {hwnd:?}
Rect: {rect:?}
Display Percent: {:.2}%
        ",
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
        Ok(_) => println!("SetWindowPos succeeded for {:?}\n", hwnd,),
        Err(e) => {
            println!("SetWindowPos failed for {:?}: {}\n", hwnd, e);
            if e.code() == E_ACCESS_DENIED {
                println!("Tip: Try running as administrator.");
            }
        }
    }

    BOOL(1)
}

const E_ACCESS_DENIED: HRESULT = HRESULT::from_win32(0x80070005);

fn main() {
    unsafe {
        match EnumWindows(Some(enum_window_callback), LPARAM(0)) {
            Ok(_) => {}
            Err(e) => panic!("EnumWindows failed: {:?}", e),
        };
    }
}
