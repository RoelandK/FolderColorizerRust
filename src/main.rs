#![allow(non_snake_case, unused)]

use std::ffi::c_void;
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::Storage::FileSystem::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Shell::Common::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// Bring in the ImageEncoder trait so PngEncoder::write_image works
use image::ImageEncoder;

const APP_NAME: &str = "Folder Colorizer (Rust)";
const REG_ROOT: &str = r"Software\Classes\Folder\shell\Colorize";
const PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026200a;

// ── Colors matching C# version ──
const COLORS: &[(u32, &str)] = &[
    (0x00BCD4, "Turquoise"),
    (0xF1C40F, "Sun Flower"),
    (0x2ECC71, "Emerald"),
    (0xE67E22, "Carrot"),
    (0x3498DB, "Peter River"),
    (0xE74C3C, "Alizarin"),
    (0x9B59B6, "Wisteria"),
];

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

const SWATCHES: [(u8, u8, u8); 16] = [
    (0xFF, 0x33, 0x33),
    (0xFF, 0x8C, 0x00),
    (0xFF, 0xD7, 0x00),
    (0x00, 0xCC, 0x66),
    (0x00, 0x88, 0xFF),
    (0x66, 0x33, 0xFF),
    (0xCC, 0x33, 0xCC),
    (0x00, 0xBC, 0xD4),
    (0xF1, 0xC4, 0x0F),
    (0x2E, 0xCC, 0x71),
    (0xE6, 0x7E, 0x22),
    (0x34, 0x98, 0xDB),
    (0xE7, 0x4C, 0x3C),
    (0x9B, 0x59, 0xB6),
    (0x00, 0x00, 0x00),
    (0xFF, 0xFF, 0xFF),
];
fn gdi_color(r: u8, g: u8, b: u8) -> u32 {
    (0xFFu32 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

static mut G_GDI_TOKEN: usize = 0;

// ── App state ──
#[derive(Clone)]
struct AppState {
    dpi: i32,
    // drag tracking
    mouse_down: bool,
    drag_mode: i32, // 0=none, 1=wheel, 2=brightness
    // name input
    name_buf: [u16; 64],
    name_pos: usize,
    // hex input
    hex_buf: [u16; 8],
    hex_pos: usize,
    focus_field: i32, // 0=none, 1=name, 2=hex
    // timer
    status_timer: i32,
    status_cnt: i32,
    // layout
    l_wheel_sz: i32,
    l_swatch_sz: i32,
    l_swatch_gap: i32,
    l_bright_h: i32,
    l_preview_sz: i32,
    l_btn_h: i32,
    l_btn_sm: i32,
    l_name_h: i32,
    l_hex_h: i32,
    l_bright_y: i32,
    l_preview_y: i32,
    l_name_y: i32,
    l_add_y: i32,
    l_browse_y: i32,
    rw: i32,
    lib_scroll_right: i32,
    // color editing
    hue: f32,
    sat: f32,
    val: f32,
    sel_swatch: i32,
    sel_lib: i32, // selected library entry (-1 = none)
    // wheel indicator position
    wheel_cx: i32,
    wheel_cy: i32,
    wheel_set: bool,
    // folder
    folder_path: [u16; 260],
    status: [u16; 128],
    browse_ok: bool,
    ctx_menu: bool,
    // wheel bitmap cache
    wheel_bitmap: *mut GpBitmap,
    wheel_graphics: *mut GpGraphics,
    wheel_size: i32,
    wheel_brightness: f32,
    // GDI objects
    tf: HFONT,
    bf: HFONT,
    sf: HFONT,
    back_brush: HBRUSH,
    panel_brush: HBRUSH,
    card_brush: HBRUSH,
    focus_pen: HPEN,
    // back buffer
    buf_bmp: HBITMAP,
    buf_dc: HDC,
    buf_w: i32,
    buf_h: i32,
    // window
    hwnd: HWND,
    // color library (loaded from file)
    library: Vec<u32>, // stored as 0xRRGGBB
    lib_names: Vec<[u16; 64]>,
    lib_scroll: i32,
    // user-editable preset colors
    presets: [u32; 16],
    presets_dirty: bool,
}

impl AppState {
    fn new() -> Self {
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
}

fn lib_file() -> String {
    format!(
        "{}\\FolderColorizerRust\\library.txt",
        std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".into())
    )
}

unsafe fn load_library(a: &mut AppState) {
    let path = lib_file();
    let data = match std::fs::read_to_string(&path) {
        std::result::Result::Ok(s) => s,
        std::result::Result::Err(_) => return,
    };
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (hex, name) = if let Some(pos) = line.find('|') {
            (&line[..pos], Some(&line[pos + 1..]))
        } else {
            (line, None)
        };
        if hex.len() != 6 {
            continue;
        }
        let w: Vec<u16> = hex.encode_utf16().collect();
        if let Some((r, g, b)) = parse_hex(&w) {
            a.library.push(rgb(r, g, b) & 0xFFFFFF);
            let mut nb = [0u16; 64];
            if let Some(n) = name {
                let mut j = 0;
                for c in n.encode_utf16() {
                    if j < 63 {
                        nb[j] = c;
                        j += 1;
                    }
                }
            }
            a.lib_names.push(nb);
        }
    }
}

fn preset_file() -> String {
    format!(
        "{}\\FolderColorizerRust\\presets.txt",
        std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".into())
    )
}

unsafe fn load_presets(a: &mut AppState) {
    let path = preset_file();
    let data = match std::fs::read_to_string(&path) {
        std::result::Result::Ok(s) => s,
        std::result::Result::Err(_) => return,
    };
    let mut i = 0;
    for line in data.lines() {
        let line = line.trim();
        if line.len() != 6 || i >= 16 {
            continue;
        }
        let w: Vec<u16> = line.encode_utf16().collect();
        if let Some((r, g, b)) = parse_hex(&w) {
            a.presets[i] = rgb(r, g, b) & 0xFFFFFF;
            i += 1;
        }
    }
}

unsafe fn save_presets(a: &AppState) {
    let path = preset_file();
    let _ = std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap());
    let mut s = String::new();
    for c in &a.presets {
        s.push_str(&format!("{:06X}\n", c));
    }
    let _ = std::fs::write(&path, &s);
}

fn lib_name(a: &AppState, i: usize) -> String {
    if i < a.lib_names.len() {
        let buf = &a.lib_names[i];
        let mut s = String::new();
        for c in buf.iter().take_while(|&&c| c != 0) {
            if let Some(ch) = char::from_u32(*c as u32) {
                s.push(ch);
            }
        }
        s
    } else {
        String::new()
    }
}

unsafe fn save_library(a: &AppState) {
    let path = lib_file();
    let _ = std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap());
    let mut s = String::new();
    for (i, c) in a.library.iter().enumerate() {
        s.push_str(&format!("{:06X}|{}\n", c, lib_name(a, i)));
    }
    let _ = std::fs::write(&path, &s);
}

fn scale_by(v: i32, dpi: i32) -> i32 {
    // ponytail: inline MulDiv to avoid hunting the crate feature
    (v * dpi + 48) / 96
}

fn sys_dpi() -> i32 {
    unsafe {
        let hdc = GetDC(None);
        let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSY);
        let _ = ReleaseDC(None, hdc);
        dpi
    }
}

// ── Hex parsing ──
fn parse_hex(s: &[u16]) -> Option<(u8, u8, u8)> {
    fn hx(c: u16) -> Option<u8> {
        match c {
            0x30..=0x39 => Some((c - 0x30) as u8),
            0x41..=0x46 => Some((c - 0x41 + 10) as u8),
            0x61..=0x66 => Some((c - 0x61 + 10) as u8),
            _ => None,
        }
    }
    if s.len() < 6 {
        return None;
    }
    Some((
        hx(s[0])? << 4 | hx(s[1])?,
        hx(s[2])? << 4 | hx(s[3])?,
        hx(s[4])? << 4 | hx(s[5])?,
    ))
}

// ── HSV <-> RGB ──
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let hi = (h * 6.0).floor() as i32 % 6;
    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

// ── Cache dir ──
fn get_cache_dir() -> String {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:".to_string());
        format!("{}\\AppData\\Local", home)
    });
    format!("{}\\FolderColorizerRust\\cache", local)
}

