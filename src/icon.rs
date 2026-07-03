use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::Storage::FileSystem::*;

use image::ImageEncoder;

use crate::app::PIXEL_FORMAT_32BPP_ARGB;
use crate::color::*;

pub(crate) fn get_cache_dir() -> String {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:".to_string());
        format!("{}\\AppData\\Local", home)
    });
    format!("{}\\FolderColorizerRust\\cache", local)
}

unsafe fn render_folder_bitmap(gr: *mut GpGraphics, w: i32, h: i32, r: u8, g: u8, b: u8) {
    let margin = (w as f32 * 0.078125) as i32;
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
        GdipCreateSolidFill(gdi_argb(r, g, b), &mut brush);
        if !brush.is_null() {
            GdipFillPath(gr, brush as *mut GpBrush, body);
            let mut shade: *mut GpSolidFill = null_mut();
            GdipCreateSolidFill(
                gdi_argb(
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

    let mut tab: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut tab);
    if !tab.is_null() {
        GdipAddPathRectangleI(tab, margin, margin, tab_w, tab_h);
        let mut tbrush: *mut GpSolidFill = null_mut();
        let tr = ((r as i32 + (255 - r as i32) / 2).min(255)) as u8;
        let tg = ((g as i32 + (255 - g as i32) / 2).min(255)) as u8;
        let tb = ((b as i32 + (255 - b as i32) / 2).min(255)) as u8;
        GdipCreateSolidFill(gdi_argb(tr, tg, tb), &mut tbrush);
        if !tbrush.is_null() {
            GdipFillPath(gr, tbrush as *mut GpBrush, tab);
            GdipDeleteBrush(tbrush as *mut GpBrush);
        }
        let mut hl: *mut GpSolidFill = null_mut();
        GdipCreateSolidFill(
            gdi_argb(
                (tr as u16 + 30).min(255) as u8,
                (tg as u16 + 30).min(255) as u8,
                (tb as u16 + 30).min(255) as u8,
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

pub(crate) unsafe fn generate_ico(path: &str, r: u8, g: u8, b: u8) -> bool {
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

fn draw_folder(img: &mut image::RgbaImage, sz: u32, r: u8, g: u8, b: u8) {
    let margin = (sz as f32 * 0.078125) as u32;
    let tab_w = (sz as f32 * 0.195) as u32;
    let tab_h = (sz as f32 * 0.098) as u32;
    let body_h = (sz as f32 * 0.766) as u32;
    let body_y = margin + tab_h;
    let shade_h = (sz as f32 * 0.078) as u32;
    let shade_y = margin + body_h - shade_h;
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
                        (tr as u16 + 30).min(255) as u8,
                        (tg as u16 + 30).min(255) as u8,
                        (tb as u16 + 30).min(255) as u8,
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

pub(crate) unsafe fn generate_simple_ico(path: &str, r: u8, g: u8, b: u8) -> bool {
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

pub(crate) unsafe fn apply_folder_color(folder: &str, r: u8, g: u8, b: u8) -> bool {
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

pub(crate) unsafe fn reset_folder_color(folder: &str) -> bool {
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
