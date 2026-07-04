use std::mem::zeroed;
use std::ptr::null_mut;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::color::*;

pub(crate) const COLORS: &[(u32, &str)] = &[
    (0x00BCD4, "Turquoise"),
    (0xF1C40F, "Sun Flower"),
    (0x2ECC71, "Emerald"),
    (0xE67E22, "Carrot"),
    (0x3498DB, "Peter River"),
    (0xE74C3C, "Alizarin"),
    (0x9B59B6, "Wisteria"),
];

pub(crate) const REG_ROOT: &str = r"Software\Classes\Folder\shell\Colorize";
pub(crate) const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026200a;

pub(crate) static mut G_GDI_TOKEN: usize = 0;

pub(crate) fn get_exe_path() -> String {
    let mut buf = [0u16; 260];
    unsafe {
        GetModuleFileNameW(None, &mut buf);
    }
    String::from_utf16_lossy(&buf)
        .trim_end_matches(char::from(0))
        .to_string()
}

pub(crate) fn cast_bytes(v: &[u16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) }
}

pub(crate) fn sys_dpi() -> i32 {
    unsafe {
        let hdc = GetDC(None);
        let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSY);
        let _ = ReleaseDC(None, hdc);
        dpi
    }
}

pub(crate) fn lx_w() -> i32 {
    280
}

pub(crate) fn rx(a: &AppState, x: i32) -> i32 {
    lx_w() + 16 + x
}

pub(crate) struct AppState {
    pub(crate) dpi: i32,
    pub(crate) mouse_down: bool,
    pub(crate) drag_mode: i32,
    pub(crate) name_buf: [u16; 64],
    pub(crate) name_pos: usize,
    pub(crate) hex_buf: [u16; 8],
    pub(crate) hex_pos: usize,
    pub(crate) focus_field: i32,
    pub(crate) status_timer: i32,
    pub(crate) status_cnt: i32,
    pub(crate) l_wheel_sz: i32,
    pub(crate) l_swatch_sz: i32,
    pub(crate) l_swatch_gap: i32,
    pub(crate) l_bright_h: i32,
    pub(crate) l_preview_sz: i32,
    pub(crate) l_btn_h: i32,
    pub(crate) l_btn_sm: i32,
    pub(crate) l_name_h: i32,
    pub(crate) l_hex_h: i32,
    pub(crate) l_bright_y: i32,
    pub(crate) l_preview_y: i32,
    pub(crate) l_name_y: i32,
    pub(crate) l_add_y: i32,
    pub(crate) l_browse_y: i32,
    pub(crate) rw: i32,
    pub(crate) lib_scroll_right: i32,
    pub(crate) hue: f32,
    pub(crate) sat: f32,
    pub(crate) val: f32,
    pub(crate) sel_swatch: i32,
    pub(crate) sel_lib: i32,
    pub(crate) wheel_cx: i32,
    pub(crate) wheel_cy: i32,
    pub(crate) wheel_set: bool,
    pub(crate) folder_path: [u16; 260],
    pub(crate) status: [u16; 128],
    pub(crate) browse_ok: bool,
    pub(crate) ctx_menu: bool,
    pub(crate) wheel_bitmap: *mut GpBitmap,
    pub(crate) wheel_graphics: *mut GpGraphics,
    pub(crate) wheel_size: i32,
    pub(crate) wheel_brightness: f32,
    pub(crate) tf: HFONT,
    pub(crate) bf: HFONT,
    pub(crate) sf: HFONT,
    pub(crate) back_brush: HBRUSH,
    pub(crate) panel_brush: HBRUSH,
    pub(crate) card_brush: HBRUSH,
    pub(crate) focus_pen: HPEN,
    pub(crate) buf_bmp: HBITMAP,
    pub(crate) buf_dc: HDC,
    pub(crate) buf_w: i32,
    pub(crate) buf_h: i32,
    pub(crate) hwnd: HWND,
    pub(crate) library: Vec<u32>,
    pub(crate) lib_names: Vec<[u16; 64]>,
    pub(crate) lib_scroll: i32,
    pub(crate) presets: [u32; 16],
    pub(crate) presets_dirty: bool,
}