// ── Folder icon rendering via GDI+ ──
unsafe fn render_folder_bitmap(gr: *mut GpGraphics, w: i32, h: i32, r: u8, g: u8, b: u8) {
    let margin = (w as f32 * 0.078125) as i32; // ~20/256
    let tab_w = (w as f32 * 0.195) as i32;
    let tab_h = (h as f32 * 0.098) as i32;
    let body_h = (h as f32 * 0.766) as i32;
    let body_y = margin + tab_h;
    let shade_h = (h as f32 * 0.078) as i32;
    let shade_y = margin + body_h - shade_h;
    let tab_hl_h = (h as f32 * 0.039) as i32;

    let mut body: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut body);
    if !body.is_null() {
        GdipAddPathRectangleI(body, margin, body_y, w - margin * 2, body_h - margin);
        let mut brush: *mut GpSolidFill = null_mut();
        GdipCreateSolidFill(gdi_color(r, g, b), &mut brush);
        if !brush.is_null() {
            GdipFillPath(gr, brush as *mut GpBrush, body);
            // Shade bottom
            let mut shade: *mut GpSolidFill = null_mut();
            GdipCreateSolidFill(
                gdi_color(
                    (r as f32 * 0.5) as u8,
                    (g as f32 * 0.5) as u8,
                    (b as f32 * 0.5) as u8,
                ),
                &mut shade,
            );
            if !shade.is_null() {
                GdipFillRectangleI(
                    gr,
                    shade as *mut GpBrush,
                    margin,
                    shade_y,
                    w - margin * 2,
                    shade_h,
                );
                GdipDeleteBrush(shade as *mut GpBrush);
            }
            GdipDeleteBrush(brush as *mut GpBrush);
        }
        GdipDeletePath(body);
    }

    // Tab
    let mut tab: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut tab);
    if !tab.is_null() {
        GdipAddPathRectangleI(tab, margin, margin, tab_w, tab_h);
        let mut tbrush: *mut GpSolidFill = null_mut();
        let tr = ((r as i32 + (255 - r as i32) / 2).min(255)) as u8;
        let tg = ((g as i32 + (255 - g as i32) / 2).min(255)) as u8;
        let tb = ((b as i32 + (255 - b as i32) / 2).min(255)) as u8;
        GdipCreateSolidFill(gdi_color(tr, tg, tb), &mut tbrush);
        if !tbrush.is_null() {
            GdipFillPath(gr, tbrush as *mut GpBrush, tab);
            GdipDeleteBrush(tbrush as *mut GpBrush);
        }
        // Tab highlight strip
        let mut hl: *mut GpSolidFill = null_mut();
        GdipCreateSolidFill(
            gdi_color(
                255.max(tr + 30).min(255) as u8,
                255.max(tg + 30).min(255) as u8,
                255.max(tb + 30).min(255) as u8,
            ),
            &mut hl,
        );
        if !hl.is_null() {
            GdipFillRectangleI(gr, hl as *mut GpBrush, margin, margin, tab_w, tab_hl_h);
            GdipDeleteBrush(hl as *mut GpBrush);
        }
        GdipDeletePath(tab);
    }
}

// ── Multi-res ICO generation (16, 32, 48, 256) ──
unsafe fn generate_ico(path: &str, r: u8, g: u8, b: u8) -> bool {
    use std::io::Write;
    let sizes: &[i32] = &[16, 32, 48, 256];
    let mut entries: Vec<(Vec<u8>, u32)> = Vec::new();

    for &sz in sizes {
        let mut bmp: *mut GpBitmap = null_mut();
        if GdipCreateBitmapFromScan0(sz, sz, 0, PIXEL_FORMAT_32BPP_ARGB, None, &mut bmp).0 != 0
            || bmp.is_null()
        {
            continue;
        }
        let mut gr: *mut GpGraphics = null_mut();
        GdipGetImageGraphicsContext(bmp as *mut GpImage, &mut gr);
        if gr.is_null() {
            GdipDisposeImage(bmp as *mut GpImage);
            continue;
        }
        GdipSetSmoothingMode(gr, SmoothingModeAntiAlias);
        GdipSetPixelOffsetMode(gr, PixelOffsetModeHighQuality);

        render_folder_bitmap(gr, sz, sz, r, g, b);
        GdipDeleteGraphics(gr);

        // Lock bits
        let rect = Rect {
            X: 0,
            Y: 0,
            Width: sz,
            Height: sz,
        };
        let mut data = vec![0u8; (sz * 4 * sz) as usize];
        let mut bmd = BitmapData::default();
        if GdipBitmapLockBits(bmp, &rect, 1u32, PIXEL_FORMAT_32BPP_ARGB, &mut bmd).0 == 0
            && !bmd.Scan0.is_null()
        {
            let src = std::slice::from_raw_parts(bmd.Scan0 as *const u8, (sz * 4 * sz) as usize);
            let stride = bmd.Stride;
            for y in 0..sz {
                for x in 0..sz {
                    let si = (y * stride + x * 4) as usize;
                    let di = (y * sz + x) as usize * 4;
                    data[di + 0] = src[si + 2];
                    data[di + 1] = src[si + 1];
                    data[di + 2] = src[si + 0];
                    data[di + 3] = src[si + 3];
                }
            }
            GdipBitmapUnlockBits(bmp, &mut bmd);
        }
        GdipDisposeImage(bmp as *mut GpImage);

        // Encode as PNG
        let mut png = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        let _ = encoder.write_image(&data, sz as u32, sz as u32, image::ColorType::Rgba8);
        entries.push((png, sz as u32));
    }

    if entries.is_empty() {
        return false;
    }

    let mut f = match std::fs::File::create(path) {
        std::result::Result::Ok(f) => f,
        std::result::Result::Err(_) => return false,
    };

    let count = entries.len() as u16;
    let header_sz = 6 + count as u32 * 16;
    let _ = f.write_all(&0u16.to_le_bytes());
    let _ = f.write_all(&1u16.to_le_bytes());
    let _ = f.write_all(&count.to_le_bytes());

    let mut offset = header_sz;
    for (png_data, sz) in &entries {
        let w = if *sz >= 256 { 0u8 } else { *sz as u8 };
        let h = if *sz >= 256 { 0u8 } else { *sz as u8 };
        let _ = f.write_all(&[w, h]); // dimensions
        let _ = f.write_all(&[0, 0]); // colors, reserved
        let _ = f.write_all(&1u16.to_le_bytes()); // planes
        let _ = f.write_all(&32u16.to_le_bytes()); // bpp
        let _ = f.write_all(&(png_data.len() as u32).to_le_bytes());
        let _ = f.write_all(&offset.to_le_bytes());
        offset += png_data.len() as u32;
    }

    for (png_data, _) in &entries {
        let _ = f.write_all(&png_data);
    }
    true
}

