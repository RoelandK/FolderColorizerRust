use crate::app::AppState;
use crate::color::*;

fn lib_file() -> String {
    format!(
        "{}\\FolderColorizerRust\\library.txt",
        std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".into())
    )
}

fn preset_file() -> String {
    format!(
        "{}\\FolderColorizerRust\\presets.txt",
        std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Local".into())
    )
}

pub(crate) unsafe fn load_presets(a: &mut AppState) {
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

pub(crate) unsafe fn save_presets(a: &AppState) {
    let path = preset_file();
    let _ = std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap());
    let mut s = String::new();
    for c in &a.presets {
        s.push_str(&format!("{:06X}\n", c));
    }
    let _ = std::fs::write(&path, &s);
}

pub(crate) unsafe fn load_library(a: &mut AppState) {
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

pub(crate) unsafe fn save_library(a: &AppState) {
    let path = lib_file();
    let _ = std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap());
    let mut s = String::new();
    for (i, c) in a.library.iter().enumerate() {
        s.push_str(&format!("{:06X}|{}\n", c, lib_name(a, i)));
    }
    let _ = std::fs::write(&path, &s);
}

pub(crate) fn lib_name(a: &AppState, i: usize) -> String {
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
