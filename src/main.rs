#![allow(non_snake_case, unused)]

mod app;
mod color;
mod context_menu;
mod events;
mod icon;
mod library;
mod render;

use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::AppState;
use crate::app::{sys_dpi, G_GDI_TOKEN};
use crate::color::{parse_hex, scale_by};
use crate::icon::{apply_folder_color, reset_folder_color};

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let a_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if a_ptr == 0 && msg != WM_CREATE {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let a = if a_ptr != 0 {
            &mut *(a_ptr as *mut AppState)
        } else {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let a_ref = &mut *(cs.lpCreateParams as *mut AppState);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, a_ref as *mut AppState as isize);
            a_ref.hwnd = hwnd;
            a_ref
        };

        match msg {
            WM_CREATE => a.on_create(),

            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => LRESULT(0),

            WM_PAINT => {
                a.paint();
                LRESULT(0)
            }

            WM_LBUTTONDOWN => {
                let mx = (lparam.0 as u16) as i32;
                let my = (lparam.0 >> 16) as i32;
                a.on_lbuttondown(mx, my)
            }

            WM_RBUTTONDOWN => {
                let mx = (lparam.0 as u16) as i32;
                let my = (lparam.0 >> 16) as i32;
                a.on_rbuttondown(mx, my)
            }

            WM_MOUSEMOVE => {
                if !a.mouse_down {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let mx = (lparam.0 as u16) as i32;
                let my = (lparam.0 >> 16) as i32;
                if a.on_mousemove(mx, my, true) {
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                }
                LRESULT(0)
            }

            WM_LBUTTONUP => {
                a.mouse_down = false;
                a.drag_mode = 0;
                if a.hex_pos > 0 {
                    a.commit_hex();
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                }
                LRESULT(0)
            }

            WM_CHAR => {
                let c = wparam.0 as u16;
                if !a.on_char(c) {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                LRESULT(0)
            }

            WM_TIMER => {
                a.on_timer();
                LRESULT(0)
            }

            WM_MOUSEWHEEL => {
                let scroll = (wparam.0 >> 16) as i16 as i32;
                a.on_mousewheel(scroll);
                LRESULT(0)
            }

            WM_DPICHANGED => {
                let new_dpi = (wparam.0 as u32 >> 16) as i32;
                a.on_dpi_changed(new_dpi);
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER,
                );
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }

            WM_GETMINMAXINFO => {
                let mmi = &mut *(lparam.0 as *mut MINMAXINFO);
                let d = if a.dpi > 0 { a.dpi } else { sys_dpi() };
                let mut mr = RECT {
                    left: 0,
                    top: 0,
                    right: scale_by(562, d),
                    bottom: scale_by(730, d),
                };
                let _ = AdjustWindowRectEx(
                    &mut mr,
                    WS_OVERLAPPEDWINDOW & !WS_MAXIMIZEBOX,
                    false,
                    WINDOW_EX_STYLE::default(),
                );
                mmi.ptMinTrackSize.x = mr.right - mr.left;
                mmi.ptMinTrackSize.y = mr.bottom - mr.top;
                LRESULT(0)
            }

            WM_DESTROY => {
                if !a.tf.is_invalid() {
                    let _ = DeleteObject(a.tf.into());
                }
                if !a.bf.is_invalid() {
                    let _ = DeleteObject(a.bf.into());
                }
                if !a.sf.is_invalid() {
                    let _ = DeleteObject(a.sf.into());
                }
                if !a.back_brush.is_invalid() {
                    let _ = DeleteObject(a.back_brush.into());
                }
                if !a.panel_brush.is_invalid() {
                    let _ = DeleteObject(a.panel_brush.into());
                }
                if !a.card_brush.is_invalid() {
                    let _ = DeleteObject(a.card_brush.into());
                }
                if !a.focus_pen.is_invalid() {
                    let _ = DeleteObject(a.focus_pen.into());
                }
                if !a.buf_dc.is_invalid() {
                    let _ = SelectObject(a.buf_dc, GetStockObject(DC_BRUSH));
                    if !a.buf_bmp.is_invalid() {
                        let _ = DeleteObject(a.buf_bmp.into());
                    }
                    let _ = DeleteDC(a.buf_dc);
                }
                if !a.wheel_bitmap.is_null() {
                    GdipDisposeImage(a.wheel_bitmap as *mut GpImage);
                }
                if !a.wheel_graphics.is_null() {
                    GdipDeleteGraphics(a.wheel_graphics);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--apply" {
        let folder = &args[2];
        let hex_w: Vec<u16> = args[3].encode_utf16().collect();
        if let Some((r, g, b)) = parse_hex(&hex_w) {
            unsafe {
                apply_folder_color(folder, r, g, b);
            }
        }
        return;
    }
    if args.len() >= 3 && args[1] == "--reset" {
        let folder = &args[2];
        unsafe {
            reset_folder_color(folder);
        }
        return;
    }

    unsafe {
        let mut input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: false.into(),
            SuppressExternalCodecs: false.into(),
        };
        let status = GdiplusStartup(&raw mut G_GDI_TOKEN, &input, null_mut());
        if status.0 != 0 {
            return;
        }

        let hinst = GetModuleHandleW(None).unwrap();
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: HBRUSH::default(),
            lpszClassName: windows::core::w!("FolderColorizerRust"),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let _ = SetProcessDPIAware();

        let dpi = sys_dpi();
        let mut wr = RECT {
            left: 0,
            top: 0,
            right: scale_by(820, dpi),
            bottom: scale_by(780, dpi),
        };
        let _ = AdjustWindowRectEx(
            &mut wr,
            WS_OVERLAPPEDWINDOW & !WS_MAXIMIZEBOX,
            false,
            WINDOW_EX_STYLE::default(),
        );

        let mut a = Box::new(AppState::new());
        if args.len() >= 2 && !args[1].starts_with('-') {
            let folder_w: Vec<u16> = std::ffi::OsStr::new(&args[1])
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            for (i, &c) in folder_w.iter().enumerate() {
                if i < 259 {
                    a.folder_path[i] = c;
                }
            }
            a.browse_ok = true;
        }

        let hwnd_res = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::w!("FolderColorizerRust"),
            windows::core::w!("Folder Colorizer"),
            WS_OVERLAPPEDWINDOW & !WS_MAXIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            wr.right - wr.left,
            wr.bottom - wr.top,
            None,
            None,
            Some(hinst.into()),
            Some(&mut *a as *mut AppState as *mut _),
        );

        if let std::result::Result::Ok(hwnd) = hwnd_res {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }
        }

        GdiplusShutdown(G_GDI_TOKEN);
    }
}