// ── Apply / Reset folder ──
unsafe fn apply_folder_color(folder: &str, r: u8, g: u8, b: u8) -> bool {
    let cache = get_cache_dir();
    let _ = std::fs::create_dir_all(&cache);

    let hex = format!("{:02X}{:02X}{:02X}", r, g, b);
    let ico_path = format!("{}\\{}.ico", cache, hex);

    if !std::path::Path::new(&ico_path).exists() {
        if !generate_ico(&ico_path, r, g, b) {
            generate_simple_ico(&ico_path, r, g, b);
        }
    }

    let ini = format!("[.ShellClassInfo]\r\nIconResource={},0\r\n", ico_path);
    let ini_path = format!("{}\\desktop.ini", folder);
    let _ = std::fs::write(&ini_path, &ini);

    let ini_w: Vec<u16> = std::ffi::OsStr::new(&ini_path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let _ = SetFileAttributesW(
        windows::core::PCWSTR(ini_w.as_ptr()),
        FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM,
    );
    let folder_w: Vec<u16> = std::ffi::OsStr::new(&folder)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // Only add SYSTEM flag, preserve existing attrs (matching C++ behavior)
    let attr_val = GetFileAttributesW(windows::core::PCWSTR(folder_w.as_ptr()));
    let sys = FILE_ATTRIBUTE_SYSTEM.0;
    if attr_val != INVALID_FILE_ATTRIBUTES {
        if attr_val & sys == 0 {
            let new_a = attr_val | sys;
            let _ = SetFileAttributesW(
                windows::core::PCWSTR(folder_w.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(new_a),
            );
        }
    } else {
        let _ = SetFileAttributesW(
            windows::core::PCWSTR(folder_w.as_ptr()),
            FILE_ATTRIBUTE_SYSTEM,
        );
    }
    true
}

fn generate_simple_ico(path: &str, r: u8, g: u8, b: u8) -> bool {
    use std::io::Write;
    fn draw_folder(img: &mut image::RgbaImage, sz: u32, r: u8, g: u8, b: u8) {
        let margin = (sz as f32 * 0.078125) as u32;
        let tab_w = (sz as f32 * 0.195) as u32;
        let tab_h = (sz as f32 * 0.098) as u32;
        let body_h = (sz as f32 * 0.766) as u32;
        let body_y = margin + tab_h;
        let shade_h = (sz as f32 * 0.078) as u32;
        let shade_y = margin + body_h - shade_h;
        let body_end = margin + body_h - margin;
        let body_right = sz - margin;
        for y in 0..sz {
            for x in 0..sz {
                let in_tab = y >= margin && y < margin + tab_h && x >= margin && x < margin + tab_w;
                let in_body =
                    y >= body_y && y < body_y + body_h - margin && x >= margin && x < sz - margin;
                let in_shade = in_body && y >= shade_y;
                let in_tab_hl = in_tab && y < margin + (sz as f32 * 0.039) as u32;
                if in_body {
                    let c = if in_shade {
                        image::Rgba([
                            (r as f32 * 0.6) as u8,
                            (g as f32 * 0.6) as u8,
                            (b as f32 * 0.6) as u8,
                            255,
                        ])
                    } else {
                        image::Rgba([r, g, b, 255])
                    };
                    img.put_pixel(x, y, c);
                } else if in_tab {
                    let tr = ((r as i32 + (255 - r as i32) / 2).min(255)) as u8;
                    let tg = ((g as i32 + (255 - g as i32) / 2).min(255)) as u8;
                    let tb = ((b as i32 + (255 - b as i32) / 2).min(255)) as u8;
                    let c = if in_tab_hl {
                        image::Rgba([
                            255.max(tr + 30).min(255) as u8,
                            255.max(tg + 30).min(255) as u8,
                            255.max(tb + 30).min(255) as u8,
                            255,
                        ])
                    } else {
                        image::Rgba([tr, tg, tb, 255])
                    };
                    img.put_pixel(x, y, c);
                }
            }
        }
    }

    let sizes: &[u32] = &[16, 32, 48, 256];
    let mut entries: Vec<(Vec<u8>, u32)> = Vec::new();
    for &sz in sizes {
        let mut img = image::RgbaImage::new(sz, sz);
        draw_folder(&mut img, sz, r, g, b);
        let mut png = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        let _ = encoder.write_image(&img, sz, sz, image::ColorType::Rgba8);
        entries.push((png, sz));
    }

    let mut f = match std::fs::File::create(path) {
        std::result::Result::Ok(f) => f,
        std::result::Result::Err(_) => return false,
    };
    let count = entries.len() as u16;
    let header_sz = 6 + count as u32 * 16;
    let _ = f.write_all(&0u16.to_le_bytes());
    let _ = f.write_all(&1u16.to_le_bytes());
    let _ = f.write_all(&count.to_le_bytes());
    let mut offset = header_sz;
    for (png_data, sz) in &entries {
        let w = if *sz >= 256 { 0u8 } else { *sz as u8 };
        let h = if *sz >= 256 { 0u8 } else { *sz as u8 };
        let _ = f.write_all(&[w, h]);
        let _ = f.write_all(&[0, 0]);
        let _ = f.write_all(&1u16.to_le_bytes());
        let _ = f.write_all(&32u16.to_le_bytes());
        let _ = f.write_all(&(png_data.len() as u32).to_le_bytes());
        let _ = f.write_all(&offset.to_le_bytes());
        offset += png_data.len() as u32;
    }
    for (png_data, _) in &entries {
        let _ = f.write_all(&png_data);
    }
    true
}

unsafe fn reset_folder_color(folder: &str) -> bool {
    let ini_path = format!("{}\\desktop.ini", folder);
    let _ = std::fs::remove_file(&ini_path);
    let folder_w: Vec<u16> = std::ffi::OsStr::new(&folder)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let _ = SetFileAttributesW(
        windows::core::PCWSTR(folder_w.as_ptr()),
        FILE_ATTRIBUTE_NORMAL,
    );
    true
}

// ── Context menu ──
unsafe fn install_context_menu(a: &AppState) {
    uninstall_context_menu();
    let exe = get_exe_path();
    let cache = get_cache_dir();
    let _ = std::fs::create_dir_all(&cache);

    // Root key
    let root_w: Vec<u16> = REG_ROOT.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk = HKEY::default();
    RegCreateKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(root_w.as_ptr()),
        &mut hk,
    );
    if hk.is_invalid() {
        return;
    }

    let muiverb: Vec<u16> = "Colorize\0".encode_utf16().collect();
    let _ = RegSetValueExW(
        hk,
        windows::core::w!("MUIVerb"),
        Some(0),
        REG_SZ,
        Some(cast_bytes(&muiverb)),
    );
    let _ = RegSetValueExW(
        hk,
        windows::core::w!("SubCommands"),
        Some(0),
        REG_SZ,
        Some(&[0u8; 2]),
    );
    // Root icon
    let menu_ico = format!("{}\\menu.ico", cache);
    if !std::path::Path::new(&menu_ico).exists() {
        generate_ico(&menu_ico, 0x34, 0x98, 0xDB);
    }
    let ico_w: Vec<u16> = std::ffi::OsStr::new(&menu_ico)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let _ = RegSetValueExW(
        hk,
        windows::core::w!("Icon"),
        Some(0),
        REG_SZ,
        Some(cast_bytes(&ico_w)),
    );
    let _ = RegCloseKey(hk);

    // ShellFolder
    let sfk = format!("{}\\ShellFolder", REG_ROOT);
    let sfk_w: Vec<u16> = sfk.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk_sf = HKEY::default();
    RegCreateKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(sfk_w.as_ptr()),
        &mut hk_sf,
    );
    if !hk_sf.is_invalid() {
        let attr = 0xa0000000u32;
        let _ = RegSetValueExW(
            hk_sf,
            windows::core::w!("Attributes"),
            Some(0),
            REG_DWORD,
            Some(&attr.to_le_bytes()),
        );
        let _ = RegCloseKey(hk_sf);
    }

    // Colors
    for (i, (hex, name)) in COLORS.iter().enumerate() {
        let kn = format!("{:02}-{}", i + 1, name);
        let r = ((hex >> 16) & 0xFF) as u8;
        let g = ((hex >> 8) & 0xFF) as u8;
        let b = (*hex & 0xFF) as u8;

        let ico_file = format!("{}\\q{}.ico", cache, i);
        if !std::path::Path::new(&ico_file).exists() {
            generate_ico(&ico_file, r, g, b);
        }

        let color_key = format!("{}\\shell\\{}", REG_ROOT, kn);
        let ck_w: Vec<u16> = color_key.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hk_c = HKEY::default();
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(ck_w.as_ptr()),
            &mut hk_c,
        );
        if !hk_c.is_invalid() {
            let nw: Vec<u16> = format!("{}\0", name).encode_utf16().collect();
            let _ = RegSetValueExW(
                hk_c,
                windows::core::w!("MUIVerb"),
                Some(0),
                REG_SZ,
                Some(cast_bytes(&nw)),
            );
            let iw: Vec<u16> = std::ffi::OsStr::new(&ico_file)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let _ = RegSetValueExW(
                hk_c,
                windows::core::w!("Icon"),
                Some(0),
                REG_SZ,
                Some(cast_bytes(&iw)),
            );
            let _ = RegCloseKey(hk_c);

            let cmd_key = format!("{}\\shell\\{}\\command", REG_ROOT, kn);
            let ck2_w: Vec<u16> = cmd_key.encode_utf16().chain(std::iter::once(0)).collect();
            let mut hk_cmd = HKEY::default();
            RegCreateKeyW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(ck2_w.as_ptr()),
                &mut hk_cmd,
            );
            if !hk_cmd.is_invalid() {
                let hex_str = format!("{:02X}{:02X}{:02X}", r, g, b);
                let cmd_val = format!("\"{}\" --apply \"%1\" {}\0", exe, hex_str);
                let cv_w: Vec<u16> = cmd_val.encode_utf16().collect();
                let _ = RegSetValueExW(hk_cmd, None, Some(0), REG_SZ, Some(cast_bytes(&cv_w)));
                let _ = RegCloseKey(hk_cmd);
            }
        }
    }

    // Separator 08
    let sep1 = format!("{}\\shell\\08-Separator", REG_ROOT);
    write_separator(&sep1);

    // Restore 09
    let rk = format!("{}\\shell\\09-Restore", REG_ROOT);
    let rk_w: Vec<u16> = rk.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk_r = HKEY::default();
    RegCreateKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(rk_w.as_ptr()),
        &mut hk_r,
    );
    if !hk_r.is_invalid() {
        let rv: Vec<u16> = "Restore original color\0".encode_utf16().collect();
        let _ = RegSetValueExW(
            hk_r,
            windows::core::w!("MUIVerb"),
            Some(0),
            REG_SZ,
            Some(cast_bytes(&rv)),
        );
        let _ = RegCloseKey(hk_r);
        let rck = format!("{}\\command", rk);
        let rck_w: Vec<u16> = rck.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hk_rc = HKEY::default();
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(rck_w.as_ptr()),
            &mut hk_rc,
        );
        if !hk_rc.is_invalid() {
            let cv = format!("\"{}\" --reset \"%1\"\0", exe);
            let cv_w: Vec<u16> = cv.encode_utf16().collect();
            let _ = RegSetValueExW(hk_rc, None, Some(0), REG_SZ, Some(cast_bytes(&cv_w)));
            let _ = RegCloseKey(hk_rc);
        }
    }

    // Separator 10
    let sep2 = format!("{}\\shell\\10-Separator", REG_ROOT);
    write_separator(&sep2);

    // Colors... 11
    let cak = format!("{}\\shell\\11-Colors", REG_ROOT);
    let cak_w: Vec<u16> = cak.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk_ca = HKEY::default();
    RegCreateKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(cak_w.as_ptr()),
        &mut hk_ca,
    );
    if !hk_ca.is_invalid() {
        let cv: Vec<u16> = "Colors...\0".encode_utf16().collect();
        let _ = RegSetValueExW(
            hk_ca,
            windows::core::w!("MUIVerb"),
            Some(0),
            REG_SZ,
            Some(cast_bytes(&cv)),
        );
        let iw: Vec<u16> = std::ffi::OsStr::new(&menu_ico)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let _ = RegSetValueExW(
            hk_ca,
            windows::core::w!("Icon"),
            Some(0),
            REG_SZ,
            Some(cast_bytes(&iw)),
        );
        let _ = RegCloseKey(hk_ca);
        let cck = format!("{}\\command", cak);
        let cck_w: Vec<u16> = cck.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hk_cc = HKEY::default();
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(cck_w.as_ptr()),
            &mut hk_cc,
        );
        if !hk_cc.is_invalid() {
            let cv2 = format!("\"{}\" \"%1\"\0", exe);
            let cv2_w: Vec<u16> = cv2.encode_utf16().collect();
            let _ = RegSetValueExW(hk_cc, None, Some(0), REG_SZ, Some(cast_bytes(&cv2_w)));
            let _ = RegCloseKey(hk_cc);
        }
    }

    let _ = SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_FLUSH, None, None);
}

