mod spring;

use eframe::egui::{self, Color32, ColorImage, RichText, Rounding, Stroke, TextureHandle, TextureOptions, Vec2};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use zibra_core::{
    AppSettings, AppSettingsStore, CatalogSortMode, Chapter, ColorTheme, DesktopDownloader,
    LibraryStore, Manga, MangaPage, ReadingProgress, SourceTag,
};
use zibra_reader::{ReaderPagePipeline, SlicedPage};
use zibra_sources::LiveSourceRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavScreen {
    History,
    Favorites,
    Explore,
    Feed,
    Downloads,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExploreSubMode {
    PinnedAndSearch,
    SourcesCatalog,
    SourceBrowse,
}

struct ZibraApp {
    rt: Arc<Runtime>,
    settings_store: AppSettingsStore,
    settings: AppSettings,
    library: LibraryStore,
    registry: Arc<LiveSourceRegistry>,
    downloader: DesktopDownloader,
    reader_pipeline: ReaderPagePipeline,

    current_screen: NavScreen,
    explore_mode: ExploreSubMode,

    catalog_search: String,
    catalog_lang: String,
    active_tag_filters: HashSet<SourceTag>,
    active_source_id: String,
    source_search: String,
    source_latest_mode: bool,
    source_page: u32,
    browse_items: Arc<RwLock<Vec<Manga>>>,
    is_loading_browse: Arc<RwLock<bool>>,
    status_banner: Arc<RwLock<String>>,

    selected_manga: Option<Manga>,
    manga_chapters: Arc<RwLock<Vec<Chapter>>>,
    is_loading_chapters: Arc<RwLock<bool>>,

    active_reader_chapter: Option<Chapter>,
    reader_pages: Arc<RwLock<Vec<MangaPage>>>,
    is_loading_pages: Arc<RwLock<bool>>,
    reader_textures: HashMap<String, Vec<TextureHandle>>,
    reader_zoom: f32,
}

impl ZibraApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let rt = Arc::new(Runtime::new().expect("Failed to initialize Tokio runtime"));

        let app_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".zibra");
        let settings_store = AppSettingsStore::new(&app_dir);
        let settings = settings_store.load();
        let library = LibraryStore::new(&app_dir).expect("Failed to open SQLite library");
        let registry = Arc::new(LiveSourceRegistry::new());
        let downloader = DesktopDownloader::new(&app_dir, settings.max_concurrent_downloads);
        let reader_pipeline = ReaderPagePipeline::new(&app_dir, settings.ram_cache_budget_mb);

        Self {
            rt,
            settings_store,
            settings,
            library,
            registry,
            downloader,
            reader_pipeline,
            current_screen: NavScreen::Explore,
            explore_mode: ExploreSubMode::PinnedAndSearch,
            catalog_search: String::new(),
            catalog_lang: "ALL".to_string(),
            active_tag_filters: HashSet::new(),
            active_source_id: "MANGADEX".to_string(),
            source_search: String::new(),
            source_latest_mode: false,
            source_page: 1,
            browse_items: Arc::new(RwLock::new(Vec::new())),
            is_loading_browse: Arc::new(RwLock::new(false)),
            status_banner: Arc::new(RwLock::new("Ready — 100% Native Rust Engine".to_string())),
            selected_manga: None,
            manga_chapters: Arc::new(RwLock::new(Vec::new())),
            is_loading_chapters: Arc::new(RwLock::new(false)),
            active_reader_chapter: None,
            reader_pages: Arc::new(RwLock::new(Vec::new())),
            is_loading_pages: Arc::new(RwLock::new(false)),
            reader_textures: HashMap::new(),
            reader_zoom: 1.0,
        }
    }

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

    fn trigger_source_fetch(&mut self, source_id: String, page: u32, latest: bool, query: Option<String>) {
        self.active_source_id = source_id.clone();
        self.source_page = page;
        self.explore_mode = ExploreSubMode::SourceBrowse;

        let reg = self.registry.clone();
        let items_lock = self.browse_items.clone();
        let loading_lock = self.is_loading_browse.clone();
        let status_lock = self.status_banner.clone();

        *loading_lock.write() = true;
        *status_lock.write() = format!("Fetching titles from {} (Page {})...", source_id, page);

        self.rt.spawn(async move {
            match reg.fetch_from_source(&source_id, page, latest, query.as_deref()).await {
                Ok(res) => {
                    *status_lock.write() = format!("Loaded {} titles from {}", res.items.len(), source_id);
                    if page <= 1 {
                        *items_lock.write() = res.items;
                    } else {
                        items_lock.write().extend(res.items);
                    }
                }
                Err(e) => {
                    *status_lock.write() = format!("Source error ({}): {}", source_id, e);
                }
            }
            *loading_lock.write() = false;
        });
    }

    fn open_manga_details(&mut self, manga: Manga) {
        self.library.record_history(&manga);
        self.selected_manga = Some(manga.clone());
        self.active_reader_chapter = None;

        let cached = self.library.get_cached_chapters(&manga.id);
        *self.manga_chapters.write() = cached;

        let reg = self.registry.clone();
        let ch_lock = self.manga_chapters.clone();
        let loading_lock = self.is_loading_chapters.clone();
        let status_lock = self.status_banner.clone();
        let lib = self.library.clone();

        *loading_lock.write() = true;
        *status_lock.write() = format!("Loading chapters for {}...", manga.title);

        self.rt.spawn(async move {
            if let Some(src) = reg.get(&manga.source_id) {
                if let Ok(chapters) = src.chapters(&manga).await {
                    if !chapters.is_empty() {
                        lib.cache_chapters(&manga.id, &chapters);
                        *ch_lock.write() = chapters.clone();
                        *status_lock.write() = format!("Loaded {} chapters for {}", chapters.len(), manga.title);
                    }
                }
            }
            *loading_lock.write() = false;
        });
    }

    fn open_chapter_reader(&mut self, manga: Manga, chapter: Chapter) {
        self.active_reader_chapter = Some(chapter.clone());
        self.reader_textures.clear();
        *self.reader_pages.write() = Vec::new();

        self.library.save_progress(&ReadingProgress {
            manga_id: manga.id.clone(),
            chapter_id: chapter.id.clone(),
            page_index: 0,
            scroll_offset_y: 0.0,
            chapter_number: chapter.number,
            updated_at: chrono::Utc::now().timestamp_millis(),
        });

        let reg = self.registry.clone();
        let pages_lock = self.reader_pages.clone();
        let loading_lock = self.is_loading_pages.clone();
        let status_lock = self.status_banner.clone();
        let pipeline = self.reader_pipeline.clone();
        let seam_trim = self.settings.seam_trim_px;
        let preload_ahead = self.settings.preload_pages_ahead;

        *loading_lock.write() = true;
        *status_lock.write() = format!("Fetching chapter pages for {}...", chapter.title);

        self.rt.spawn(async move {
            if let Some(src) = reg.get(&chapter.source_id) {
                match src.pages(&chapter).await {
                    Ok(pages) => {
                        *status_lock.write() = format!("Loaded {} pages — slicing 2048px GPU tiles...", pages.len());
                        *pages_lock.write() = pages.clone();
                        for p in pages.into_iter().take(preload_ahead.max(3)) {
                            let pipe = pipeline.clone();
                            tokio::spawn(async move {
                                let _ = pipe.load_and_slice_page(&p, seam_trim).await;
                            });
                        }
                    }
                    Err(e) => {
                        *status_lock.write() = format!("Failed to fetch chapter pages: {}", e);
                    }
                }
            }
            *loading_lock.write() = false;
        });
    }
}

