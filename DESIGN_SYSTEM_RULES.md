# ZERAVRIX / ZIBRA Design System & Figma MCP Integration Rules

> Authoritative reference for Figma Model Context Protocol (MCP) integration, design tokens, UI components, styling, and project architecture for the 100% Rust rewrite (**Project ZIBRA**).

---

## 1. Token Definitions

### File Locations
- **Active Rust Model**: [`crates/zibra-core/src/settings.rs`](file:///home/u1m/Documents/ZERAVRIX/ZIBRA-RS/crates/zibra-core/src/settings.rs)
- **Active Theme Engine & Token Mapper**: [`crates/zibra-app/src/main.rs`](file:///home/u1m/Documents/ZERAVRIX/ZIBRA-RS/crates/zibra-app/src/main.rs) (`apply_theme`)
- **Legacy Material 3 Design Token Reference**: [`pc_repo/desktop-app/src/main/kotlin/com/zeravrix/ZERAVRIX/theme/ThemeEngine.kt`](file:///home/u1m/Documents/ZERAVRIX/pc_repo/desktop-app/src/main/kotlin/com/zeravrix/ZERAVRIX/theme/ThemeEngine.kt)

### Token Structure & 13 Authentic Kotatsu Schemes
The application uses 13 preset Material Design 3 palettes, switchable dynamically with AMOLED pure-black overrides:

| Theme Name | Enum ID | Primary RGB (`primary_rgb()`) | Dark Surface (`dark_bg_rgb()`) | Hex / Accent Role |
| :--- | :--- | :--- | :--- | :--- |
| **Totoro (Indigo)** | `totoro` | `(0xA6, 0xC8, 0xFF)` | `(0x0E, 0x11, 0x16)` | `#A6C8FF` Default Brand |
| **Dynamic Monet** | `monet` | `(0xA8, 0xC7, 0xFA)` | `(0x0D, 0x12, 0x19)` | `#A8C7FA` Dynamic Blue |
| **Expressive (Lilac)** | `expressive` | `(0xDD, 0xBB, 0xFA)` | `(0x14, 0x0F, 0x19)` | `#DDBBFA` Anime Violet |
| **Miku (Teal)** | `miku` | `(0x6F, 0xDD, 0xE2)` | `(0x0B, 0x12, 0x13)` | `#6FDDE2` Cyber Cyan |
| **Asuka (Crimson)** | `asuka` | `(0xFF, 0xB4, 0xA8)` | `(0x16, 0x0E, 0x0D)` | `#FFB4A8` Eva Coral Red |
| **Mion (Emerald)** | `mion` | `(0xA1, 0xD3, 0x9A)` | `(0x0D, 0x14, 0x0C)` | `#A1D39A` Forest Sage |
| **Rikka (Lavender)** | `rikka` | `(0xD3, 0xBB, 0xFD)` | `(0x12, 0x0E, 0x18)` | `#D3BBFD` Pastel Purple |
| **Sakura (Rose)** | `sakura` | `(0xFF, 0xB1, 0xC8)` | `(0x16, 0x0E, 0x12)` | `#FFB1C8` Cherry Blossom |
| **Mamimi (Cobalt)** | `mamimi` | `(0xAF, 0xC6, 0xFF)` | `(0x0E, 0x11, 0x18)` | `#AFC6FF` Electric Indigo |
| **Kanade (Graphite)**| `kanade` | `(0xE2, 0xE2, 0xE6)` | `(0x10, 0x10, 0x12)` | `#E2E2E6` Monochrome Platinum |
| **Itsuka (Amber)** | `itsuka` | `(0xFF, 0xBA, 0x8F)` | `(0x16, 0x10, 0x0A)` | `#FFBA8F` Sunset Peach |
| **AMOLED Void** | `amoled` | `(0xA6, 0xC8, 0xFF)` | `(0x00, 0x00, 0x00)` | `#000000` True Black |
| **High Contrast** | `mono` | `(0xFF, 0xFF, 0xFF)` | `(0x05, 0x05, 0x05)` | `#FFFFFF` Ultra Contrast |

### Spacing & Corner Radii
```rust
// Corner Radius Tokens
pub const RADIUS_SM: f32 = 8.0;   // Chapter items, badge chips
pub const RADIUS_MD: f32 = 10.0;  // Manga cover cards, dialog frames
pub const RADIUS_LG: f32 = 12.0;  // Pinned source cards, search bars

// Spacing & Padding Tokens
pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 6.0;
pub const SPACE_MD: f32 = 8.0;
pub const SPACE_LG: f32 = 12.0;
pub const SPACE_XL: f32 = 16.0;

// Card Dimensions
pub const MANGA_CARD_WIDTH: f32 = 145.0;
pub const MANGA_COVER_SIZE: [f32; 2] = [135.0, 185.0]; // 3:4.1 aspect ratio
pub const SOURCE_CARD_WIDTH: f32 = 210.0;
```

### Token Transformation System
At startup or on settings update, `apply_theme` maps the tokens into `egui::Visuals`:
```rust
fn apply_theme(&self, ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    let (pr, pg, pb) = self.settings.color_theme.primary_rgb();
    let (br, bg, bb) = if self.settings.is_amoled {
        (0, 0, 0)
    } else {
        self.settings.color_theme.dark_bg_rgb()
    };
    let primary = Color32::from_rgb(pr, pg, pb);
    let bg_color = Color32::from_rgb(br, bg, bb);
    let panel_color = if self.settings.is_amoled {
        Color32::from_rgb(8, 10, 14)
    } else {
        Color32::from_rgb(br.saturating_add(10), bg.saturating_add(12), bb.saturating_add(16))
    };

    visuals.panel_fill = bg_color;
    visuals.window_fill = panel_color;
    visuals.selection.bg_fill = primary.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.5_f32, primary);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, primary);
    visuals.widgets.active.bg_fill = primary.gamma_multiply(0.25);
    ctx.set_visuals(visuals);
}
```

---

## 2. Component Library

### Component Architecture
Components are structured as immediate-mode render routines in `crates/zibra-app/src/main.rs`, backed by asynchronous Tokio tasks communicating via `parking_lot::RwLock` state containers:

1. **Navigation Rail (`NavScreen`)**:
   - History (`🕒 History`)
   - Favorites (`❤ Favorites`)
   - Explore (`🧭 Explore`)
   - Feed (`📡 Feed`)
   - Downloads (`⬇ Downloads`)
   - Settings (`⚙ Settings`)
2. **Search & Tag Filter Bar**:
   - Single-line search input with dynamic query submission.
   - Tag filter chips: `#Manhwa`, `#Webtoon`, `#Arabic`, `#English`, `#OfficialApi`, `#FastCdn`, `#VerifiedWorking`.
3. **Source Card**:
   - Priority rank badge (`#{:02}`), name, language tag (`[EN]`, `[AR]`), engine tags, and live latency indicator (`{}ms`).
   - Actions: `Open Source ➡` and `📌 Pin / Unpin`.
4. **Manga Card (Grid Item)**:
   - Rounded 8px cover thumbnail (`135x185px`), strong title label (`12.0pt`), and `View Chapters` button.
5. **Details Screen**:
   - Header with back navigation, favorite toggle, description, genres chips, and scrollable chapter list with direct read (`▶ Read`) and CBZ download (`⬇ CBZ`).
6. **Reader Viewport**:
   - Continuous vertical webtoon strip with 2048px GPU texture tiles.
   - Zoom/pan controls (`reader_zoom`) and generation-tracked page fetching.

---

## 3. Frameworks & Libraries

- **Desktop GUI Shell**: `eframe 0.29` / `egui 0.29` with `glow` (OpenGL) and `winit` (supporting Wayland and X11 on Linux; DirectX/GL on Windows).
- **Image Loaders**: `egui_extras 0.29` (`install_image_loaders`).
- **Async Runtime**: `tokio 1.40` (`features = ["full"]`).
- **Image Slicing & Codecs**: `image 0.25`, `imagesize 0.13`, `moka 0.12` (sync LRU cache).
- **Embedded Persistence**: `rusqlite 0.32` (`bundled` SQLite with WAL mode; migrating to `redb 4.5` per Blueprint ADR-001).
- **Archiving**: `zip 2.2` with `crc32fast 1.4` and atomic `.part` rename.
- **Crypto & Security**: `p256 0.13` (ECDH), `aes-gcm 0.10`, `sha2 0.10`, `hmac 0.12`, `hkdf 0.12`.

---

## 4. Asset Management

- **Branding Assets**:
  - `ZIBRA-RS/assets/logo.png`
  - `ZIBRA-RS/assets/logo.ico`
  - `ZIBRA-RS/assets/branding/logo-mark.png`
- **Cache Storage**:
  - Base Directory: `<user-data-dir>/.zibra/`
  - HTTP Image Cache: `<user-data-dir>/.zibra/cache/http/` (bounded disk budget, default 512 MiB).
  - Downloaded Chapters: `<user-data-dir>/.zibra/downloads/<source>/<manga>/<chapter>.cbz`.
- **Memory Bounding & Tiling**:
  - 2048px maximum texture slice dimensions to prevent GPU OOM crashes on ultra-tall manhwa chapters.
  - Checked `u64` arithmetic for byte estimations: `width as u64 * height as u64 * 4`.

---

## 5. Icon System

- **Approach**: High-legibility Unicode emoji and geometric vector symbols embedded directly in button and label text strings, paired with contextual color tokens:
  - Navigation: `🕒 History`, `❤ Favorites`, `🧭 Explore`, `📡 Feed`, `⬇ Downloads`, `⚙ Settings`
  - Status & Controls: `🔍 Search`, `⬅ Back`, `➡ Next`, `▶ Read`, `⬇ CBZ`, `📌 Pinned`, `❤ Favorite`
  - Latency: Colored pills with millisecond display (e.g. `24ms` in `Color32::LIGHT_GREEN`).
- **Badge Color Conventions**:
  - `SourceTag::Manhwa`: Crimson `(0xFF, 0x6B, 0x6B)`
  - `SourceTag::Webtoon`: Indigo `(0x74, 0x8F, 0xFC)`
  - `SourceTag::Arabic`: Emerald `(0x51, 0xCF, 0x66)`
  - `SourceTag::OfficialApi`: Amber `(0xFF, 0x92, 0x2B)`
  - `SourceTag::FastCdn`: Teal `(0x20, 0xC9, 0x97)`
  - `SourceTag::VerifiedWorking`: Green `(0x40, 0xC0, 0x57)`

---

## 6. Styling Approach

- **Immediate-Mode Widget Styling**: Configured via `egui::Frame`, `egui::Rounding`, `egui::Stroke`, and `egui::Margin`.
- **Responsive Layout**: Dynamic wrap grids using `ui.horizontal_wrapped()` inside `egui::ScrollArea::vertical()`. Cards automatically flow across available viewport width without static column breakages.
- **RTL & Bidirectional Text**:
  - Arabic and Latin mixed-direction support for manhwa titles and chapter names.
  - Directional overrides are scoped to content text rather than reversing the whole application shell.

---

## 7. Project Structure

```
ZIBRA-RS/
├── Cargo.toml                     # Root workspace manifest
├── assets/                        # Logo, branding, icons
│   ├── logo.png
│   ├── logo.ico
│   └── branding/
├── crates/
│   ├── zibra-core/                # Data models, SQLite DB, settings, downloader, archiver
│   │   ├── src/
│   │   │   ├── models.rs          # Manga, Chapter, ReadingProgress, SourceTag
│   │   │   ├── settings.rs        # AppSettings, ColorTheme (13 Kotatsu palettes)
│   │   │   ├── library.rs         # SQLite WAL storage & history
│   │   │   ├── downloader.rs      # Parallel chapter downloader
│   │   │   ├── archive.rs         # Atomic CBZ archiver & natural string sorting
│   │   │   ├── backup.rs          # V1/V2 backup manager
│   │   │   └── lib.rs
│   │   └── tests/
│   │       └── core_baseline.rs   # Unit test baseline (M0 gate)
│   ├── zibra-sources/             # Extractor, registry, and 18 native source engines
│   │   └── src/
│   │       ├── extractor.rs       # Reusable 4-stage HTTP client & cookie manager
│   │       ├── registry.rs        # LiveSourceRegistry with latency probes
│   │       └── engines/           # Madara, Themesia, MangaDex, Swat, Azora, TeamX, etc.
│   ├── zibra-reader/              # 2048px tile slicer and moka memory cache
│   │   └── src/
│   │       └── lib.rs
│   └── zibra-app/                 # Native eframe desktop application shell
│       └── src/
│           ├── main.rs            # Application event loop, views, navigation, theme
│           └── spring.rs          # Smooth spring-damper scrolling physics
```