unsafe fn write_separator(key: &str) {
    let sep_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk = HKEY::default();
    RegCreateKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(sep_w.as_ptr()),
        &mut hk,
    );
    if !hk.is_invalid() {
        let flags = 0x00000008u32;
        let _ = RegSetValueExW(
            hk,
            windows::core::w!("CommandFlags"),
            Some(0),
            REG_DWORD,
            Some(&flags.to_le_bytes()),
        );
        let _ = RegCloseKey(hk);
    }
}

unsafe fn uninstall_context_menu() {
    fn del_tree(parent: HKEY, path: &[u16]) {
        unsafe {
            let mut hk = HKEY::default();
            if RegOpenKeyW(parent, windows::core::PCWSTR(path.as_ptr()), &mut hk) == WIN32_ERROR(0)
            {
                loop {
                    let mut name = [0u16; 256];
                    let mut len = 256u32;
                    if RegEnumKeyExW(
                        hk,
                        0,
                        Some(windows::core::PWSTR(name.as_mut_ptr())),
                        &mut len,
                        None,
                        None::<windows::core::PWSTR>,
                        None,
                        None::<*mut FILETIME>,
                    )
                    .is_err()
                    {
                        break;
                    }
                    let sub = name[..len as usize].to_vec();
                    let mut full = path.to_vec();
                    if full.last() == Some(&0) {
                        full.pop();
                    }
                    full.push('\\' as u16);
                    full.extend_from_slice(&sub);
                    full.push(0);
                    del_tree(parent, &full);
                }
                let _ = RegCloseKey(hk);
            }
            let _ = RegDeleteKeyW(parent, windows::core::PCWSTR(path.as_ptr()));
        }
    }
    let root_w: Vec<u16> = REG_ROOT.encode_utf16().chain(std::iter::once(0)).collect();
    del_tree(HKEY_CURRENT_USER, &root_w);
    let _ = SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_FLUSH, None, None);
}

unsafe fn is_context_menu_installed() -> bool {
    let root_w: Vec<u16> = REG_ROOT.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk = HKEY::default();
    RegOpenKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(root_w.as_ptr()),
        &mut hk,
    ) == WIN32_ERROR(0)
}

fn get_exe_path() -> String {
    let mut buf = [0u16; 260];
    unsafe {
        GetModuleFileNameW(None, &mut buf);
    }
    String::from_utf16_lossy(&buf)
        .trim_end_matches(char::from(0))
        .to_string()
}

fn cast_bytes(v: &[u16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) }
}

unsafe fn set_status(a: &mut AppState, msg: &str) {
    let w: Vec<u16> = format!("{}\0", msg).encode_utf16().collect();
    for (i, &c) in w.iter().enumerate() {
        if i < 127 {
            a.status[i] = c;
        }
    }
    a.status[127] = 0;
    // Auto-fade after ~5 seconds using 100ms interval (C++ approach)
    a.status_timer = 1; // timer ID
    a.status_cnt = 50; // 50 * 100ms = 5s
    let _ = SetTimer(Some(a.hwnd), a.status_timer as usize, 100, None);
}

// ── Layout ──
unsafe fn layout_init(a: &mut AppState) {
    a.l_wheel_sz = scale_by(190, a.dpi);
    a.l_swatch_sz = scale_by(28, a.dpi);
    a.l_swatch_gap = scale_by(6, a.dpi);
    a.l_bright_h = scale_by(22, a.dpi);
    a.l_preview_sz = scale_by(72, a.dpi);
    a.l_btn_h = scale_by(36, a.dpi);
    a.l_btn_sm = scale_by(26, a.dpi);
    a.l_name_h = scale_by(26, a.dpi);
    a.l_hex_h = scale_by(26, a.dpi);
    a.l_bright_y = scale_by(259, a.dpi);
    a.l_preview_y = scale_by(310, a.dpi);
    a.l_name_y = scale_by(392, a.dpi);
    a.l_add_y = scale_by(540, a.dpi);
    a.l_browse_y = scale_by(590, a.dpi);
}

fn lx_w() -> i32 {
    280 // left panel width for library
}
fn rx(a: &AppState, x: i32) -> i32 {
    lx_w() + 16 + x // right panel starts after left panel + gap
}

// ── Render color wheel to GDI+ bitmap ──
unsafe fn render_wheel(a: &mut AppState) {
    let sz = a.l_wheel_sz;
    if sz <= 0 {
        return;
    }
    if a.wheel_brightness == a.val && !a.wheel_bitmap.is_null() {
        return;
    }

    // Create new GDI+ bitmap if needed
    if !a.wheel_bitmap.is_null() {
        GdipDisposeImage(a.wheel_bitmap as *mut GpImage);
        a.wheel_bitmap = null_mut();
    }
    if !a.wheel_graphics.is_null() {
        GdipDeleteGraphics(a.wheel_graphics);
        a.wheel_graphics = null_mut();
    }

    let status = GdipCreateBitmapFromScan0(
        sz,
        sz,
        0,
        PIXEL_FORMAT_32BPP_ARGB,
        None,
        &mut a.wheel_bitmap,
    );
    if status.0 != 0 || a.wheel_bitmap.is_null() {
        return;
    }

    GdipGetImageGraphicsContext(a.wheel_bitmap as *mut GpImage, &mut a.wheel_graphics);
    if a.wheel_graphics.is_null() {
        GdipDisposeImage(a.wheel_bitmap as *mut GpImage);
        a.wheel_bitmap = null_mut();
        return;
    }

    GdipSetSmoothingMode(a.wheel_graphics, SmoothingModeHighSpeed);

    let cx = sz as f32 / 2.0;
    let cy = sz as f32 / 2.0;
    let r = cx;
    let v = a.val;

    // Render wheel pixel by pixel
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
            let mut brush: *mut GpSolidFill = null_mut();
            GdipCreateSolidFill(gdi_color(r8, g8, b8), &mut brush);
            if !brush.is_null() {
                GdipFillRectangleI(
                    a.wheel_graphics,
                    brush as *mut GpBrush,
                    x as i32,
                    y as i32,
                    1,
                    1,
                );
                GdipDeleteBrush(brush as *mut GpBrush);
            }
        }
    }

    a.wheel_brightness = a.val;
    a.wheel_size = sz;
}

