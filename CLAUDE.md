# CLAUDE.md — ZERAVRIX / ZIBRA Design System & Architecture Rules

> Authoritative instructions for integrating Figma designs via MCP, maintaining the pure Rust desktop application, and adhering to Blueprint v2.0 and the Bug Audit / 1,000-Source Roadmap.

---

## 1. Design System Structure

### 1.1 Token Definitions
- **Color Themes**: 13 Material Design 3 palettes defined in [`crates/zibra-core/src/settings.rs`](file:///home/u1m/Documents/ZERAVRIX/ZIBRA-RS/crates/zibra-core/src/settings.rs).
- **Core Themes**:
  - `Totoro (Indigo)`: Primary `(0xA6, 0xC8, 0xFF)`, Dark Surface `(0x0E, 0x11, 0x16)`
  - `Dynamic Monet`: Primary `(0xA8, 0xC7, 0xFA)`, Dark Surface `(0x0D, 0x12, 0x19)`
  - `Expressive (Lilac)`: Primary `(0xDD, 0xBB, 0xFA)`, Dark Surface `(0x14, 0x0F, 0x19)`
  - `Miku (Teal)`: Primary `(0x6F, 0xDD, 0xE2)`, Dark Surface `(0x0B, 0x12, 0x13)`
  - `Asuka (Crimson)`: Primary `(0xFF, 0xB4, 0xA8)`, Dark Surface `(0x16, 0x0E, 0x0D)`
  - `Mion (Emerald)`: Primary `(0xA1, 0xD3, 0x9A)`, Dark Surface `(0x0D, 0x14, 0x0C)`
  - `Rikka (Lavender)`: Primary `(0xD3, 0xBB, 0xFD)`, Dark Surface `(0x12, 0x0E, 0x18)`
  - `Sakura (Rose)`: Primary `(0xFF, 0xB1, 0xC8)`, Dark Surface `(0x16, 0x0E, 0x12)`
  - `Mamimi (Cobalt)`: Primary `(0xAF, 0xC6, 0xFF)`, Dark Surface `(0x0E, 0x11, 0x18)`
  - `Kanade (Graphite)`: Primary `(0xE2, 0xE2, 0xE6)`, Dark Surface `(0x10, 0x10, 0x12)`
  - `Itsuka (Amber)`: Primary `(0xFF, 0xBA, 0x8F)`, Dark Surface `(0x16, 0x10, 0x0A)`
  - `AMOLED Void`: Primary `(0xA6, 0xC8, 0xFF)`, Dark Surface `(0x00, 0x00, 0x00)`
  - `High Contrast`: Primary `(0xFF, 0xFF, 0xFF)`, Dark Surface `(0x05, 0x05, 0x05)`

- **Spacing & Radii**:
  - Small Radius: `8.0` (chips, badges, chapter items)
  - Medium Radius: `10.0` (manga cover cards, details dialogs)
  - Large Radius: `12.0` (pinned source cards, search fields)
  - Layout Margins: `8.0` to `10.0` inner margin; `4.0` to `12.0` spacing
  - Manga Card Dimensions: `145.0` width, `135.0 x 185.0` image cover (3:4.1 aspect ratio)
  - Source Card Dimensions: `210.0` width

- **Token Transformation**:
  Implemented in `apply_theme` in [`crates/zibra-app/src/main.rs`](file:///home/u1m/Documents/ZERAVRIX/ZIBRA-RS/crates/zibra-app/src/main.rs):
  ```rust
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
  ```

---

### 1.2 Component Library
Defined in [`crates/zibra-app/src/main.rs`](file:///home/u1m/Documents/ZERAVRIX/ZIBRA-RS/crates/zibra-app/src/main.rs):
- `NavScreen`: Navigation rail (`History`, `Favorites`, `Explore`, `Feed`, `Downloads`, `Settings`).
- `render_explore_screen`: Catalog search, language dropdown, tag chips, and source card grid.
- `render_details_screen`: Manga synopsis, genres, cover preview, and chapter listing.
- `render_shelf_screen`: Library / history item grid with cover art.
- `render_reader_screen`: Continuous vertical webtoon reader with 2048px tile slicing.
- `render_feed_screen`: Latest chapter release updates for favorite titles.
- `render_downloads_screen`: Download status, speed, active jobs, and CBZ archiving progress.

---

### 1.3 Frameworks & Libraries
- **GUI Framework**: `eframe 0.29` / `egui 0.29` with `glow` (OpenGL) / `winit` backend (supports Wayland and X11 on Linux, DirectX/GL on Windows).
- **Async Runtime**: `tokio 1.40` (multi-threaded, bounded concurrency).
- **Networking**: `reqwest 0.12` with `rustls-tls`, gzip, brotli, and cookie store.
- **HTML Parsing**: `scraper 0.20` with CSS selectors.
- **Embedded Database**: Pure SQLite via `rusqlite 0.32` (`bundled` WAL mode); isolated interface allowing `redb 4.5` migration per Blueprint ADR-001.
- **Image Pipeline**: `image 0.25`, `imagesize 0.13`, `moka 0.12` memory LRU cache.

---

### 1.4 Asset Management
- **Static Assets**:
  - `ZIBRA-RS/assets/logo.png`, `logo.ico`, `branding/logo-mark.png`.
- **Dynamic Assets**:
  - Image caching under `<user-data-dir>/.zibra/cache/http/`.
  - Offline CBZ archives in `<user-data-dir>/.zibra/downloads/<source>/<manga>/<chapter>.cbz`.
- **Safety**: Checked `u64` arithmetic for all image buffer estimates (`width as u64 * height as u64 * 4`) to reject image bombs and corrupted files before allocation.

---

### 1.5 Icon System
- Native glyphs & Unicode icon conventions:
  - `📚 Library`, `🕒 History`, `🧭 Explore`, `📡 Feed`, `⬇ Downloads`, `⚙ Settings`
  - `🔍 Search`, `⬅ Back`, `➡ Next`, `▶ Read`, `⬇ CBZ`, `📌 Pinned`, `❤ Favorites`
  - Latency badges: `{}ms` (e.g. `LIGHT_GREEN`).
- Tag Badges: `SourceTag::badge_color_rgb()` maps custom colors to content tags (`#Manhwa`, `#Webtoon`, `#Arabic`, `#OfficialApi`).

---

### 1.6 Styling Approach
- Immediate-mode UI layout with `egui::Visuals` & `egui::Frame`.
- Responsive grids using `ui.horizontal_wrapped()` inside `egui::ScrollArea::vertical()`.
- RTL/Bidirectional text shaping for Arabic manhwa titles and chapter labels.

---

### 1.7 Project Structure
```
ZIBRA-RS/
├── Cargo.toml
├── crates/
│   ├── zibra-core/       # Domain models, SQLite DB, settings, downloader, archiver
│   │   ├── src/models.rs
│   │   ├── src/settings.rs
│   │   ├── src/library.rs
│   │   ├── src/downloader.rs
│   │   └── tests/core_baseline.rs
│   ├── zibra-sources/    # Source trait, extractor, and 18 native engines
│   │   ├── src/extractor.rs
│   │   ├── src/registry.rs
│   │   └── src/engines/
│   ├── zibra-reader/     # 2048px tile slicer and byte-bounded moka cache
│   └── zibra-app/        # Native desktop GUI binary (eframe/egui)
```
