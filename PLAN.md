# Folder Colorizer — Rust Port Plan

## Overview

**Folder Colorizer** is a Windows desktop app that lets you change folder icon colors by writing `desktop.ini` files with custom `.ico` resources. It includes a color picker (HSV wheel + swatches), a color library, and a Windows Explorer context menu integration.

This is a **Rust port** of the C++ version at `FolderColorizerC/` (1003 lines, Win32 + GDI+), which was itself a port of the C# .NET 8 WinForms app at `FolderColorizer2/`. The Rust version uses raw Win32 API through the `windows` crate and GDI+ for rendering — no GUI framework beyond the OS.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                       main.rs                               │
│                                                             │
│  ┌──────────────┐  ┌─────────────┐  ┌──────────────────┐   │
│  │  GUI Loop     │  │  CLI Mode   │  │  WndProc         │   │
│  │  (Win32 msg)  │  │  --apply    │  │  (message pump)  │   │
│  │               │  │  --reset    │  │                   │   │
│  └──────┬───────┘  └──────┬──────┘  └────────┬──────────┘   │
│         │                 │                   │              │
│         └─────────┬───────┴───────────────────┘              │
│                   │                                          │
│  ┌────────────────▼────────────────────────────────────┐    │
│  │  AppState — all UI state + GDI objects              │    │
│  │  (fonts, brushes, pens, backbuffer, wheel bitmap)   │    │
│  └────────────────┬────────────────────────────────────┘    │
│                   │                                          │
│  ┌────────────────▼────────────────────────────────────┐    │
│  │  Core Features                                       │    │
│  │                                                      │    │
│  │  • Color math: HSV↔RGB, hex parsing                 │    │
│  │  • Color wheel rendering (GDI+, pixel-by-pixel)     │    │
│  │  • Brightness bar (GDI+ gradient)                   │    │
│  │  • Swatch library (16 presets)                      │    │
│  │  • Folder icon ICO generation (GDI+ or fallback)    │    │
│  │  • Folder service (desktop.ini + file attributes)   │    │
│  │  • Context menu (registry: HKCU\Software\Classes)   │    │
│  │  • DPI scaling                                      │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

---

## C++ → Rust Feature Mapping

Source: `FolderColorizerC/main.cpp` (~1003 lines) + `theming.h` (~119 lines)

| C++ Feature                         | Rust Equivalent                                        | Status        |
| ----------------------------------- | ------------------------------------------------------ | ------------- |
| Win32 window + dark theme           | `wnd_proc()` + `paint()` + double-buffered             | ✅ Done       |
| HSV↔RGB color math + hex parsing    | `hsv_to_rgb()`, `parse_hex()`, `RgbToHsv()`            | ✅ Done       |
| Color wheel (GDI+ DIBSection cache) | `render_wheel()` (GDI+ bitmap)                         | ✅ Done       |
| **Wheel drag (WM_MOUSEMOVE)**       | Click only — no `SetCapture`/mouse tracking            | ❌ Missing    |
| Brightness slider + **thumb drag**  | Click-to-set only — no WM_MOUSEMOVE                    | ❌ Missing    |
| 16 swatches with cached brushes     | Inline in paint + swatch selection                     | ✅ Done       |
| **Text input fields (WM_CHAR)**     | Name + hex keyboard input with focus management        | ❌ Missing    |
| **Persistent color library**        | Save/load to registry, scrollable cards, apply/delete  | ❌ Missing    |
| **Library scrollbar + mouse wheel** | Scrollbar hit + WM_MOUSEWHEEL handler                  | ❌ Missing    |
| **Multi-res ICO (16/32/48/256)**    | Single-res 256x256 only                                | ❌ Missing    |
| GDI+ vector folder preview          | GDI+ preview via `GdipDrawImageI`                      | ✅ Done       |
| Browse folder                       | `SHBrowseForFolderW` (Rust) vs `IFileOpenDialog` (C++) | ✅ Done       |
| Apply/reset folder color            | `desktop.ini` + file attributes + `SHChangeNotify`     | ✅ Done       |
| Context menu install/uninstall      | Registry + `SHCNE_ASSOCCHANGED`                        | ✅ Done       |
| Context menu checkbox toggle        | Check on/off toggle                                    | ✅ Done       |
| CLI headless (`--apply`/`--reset`)  | CLI arg parsing + headless mode                        | ✅ Done       |
| DPI scaling (WM_DPICHANGED)         | Font recreation + layout reinit + window resize        | ✅ Done       |
| Min window size                     | `WM_GETMINMAXINFO` handler                             | ✅ Done       |
| **Status auto-fade timer**          | Timer-based status clear (C++ has it)                  | ❌ Missing    |
| Rounded rect folder shape           | C++ uses `Gdiplus::GraphicsPath` arcs                  | ❌ Flat rects |
| `CreateFontIndirectW` (Segoe UI)    | `CreateFontW` with DPI-scaled size                     | ✅ Done       |
| 7 context menu quick colors         | 7 presets in context menu                              | ✅ Done       |

---

## What's Done