// ── Paint ──
unsafe fn paint(a: &mut AppState) {
    let mut ps = zeroed::<PAINTSTRUCT>();
    let hdc = BeginPaint(a.hwnd, &mut ps);
    let w = ps.rcPaint.right - ps.rcPaint.left;
    let h = ps.rcPaint.bottom - ps.rcPaint.top;

    // Back buffer
    if a.buf_dc.is_invalid() || a.buf_w != w || a.buf_h != h {
        if !a.buf_dc.is_invalid() {
            let _ = SelectObject(a.buf_dc, GetStockObject(DC_BRUSH));
            let _ = DeleteObject(a.buf_bmp.into());
            let _ = DeleteDC(a.buf_dc);
        }
        a.buf_dc = CreateCompatibleDC(Some(hdc));
        a.buf_bmp = CreateCompatibleBitmap(hdc, w, h);
        let _ = SelectObject(a.buf_dc, a.buf_bmp.into());
        a.buf_w = w;
        a.buf_h = h;
    }

    let dc = a.buf_dc;
    let rw = w;
    a.rw = rw;
    let rxs = rx(a, 0);

    // Background
    fill_rect(dc, 0, 0, w, h, a.back_brush);
    fill_rect(dc, 0, 0, lx_w(), h, a.panel_brush);

    // ── Left panel: COLOR LIBRARY with per-entry Apply + Remove ──
    let lib_lbl: Vec<u16> = "COLOR LIBRARY\0".encode_utf16().collect();
    draw_txt(
        dc,
        16,
        16,
        &lib_lbl,
        COLORREF(rgb(0xCC, 0xCC, 0xCC)),
        a.bf,
        lx_w() - 32,
    );
    let sub_lbl: Vec<u16> = "Apply to folder or remove\0".encode_utf16().collect();
    draw_txt(
        dc,
        16,
        38,
        &sub_lbl,
        COLORREF(rgb(0x88, 0x88, 0x88)),
        a.sf,
        lx_w() - 32,
    );

    let lib_card_h = scale_by(34, a.dpi);
    let lib_top = 56;
    let lib_h = h - lib_top;
    let lib_rows = if lib_card_h > 0 {
        lib_h / lib_card_h
    } else {
        0
    };
    let lx_cnt = a.library.len() as i32;
    if lx_cnt == 0 {
        let empty: Vec<u16> = "No saved colors yet\0".encode_utf16().collect();
        draw_txt(
            dc,
            16,
            52,
            &empty,
            COLORREF(rgb(0x66, 0x66, 0x66)),
            a.sf,
            lx_w() - 32,
        );
    } else {
        for i in 0..lx_cnt.min(lib_rows) {
            let ri = i + a.lib_scroll;
            if ri < 0 || ri >= lx_cnt {
                continue;
            }
            let sy = lib_top + i * lib_card_h;

            // card background
            let is_sel = a.sel_lib == ri;
            let card_bg = if is_sel {
                rgb(0x3A, 0x3D, 0x47)
            } else {
                rgb(0x2A, 0x2C, 0x34)
            };
            let card_b = CreateSolidBrush(COLORREF(card_bg));
            fill_rect(dc, 8, sy, lx_w() - 16, lib_card_h - 2, card_b);
            let _ = DeleteObject(card_b.into());

            // color swatch
            let c = a.library[ri as usize];
            let cr = (c >> 16) as u8;
            let cg = (c >> 8) as u8;
            let cb = c as u8;
            let sw_b = CreateSolidBrush(COLORREF(rgb(cr, cg, cb)));
            fill_rect(
                dc,
                14,
                sy + 5,
                scale_by(22, a.dpi),
                scale_by(22, a.dpi),
                sw_b,
            );
            let _ = DeleteObject(sw_b.into());

            // name text (wider gap from swatch)
            let nm = lib_name(a, ri as usize);
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
                COLORREF(rgb(0xCC, 0xCC, 0xCC)),
                a.bf,
                lx_w() - 170,
            );

            // Apply button
            let ap_x = lx_w() - 102;
            let ap_w = scale_by(44, a.dpi);
            let ap_h = scale_by(22, a.dpi);
            let ap_b = CreateSolidBrush(COLORREF(rgb(0x37, 0x3A, 0x43)));
            fill_rect(dc, ap_x, sy + 6, ap_w, ap_h, ap_b);
            let _ = DeleteObject(ap_b.into());
            let ap_pen = SelectObject(dc, GetStockObject(DC_PEN));
            SetDCPenColor(dc, COLORREF(rgb(0x55, 0x55, 0x55)));
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
            SetTextColor(dc, COLORREF(rgb(0xAA, 0xCC, 0xFF)));
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
            let rm_b = CreateSolidBrush(COLORREF(rgb(0x43, 0x2C, 0x2C)));
            fill_rect(dc, rm_x, sy + 6, rm_sz, rm_sz, rm_b);
            let _ = DeleteObject(rm_b.into());
            let rm_pen = SelectObject(dc, GetStockObject(DC_PEN));
            SetDCPenColor(dc, COLORREF(rgb(0x88, 0x44, 0x44)));
            SelectObject(dc, GetStockObject(NULL_BRUSH));
            Rectangle(dc, rm_x, sy + 6, rm_x + rm_sz, sy + 6 + rm_sz);
            SelectObject(dc, rm_pen);
            // ✕ as two crossing lines
            let x_pen = CreatePen(PS_SOLID, 2, COLORREF(rgb(0xFF, 0x66, 0x66)));
            let old_xp = SelectObject(dc, x_pen.into());
            MoveToEx(dc, rm_x + 5, sy + 6 + 5, None);
            LineTo(dc, rm_x + rm_sz - 5, sy + 6 + rm_sz - 5);
            MoveToEx(dc, rm_x + rm_sz - 5, sy + 6 + 5, None);
            LineTo(dc, rm_x + 5, sy + 6 + rm_sz - 5);
            SelectObject(dc, old_xp);
            DeleteObject(x_pen.into());
        }
    }

    // ── Right panel ──
    let ed_lbl: Vec<u16> = "COLOR EDITOR\0".encode_utf16().collect();
    draw_txt(
        dc,
        rxs,
        20,
        &ed_lbl,
        COLORREF(rgb(0xCC, 0xCC, 0xCC)),
        a.bf,
        300,
    );

    // Color wheel via GDI+
    render_wheel(a);
    if !a.wheel_bitmap.is_null() {
        // Draw GDI+ bitmap onto DC
        let mut gr: *mut GpGraphics = null_mut();
        GdipCreateFromHDC(dc, &mut gr);
        if !gr.is_null() {
            GdipDrawImageI(gr, a.wheel_bitmap as *mut GpImage, rxs, 56);
            GdipDeleteGraphics(gr);
        }
    }

    // Selection indicator on wheel (at stored click position)
    if a.wheel_set {
        let old_b = SelectObject(dc, GetStockObject(NULL_BRUSH));
        let ip = CreatePen(PS_SOLID, 2, COLORREF(rgb(0xFF, 0xFF, 0xFF)));
        let old_p = SelectObject(dc, ip.into());
        let _ = Rectangle(
            dc,
            a.wheel_cx - 6,
            a.wheel_cy - 6,
            a.wheel_cx + 6,
            a.wheel_cy + 6,
        );
        let _ = SelectObject(dc, old_p);
        let _ = DeleteObject(ip.into());
        let _ = SelectObject(dc, old_b);
        // inner dot
        let ib = CreateSolidBrush(COLORREF(rgb(0x3C, 0x3F, 0x48)));
        let old_b2 = SelectObject(dc, ib.into());
        let _ = Rectangle(
            dc,
            a.wheel_cx - 2,
            a.wheel_cy - 2,
            a.wheel_cx + 3,
            a.wheel_cy + 3,
        );
        let _ = SelectObject(dc, old_b2);
        let _ = DeleteObject(ib.into());
    }

    // Brightness bar (use GDI+ for gradient)
    let by = a.l_bright_y;
    let bw = a.l_wheel_sz;
    let bh = a.l_bright_h;
    let mut bgr: *mut GpGraphics = null_mut();
    GdipCreateFromHDC(dc, &mut bgr);
    if !bgr.is_null() {
        for bx in 0..bw {
            let t = bx as f32 / bw as f32;
            let (r8, g8, b8) = hsv_to_rgb(a.hue, a.sat, t);
            let mut brush: *mut GpSolidFill = null_mut();
            GdipCreateSolidFill(gdi_color(r8, g8, b8), &mut brush);
            if !brush.is_null() {
                GdipFillRectangleI(bgr, brush as *mut GpBrush, rxs + bx, by, 1, bh);
                GdipDeleteBrush(brush as *mut GpBrush);
            }
        }
        GdipDeleteGraphics(bgr);
    }
    // Brightness indicator
    let idx = (a.val * bw as f32) as i32;
    let old_p2 = SelectObject(dc, a.focus_pen.into());
    let _ = MoveToEx(dc, rxs + idx, by, None);
    let _ = LineTo(dc, rxs + idx, by + bh);
    let _ = SelectObject(dc, old_p2);

    // ── Preview + Name + Hex ──
    let py = a.l_preview_y;
    let psz = a.l_preview_sz;
    let (pr, pg, pb) = hsv_to_rgb(a.hue, a.sat, a.val);
    // Color preview swatch
    let prev_b = CreateSolidBrush(COLORREF(rgb(pr, pg, pb)));
    fill_rect(dc, rxs, py, psz, psz, prev_b);
    let _ = DeleteObject(prev_b.into());
    // White border around preview
    let old_pb = SelectObject(dc, GetStockObject(DC_PEN));
    let _ = SetDCPenColor(dc, COLORREF(rgb(0x55, 0x55, 0x55)));
    let _ = SelectObject(dc, GetStockObject(NULL_BRUSH));
    let _ = Rectangle(dc, rxs, py, rxs + psz, py + psz);
    let _ = SelectObject(dc, old_pb);

    // Name label + input
    let name_lbl: Vec<u16> = "Name\0".encode_utf16().collect();
    draw_txt(
        dc,
        rxs + psz + 16,
        py,
        &name_lbl,
        COLORREF(rgb(0x88, 0x88, 0x88)),
        a.sf,
        100,
    );
    let name_x = rxs + psz + 16;
    let name_y = py + 18;
    let name_w = rw - psz - 40;
    fill_rect(dc, name_x, name_y, name_w, a.l_name_h, a.card_brush);
    // Name text
    let name_str = if a.name_pos > 0 {
        let mut buf = vec![0u16; 64];
        for i in 0..a.name_pos.min(63) {
            buf[i] = a.name_buf[i];
        }
        buf[a.name_pos.min(63)] = 0;
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
        COLORREF(rgb(0xCC, 0xCC, 0xCC)),
        a.bf,
        name_w - 8,
    );

    // Hex label + input
    let hex_lbl: Vec<u16> = "Hex\0".encode_utf16().collect();
    let hex_y = name_y + a.l_name_h + 8;
    draw_txt(
        dc,
        name_x,
        hex_y,
        &hex_lbl,
        COLORREF(rgb(0x88, 0x88, 0x88)),
        a.sf,
        60,
    );
    let hex_sy = hex_y + 18;
    fill_rect(dc, name_x, hex_sy, name_w.min(90), a.l_hex_h, a.card_brush);
    let hex_label = if a.hex_pos > 0 {
        let mut buf = vec![0u16; 8];
        for i in 0..a.hex_pos.min(6) {
            buf[i] = a.hex_buf[i];
        }
        buf[a.hex_pos.min(6)] = 0;
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
        COLORREF(rgb(0xAA, 0xCC, 0xFF)),
        a.bf,
        name_w.min(90) - 8,
    );
    let input_end = hex_sy + a.l_hex_h;

    // ── 16 Presets to the right of the wheel ──
    let ps_x = rxs + a.l_wheel_sz + 12;
    let ps_y = 56;
    let ps_sz = a.l_swatch_sz;
    let ps_gap = a.l_swatch_gap;
    let ps_cols = 4;
    for idx in 0..16 {
        let row = idx / ps_cols;
        let col = idx % ps_cols;
        let sx = ps_x + col * (ps_sz + ps_gap);
        let sy = ps_y + row * (ps_sz + ps_gap);
        let c = a.presets[idx as usize];
        let cr = (c >> 16) as u8;
        let cg = (c >> 8) as u8;
        let cb = c as u8;
        let brush = CreateSolidBrush(COLORREF(rgb(cr, cg, cb)));
        fill_rect(dc, sx, sy, ps_sz, ps_sz, brush);
        let _ = DeleteObject(brush.into());
        if a.sel_swatch == idx as i32 {
            let old = SelectObject(dc, GetStockObject(NULL_BRUSH));
            let old_p = SelectObject(dc, a.focus_pen.into());
            let _ = Rectangle(dc, sx - 2, sy - 2, sx + ps_sz + 2, sy + ps_sz + 2);
            let _ = SelectObject(dc, old_p);
            let _ = SelectObject(dc, old);
        }
    }

    // ── Add to Library button ──
    let add_t: Vec<u16> = "+ Add Color to Library\0".encode_utf16().collect();
    draw_btn(dc, rxs, a.l_add_y, a.rw - rxs - 12, a.l_btn_h, &add_t);

    // ── Browse ──
    let br_t: Vec<u16> = "Browse\0".encode_utf16().collect();
    draw_btn(dc, rxs, a.l_browse_y, 90, a.l_btn_sm, &br_t);
    // Folder path
    let fp = String::from_utf16_lossy(&a.folder_path);
    if a.browse_ok && a.folder_path[0] != 0 {
        let fp_w: Vec<u16> = format!("{}\0", fp.trim_end_matches(char::from(0)))
            .encode_utf16()
            .collect();
        draw_txt(
            dc,
            rxs + 90 + 12,
            a.l_browse_y + 4,
            &fp_w,
            COLORREF(rgb(0x88, 0x88, 0x88)),
            a.sf,
            rw - 24 - 90 - 12,
        );
    }

    // ── Apply + Reset ──
    if a.browse_ok && a.folder_path[0] != 0 {
        let ap_t: Vec<u16> = "Apply\0".encode_utf16().collect();
        draw_btn(
            dc,
            rxs,
            a.l_browse_y + a.l_btn_sm + 8,
            70,
            a.l_btn_sm,
            &ap_t,
        );
        let rs_t: Vec<u16> = "Reset\0".encode_utf16().collect();
        draw_btn(
            dc,
            rxs + 76,
            a.l_browse_y + a.l_btn_sm + 8,
            70,
            a.l_btn_sm,
            &rs_t,
        );
    }

    // ── Context menu checkbox ──
    let cb_x = rxs + 90 + 12;
    let cs = 14;
    let cb_brush = CreateSolidBrush(if a.ctx_menu {
        COLORREF(rgb(0x34, 0x98, 0xDB))
    } else {
        COLORREF(rgb(0x33, 0x33, 0x33))
    });
    fill_rect(
        dc,
        cb_x,
        a.l_browse_y + (a.l_btn_sm - cs) / 2,
        cs,
        cs,
        cb_brush,
    );
    let _ = DeleteObject(cb_brush.into());
    let old_p3 = SelectObject(dc, GetStockObject(DC_PEN));
    let _ = SetDCPenColor(dc, COLORREF(rgb(0x66, 0x66, 0x66)));
    let _ = SelectObject(dc, GetStockObject(NULL_BRUSH));
    let _ = Rectangle(
        dc,
        cb_x,
        a.l_browse_y + (a.l_btn_sm - cs) / 2,
        cb_x + cs,
        a.l_browse_y + (a.l_btn_sm - cs) / 2 + cs,
    );
    let _ = SelectObject(dc, old_p3);
    if a.ctx_menu {
        let old_p4 = SelectObject(dc, GetStockObject(WHITE_PEN));
        let _ = MoveToEx(dc, cb_x + 3, a.l_browse_y + (a.l_btn_sm - cs) / 2 + 7, None);
        let _ = LineTo(dc, cb_x + 6, a.l_browse_y + (a.l_btn_sm - cs) / 2 + 10);
        let _ = LineTo(dc, cb_x + 11, a.l_browse_y + (a.l_btn_sm - cs) / 2 + 4);
        let _ = SelectObject(dc, old_p4);
    }
    let cb_lbl: Vec<u16> = "Add to right-click menu\0".encode_utf16().collect();
    draw_txt(
        dc,
        cb_x + cs + 8,
        a.l_browse_y + 4,
        &cb_lbl,
        COLORREF(rgb(0x88, 0x88, 0x88)),
        a.sf,
        rw - cb_x - cs - 24,
    );

    // ── Status ──
    if a.status[0] != 0 {
        let st = String::from_utf16_lossy(&a.status);
        let st_w: Vec<u16> = format!("{}\0", st.trim_end_matches(char::from(0)))
            .encode_utf16()
            .collect();
        draw_txt(
            dc,
            rxs,
            a.l_browse_y + a.l_btn_sm * 2 + 16,
            &st_w,
            COLORREF(rgb(0x00, 0xCC, 0x66)),
            a.bf,
            rw - 48,
        );
    }

    // Blit
    let _ = BitBlt(hdc, 0, 0, w, h, Some(dc), 0, 0, SRCCOPY);
    EndPaint(a.hwnd, &mut ps);
}

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
    let btn_b = CreateSolidBrush(COLORREF(rgb(0x37, 0x3A, 0x43)));
    fill_rect(hdc, x, y, w, h, btn_b);
    let _ = DeleteObject(btn_b.into());
    let mut r = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    let old_pen = SelectObject(hdc, GetStockObject(DC_PEN));
    let _ = SetDCPenColor(hdc, COLORREF(rgb(0x55, 0x55, 0x55)));
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = Rectangle(hdc, x, y, x + w, y + h);
    let _ = SelectObject(hdc, old_brush);
    let _ = SelectObject(hdc, old_pen);
    let _ = SetTextColor(hdc, COLORREF(rgb(0xCC, 0xCC, 0xCC)));
    let _ = SetBkMode(hdc, TRANSPARENT);
    let mut text_mut: Vec<u16> = text.to_vec();
    let _ = DrawTextW(
        hdc,
        &mut text_mut,
        &mut r,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

unsafe fn commit_hex(a: &mut AppState) {
    let h = &a.hex_buf[..a.hex_pos.min(6)];
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
        a.hue = hue / 360.0;
        a.sat = s;
        a.val = max;
        a.sel_swatch = -1;
        a.wheel_set = false;
        a.wheel_brightness = -1.0;
    }
    a.hex_pos = 0;
    a.drag_mode = 0;
}

