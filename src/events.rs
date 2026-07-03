use std::ffi::c_void;
use std::ptr::null_mut;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::AppState;
use crate::app::{lx_w, rx, sys_dpi};
use crate::color::*;
use crate::context_menu::{
    install_context_menu, is_context_menu_installed, uninstall_context_menu,
};
use crate::icon::{apply_folder_color, reset_folder_color};
use crate::library::{load_library, load_presets, save_library, save_presets};

impl AppState {
    pub(crate) unsafe fn on_create(&mut self) -> LRESULT {
        let dpi = sys_dpi();
        self.dpi = dpi;
        self.layout_init();

        self.tf = CreateFontW(
            -scale_by(20, self.dpi),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            windows::core::w!("Segoe UI"),
        );
        self.bf = CreateFontW(
            -scale_by(16, self.dpi),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            windows::core::w!("Segoe UI"),
        );
        self.sf = CreateFontW(
            -scale_by(13, self.dpi),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            windows::core::w!("Segoe UI"),
        );

        self.back_brush = CreateSolidBrush(COLORREF(rgb(0x1E, 0x1E, 0x1E)));
        self.panel_brush = CreateSolidBrush(COLORREF(rgb(0x25, 0x25, 0x25)));
        self.card_brush = CreateSolidBrush(COLORREF(rgb(0x2D, 0x2D, 0x2D)));
        self.focus_pen = CreatePen(PS_SOLID, 2, COLORREF(rgb(0xFF, 0xFF, 0xFF)));

        self.hue = 0.58;
        self.sat = 0.7;
        self.val = 0.8;
        self.ctx_menu = is_context_menu_installed();
        load_library(self);
        load_presets(self);
        LRESULT(0)
    }