- [x] Full Win32 GUI window with dark theme (WM_PAINT, double-buffered)
- [x] HSV color wheel (GDI+ GpBitmap, pixel-rendered, cached per brightness)
- [x] Brightness bar (GDI+ gradient)
- [x] 16-color swatch grid with selection highlight
- [x] Color preview + hex display
- [x] Folder browser dialog (SHBrowseForFolderW)
- [x] Apply — generates `.ico`, writes `desktop.ini`, sets folder attributes
- [x] Reset — removes `desktop.ini`, restores attributes
- [x] Context menu install/uninstall (7 preset colors + Restore + Colors...)
- [x] Context menu checkbox toggle
- [x] CLI headless mode (`--apply <folder> <hex>`, `--reset <folder>`)
- [x] ICO generation with GDI+ folder shape
- [x] Fallback ICO generation (pure `image` crate, no GDI+)
- [x] Icon caching in `%LOCALAPPDATA%\FolderColorizerRust\cache\`
- [x] DPI awareness and scaling (WM_DPICHANGED, font/layout reinit)
- [x] GDI+ startup/shutdown
- [x] Cleanup on WM_DESTROY (fonts, brushes, pens, backbuffer, GDI+ objects)
- [x] Min window size enforcement
- [x] Folder arg via command line (pre-select folder)

---

## What Needs Porting from C++

Priority order:

### P1 — Core UX (C++ has, Rust doesn't)

- [ ] **Color wheel mouse drag** — add `WM_MOUSEMOVE`/`WM_LBUTTONUP` with `SetCapture` for smooth picking
- [ ] **Brightness slider drag** — same, track mouse while held
- [ ] **Hex text input field** — `WM_CHAR` handler with focus management (bidirectional: typing hex updates color, picking color updates hex)
- [ ] **Name text input field** — for labeling library entries
- [ ] **Persistent color library** — save/load to registry (like C++ does: `LibCount`, `Lib0`..`LibN` names + `LibC0`..`LibCN` colors), scrollable card panel with Apply + Delete per card
- [ ] **Library scrollbar + mouse wheel** — scroll hit testing, `WM_MOUSEWHEEL` handler
- [ ] **Multi-resolution ICO** — 16×16, 32×32, 48×48, 256×256 in single `.ico` (C++ uses GDI+ PNG streams per size)

### P2 — Polish

- [ ] **Status auto-fade timer** — clear status after ~5s via `WM_TIMER`
- [ ] **Rounded folder icon shape** — use `GdipAddPathArc`/`GdipAddPathLine` like C++ `AddRoundedRect`
- [ ] **Error handling** — replace `let _ =` silence with meaningful fallbacks
- [ ] **Unicode path safety** — replace fixed `[u16; 260]` buffers with dynamic allocation

### P3 — Nice-to-have

- [ ] **Color naming** — auto-classify RGB to human name (Red, Blue, etc.)
- [ ] **Persistent library** — save library entries to registry

---

## C++ vs Rust Implementation Notes

| Aspect           | C++ (`FolderColorizerC`)                | Rust (`FolderColorizerRust`)                     |
| ---------------- | --------------------------------------- | ------------------------------------------------ |
| Lines of code    | ~1122 (main.cpp + theming.h)            | ~1754 (all in main.rs)                           |
| Color wheel      | DIBSection + raw pixel buffer           | GDI+ `GpBitmap` + per-pixel `GdipFillRectangleI` |
| ICO generation   | GDI+ `Bitmap::Save` to IStream per size | GDI+ LockBits + `image` crate PNG encode         |
| Folder preview   | GDI+ `Graphics` from HDC                | GDI+ `GpGraphics` from HDC                       |
| Font creation    | `CreateFontIndirectW` (LOGFONTW)        | `CreateFontW` (direct params)                    |
| Brightness thumb | `RoundRect` DC_BRUSH                    | No thumb (just line indicator)                   |
| Swatch rendering | Pre-cached `HBRUSH[16]`                 | Per-frame `CreateSolidBrush` + delete            |
| Library storage  | Registry (`HKCU\...\LibCount`, `LibN`)  | Not implemented                                  |
| Library cards    | `DrawLibrary()` with scrollbar          | Not implemented                                  |
| Browse dialog    | `IFileOpenDialog` (COM)                 | `SHBrowseForFolderW` (older)                     |
| Error handling   | Early returns + status messages         | Most errors silently ignored (`let _ =`)         |
| Timer            | `WM_TIMER` for status auto-fade         | `SetTimer` called but no handler for it          |

---

## Build & Run

---

## Build & Run

```powershell
# Build
cargo build --release

# Run GUI
.\target\release\folder-colorizer-rust.exe

# Run with folder pre-selected
.\target\release\folder-colorizer-rust.exe "C:\Users\me\some-folder"

# Apply color via CLI
.\target\release\folder-colorizer-rust.exe --apply "C:\path\to\folder" 3498DB

# Reset folder
.\target\release\folder-colorizer-rust.exe --reset "C:\path\to\folder"
```

---

## Dependencies

| Crate          | Purpose                                              |
| -------------- | ---------------------------------------------------- |
| `windows` 0.61 | Win32 API bindings (GDI, GDI+, registry, shell, COM) |
| `image` 0.24   | PNG encoding for fallback ICO generation             |

Only 2 external crates. Pure Win32 rendering — no GUI framework dependency.

---

## File Layout

```
FolderColorizerRust/
├── Cargo.toml
├── Cargo.lock
├── PLAN.md
├── .gitignore
└── src/
    └── main.rs          # Everything: ~1754 lines
```