// ── Window proc ──
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
            WM_CREATE => {
                let dpi = sys_dpi();
                a.dpi = dpi;
                layout_init(a);

                a.tf = CreateFontW(
                    -scale_by(20, a.dpi),
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
                a.bf = CreateFontW(
                    -scale_by(16, a.dpi),
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
                a.sf = CreateFontW(
                    -scale_by(13, a.dpi),
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

                a.back_brush = CreateSolidBrush(COLORREF(rgb(0x1E, 0x1E, 0x1E)));
                a.panel_brush = CreateSolidBrush(COLORREF(rgb(0x25, 0x25, 0x25)));
                a.card_brush = CreateSolidBrush(COLORREF(rgb(0x2D, 0x2D, 0x2D)));
                a.focus_pen = CreatePen(PS_SOLID, 2, COLORREF(rgb(0xFF, 0xFF, 0xFF)));

                a.hue = 0.58;
                a.sat = 0.7;
                a.val = 0.8;
                a.ctx_menu = is_context_menu_installed();
                load_library(a);
                load_presets(a);
                LRESULT(0)
            }

            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => LRESULT(0),

            WM_PAINT => {
                paint(a);
                LRESULT(0)
            }

            WM_LBUTTONDOWN => {
                let mx = (lparam.0 as u16) as i32;
                let my = (lparam.0 >> 16) as i32;
                let rxs = rx(a, 0);
                // Wheel
                if mx >= rxs && mx < rxs + a.l_wheel_sz && my >= 56 && my < 56 + a.l_wheel_sz {
                    let cx = (rxs + a.l_wheel_sz / 2) as f32;
                    let cy = (56 + a.l_wheel_sz / 2) as f32;
                    let dx = mx as f32 - cx;
                    let dy = my as f32 - cy;
                    let dist = (dx * dx + dy * dy).sqrt() / (a.l_wheel_sz as f32 / 2.0);
                    if dist <= 1.0 {
                        let ang = (-dy).atan2(dx) / (2.0 * std::f32::consts::PI) + 0.25;
                        a.hue = ang - ang.floor();
                        a.sat = dist;
                        a.sel_swatch = -1;
                        a.sel_lib = -1;
                        a.name_pos = 0;
                        a.hex_pos = 0;
                        a.wheel_cx = mx;
                        a.wheel_cy = my;
                        a.wheel_set = true;
                        a.mouse_down = true;
                        a.drag_mode = 1;
                        let _ = InvalidateRect(Some(a.hwnd), None, false);
                    }
                    return LRESULT(0);
                }

                // Brightness bar
                if mx >= rxs
                    && mx < rxs + a.l_wheel_sz
                    && my >= a.l_bright_y
                    && my < a.l_bright_y + a.l_bright_h
                {
                    a.val = ((mx - rxs) as f32 / a.l_wheel_sz as f32).clamp(0.0, 1.0);
                    a.sel_swatch = -1;
                    a.sel_lib = -1;
                    a.name_pos = 0;
                    a.hex_pos = 0;
                    a.mouse_down = true;
                    a.drag_mode = 2;
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }

                // Library cards on left panel (swatch/name, Apply btn, Remove btn)
                let lib_card_h = scale_by(34, a.dpi);
                if mx < lx_w() {
                    for i in 0..a.library.len() as i32 {
                        let ri = i + a.lib_scroll;
                        if ri < 0 || ri >= a.library.len() as i32 {
                            continue;
                        }
                        let sy = 56 + i * lib_card_h;
                        if my < sy || my >= sy + lib_card_h - 2 {
                            continue;
                        }
                        // Remove button (✕, rightmost)
                        let rm_x = lx_w() - 102 + scale_by(44, a.dpi) + 4;
                        let rm_sz = scale_by(22, a.dpi);
                        if mx >= rm_x && mx < rm_x + rm_sz {
                            a.library.remove(ri as usize);
                            a.lib_names.remove(ri as usize);
                            save_library(a);
                            if a.sel_lib == ri {
                                a.sel_lib = -1;
                            }
                            let _ = InvalidateRect(Some(a.hwnd), None, false);
                            return LRESULT(0);
                        }
                        // Apply button (left of Remove)
                        let ap_x = lx_w() - 102;
                        let ap_w = scale_by(44, a.dpi);
                        if mx >= ap_x && mx < ap_x + ap_w {
                            if a.browse_ok && a.folder_path[0] != 0 {
                                let c = a.library[ri as usize];
                                let (r8, g8, b8) = ((c >> 16) as u8, (c >> 8) as u8, c as u8);
                                let folder = String::from_utf16_lossy(&a.folder_path);
                                let folder = folder.trim_end_matches(char::from(0));
                                if apply_folder_color(folder, r8, g8, b8) {
                                    set_status(a, "Applied to folder!");
                                }
                            } else {
                                set_status(a, "Browse a folder first");
                            }
                            let _ = InvalidateRect(Some(a.hwnd), None, false);
                            return LRESULT(0);
                        }
                        // Click on swatch/name area → select color & fill name
                        if mx >= 8 && mx < ap_x {
                            let c = a.library[ri as usize];
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
                            // restore name from library
                            if (ri as usize) < a.lib_names.len() {
                                let nb = &a.lib_names[ri as usize];
                                for j in 0..63 {
                                    a.name_buf[j] = nb[j];
                                }
                                a.name_pos = 63;
                                while a.name_pos > 0 && a.name_buf[a.name_pos - 1] == 0 {
                                    a.name_pos -= 1;
                                }
                            } else {
                                a.name_pos = 0;
                            }
                            a.hue = h / 360.0;
                            a.sat = s;
                            a.val = max;
                            a.sel_swatch = -1;
                            a.sel_lib = ri;
                            a.hex_pos = 0;
                            a.wheel_set = false;
                            a.wheel_brightness = -1.0;
                            let _ = InvalidateRect(Some(a.hwnd), None, false);
                            return LRESULT(0);
                        }
                    }
                }

                // 16 presets to the right of the wheel
                let ps_x = rxs + a.l_wheel_sz + 12;
                let ps_y = 56;
                let ps_cols = 4;
                for i in 0..16 {
                    let row = i / ps_cols;
                    let col = i % ps_cols;
                    let sx = ps_x + col * (a.l_swatch_sz + a.l_swatch_gap);
                    let sy = ps_y + row * (a.l_swatch_sz + a.l_swatch_gap);
                    if mx >= sx && mx < sx + a.l_swatch_sz && my >= sy && my < sy + a.l_swatch_sz {
                        let c = a.presets[i as usize];
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
                        a.hue = h / 360.0;
                        a.sat = s;
                        a.val = max;
                        a.sel_swatch = i as i32;
                        a.sel_lib = -1;
                        a.name_pos = 0;
                        a.hex_pos = 0;
                        a.wheel_set = false;
                        a.wheel_brightness = -1.0;
                        let _ = InvalidateRect(Some(a.hwnd), None, false);
                        return LRESULT(0);
                    }
                }

                // Name input area
                let name_x = rxs + a.l_preview_sz + 16;
                let name_y = a.l_preview_y + 18;
                let name_w = a.rw - a.l_preview_sz - 40;
                if mx >= name_x && mx < name_x + name_w && my >= name_y && my < name_y + a.l_name_h
                {
                    a.focus_field = 1;
                    a.drag_mode = 0;
                    if a.hex_pos > 0 {
                        commit_hex(a);
                    }
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }
                // Hex input area
                let hex_sy = name_y + a.l_name_h + 8 + 18;
                if mx >= name_x
                    && mx < name_x + name_w.min(90)
                    && my >= hex_sy
                    && my < hex_sy + a.l_hex_h
                {
                    if a.name_pos > 0 { /* name stays */ }
                    let (pr, pg, pb) = hsv_to_rgb(a.hue, a.sat, a.val);
                    let s = format!("{:02X}{:02X}{:02X}", pr, pg, pb);
                    for (j, c) in s.encode_utf16().enumerate() {
                        if j < 6 {
                            a.hex_buf[j] = c;
                        }
                    }
                    a.hex_pos = 6;
                    a.focus_field = 2;
                    a.drag_mode = 3;
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }

                // Add to Library
                if mx >= rxs
                    && mx <= rxs + (a.rw - rxs - 12)
                    && my >= a.l_add_y
                    && my <= a.l_add_y + a.l_btn_h
                {
                    let (r8, g8, b8) = hsv_to_rgb(a.hue, a.sat, a.val);
                    let color = rgb(r8, g8, b8) & 0xFFFFFF;
                    // save name from input
                    let mut nb = [0u16; 64];
                    for j in 0..a.name_pos.min(63) {
                        nb[j] = a.name_buf[j];
                    }
                    if let Some(pos) = a.library.iter().position(|&c| c == color) {
                        a.lib_names[pos] = nb; // update name for existing color
                        set_status(a, "Name updated \u{2713}");
                    } else {
                        a.library.push(color);
                        a.lib_names.push(nb);
                        set_status(a, "Added to library \u{2713}");
                    }
                    save_library(a);
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }

                // Browse
                if mx >= rxs
                    && mx <= rxs + 90
                    && my >= a.l_browse_y
                    && my <= a.l_browse_y + a.l_btn_sm
                {
                    let mut bi = BROWSEINFOW {
                        hwndOwner: a.hwnd,
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
                        a.folder_path = path_buf;
                        a.browse_ok = true;
                        CoTaskMemFree(Some(pidl as *const c_void));
                    }
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }

                // Apply + Reset
                let apply_y = a.l_browse_y + a.l_btn_sm + 8;
                if a.browse_ok && a.folder_path[0] != 0 {
                    if mx >= rxs && mx <= rxs + 70 && my >= apply_y && my <= apply_y + a.l_btn_sm {
                        let (r8, g8, b8) = hsv_to_rgb(a.hue, a.sat, a.val);
                        let folder = String::from_utf16_lossy(&a.folder_path);
                        let folder = folder.trim_end_matches(char::from(0));
                        if apply_folder_color(folder, r8, g8, b8) {
                            set_status(a, "Applied \u{2713}");
                        }
                        let _ = InvalidateRect(Some(a.hwnd), None, false);
                        return LRESULT(0);
                    }
                    // Reset button
                    if mx >= rxs + 76
                        && mx <= rxs + 76 + 70
                        && my >= apply_y
                        && my <= apply_y + a.l_btn_sm
                    {
                        let folder = String::from_utf16_lossy(&a.folder_path);
                        let folder = folder.trim_end_matches(char::from(0));
                        reset_folder_color(folder);
                        set_status(a, "Reset \u{2713}");
                        let _ = InvalidateRect(Some(a.hwnd), None, false);
                        return LRESULT(0);
                    }
                }

                // Checkbox (right next to Browse)
                let cb_x = rxs + 90 + 12;
                let cs = 14;
                let cbx = cb_x;
                let cby = a.l_browse_y + (a.l_btn_sm - cs) / 2;
                if mx >= cbx && mx <= cbx + cs + 200 && my >= cby && my <= cby + cs {
                    if a.ctx_menu {
                        uninstall_context_menu();
                        a.ctx_menu = false;
                    } else {
                        install_context_menu(a);
                        a.ctx_menu = true;
                    }
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }

                LRESULT(0)
            }

            WM_MOUSEMOVE => {
                if !a.mouse_down {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let mx = (lparam.0 as u16) as i32;
                let my = (lparam.0 >> 16) as i32;
                let rxs = rx(a, 0);
                let redraw = if a.drag_mode == 1 {
                    // Wheel drag
                    let cx = (rxs + a.l_wheel_sz / 2) as f32;
                    let cy = (56 + a.l_wheel_sz / 2) as f32;
                    let dx = mx as f32 - cx;
                    let dy = my as f32 - cy;
                    let dist = (dx * dx + dy * dy).sqrt() / (a.l_wheel_sz as f32 / 2.0);
                    if dist <= 1.0 {
                        let ang = (-dy).atan2(dx) / (2.0 * std::f32::consts::PI) + 0.25;
                        a.hue = ang - ang.floor();
                        a.sat = dist;
                        a.sel_swatch = -1;
                        a.sel_lib = -1;
                        a.name_pos = 0;
                        a.hex_pos = 0;
                        a.wheel_cx = mx;
                        a.wheel_cy = my;
                        true
                    } else {
                        false
                    }
                } else if a.drag_mode == 2 {
                    // Brightness drag
                    if mx >= rxs && mx < rxs + a.l_wheel_sz {
                        a.val = ((mx - rxs) as f32 / a.l_wheel_sz as f32).clamp(0.0, 1.0);
                        a.sel_swatch = -1;
                        a.sel_lib = -1;
                        a.name_pos = 0;
                        a.hex_pos = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if redraw {
                    a.wheel_brightness = -1.0;
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                }
                LRESULT(0)
            }

            WM_RBUTTONDOWN => {
                let mx = (lparam.0 as u16) as i32;
                let my = (lparam.0 >> 16) as i32;
                let rxs = rx(a, 0);
                // Right-click on left panel library → remove entry
                let lib_sw_sz = scale_by(22, a.dpi);
                let lib_row_h = scale_by(30, a.dpi);
                if mx < lx_w() {
                    for i in 0..a.library.len() as i32 {
                        let ri = i + a.lib_scroll;
                        if ri < 0 || ri >= a.library.len() as i32 {
                            continue;
                        }
                        let sy = 52 + i * lib_row_h;
                        if mx >= 16 && mx < lx_w() - 4 && my >= sy && my < sy + lib_row_h {
                            a.library.remove(ri as usize);
                            save_library(a);
                            if a.sel_lib == ri {
                                a.sel_lib = -1;
                            }
                            let _ = InvalidateRect(Some(a.hwnd), None, false);
                            return LRESULT(0);
                        }
                    }
                }
                // Right-click on right panel preset → replace with current color
                let ps_x = rxs + a.l_wheel_sz + 12;
                let ps_y = 56;
                let ps_cols = 4;
                for i in 0..16 {
                    let row = i / ps_cols;
                    let col = i % ps_cols;
                    let sx = ps_x + col * (a.l_swatch_sz + a.l_swatch_gap);
                    let sy = ps_y + row * (a.l_swatch_sz + a.l_swatch_gap);
                    if mx >= sx && mx < sx + a.l_swatch_sz && my >= sy && my < sy + a.l_swatch_sz {
                        let (r8, g8, b8) = hsv_to_rgb(a.hue, a.sat, a.val);
                        a.presets[i as usize] = rgb(r8, g8, b8) & 0xFFFFFF;
                        save_presets(a);
                        let _ = InvalidateRect(Some(a.hwnd), None, false);
                        return LRESULT(0);
                    }
                }
                LRESULT(0)
            }

            WM_LBUTTONUP => {
                a.mouse_down = false;
                a.drag_mode = 0;
                if a.hex_pos > 0 {
                    commit_hex(a);
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                }
                LRESULT(0)
            }

            WM_CHAR => {
                let c = wparam.0 as u16;
                if a.focus_field == 1 {
                    // Name input
                    match c {
                        0x08 => {
                            if a.name_pos > 0 {
                                a.name_pos -= 1;
                                a.name_buf[a.name_pos] = 0;
                            }
                        }
                        0x0D | 0x1B => {
                            a.focus_field = 0;
                        }
                        0x20..=0x7E => {
                            if a.name_pos < 63 {
                                a.name_buf[a.name_pos] = c;
                                a.name_pos += 1;
                            }
                        }
                        _ => {}
                    }
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }
                if a.focus_field == 2 || a.hex_pos > 0 {
                    // Hex input
                    match c {
                        0x08 => {
                            if a.hex_pos > 0 {
                                a.hex_pos -= 1;
                                a.hex_buf[a.hex_pos] = 0;
                            }
                        }
                        0x0D | 0x1B => {
                            commit_hex(a);
                            a.focus_field = 0;
                        }
                        0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66 => {
                            if a.hex_pos < 6 {
                                let uc = if c >= 0x61 { c - 0x20 } else { c };
                                a.hex_buf[a.hex_pos] = uc;
                                a.hex_pos += 1;
                            }
                        }
                        _ => {}
                    }
                    let _ = InvalidateRect(Some(a.hwnd), None, false);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }

            WM_TIMER => {
                a.status_cnt -= 1;
                if a.status_cnt <= 0 {
                    a.status[0] = 0;
                    let _ = KillTimer(Some(a.hwnd), wparam.0);
                }
                let _ = InvalidateRect(Some(a.hwnd), None, false);
                LRESULT(0)
            }

            WM_MOUSEWHEEL => {
                let scroll = (wparam.0 >> 16) as i16 as i32;
                let lib_row_h = scale_by(30, a.dpi);
                if lib_row_h > 0 {
                    let max_scroll = (a.library.len() as i32) - (520 / lib_row_h);
                    a.lib_scroll = (a.lib_scroll - scroll / 120).max(0).min(max_scroll.max(0));
                }
                let _ = InvalidateRect(Some(a.hwnd), None, false);
                LRESULT(0)
            }

            WM_DPICHANGED => {
                a.dpi = (wparam.0 as u32 >> 16) as i32;
                layout_init(a);
                if !a.tf.is_invalid() {
                    let _ = DeleteObject(a.tf.into());
                }
                if !a.bf.is_invalid() {
                    let _ = DeleteObject(a.bf.into());
                }
                if !a.sf.is_invalid() {
                    let _ = DeleteObject(a.sf.into());
                }
                a.tf = CreateFontW(
                    -scale_by(20, a.dpi),
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
                a.bf = CreateFontW(
                    -scale_by(16, a.dpi),
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
                a.sf = CreateFontW(
                    -scale_by(13, a.dpi),
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
                if !a.wheel_bitmap.is_null() {
                    GdipDisposeImage(a.wheel_bitmap as *mut GpImage);
                    a.wheel_bitmap = null_mut();
                }
                if !a.wheel_graphics.is_null() {
                    GdipDeleteGraphics(a.wheel_graphics);
                    a.wheel_graphics = null_mut();
                }
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

// ── Main ──
fn main() {
    // Parse command line
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
    // Handle folder path arg (open with folder)

    unsafe {
        // GDI+ startup
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
        // Pass folder arg if provided
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
