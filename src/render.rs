use std::mem::zeroed;
use std::ptr::null_mut;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;

use crate::app::AppState;
use crate::app::{lx_w, rx};
use crate::color::*;
use crate::library::lib_name;

pub(crate) const LIB_CARD_TOP: i32 = 56;
pub(crate) const LIB_CARD_H_BASE: i32 = 34;

unsafe fn fill_rect(hdc: HDC, x: i32, y: i32, w: i32, h: i32, brush: HBRUSH) {
    let r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    FillRect(hdc, &r, brush);
}

unsafe fn draw_txt(
    hdc: HDC,
    x: i32,
    y: i32,
    text: &[u16],
    color: COLORREF,
    font: HFONT,
    max_w: i32,
) {
    let old = SelectObject(hdc, font.into());
    let _ = SetTextColor(hdc, color);
    let _ = SetBkMode(hdc, TRANSPARENT);
    let mut r = RECT {
        left: x,
        top: y,
        right: x + max_w,
        bottom: y + 200,
    };
    let mut text_mut: Vec<u16> = text.to_vec();
    let _ = DrawTextW(hdc, &mut text_mut, &mut r, DT_SINGLELINE | DT_LEFT | DT_TOP);
    let _ = SelectObject(hdc, old);
}

unsafe fn draw_btn(hdc: HDC, x: i32, y: i32, w: i32, h: i32, text: &[u16]) {
    let btn_b = CreateSolidBrush(COLORREF(colorref(0x37, 0x3A, 0x43)));
    fill_rect(hdc, x, y, w, h, btn_b);
    let _ = DeleteObject(btn_b.into());
    let mut r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let old_pen = SelectObject(hdc, GetStockObject(DC_PEN));
    let _ = SetDCPenColor(hdc, COLORREF(colorref(0x55, 0x55, 0x55)));
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = Rectangle(hdc, x, y, x + w, y + h);
    let _ = SelectObject(hdc, old_brush);
    let _ = SelectObject(hdc, old_pen);
    let _ = SetTextColor(hdc, COLORREF(colorref(0xCC, 0xCC, 0xCC)));
    let _ = SetBkMode(hdc, TRANSPARENT);
    let mut text_mut: Vec<u16> = text.to_vec();
    let _ = DrawTextW(
        hdc,
        &mut text_mut,
        &mut r,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

impl AppState {
    pub(crate) unsafe fn paint(&mut self) {
        let mut ps = zeroed::<PAINTSTRUCT>();
        let hdc = BeginPaint(self.hwnd, &mut ps);
        let w = ps.rcPaint.right - ps.rcPaint.left;
        let h = ps.rcPaint.bottom - ps.rcPaint.top;

        // Back buffer
        if self.buf_dc.is_invalid() || self.buf_w != w || self.buf_h != h {
            if !self.buf_dc.is_invalid() {
                let _ = SelectObject(self.buf_dc, GetStockObject(DC_BRUSH));
                let _ = DeleteObject(self.buf_bmp.into());
                let _ = DeleteDC(self.buf_dc);
            }
            self.buf_dc = CreateCompatibleDC(Some(hdc));
            self.buf_bmp = CreateCompatibleBitmap(hdc, w, h);
            let _ = SelectObject(self.buf_dc, self.buf_bmp.into());
            self.buf_w = w;
            self.buf_h = h;
        }

        let dc = self.buf_dc;
        let rw = w;
        self.rw = rw;
        let rxs = rx(self, 0);

        // Background
        fill_rect(dc, 0, 0, w, h, self.back_brush);
        fill_rect(dc, 0, 0, lx_w(), h, self.panel_brush);

        self.render_wheel();
        self.paint_left_panel(dc, w, h);
        self.paint_library_cards(dc, w, h);
        self.paint_presets(dc, rxs);
        self.paint_editor(dc, rxs, rw);
        self.paint_buttons(dc, rxs, rw);

        let _ = BitBlt(hdc, 0, 0, w, h, Some(dc), 0, 0, SRCCOPY);
        EndPaint(self.hwnd, &mut ps);
    }

    unsafe fn paint_left_panel(&self, dc: HDC, w: i32, h: i32) {
        let lib_lbl: Vec<u16> = "COLOR LIBRARY\0".encode_utf16().collect();
        draw_txt(
            dc,
            16,
            16,
            &lib_lbl,
            COLORREF(colorref(0xCC, 0xCC, 0xCC)),
            self.bf,
            lx_w() - 32,
        );
        let sub_lbl: Vec<u16> = "Apply to folder or remove\0".encode_utf16().collect();
        draw_txt(
            dc,
            16,
            38,
            &sub_lbl,
            COLORREF(colorref(0x88, 0x88, 0x88)),
            self.sf,
            lx_w() - 32,
        );
    }

    unsafe fn paint_library_cards(&self, dc: HDC, w: i32, h: i32) {
        let lib_card_h = scale_by(LIB_CARD_H_BASE, self.dpi);
        let lib_top = LIB_CARD_TOP;
        let lib_h = h - lib_top;
        let lib_rows = if lib_card_h > 0 {
            lib_h / lib_card_h
        } else {
            0
        };
        let lx_cnt = self.library.len() as i32;
        if lx_cnt == 0 {
            let empty: Vec<u16> = "No saved colors yet\0".encode_utf16().collect();
            draw_txt(
                dc,
                16,
                52,
                &empty,
                COLORREF(colorref(0x66, 0x66, 0x66)),
                self.sf,
                lx_w() - 32,
            );
        } else {
            for i in 0..lx_cnt.min(lib_rows) {
                let ri = i + self.lib_scroll;
                if ri < 0 || ri >= lx_cnt {
                    continue;
                }
                let sy = lib_top + i * lib_card_h;

                let is_sel = self.sel_lib == ri;
                let card_bg = if is_sel {
                    colorref(0x3A, 0x3D, 0x47)
                } else {
                    colorref(0x2A, 0x2C, 0x34)
                };
                let card_b = CreateSolidBrush(COLORREF(card_bg));
                fill_rect(dc, 8, sy, lx_w() - 16, lib_card_h - 2, card_b);
                let _ = DeleteObject(card_b.into());

                let c = self.library[ri as usize];
                let (cr, cg, cb) = unpack_color(c);
                let sw_b = CreateSolidBrush(COLORREF(colorref(cr, cg, cb)));
                fill_rect(
                    dc,
                    14,
                    sy + 5,
                    scale_by(22, self.dpi),
                    scale_by(22, self.dpi),
                    sw_b,
                );
                let _ = DeleteObject(sw_b.into());

                let nm = lib_name(self, ri as usize);
                let nm_w: Vec<u16> = if nm.is_empty() {
                    format!("#{:06X}\0", c).encode_utf16().collect()
                } else {
                    format!("{}\0", nm).encode_utf16().collect()
                };
                draw_txt(
                    dc,
                    52,
                    sy + 6,
                    &nm_w,
                    COLORREF(colorref(0xCC, 0xCC, 0xCC)),
                    self.bf,
                    lx_w() - 170,
                );

                // Apply button
                let ap_x = lx_w() - 102;
                let ap_w = scale_by(44, self.dpi);
                let ap_h = scale_by(22, self.dpi);
                let ap_b = CreateSolidBrush(COLORREF(colorref(0x37, 0x3A, 0x43)));
                fill_rect(dc, ap_x, sy + 6, ap_w, ap_h, ap_b);
                let _ = DeleteObject(ap_b.into());
                let ap_pen = SelectObject(dc, GetStockObject(DC_PEN));
                SetDCPenColor(dc, COLORREF(colorref(0x55, 0x55, 0x55)));
                let ap_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                Rectangle(dc, ap_x, sy + 6, ap_x + ap_w, sy + 6 + ap_h);
                SelectObject(dc, ap_brush);
                SelectObject(dc, ap_pen);
                let ap_t: Vec<u16> = "Apply\0".encode_utf16().collect();
                let mut ap_r = RECT {
                    left: ap_x,
                    top: sy + 6,
                    right: ap_x + ap_w,
                    bottom: sy + 6 + ap_h,
                };
                SetTextColor(dc, COLORREF(colorref(0xAA, 0xCC, 0xFF)));
                SetBkMode(dc, TRANSPARENT);
                DrawTextW(
                    dc,
                    &mut ap_t.clone(),
                    &mut ap_r,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                // Remove (✕) button
                let rm_x = ap_x + ap_w + 4;
                let rm_sz = ap_h;
                let rm_b = CreateSolidBrush(COLORREF(colorref(0x43, 0x2C, 0x2C)));
                fill_rect(dc, rm_x, sy + 6, rm_sz, rm_sz, rm_b);
                let _ = DeleteObject(rm_b.into());
                let rm_pen = SelectObject(dc, GetStockObject(DC_PEN));
                SetDCPenColor(dc, COLORREF(colorref(0x88, 0x44, 0x44)));
                SelectObject(dc, GetStockObject(NULL_BRUSH));
                Rectangle(dc, rm_x, sy + 6, rm_x + rm_sz, sy + 6 + rm_sz);
                SelectObject(dc, rm_pen);
                let x_pen = CreatePen(PS_SOLID, 2, COLORREF(colorref(0xFF, 0x66, 0x66)));
                let old_xp = SelectObject(dc, x_pen.into());
                MoveToEx(dc, rm_x + 5, sy + 6 + 5, None);
                LineTo(dc, rm_x + rm_sz - 5, sy + 6 + rm_sz - 5);
                MoveToEx(dc, rm_x + rm_sz - 5, sy + 6 + 5, None);
                LineTo(dc, rm_x + 5, sy + 6 + rm_sz - 5);
                SelectObject(dc, old_xp);
                DeleteObject(x_pen.into());
            }
        }
    }

    unsafe fn paint_presets(&self, dc: HDC, rxs: i32) {
        let ps_x = rxs + self.l_wheel_sz + 12;
        let ps_y = 56;
        let ps_sz = self.l_swatch_sz;
        let ps_gap = self.l_swatch_gap;
        let ps_cols = 4;
        for idx in 0..16 {
            let row = idx / ps_cols;
            let col = idx % ps_cols;
            let sx = ps_x + col * (ps_sz + ps_gap);
            let sy = ps_y + row * (ps_sz + ps_gap);
            let c = self.presets[idx as usize];
            let (cr, cg, cb) = unpack_color(c);
            let brush = CreateSolidBrush(COLORREF(colorref(cr, cg, cb)));
            fill_rect(dc, sx, sy, ps_sz, ps_sz, brush);
            let _ = DeleteObject(brush.into());
            if self.sel_swatch == idx as i32 {
                let old = SelectObject(dc, GetStockObject(NULL_BRUSH));
                let old_p = SelectObject(dc, self.focus_pen.into());
                let _ = Rectangle(dc, sx - 2, sy - 2, sx + ps_sz + 2, sy + ps_sz + 2);
                let _ = SelectObject(dc, old_p);
                let _ = SelectObject(dc, old);
            }
        }
    }

    unsafe fn paint_editor(&self, dc: HDC, rxs: i32, rw: i32) {
        let ed_lbl: Vec<u16> = "COLOR EDITOR\0".encode_utf16().collect();
        draw_txt(
            dc,
            rxs,
            20,
            &ed_lbl,
            COLORREF(colorref(0xCC, 0xCC, 0xCC)),
            self.bf,
            300,
        );

        // Color wheel via GDI+
        if !self.wheel_bitmap.is_null() {
            let mut gr: *mut GpGraphics = null_mut();
            GdipCreateFromHDC(dc, &mut gr);
            if !gr.is_null() {
                GdipDrawImageI(gr, self.wheel_bitmap as *mut GpImage, rxs, 56);
                GdipDeleteGraphics(gr);
            }
        }

        // Selection indicator on wheel
        if self.wheel_set {
            let old_b = SelectObject(dc, GetStockObject(NULL_BRUSH));
            let ip = CreatePen(PS_SOLID, 2, COLORREF(colorref(0xFF, 0xFF, 0xFF)));
            let old_p = SelectObject(dc, ip.into());
            let _ = Rectangle(
                dc,
                self.wheel_cx - 6,
                self.wheel_cy - 6,
                self.wheel_cx + 6,
                self.wheel_cy + 6,
            );
            let _ = SelectObject(dc, old_p);
            let _ = DeleteObject(ip.into());
            let _ = SelectObject(dc, old_b);
            let ib = CreateSolidBrush(COLORREF(colorref(0x3C, 0x3F, 0x48)));
            let old_b2 = SelectObject(dc, ib.into());
            let _ = Rectangle(
                dc,
                self.wheel_cx - 2,
                self.wheel_cy - 2,
                self.wheel_cx + 3,
                self.wheel_cy + 3,
            );
            let _ = SelectObject(dc, old_b2);
            let _ = DeleteObject(ib.into());
        }

        // Brightness bar
        let by = self.l_bright_y;
        let bw = self.l_wheel_sz;
        let bh = self.l_bright_h;
        let mut bgr: *mut GpGraphics = null_mut();
        GdipCreateFromHDC(dc, &mut bgr);
        if !bgr.is_null() {
            for bx in 0..bw {
                let t = bx as f32 / bw as f32;
                let (r8, g8, b8) = hsv_to_rgb(self.hue, self.sat, t);
                let mut brush: *mut GpSolidFill = null_mut();
                GdipCreateSolidFill(gdi_argb(r8, g8, b8), &mut brush);
                if !brush.is_null() {
                    GdipFillRectangleI(bgr, brush as *mut GpBrush, rxs + bx, by, 1, bh);
                    GdipDeleteBrush(brush as *mut GpBrush);
                }
            }
            GdipDeleteGraphics(bgr);
        }
        let idx = (self.val * bw as f32) as i32;
        let old_p2 = SelectObject(dc, self.focus_pen.into());
        let _ = MoveToEx(dc, rxs + idx, by, None);
        let _ = LineTo(dc, rxs + idx, by + bh);
        let _ = SelectObject(dc, old_p2);

        // Preview + Name + Hex
        let py = self.l_preview_y;
        let psz = self.l_preview_sz;
        let (pr, pg, pb) = hsv_to_rgb(self.hue, self.sat, self.val);
        let prev_b = CreateSolidBrush(COLORREF(colorref(pr, pg, pb)));
        fill_rect(dc, rxs, py, psz, psz, prev_b);
        let _ = DeleteObject(prev_b.into());
        let old_pb = SelectObject(dc, GetStockObject(DC_PEN));
        let _ = SetDCPenColor(dc, COLORREF(colorref(0x55, 0x55, 0x55)));
        let _ = SelectObject(dc, GetStockObject(NULL_BRUSH));
        let _ = Rectangle(dc, rxs, py, rxs + psz, py + psz);
        let _ = SelectObject(dc, old_pb);

        // Name
        let name_lbl: Vec<u16> = "Name\0".encode_utf16().collect();
        draw_txt(
            dc,
            rxs + psz + 16,
            py,
            &name_lbl,
            COLORREF(colorref(0x88, 0x88, 0x88)),
            self.sf,
            100,
        );
        let name_x = rxs + psz + 16;
        let name_y = py + 18;
        let name_w = rw - psz - 40;
        fill_rect(dc, name_x, name_y, name_w, self.l_name_h, self.card_brush);
        let name_str = if self.name_pos > 0 {
            let mut buf = vec![0u16; 64];
            for i in 0..self.name_pos.min(63) {
                buf[i] = self.name_buf[i];
            }
            buf[self.name_pos.min(63)] = 0;
            buf
        } else {
            let s = format!("Color #{:02X}{:02X}{:02X}\0", pr, pg, pb);
            s.encode_utf16().collect()
        };
        draw_txt(
            dc,
            name_x + 4,
            name_y + 2,
            &name_str,
            COLORREF(colorref(0xCC, 0xCC, 0xCC)),
            self.bf,
            name_w - 8,
        );

        // Hex
        let hex_lbl: Vec<u16> = "Hex\0".encode_utf16().collect();
        let hex_y = name_y + self.l_name_h + 8;
        draw_txt(
            dc,
            name_x,
            hex_y,
            &hex_lbl,
            COLORREF(colorref(0x88, 0x88, 0x88)),
            self.sf,
            60,
        );
        let hex_sy = hex_y + 18;
        fill_rect(
            dc,
            name_x,
            hex_sy,
            name_w.min(90),
            self.l_hex_h,
            self.card_brush,
        );
        let hex_label = if self.hex_pos > 0 {
            let mut buf = vec![0u16; 8];
            for i in 0..self.hex_pos.min(6) {
                buf[i] = self.hex_buf[i];
            }
            buf[self.hex_pos.min(6)] = 0;
            buf
        } else {
            let s = format!("{:02X}{:02X}{:02X}\0", pr, pg, pb);
            s.encode_utf16().collect()
        };
        draw_txt(
            dc,
            name_x + 4,
            hex_sy + 2,
            &hex_label,
            COLORREF(colorref(0xAA, 0xCC, 0xFF)),
            self.bf,
            name_w.min(90) - 8,
        );
    }

    unsafe fn paint_buttons(&self, dc: HDC, rxs: i32, rw: i32) {
        // Add to Library
        let add_t: Vec<u16> = "+ Add Color to Library\0".encode_utf16().collect();
        draw_btn(
            dc,
            rxs,
            self.l_add_y,
            self.rw - rxs - 12,
            self.l_btn_h,
            &add_t,
        );

        // Browse
        let br_t: Vec<u16> = "Browse\0".encode_utf16().collect();
        draw_btn(dc, rxs, self.l_browse_y, 90, self.l_btn_sm, &br_t);
        let fp = String::from_utf16_lossy(&self.folder_path);
        if self.browse_ok && self.folder_path[0] != 0 {
            let fp_w: Vec<u16> = format!("{}\0", fp.trim_end_matches(char::from(0)))
                .encode_utf16()
                .collect();
            draw_txt(
                dc,
                rxs + 90 + 12,
                self.l_browse_y + 4,
                &fp_w,
                COLORREF(colorref(0x88, 0x88, 0x88)),
                self.sf,
                rw - 24 - 90 - 12,
            );
        }

        // Apply + Reset
        if self.browse_ok && self.folder_path[0] != 0 {
            let ap_t: Vec<u16> = "Apply\0".encode_utf16().collect();
            draw_btn(
                dc,
                rxs,
                self.l_browse_y + self.l_btn_sm + 8,
                70,
                self.l_btn_sm,
                &ap_t,
            );
            let rs_t: Vec<u16> = "Reset\0".encode_utf16().collect();
            draw_btn(
                dc,
                rxs + 76,
                self.l_browse_y + self.l_btn_sm + 8,
                70,
                self.l_btn_sm,
                &rs_t,
            );
        }

        // Context menu checkbox
        let cb_x = rxs + 90 + 12;
        let cs = 14;
        let cb_brush = CreateSolidBrush(if self.ctx_menu {
            COLORREF(colorref(0x34, 0x98, 0xDB))
        } else {
            COLORREF(colorref(0x33, 0x33, 0x33))
        });
        fill_rect(
            dc,
            cb_x,
            self.l_browse_y + (self.l_btn_sm - cs) / 2,
            cs,
            cs,
            cb_brush,
        );
        let _ = DeleteObject(cb_brush.into());
        let old_p3 = SelectObject(dc, GetStockObject(DC_PEN));
        let _ = SetDCPenColor(dc, COLORREF(colorref(0x66, 0x66, 0x66)));
        let _ = SelectObject(dc, GetStockObject(NULL_BRUSH));
        let _ = Rectangle(
            dc,
            cb_x,
            self.l_browse_y + (self.l_btn_sm - cs) / 2,
            cb_x + cs,
            self.l_browse_y + (self.l_btn_sm - cs) / 2 + cs,
        );
        let _ = SelectObject(dc, old_p3);
        if self.ctx_menu {
            let old_p4 = SelectObject(dc, GetStockObject(WHITE_PEN));
            let _ = MoveToEx(
                dc,
                cb_x + 3,
                self.l_browse_y + (self.l_btn_sm - cs) / 2 + 7,
                None,
            );
            let _ = LineTo(
                dc,
                cb_x + 6,
                self.l_browse_y + (self.l_btn_sm - cs) / 2 + 10,
            );
            let _ = LineTo(
                dc,
                cb_x + 11,
                self.l_browse_y + (self.l_btn_sm - cs) / 2 + 4,
            );
            let _ = SelectObject(dc, old_p4);
        }
        let cb_lbl: Vec<u16> = "Add to right-click menu\0".encode_utf16().collect();
        draw_txt(
            dc,
            cb_x + cs + 8,
            self.l_browse_y + 4,
            &cb_lbl,
            COLORREF(colorref(0x88, 0x88, 0x88)),
            self.sf,
            rw - cb_x - cs - 24,
        );

        // Status
        if self.status[0] != 0 {
            let st = String::from_utf16_lossy(&self.status);
            let st_w: Vec<u16> = format!("{}\0", st.trim_end_matches(char::from(0)))
                .encode_utf16()
                .collect();
            draw_txt(
                dc,
                rxs,
                self.l_browse_y + self.l_btn_sm * 2 + 16,
                &st_w,
                COLORREF(colorref(0x00, 0xCC, 0x66)),
                self.bf,
                rw - 48,
            );
        }
    }
}