    pub(crate) unsafe fn on_lbuttondown(&mut self, mx: i32, my: i32) -> LRESULT {
        let rxs = rx(self, 0);

        // Wheel
        if mx >= rxs && mx < rxs + self.l_wheel_sz && my >= 56 && my < 56 + self.l_wheel_sz {
            let cx = (rxs + self.l_wheel_sz / 2) as f32;
            let cy = (56 + self.l_wheel_sz / 2) as f32;
            let dx = mx as f32 - cx;
            let dy = my as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt() / (self.l_wheel_sz as f32 / 2.0);
            if dist <= 1.0 {
                let ang = (-dy).atan2(dx) / (2.0 * std::f32::consts::PI) + 0.25;
                self.hue = ang - ang.floor();
                self.sat = dist;
                self.sel_swatch = -1;
                self.sel_lib = -1;
                self.name_pos = 0;
                self.hex_pos = 0;
                self.wheel_cx = mx;
                self.wheel_cy = my;
                self.wheel_set = true;
                self.mouse_down = true;
                self.drag_mode = 1;
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
            return LRESULT(0);
        }

        // Brightness bar
        if mx >= rxs
            && mx < rxs + self.l_wheel_sz
            && my >= self.l_bright_y
            && my < self.l_bright_y + self.l_bright_h
        {
            self.val = ((mx - rxs) as f32 / self.l_wheel_sz as f32).clamp(0.0, 1.0);
            self.sel_swatch = -1;
            self.sel_lib = -1;
            self.name_pos = 0;
            self.hex_pos = 0;
            self.mouse_down = true;
            self.drag_mode = 2;
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return LRESULT(0);
        }

        // Library cards on left panel
        let lib_card_h = scale_by(34, self.dpi);
        if mx < lx_w() {
            for i in 0..self.library.len() as i32 {
                let ri = i + self.lib_scroll;
                if ri < 0 || ri >= self.library.len() as i32 {
                    continue;
                }
                let sy = 56 + i * lib_card_h;
                if my < sy || my >= sy + lib_card_h - 2 {
                    continue;
                }
                // Remove button (✕, rightmost)
                let rm_x = lx_w() - 102 + scale_by(44, self.dpi) + 4;
                let rm_sz = scale_by(22, self.dpi);
                if mx >= rm_x && mx < rm_x + rm_sz {
                    self.library.remove(ri as usize);
                    self.lib_names.remove(ri as usize);
                    save_library(self);
                    if self.sel_lib == ri {
                        self.sel_lib = -1;
                    }
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                    return LRESULT(0);
                }
                // Apply button (left of Remove)
                let ap_x = lx_w() - 102;
                let ap_w = scale_by(44, self.dpi);
                if mx >= ap_x && mx < ap_x + ap_w {
                    if self.browse_ok && self.folder_path[0] != 0 {
                        let c = self.library[ri as usize];
                        let (r8, g8, b8) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
                        let folder = String::from_utf16_lossy(&self.folder_path);
                        let folder = folder.trim_end_matches(char::from(0));
                        if apply_folder_color(folder, r8, g8, b8) {
                            self.set_status("Applied to folder!");
                        }
                    } else {
                        self.set_status("Browse a folder first");
                    }
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                    return LRESULT(0);
                }
                // Click on swatch/name area
                if mx >= 8 && mx < ap_x {
                    let c = self.library[ri as usize];
                    let (r8, g8, b8) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
                    let rf = r8 as f32 / 255.0;
                    let gf = g8 as f32 / 255.0;
                    let bf = b8 as f32 / 255.0;
                    let max = rf.max(gf).max(bf);
                    let min = rf.min(gf).min(bf);
                    let d = max - min;
                    let h = if d == 0.0 {
                        0.0
                    } else if max == rf {
                        60.0 * ((gf - bf) / d).rem_euclid(6.0)
                    } else if max == gf {
                        60.0 * ((bf - rf) / d + 2.0)
                    } else {
                        60.0 * ((rf - gf) / d + 4.0)
                    };
                    let s = if max == 0.0 { 0.0 } else { d / max };
                    if (ri as usize) < self.lib_names.len() {
                        let nb = &self.lib_names[ri as usize];
                        for j in 0..63 {
                            self.name_buf[j] = nb[j];
                        }
                        self.name_pos = 63;
                        while self.name_pos > 0 && self.name_buf[self.name_pos - 1] == 0 {
                            self.name_pos -= 1;
                        }
                    } else {
                        self.name_pos = 0;
                    }
                    self.hue = h / 360.0;
                    self.sat = s;
                    self.val = max;
                    self.sel_swatch = -1;
                    self.sel_lib = ri;
                    self.hex_pos = 0;
                    self.wheel_set = false;
                    self.wheel_brightness = -1.0;
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                    return LRESULT(0);
                }
            }
        }

        // 16 presets to the right of the wheel
        let ps_x = rxs + self.l_wheel_sz + 12;
        let ps_y = 56;
        let ps_cols = 4;
        for i in 0..16 {
            let row = i / ps_cols;
            let col = i % ps_cols;
            let sx = ps_x + col * (self.l_swatch_sz + self.l_swatch_gap);
            let sy = ps_y + row * (self.l_swatch_sz + self.l_swatch_gap);
            if mx >= sx && mx < sx + self.l_swatch_sz && my >= sy && my < sy + self.l_swatch_sz {
                let c = self.presets[i as usize];
                let r8 = (c >> 16) as u8;
                let g8 = (c >> 8) as u8;
                let b8 = c as u8;
                let rf = r8 as f32 / 255.0;
                let gf = g8 as f32 / 255.0;
                let bf = b8 as f32 / 255.0;
                let max = rf.max(gf).max(bf);
                let min = rf.min(gf).min(bf);
                let d = max - min;
                let h = if d == 0.0 {
                    0.0
                } else if max == rf {
                    60.0 * ((gf - bf) / d).rem_euclid(6.0)
                } else if max == gf {
                    60.0 * ((bf - rf) / d + 2.0)
                } else {
                    60.0 * ((rf - gf) / d + 4.0)
                };
                let s = if max == 0.0 { 0.0 } else { d / max };
                self.hue = h / 360.0;
                self.sat = s;
                self.val = max;
                self.sel_swatch = i as i32;
                self.sel_lib = -1;
                self.name_pos = 0;
                self.hex_pos = 0;
                self.wheel_set = false;
                self.wheel_brightness = -1.0;
                let _ = InvalidateRect(Some(self.hwnd), None, false);
                return LRESULT(0);
            }
        }

        // Name input area
        let name_x = rxs + self.l_preview_sz + 16;
        let name_y = self.l_preview_y + 18;
        let name_w = self.rw - self.l_preview_sz - 40;
        if mx >= name_x && mx < name_x + name_w && my >= name_y && my < name_y + self.l_name_h {
            self.focus_field = 1;
            self.drag_mode = 0;
            if self.hex_pos > 0 {
                self.commit_hex();
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return LRESULT(0);
        }
        // Hex input area
        let hex_sy = name_y + self.l_name_h + 8 + 18;
        if mx >= name_x
            && mx < name_x + name_w.min(90)
            && my >= hex_sy
            && my < hex_sy + self.l_hex_h
        {
            let (pr, pg, pb) = hsv_to_rgb(self.hue, self.sat, self.val);
            let s = format!("{:02X}{:02X}{:02X}", pr, pg, pb);
            for (j, c) in s.encode_utf16().enumerate() {
                if j < 6 {
                    self.hex_buf[j] = c;
                }
            }
            self.hex_pos = 6;
            self.focus_field = 2;
            self.drag_mode = 3;
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return LRESULT(0);
        }

        // Add to Library
        if mx >= rxs
            && mx <= rxs + (self.rw - rxs - 12)
            && my >= self.l_add_y
            && my <= self.l_add_y + self.l_btn_h
        {
            let (r8, g8, b8) = hsv_to_rgb(self.hue, self.sat, self.val);
            let color = rgb(r8, g8, b8) & 0xFFFFFF;
            let mut nb = [0u16; 64];
            for j in 0..self.name_pos.min(63) {
                nb[j] = self.name_buf[j];
            }
            if let Some(pos) = self.library.iter().position(|&c| c == color) {
                self.lib_names[pos] = nb;
                self.set_status("Name updated \u{2713}");
            } else {
                self.library.push(color);
                self.lib_names.push(nb);
                self.set_status("Added to library \u{2713}");
            }
            save_library(self);
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return LRESULT(0);
        }

        // Browse
        if mx >= rxs
            && mx <= rxs + 90
            && my >= self.l_browse_y
            && my <= self.l_browse_y + self.l_btn_sm
        {
            let mut bi = BROWSEINFOW {
                hwndOwner: self.hwnd,
                pidlRoot: null_mut(),
                pszDisplayName: windows::core::PWSTR(null_mut()),
                ulFlags: BIF_RETURNONLYFSDIRS,
                lpfn: None,
                lParam: LPARAM(0),
                iImage: 0,
                ..Default::default()
            };
            let title: Vec<u16> = "Select a folder\0".encode_utf16().collect();
            bi.lpszTitle = windows::core::PCWSTR(title.as_ptr());
            let pidl = SHBrowseForFolderW(&bi);
            if !pidl.is_null() {
                let mut path_buf = [0u16; 260];
                let _ = SHGetPathFromIDListW(pidl, &mut path_buf);
                self.folder_path = path_buf;
                self.browse_ok = true;
                CoTaskMemFree(Some(pidl as *const c_void));
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return LRESULT(0);
        }

        // Apply + Reset
        let apply_y = self.l_browse_y + self.l_btn_sm + 8;
        if self.browse_ok && self.folder_path[0] != 0 {
            if mx >= rxs && mx <= rxs + 70 && my >= apply_y && my <= apply_y + self.l_btn_sm {
                let (r8, g8, b8) = hsv_to_rgb(self.hue, self.sat, self.val);
                let folder = String::from_utf16_lossy(&self.folder_path);
                let folder = folder.trim_end_matches(char::from(0));
                if apply_folder_color(folder, r8, g8, b8) {
                    self.set_status("Applied \u{2713}");
                }
                let _ = InvalidateRect(Some(self.hwnd), None, false);
                return LRESULT(0);
            }
            if mx >= rxs + 76
                && mx <= rxs + 76 + 70
                && my >= apply_y
                && my <= apply_y + self.l_btn_sm
            {
                let folder = String::from_utf16_lossy(&self.folder_path);
                let folder = folder.trim_end_matches(char::from(0));
                reset_folder_color(folder);
                self.set_status("Reset \u{2713}");
                let _ = InvalidateRect(Some(self.hwnd), None, false);
                return LRESULT(0);
            }
        }

        // Checkbox
        let cb_x = rxs + 90 + 12;
        let cs = 14;
        let cbx = cb_x;
        let cby = self.l_browse_y + (self.l_btn_sm - cs) / 2;
        if mx >= cbx && mx <= cbx + cs + 200 && my >= cby && my <= cby + cs {
            if self.ctx_menu {
                uninstall_context_menu();
                self.ctx_menu = false;
            } else {
                install_context_menu(self);
                self.ctx_menu = true;
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return LRESULT(0);
        }

        LRESULT(0)
    }

    pub(crate) unsafe fn on_rbuttondown(&mut self, mx: i32, my: i32) -> LRESULT {
        let rxs = rx(self, 0);

        // Right-click on left panel library → remove entry
        let lib_row_h = scale_by(30, self.dpi);
        if mx < lx_w() {
            for i in 0..self.library.len() as i32 {
                let ri = i + self.lib_scroll;
                if ri < 0 || ri >= self.library.len() as i32 {
                    continue;
                }
                let sy = 52 + i * lib_row_h;
                if mx >= 16 && mx < lx_w() - 4 && my >= sy && my < sy + lib_row_h {
                    self.library.remove(ri as usize);
                    save_library(self);
                    if self.sel_lib == ri {
                        self.sel_lib = -1;
                    }
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                    return LRESULT(0);
                }
            }
        }

        // Right-click on right panel preset → replace with current color
        let ps_x = rxs + self.l_wheel_sz + 12;
        let ps_y = 56;
        let ps_cols = 4;
        for i in 0..16 {
            let row = i / ps_cols;
            let col = i % ps_cols;
            let sx = ps_x + col * (self.l_swatch_sz + self.l_swatch_gap);
            let sy = ps_y + row * (self.l_swatch_sz + self.l_swatch_gap);
            if mx >= sx && mx < sx + self.l_swatch_sz && my >= sy && my < sy + self.l_swatch_sz {
                let (r8, g8, b8) = hsv_to_rgb(self.hue, self.sat, self.val);
                self.presets[i as usize] = rgb(r8, g8, b8) & 0xFFFFFF;
                save_presets(self);
                let _ = InvalidateRect(Some(self.hwnd), None, false);
                return LRESULT(0);
            }
        }
        LRESULT(0)
    }

    pub(crate) unsafe fn on_mousemove(&mut self, mx: i32, my: i32, _buttons: bool) -> bool {
        let rxs = rx(self, 0);
        let redraw = if self.drag_mode == 1 {
            let cx = (rxs + self.l_wheel_sz / 2) as f32;
            let cy = (56 + self.l_wheel_sz / 2) as f32;
            let dx = mx as f32 - cx;
            let dy = my as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt() / (self.l_wheel_sz as f32 / 2.0);
            if dist <= 1.0 {
                let ang = (-dy).atan2(dx) / (2.0 * std::f32::consts::PI) + 0.25;
                self.hue = ang - ang.floor();
                self.sat = dist;
                self.sel_swatch = -1;
                self.sel_lib = -1;
                self.name_pos = 0;
                self.hex_pos = 0;
                self.wheel_cx = mx;
                self.wheel_cy = my;
                true
            } else {
                false
            }
        } else if self.drag_mode == 2 {
            if mx >= rxs && mx < rxs + self.l_wheel_sz {
                self.val = ((mx - rxs) as f32 / self.l_wheel_sz as f32).clamp(0.0, 1.0);
                self.sel_swatch = -1;
                self.sel_lib = -1;
                self.name_pos = 0;
                self.hex_pos = 0;
                true
            } else {
                false
            }
        } else {
            false
        };
        if redraw {
            self.wheel_brightness = -1.0;
        }
        redraw
    }

    pub(crate) unsafe fn on_mousewheel(&mut self, scroll: i32) {
        let lib_row_h = scale_by(30, self.dpi);
        if lib_row_h > 0 {
            let max_scroll = (self.library.len() as i32) - (520 / lib_row_h);
            self.lib_scroll = (self.lib_scroll - scroll / 120)
                .max(0)
                .min(max_scroll.max(0));
        }
        let _ = InvalidateRect(Some(self.hwnd), None, false);
    }

    pub(crate) unsafe fn on_char(&mut self, c: u16) -> bool {
        if self.focus_field == 1 {
            match c {
                0x08 => {
                    if self.name_pos > 0 {
                        self.name_pos -= 1;
                        self.name_buf[self.name_pos] = 0;
                    }
                }
                0x0D | 0x1B => {
                    self.focus_field = 0;
                }
                0x20..=0x7E => {
                    if self.name_pos < 63 {
                        self.name_buf[self.name_pos] = c;
                        self.name_pos += 1;
                    }
                }
                _ => {}
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return true;
        }
        if self.focus_field == 2 || self.hex_pos > 0 {
            match c {
                0x08 => {
                    if self.hex_pos > 0 {
                        self.hex_pos -= 1;
                        self.hex_buf[self.hex_pos] = 0;
                    }
                }
                0x0D | 0x1B => {
                    self.commit_hex();
                    self.focus_field = 0;
                }
                0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66 => {
                    if self.hex_pos < 6 {
                        let uc = if c >= 0x61 { c - 0x20 } else { c };
                        self.hex_buf[self.hex_pos] = uc;
                        self.hex_pos += 1;
                    }
                }
                _ => {}
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            return true;
        }
        false
    }

    pub(crate) unsafe fn on_timer(&mut self) {
        self.status_cnt -= 1;
        if self.status_cnt <= 0 {
            self.status[0] = 0;
            let _ = KillTimer(Some(self.hwnd), self.status_timer as usize);
        }
        let _ = InvalidateRect(Some(self.hwnd), None, false);
    }

    pub(crate) unsafe fn on_dpi_changed(&mut self, new_dpi: i32) {
        self.dpi = new_dpi;
        self.layout_init();
        if !self.tf.is_invalid() {
            let _ = DeleteObject(self.tf.into());
        }
        if !self.bf.is_invalid() {
            let _ = DeleteObject(self.bf.into());
        }
        if !self.sf.is_invalid() {
            let _ = DeleteObject(self.sf.into());
        }
        self.tf = CreateFontW(
            -scale_by(20, self.dpi),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            windows::core::w!("Segoe UI"),
        );
        self.bf = CreateFontW(
            -scale_by(16, self.dpi),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            windows::core::w!("Segoe UI"),
        );
        self.sf = CreateFontW(
            -scale_by(13, self.dpi),
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            0,
            windows::core::w!("Segoe UI"),
        );
        if !self.wheel_bitmap.is_null() {
            GdipDisposeImage(self.wheel_bitmap as *mut GpImage);
            self.wheel_bitmap = null_mut();
        }
        if !self.wheel_graphics.is_null() {
            GdipDeleteGraphics(self.wheel_graphics);
            self.wheel_graphics = null_mut();
        }
    }
}