impl eframe::App for ZibraApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        let (pr, pg, pb) = self.settings.color_theme.primary_rgb();
        let primary = Color32::from_rgb(pr, pg, pb);

        if let Some(chapter) = self.active_reader_chapter.clone() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("⬅ Back to Chapters").strong()).clicked() {
                        self.active_reader_chapter = None;
                    }
                    ui.separator();
                    ui.label(RichText::new(&chapter.title).strong().color(primary).size(15.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("➕ Zoom In").clicked() {
                            self.reader_zoom = (self.reader_zoom + 0.15).min(2.5);
                        }
                        if ui.button("➖ Zoom Out").clicked() {
                            self.reader_zoom = (self.reader_zoom - 0.15).max(0.4);
                        }
                        ui.label(format!("Zoom: {:.0}%", self.reader_zoom * 100.0));
                    });
                });
                ui.separator();

                let is_loading = *self.is_loading_pages.read();
                let pages = self.reader_pages.read().clone();

                if is_loading && pages.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                        ui.label("Decoding & slicing chapter pages...");
                    });
                } else if pages.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("No pages returned by source. Check source domain or cookies in Settings.");
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                let avail_w = (ui.available_width() * 0.78 * self.reader_zoom).clamp(280.0, 1400.0);
                                for page in &pages {
                                    if let Some(sliced) = self.reader_pipeline.get_cached(&page.image_url) {
                                        let handles = self.reader_textures.entry(page.image_url.clone()).or_insert_with(|| {
                                            sliced_to_textures(ctx, &page.image_url, &sliced)
                                        });
                                        for (idx, tex) in handles.iter().enumerate() {
                                            let tile = &sliced.tiles[idx];
                                            let scale = avail_w / (tile.width.max(1) as f32);
                                            let draw_h = (tile.height as f32) * scale;
                                            ui.add(egui::Image::from_texture(tex).fit_to_exact_size(Vec2::new(avail_w, draw_h)));
                                        }
                                    } else {
                                        if !self.reader_pipeline.is_in_flight(&page.image_url) {
                                            let pipe = self.reader_pipeline.clone();
                                            let p_clone = page.clone();
                                            let trim = self.settings.seam_trim_px;
                                            self.rt.spawn(async move {
                                                let _ = pipe.load_and_slice_page(&p_clone, trim).await;
                                            });
                                        }
                                        ui.allocate_space(Vec2::new(avail_w, 420.0));
                                        ui.label(RichText::new(format!("Loading Page {}...", page.index + 1)).weak());
                                    }
                                }

                                // Bounded GPU texture cache (P0-03): Evict distant textures when exceeding 25 pages
                                if self.reader_textures.len() > 25 {
                                    let active_urls: HashSet<String> = pages.iter().take(20).map(|p| p.image_url.clone()).collect();
                                    self.reader_textures.retain(|url, _| active_urls.contains(url));
                                }
                            });
                        });
                }
            });
            return;
        }

        egui::SidePanel::left("zibra_nav_rail")
            .resizable(false)
            .exact_width(175.0)
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("⚡ ZIBRA").size(20.0).strong().color(primary));
                    ui.label(RichText::new("Native Rust Reader").size(11.0).weak());
                });
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                let nav_items = [
                    (NavScreen::Explore, "🧭  Explore"),
                    (NavScreen::Favorites, "❤  Favorites"),
                    (NavScreen::History, "🕒  History"),
                    (NavScreen::Feed, "📡  Feed"),
                    (NavScreen::Downloads, "⬇  Downloads"),
                    (NavScreen::Settings, "⚙  Settings"),
                ];

                for (scr, label) in nav_items {
                    let selected = self.current_screen == scr && self.selected_manga.is_none();
                    let btn = egui::Button::new(
                        RichText::new(label)
                            .size(14.0)
                            .color(if selected { primary } else { Color32::LIGHT_GRAY }),
                    )
                    .fill(if selected {
                        primary.gamma_multiply(0.18)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .rounding(Rounding::same(10.0))
                    .min_size(Vec2::new(150.0, 38.0));

                    if ui.add(btn).clicked() {
                        self.current_screen = scr;
                        self.selected_manga = None;
                    }
                    ui.add_space(4.0);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    let status = self.status_banner.read().clone();
                    ui.label(RichText::new(status).size(10.5).weak());
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(manga) = self.selected_manga.clone() {
                self.render_details_screen(ui, manga, primary);
                return;
            }

            match self.current_screen {
                NavScreen::Explore => self.render_explore_screen(ui, primary),
                NavScreen::Favorites => self.render_shelf_screen(ui, true, primary),
                NavScreen::History => self.render_shelf_screen(ui, false, primary),
                NavScreen::Feed => self.render_feed_screen(ui, primary),
                NavScreen::Downloads => self.render_downloads_screen(ui, primary),
                NavScreen::Settings => self.render_settings_screen(ui, primary),
            }
        });
    }
}

