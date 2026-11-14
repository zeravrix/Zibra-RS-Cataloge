# ⚡ ZIBRA-RS (v2.1.0)
**100% Pure Native Rust Manhwa & Manga Reader**

## Workspace Structure
- `Cargo.toml` — Root workspace manifest
- `crates/zibra-core/` — Core models (`Manga`, `Chapter`, `MangaPage`, `SourceTag`), 13 Kotatsu color themes, SQLite WAL `LibraryStore`, atomic `.cbz` `ChapterArchive`, parallel `DesktopDownloader`, and `BackupManager`.
- `crates/zibra-sources/` — `Source` trait, 4-stage `extractor.rs`, ordered & tagged `LiveSourceRegistry`, and native engines (`MangaDex`, `MangaSwat`, `Azora`, `Team X`, `Mangalek`, `MangaSpark`, `Dilar` ECDH-AES, `Hijala`, `Asura Scans`, `Flame Comics`, `Toonily`, `HariManga`, `LekManga`, `ArabToons`, `ManhwaClan`, `KunManga`, `Komga`, `Kavita`, `Local`).
- `crates/zibra-reader/` — 2048px GPU texture tile slicer and byte-bounded `moka` LRU cache.
- `crates/zibra-app/` — Native `eframe`/`egui` desktop application with spring-damper physics.

## Build & Run on Fedora Linux
```bash
sudo dnf install rust cargo gcc pkg-config openssl-devel libX11-devel libXcursor-devel libXrandr-devel libXi-devel mesa-libGL-devel wayland-devel libxkbcommon-devel
cargo run --release -p zibra-app
```