impl AppState {
    pub(crate) fn new() -> Self {
        AppState {
            dpi: 0,
            mouse_down: false,
            drag_mode: 0,
            name_buf: [0u16; 64],
            name_pos: 0,
            hex_buf: [0u16; 8],
            hex_pos: 0,
            focus_field: 0,
            status_timer: 0,
            status_cnt: 0,
            l_wheel_sz: 0,
            l_swatch_sz: 0,
            l_swatch_gap: 0,
            l_bright_h: 0,
            l_preview_sz: 0,
            l_btn_h: 0,
            l_btn_sm: 0,
            l_name_h: 0,
            l_hex_h: 0,
            l_bright_y: 0,
            l_preview_y: 0,
            l_name_y: 0,
            l_add_y: 0,
            l_browse_y: 0,
            rw: 0,
            lib_scroll_right: 0,
            hue: 0.0,
            sat: 0.0,
            val: 0.0,
            sel_swatch: -1,
            sel_lib: -1,
            wheel_cx: 0,
            wheel_cy: 0,
            wheel_set: false,
            folder_path: [0u16; 260],
            status: [0u16; 128],
            browse_ok: false,
            ctx_menu: false,
            wheel_bitmap: null_mut(),
            wheel_graphics: null_mut(),
            wheel_size: 0,
            wheel_brightness: -1.0,
            tf: HFONT::default(),
            bf: HFONT::default(),
            sf: HFONT::default(),
            back_brush: HBRUSH::default(),
            panel_brush: HBRUSH::default(),
            card_brush: HBRUSH::default(),
            focus_pen: HPEN::default(),
            buf_bmp: HBITMAP::default(),
            buf_dc: HDC::default(),
            buf_w: 0,
            buf_h: 0,
            hwnd: HWND::default(),
            library: Vec::new(),
            lib_names: Vec::new(),
            lib_scroll: 0,
            presets: {
                let mut p = [0u32; 16];
                for i in 0..16 {
                    let (r, g, b) = SWATCHES[i];
                    p[i] = rgb(r, g, b) & 0xFFFFFF;
                }
                p
            },
            presets_dirty: false,
        }
    }

    pub(crate) unsafe fn layout_init(&mut self) {
        self.l_wheel_sz = scale_by(190, self.dpi);
        self.l_swatch_sz = scale_by(28, self.dpi);
        self.l_swatch_gap = scale_by(6, self.dpi);
        self.l_bright_h = scale_by(22, self.dpi);
        self.l_preview_sz = scale_by(72, self.dpi);
        self.l_btn_h = scale_by(36, self.dpi);
        self.l_btn_sm = scale_by(26, self.dpi);
        self.l_name_h = scale_by(26, self.dpi);
        self.l_hex_h = scale_by(26, self.dpi);
        self.l_bright_y = scale_by(259, self.dpi);
        self.l_preview_y = scale_by(310, self.dpi);
        self.l_name_y = scale_by(392, self.dpi);
        self.l_add_y = scale_by(540, self.dpi);
        self.l_browse_y = scale_by(590, self.dpi);
    }

    pub(crate) unsafe fn set_status(&mut self, msg: &str) {
        let w: Vec<u16> = format!("{}\0", msg).encode_utf16().collect();
        for (i, &c) in w.iter().enumerate() {
            if i < 127 {
                self.status[i] = c;
            }
        }
        self.status[127] = 0;
        self.status_timer = 1;
        self.status_cnt = 50;
        let _ = SetTimer(Some(self.hwnd), self.status_timer as usize, 100, None);
    }

