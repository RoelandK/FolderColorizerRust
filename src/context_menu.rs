use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::AppState;
use crate::app::{cast_bytes, get_exe_path, COLORS, REG_ROOT};
use crate::color::*;
use crate::icon::{generate_ico, get_cache_dir};

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

pub(crate) unsafe fn install_context_menu(a: &AppState) {
    uninstall_context_menu();
    let exe = get_exe_path();
    let cache = get_cache_dir();
    let _ = std::fs::create_dir_all(&cache);

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

    let sep1 = format!("{}\\shell\\08-Separator", REG_ROOT);
    write_separator(&sep1);

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

    let sep2 = format!("{}\\shell\\10-Separator", REG_ROOT);
    write_separator(&sep2);

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

unsafe fn del_tree(parent: HKEY, path: &[u16]) {
    let mut hk = HKEY::default();
    if RegOpenKeyW(parent, windows::core::PCWSTR(path.as_ptr()), &mut hk) == WIN32_ERROR(0) {
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

pub(crate) unsafe fn uninstall_context_menu() {
    let root_w: Vec<u16> = REG_ROOT.encode_utf16().chain(std::iter::once(0)).collect();
    del_tree(HKEY_CURRENT_USER, &root_w);
    let _ = SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_FLUSH, None, None);
}

pub(crate) unsafe fn is_context_menu_installed() -> bool {
    let root_w: Vec<u16> = REG_ROOT.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hk = HKEY::default();
    RegOpenKeyW(
        HKEY_CURRENT_USER,
        windows::core::PCWSTR(root_w.as_ptr()),
        &mut hk,
    ) == WIN32_ERROR(0)
}
