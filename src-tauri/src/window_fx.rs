//! Плавное сворачивание и разворачивание окна.
//!
//! Окно создаётся без системной рамки (`decorations: false`), а Windows решает,
//! проигрывать ли анимацию сворачивания, по стилю окна: без `WS_CAPTION` и
//! `WS_MINIMIZEBOX` оно просто исчезает и появляется рывком.
//!
//! Возвращаем эти биты стиля напрямую. Рамка при этом не появляется: мы не
//! просим Windows перерисовать её (`SWP_FRAMECHANGED`), поэтому меняется только
//! поведение, а вид остаётся прежним. Этим же приёмом пользуются Electron и
//! другие оболочки для окон без рамки.

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

/// Окна, которым нужна анимация. Панель трея и заставка всплывают сами.
const ANIMATED: [&str; 1] = ["main"];

/// Возвращает окну биты стиля, по которым Windows включает анимацию.
pub fn enable_native_animations(window: &WebviewWindow) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, GWL_STYLE, WS_CAPTION, WS_MAXIMIZEBOX,
            WS_MINIMIZEBOX, WS_SYSMENU,
        };

        let Ok(handle) = window.hwnd() else {
            return;
        };

        let hwnd = handle.0 as windows_sys::Win32::Foundation::HWND;

        // SAFETY: hwnd получен от самого окна и живёт, пока живёт окно.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let wanted = (WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU) as isize;

            if style & wanted != wanted {
                SetWindowLongPtrW(hwnd, GWL_STYLE, style | wanted);
            }
        }
    }

    #[cfg(not(windows))]
    let _ = window;
}

/// Включает анимацию для всех окон, которым она нужна.
pub fn setup(app: &AppHandle) {
    for label in ANIMATED {
        if let Some(window) = app.get_webview_window(label) {
            enable_native_animations(&window);
        }
    }
}

/// Сообщает интерфейсу, что окно свернули или развернули.
///
/// Отдельного события «свернули» у Tauri нет, но при сворачивании Windows
/// присылает изменение размера в 0×0 — по нему и определяем.
pub fn note_resize(window: &tauri::Window, width: u32, height: u32) {
    if !ANIMATED.contains(&window.label()) {
        return;
    }

    let minimized = width == 0 || height == 0;
    let event = if minimized {
        "window:minimized"
    } else {
        "window:restored"
    };

    let _ = window.app_handle().emit(event, ());
}