    pub(crate) unsafe fn render_wheel(&mut self) {
        let sz = self.l_wheel_sz;
        if sz <= 0 {
            return;
        }
        if self.wheel_brightness == self.val && !self.wheel_bitmap.is_null() {
            return;
        }

        if !self.wheel_bitmap.is_null() {
            GdipDisposeImage(self.wheel_bitmap as *mut GpImage);
            self.wheel_bitmap = null_mut();
        }
        if !self.wheel_graphics.is_null() {
            GdipDeleteGraphics(self.wheel_graphics);
            self.wheel_graphics = null_mut();
        }

        let status = GdipCreateBitmapFromScan0(
            sz,
            sz,
            0,
            PIXEL_FORMAT_32BPP_ARGB,
            None,
            &mut self.wheel_bitmap,
        );
        if status.0 != 0 || self.wheel_bitmap.is_null() {
            return;
        }

        GdipGetImageGraphicsContext(self.wheel_bitmap as *mut GpImage, &mut self.wheel_graphics);
        if self.wheel_graphics.is_null() {
            GdipDisposeImage(self.wheel_bitmap as *mut GpImage);
            self.wheel_bitmap = null_mut();
            return;
        }

        let cx = sz as f32 / 2.0;
        let cy = sz as f32 / 2.0;
        let r = cx;
        let v = self.val;

        let rect = Rect {
            X: 0,
            Y: 0,
            Width: sz,
            Height: sz,
        };
        let mut bmd = BitmapData::default();
        if GdipBitmapLockBits(
            self.wheel_bitmap,
            &rect,
            2u32,
            PIXEL_FORMAT_32BPP_ARGB,
            &mut bmd,
        )
        .0 == 0
            && !bmd.Scan0.is_null()
        {
            let stride = bmd.Stride;
            let pixels =
                std::slice::from_raw_parts_mut(bmd.Scan0 as *mut u8, (stride * sz) as usize);
            for y in 0..sz {
                for x in 0..sz {
                    let dx = x as f32 - cx;
                    let dy = y as f32 - cy;
                    let dist = (dx * dx + dy * dy).sqrt() / r;
                    if dist > 1.0 {
                        continue;
                    }
                    let ang = (-dy).atan2(dx) / (2.0 * std::f32::consts::PI) + 0.25;
                    let h = ang - ang.floor();
                    let (r8, g8, b8) = hsv_to_rgb(h, dist, v);
                    let offset = (y * stride + x * 4) as usize;
                    pixels[offset + 0] = b8; // BGRA
                    pixels[offset + 1] = g8;
                    pixels[offset + 2] = r8;
                    pixels[offset + 3] = 255;
                }
            }
            GdipBitmapUnlockBits(self.wheel_bitmap, &mut bmd);
        }

        self.wheel_brightness = self.val;
        self.wheel_size = sz;
    }

    pub(crate) unsafe fn commit_hex(&mut self) {
        let h = &self.hex_buf[..self.hex_pos.min(6)];
        if let Some((r, g, b)) = parse_hex(h) {
            let rf = r as f32 / 255.0;
            let gf = g as f32 / 255.0;
            let bf = b as f32 / 255.0;
            let max = rf.max(gf).max(bf);
            let min = rf.min(gf).min(bf);
            let d = max - min;
            let hue = if d == 0.0 {
                0.0
            } else if max == rf {
                60.0 * ((gf - bf) / d).rem_euclid(6.0)
            } else if max == gf {
                60.0 * ((bf - rf) / d + 2.0)
            } else {
                60.0 * ((rf - gf) / d + 4.0)
            };
            let s = if max == 0.0 { 0.0 } else { d / max };
            self.hue = hue / 360.0;
            self.sat = s;
            self.val = max;
            self.sel_swatch = -1;
            self.wheel_set = false;
            self.wheel_brightness = -1.0;
        }
        self.hex_pos = 0;
        self.drag_mode = 0;
    }
}