impl ZibraApp {
    fn render_explore_screen(&mut self, ui: &mut egui::Ui, primary: Color32) {
        match self.explore_mode {
            ExploreSubMode::PinnedAndSearch => {
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Explore Manhwa & Manga").color(primary));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("🌐 Sources Catalog (Ordered & Tagged)").color(primary).strong())
                            .clicked()
                        {
                            self.explore_mode = ExploreSubMode::SourcesCatalog;
                        }
                    });
                });
                ui.add_space(8.0);

                ui.label(RichText::new("PINNED SOURCES (Ordered by Priority & Tags)").size(11.5).strong());
                ui.add_space(6.0);

                let pinned_sources = self.registry.ordered_catalog(
                    self.settings.catalog_sort_mode,
                    &self.settings.pinned_source_ids,
                    &HashSet::new(),
                    "ALL",
                    "",
                );

                let mut to_fetch: Option<String> = None;
                let pinned_ids = self.settings.pinned_source_ids.clone();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for src in pinned_sources.iter().filter(|s| pinned_ids.contains(s.id())) {
                            egui::Frame::group(ui.style())
                                .rounding(Rounding::same(12.0))
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.set_width(210.0);
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("#{} {}", src.priority_rank(), src.name())).strong());
                                            if let Some(ms) = self.registry.latency_for(src.id()) {
                                                ui.label(RichText::new(format!("{}ms", ms)).size(10.0).color(Color32::LIGHT_GREEN));
                                            }
                                        });
                                        ui.horizontal_wrapped(|ui| {
                                            for tag in src.tags().iter().take(3) {
                                                let (r, g, b) = tag.badge_color_rgb();
                                                ui.label(RichText::new(tag.label()).size(9.5).color(Color32::from_rgb(r, g, b)));
                                            }
                                        });
                                        ui.add_space(4.0);
                                        if ui.button("Browse Titles ➡").clicked() {
                                            to_fetch = Some(src.id().to_string());
                                        }
                                    });
                                });
                        }
                    });
                });
                if let Some(src_id) = to_fetch {
                    self.trigger_source_fetch(src_id, 1, false, None);
                }
            }

            ExploreSubMode::SourcesCatalog => {
                ui.horizontal(|ui| {
                    if ui.button("⬅ Back to Explore").clicked() {
                        self.explore_mode = ExploreSubMode::PinnedAndSearch;
                    }
                    ui.heading(RichText::new("Ordered & Tagged Sources Catalog").color(primary));
                });
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut self.catalog_search);
                    ui.separator();
                    ui.label("Order By:");
                    egui::ComboBox::from_id_salt("catalog_sort_combo")
                        .selected_text(self.settings.catalog_sort_mode.label())
                        .show_ui(ui, |ui| {
                            for mode in CatalogSortMode::ALL {
                                if ui.selectable_value(&mut self.settings.catalog_sort_mode, *mode, mode.label()).clicked() {
                                    let _ = self.settings_store.save(&self.settings);
                                }
                            }
                        });
                    ui.separator();
                    for lang in ["ALL", "ar", "en", "multi"] {
                        if ui.selectable_label(self.catalog_lang == lang, lang.to_uppercase()).clicked() {
                            self.catalog_lang = lang.to_string();
                        }
                    }
                });

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Filter by Tags:").size(11.5).strong());
                    let filterable_tags = [
                        SourceTag::Manhwa,
                        SourceTag::Webtoon,
                        SourceTag::Arabic,
                        SourceTag::English,
                        SourceTag::OfficialApi,
                        SourceTag::FastCdn,
                        SourceTag::VerifiedWorking,
                        SourceTag::Popular,
                    ];
                    for tag in filterable_tags {
                        let active = self.active_tag_filters.contains(&tag);
                        let (r, g, b) = tag.badge_color_rgb();
                        let chip = ui.selectable_label(
                            active,
                            RichText::new(format!("#{}", tag.label())).color(Color32::from_rgb(r, g, b)),
                        );
                        if chip.clicked() {
                            if active {
                                self.active_tag_filters.remove(&tag);
                            } else {
                                self.active_tag_filters.insert(tag);
                            }
                        }
                    }
                    if !self.active_tag_filters.is_empty() && ui.small_button("Clear Tags").clicked() {
                        self.active_tag_filters.clear();
                    }
                });
                ui.separator();

                let ordered = self.registry.ordered_catalog(
                    self.settings.catalog_sort_mode,
                    &self.settings.pinned_source_ids,
                    &self.active_tag_filters,
                    &self.catalog_lang,
                    &self.catalog_search,
                );

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for src in ordered {
                        let is_pinned = self.settings.pinned_source_ids.contains(src.id());
                        egui::Frame::group(ui.style())
                            .rounding(Rounding::same(10.0))
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("#{:02}", src.priority_rank())).strong().color(primary));
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(src.name()).size(14.5).strong());
                                            ui.label(RichText::new(format!("[{}]", src.language().to_uppercase())).weak());
                                            for tag in src.tags() {
                                                let (r, g, b) = tag.badge_color_rgb();
                                                ui.label(RichText::new(tag.label()).size(10.0).color(Color32::from_rgb(r, g, b)));
                                            }
                                        });
                                        ui.label(RichText::new(src.base_url()).size(11.0).weak());
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Open Source ➡").clicked() {
                                            self.trigger_source_fetch(src.id().to_string(), 1, false, None);
                                        }
                                        let pin_label = if is_pinned { "📌 Pinned" } else { "Pin" };
                                        if ui.button(pin_label).clicked() {
                                            if is_pinned {
                                                self.settings.pinned_source_ids.remove(src.id());
                                            } else {
                                                self.settings.pinned_source_ids.insert(src.id().to_string());
                                            }
                                            let _ = self.settings_store.save(&self.settings);
                                        }
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
            }

            ExploreSubMode::SourceBrowse => {
                let src_id = self.active_source_id.clone();
                ui.horizontal(|ui| {
                    if ui.button("⬅ Sources").clicked() {
                        self.explore_mode = ExploreSubMode::PinnedAndSearch;
                    }
                    ui.heading(RichText::new(format!("Source: {}", src_id)).color(primary));
                    ui.separator();
                    if ui.selectable_label(!self.source_latest_mode, "🔥 Popular").clicked() {
                        self.source_latest_mode = false;
                        self.trigger_source_fetch(src_id.clone(), 1, false, None);
                    }
                    if ui.selectable_label(self.source_latest_mode, "🆕 Latest").clicked() {
                        self.source_latest_mode = true;
                        self.trigger_source_fetch(src_id.clone(), 1, true, None);
                    }
                    ui.separator();
                    ui.text_edit_singleline(&mut self.source_search);
                    if ui.button("🔍 Search").clicked() {
                        let q = self.source_search.clone();
                        self.trigger_source_fetch(src_id.clone(), 1, false, Some(q));
                    }
                });
                ui.separator();

                let loading = *self.is_loading_browse.read();
                let items = self.browse_items.read().clone();

                if loading && items.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.spinner();
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for m in &items {
                                egui::Frame::group(ui.style())
                                    .rounding(Rounding::same(10.0))
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        ui.set_width(145.0);
                                        ui.vertical(|ui| {
                                            if let Some(ref cov) = m.cover_url {
                                                ui.add(egui::Image::from_uri(cov).fit_to_exact_size(Vec2::new(135.0, 185.0)).rounding(Rounding::same(8.0)));
                                            } else {
                                                ui.allocate_space(Vec2::new(135.0, 185.0));
                                            }
                                            ui.label(RichText::new(&m.title).strong().size(12.0));
                                            if ui.button("View Chapters").clicked() {
                                                self.open_manga_details(m.clone());
                                            }
                                        });
                                    });
                            }
                        });
                        if !items.is_empty() && ui.button("Load Next Page ⬇").clicked() {
                            let next_p = self.source_page + 1;
                            let latest = self.source_latest_mode;
                            let q = if self.source_search.is_empty() { None } else { Some(self.source_search.clone()) };
                            self.trigger_source_fetch(src_id, next_p, latest, q);
                        }
                    });
                }
            }
        }
    }

    fn render_details_screen(&mut self, ui: &mut egui::Ui, manga: Manga, primary: Color32) {
        ui.horizontal(|ui| {
            if ui.button("⬅ Back").clicked() {
                self.selected_manga = None;
            }
            ui.heading(RichText::new(&manga.title).color(primary));
            let is_fav = self.library.is_favorite(&manga.id);
            if ui.button(if is_fav { "❤ In Favorites" } else { "🤍 Add to Favorites" }).clicked() {
                self.library.toggle_favorite(&manga);
            }
        });
        ui.separator();

        if !manga.description.is_empty() {
            ui.label(RichText::new(&manga.description).size(12.5).weak());
            ui.add_space(6.0);
        }

        ui.horizontal_wrapped(|ui| {
            for g in &manga.genres {
                ui.label(RichText::new(format!("#{}", g)).size(11.0).color(primary));
            }
        });
        ui.separator();

        let chapters = self.manga_chapters.read().clone();
        ui.label(RichText::new(format!("Chapters ({})", chapters.len())).strong().size(14.0));
        ui.add_space(6.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            for ch in chapters {
                egui::Frame::group(ui.style())
                    .rounding(Rounding::same(8.0))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&ch.title).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("▶ Read").color(primary).strong()).clicked() {
                                    self.open_chapter_reader(manga.clone(), ch.clone());
                                }
                                if ui.button("⬇ CBZ").clicked() {
                                    let reg = self.registry.clone();
                                    let dl = self.downloader.clone();
                                    let m_clone = manga.clone();
                                    let ch_clone = ch.clone();
                                    self.rt.spawn(async move {
                                        if let Some(src) = reg.get(&ch_clone.source_id) {
                                            if let Ok(pages) = src.pages(&ch_clone).await {
                                                let _ = dl.download_chapter(m_clone, ch_clone, pages).await;
                                            }
                                        }
                                    });
                                }
                            });
                        });
                    });
                ui.add_space(3.0);
            }
        });
    }

    fn render_shelf_screen(&mut self, ui: &mut egui::Ui, favorites: bool, primary: Color32) {
        ui.heading(RichText::new(if favorites { "❤ Favorites Library" } else { "🕒 Reading History" }).color(primary));
        ui.separator();
        let list = if favorites {
            self.library.list_favorites()
        } else {
            self.library.list_history()
        };
        if list.is_empty() {
            ui.label("No titles yet. Explore a source and open any Manhwa!");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for m in list {
                    egui::Frame::group(ui.style())
                        .rounding(Rounding::same(10.0))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_width(145.0);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&m.title).strong());
                                ui.label(RichText::new(&m.source_id).size(10.5).weak());
                                if ui.button("Open").clicked() {
                                    self.open_manga_details(m);
                                }
                            });
                        });
                }
            });
        });
    }

    fn render_feed_screen(&mut self, ui: &mut egui::Ui, primary: Color32) {
        ui.heading(RichText::new("📡 Latest Chapter Feed").color(primary));
        ui.separator();
        let favs = self.library.list_favorites();
        if favs.is_empty() {
            ui.label("Add Manhwa to Favorites to track new chapter releases here.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for m in favs {
                let cached = self.library.get_cached_chapters(&m.id);
                if let Some(latest_ch) = cached.first() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&m.title).strong());
                        ui.label("—");
                        ui.label(RichText::new(&latest_ch.title).color(primary));
                        if ui.button("Read").clicked() {
                            self.open_chapter_reader(m.clone(), latest_ch.clone());
                        }
                    });
                    ui.separator();
                }
            }
        });
    }

    fn render_downloads_screen(&mut self, ui: &mut egui::Ui, primary: Color32) {
        ui.heading(RichText::new("⬇ Offline CBZ Downloads").color(primary));
        ui.separator();
        for job in self.downloader.jobs() {
            ui.label(format!("{} — {} : {:?}", job.manga.title, job.chapter.title, job.status));
        }
        ui.separator();
        for saved in self.downloader.list_downloaded() {
            ui.label(format!("📦 {} — {} ({} pages)", saved.manga_title, saved.chapter_title, saved.page_count));
        }
    }

    fn render_settings_screen(&mut self, ui: &mut egui::Ui, primary: Color32) {
        ui.heading(RichText::new("⚙ ZIBRA Settings & Performance").color(primary));
        ui.separator();

        ui.label(RichText::new("13 Kotatsu Color Palettes").strong());
        ui.horizontal_wrapped(|ui| {
            for theme in ColorTheme::ALL {
                if ui.selectable_value(&mut self.settings.color_theme, *theme, theme.title()).clicked() {
                    let _ = self.settings_store.save(&self.settings);
                }
            }
        });
        if ui.checkbox(&mut self.settings.is_amoled, "True Pitch-Black AMOLED (#000000)").changed() {
            let _ = self.settings_store.save(&self.settings);
        }
        ui.separator();

        ui.label(RichText::new("Webtoon Reader & GPU Memory Budget").strong());
        if ui.add(egui::Slider::new(&mut self.settings.ram_cache_budget_mb, 64..=512).text("RAM Tile Cache Budget (MB)")).changed() {
            let _ = self.settings_store.save(&self.settings);
        }
        if ui.add(egui::Slider::new(&mut self.settings.preload_pages_ahead, 1..=10).text("Preload Pages Ahead")).changed() {
            let _ = self.settings_store.save(&self.settings);
        }
        if ui.add(egui::Slider::new(&mut self.settings.seam_trim_px, 0..=12).text("Webtoon White-Seam Trim (px)")).changed() {
            let _ = self.settings_store.save(&self.settings);
        }
    }
}

fn sliced_to_textures(ctx: &egui::Context, url: &str, sliced: &SlicedPage) -> Vec<TextureHandle> {
    sliced
        .tiles
        .iter()
        .map(|tile| {
            let color_img = ColorImage::from_rgba_unmultiplied(
                [tile.width as usize, tile.height as usize],
                &tile.rgba_bytes,
            );
            ctx.load_texture(
                format!("{}_tile_{}", url, tile.tile_index),
                color_img,
                TextureOptions::LINEAR,
            )
        })
        .collect()
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("ZIBRA — Native Rust Manhwa & Manga Reader"),
        ..Default::default()
    };
    eframe::run_native(
        "ZIBRA",
        options,
        Box::new(|cc| Ok(Box::new(ZibraApp::new(cc)))),
    )
}
