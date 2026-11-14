# Zibra Source Catalog (`zibra-rs-cataloge`)

**High-Performance Pure Rust Manga & Manhwa Source Catalog & Parser Engines**

[![Crates.io](https://img.shields.io/badge/Rust-2021_Edition-DEA584?style=flat-square&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg?style=flat-square)](LICENSE)
[![Pure Rust](https://img.shields.io/badge/Dependencies-Zero_Java%20%7C%20100%25_Rust-brightgreen?style=flat-square)](#architecture)

---

## Overview

`zibra-rs-cataloge` provides a comprehensive, production-grade catalog of over **1,250+ manga, manhwa, and comic sources** implemented completely in **100% pure Rust** with zero Java/Kotlin runtimes.

Engineered for performance, safety, and strict asynchronous concurrency, this crate powers content discovery, metadata aggregation, chapter scraping, and multi-engine parsing across 18 languages.

---

## Workspace Crates

- **`crates/zibra-core`**: Core domain models (`Manga`, `Chapter`, `MangaPage`, `SourceTag`), Material Design 3 color themes, SQLite WAL `LibraryStore`, atomic `.cbz` archiving, and asynchronous download management.
- **`crates/zibra-sources`**: Dynamic data-driven catalog (`sources.json`) and native scraper engines:
  - **Madara WordPress Engine**: High-performance JSON/AJAX scraper for Madara-based platforms.
  - **MangaThemesia / MangaStream Engine**: Multi-selector chapter extraction with lazy image resolving.
  - **Zeist / Blogger Engine**: Fast extraction for ZeistManga webtoon platforms.
  - **Generic HTML Engine**: Fallback scraper supporting WpComics, Mmrcms, Foolslide, HeanCms, and universal HTML/CSS selectors.
  - **Dedicated Native Engines**: Official MangaDex API v5, MangaSwat, Azora, Team-X, Dilar, Asura Scans, Flame Comics, Toonily, HariManga, Komga, Kavita, and local filesystem sources.

---

## Key Features

- **Pure Rust Scrapers**: Zero JVM, zero external JavaScript engines.
- **NSFW Content Filtering**: Granular content tagging (`SourceTag::Nsfw`) and adult source isolation enabled by default.
- **Resilient Parsing**: Automatic thumb size stripping, lazy-loaded image extraction (`data-src`, `data-cfsrc`, `data-lazy-src`), and anti-hotlink Referer headers.
- **Live Health & Latency Metrics**: Built-in ping latency tracking and health filtering.

---

## Testing

Run the full automated test suite:

```bash
cargo test
```

---

## License

This project is licensed under the **GNU General Public License v3.0**.
