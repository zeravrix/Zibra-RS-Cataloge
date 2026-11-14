use crate::engines::{
    azora::AzoraSource,
    dilar::DilarSource,
    hijala::HijalaSource,
    local::LocalFolderSource,
    madara::MadaraEngine,
    mangadex::MangaDexSource,
    mangaswat::MangaSwatSource,
    mangathemesia::MangaThemesiaEngine,
    servers::SelfHostedServerSource,
    teamx::TeamXSource,
};
use crate::{MangaPageResult, Source};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use zibra_core::{CatalogSortMode, SourceTag};

pub struct LiveSourceRegistry {
    sources: Vec<Arc<dyn Source>>,
    source_map: HashMap<String, Arc<dyn Source>>,
    latency_ms: Arc<RwLock<HashMap<String, u64>>>,
}

impl Default for LiveSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveSourceRegistry {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();

        let mut sources: Vec<Arc<dyn Source>> = vec![
            Arc::new(MangaDexSource::new(client.clone())),
            Arc::new(MangaSwatSource::new(client.clone())),
            Arc::new(AzoraSource::new(client.clone())),
            Arc::new(TeamXSource::new(client.clone())),
            Arc::new(DilarSource::new(client.clone())),
            Arc::new(MadaraEngine::new(
                "MANGALEK",
                "Mangalek",
                "https://mangalik.net",
                "ar",
                5,
                vec![SourceTag::Arabic, SourceTag::Manhwa, SourceTag::Popular, SourceTag::VerifiedWorking],
                client.clone(),
            )),
            Arc::new(MadaraEngine::new(
                "MANGASPARK",
                "MangaSpark",
                "https://sparkmanga.net",
                "ar",
                6,
                vec![SourceTag::Arabic, SourceTag::Manhwa, SourceTag::VerifiedWorking],
                client.clone(),
            )),
            Arc::new(HijalaSource::new(client.clone())),
            Arc::new(MangaThemesiaEngine::new(
                "ASURASCANS",
                "Asura Scans",
                "https://asuracomic.net",
                "en",
                8,
                vec![SourceTag::English, SourceTag::Manhwa, SourceTag::Webtoon, SourceTag::Popular, SourceTag::VerifiedWorking],
                client.clone(),
            )),
            Arc::new(MangaThemesiaEngine::new(
                "FLAMESCANS",
                "Flame Comics",
                "https://flamecomics.xyz",
                "en",
                9,
                vec![SourceTag::English, SourceTag::Manhwa, SourceTag::Webtoon, SourceTag::VerifiedWorking],
                client.clone(),
            )),
            Arc::new(
                MadaraEngine::new(
                    "TOONILY",
                    "Toonily",
                    "https://toonily.com",
                    "en",
                    10,
                    vec![SourceTag::English, SourceTag::Manhwa, SourceTag::Webtoon, SourceTag::FastCdn],
                    client.clone(),
                )
                .with_list_path("webtoon"),
            ),
            Arc::new(MadaraEngine::new(
                "HARIMANGA",
                "HariManga",
                "https://harimanga.me",
                "en",
                11,
                vec![SourceTag::English, SourceTag::Manhwa, SourceTag::Webtoon],
                client.clone(),
            )),
            Arc::new(MadaraEngine::new(
                "LEKMANGA",
                "LekManga",
                "https://lekmanga.net",
                "ar",
                12,
                vec![SourceTag::Arabic, SourceTag::Manhwa, SourceTag::Popular],
                client.clone(),
            )),
            Arc::new(MadaraEngine::new(
                "ARABTOONS",
                "ArabToons",
                "https://arabtoons.net",
                "ar",
                13,
                vec![SourceTag::Arabic, SourceTag::Manhwa, SourceTag::Webtoon],
                client.clone(),
            )),
            Arc::new(MadaraEngine::new(
                "MANHWACLAN",
                "ManhwaClan",
                "https://manhwaclan.com",
                "en",
                14,
                vec![SourceTag::English, SourceTag::Manhwa, SourceTag::Webtoon],
                client.clone(),
            )),
            Arc::new(MadaraEngine::new(
                "KUNMANGA",
                "KunManga",
                "https://kunmanga.com",
                "en",
                15,
                vec![SourceTag::English, SourceTag::Manhwa, SourceTag::Manhua],
                client.clone(),
            )),
            Arc::new(LocalFolderSource::new(PathBuf::from("./downloads"))),
            Arc::new(SelfHostedServerSource::new_komga("http://localhost:25600")),
            Arc::new(SelfHostedServerSource::new_kavita("http://localhost:5000")),
        ];

        // Catalog sources never replace hand-written ones that share a host.
        let taken: HashSet<String> = sources.iter().map(|s| crate::catalog::host_key(s.base_url())).collect();
        match crate::catalog::parse(crate::catalog::BUNDLED) {
            Ok(records) => sources.extend(crate::catalog::build(&records, &client, &taken, 100).0),
            Err(e) => eprintln!("zibra: bundled source catalog is invalid: {e}"),
        }

        let mut source_map = HashMap::new();
        for s in &sources {
            source_map.insert(s.id().to_string(), s.clone());
        }

        Self {
            sources,
            source_map,
            latency_ms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn total_count(&self) -> usize {
        self.sources.len()
    }

    pub fn visible_count(&self, nsfw_enabled: bool) -> usize {
        if nsfw_enabled {
            self.sources.len()
        } else {
            self.sources.iter().filter(|s| !s.is_nsfw()).count()
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn Source>> {
        self.source_map.get(id).cloned()
    }

    pub fn all_sources(&self) -> &[Arc<dyn Source>] {
        &self.sources
    }

    pub fn latency_for(&self, id: &str) -> Option<u64> {
        self.latency_ms.read().get(id).copied()
    }

    pub fn record_latency(&self, id: &str, ms: u64) {
        self.latency_ms.write().insert(id.to_string(), ms);
    }

    pub fn ordered_catalog(
        &self,
        sort_mode: CatalogSortMode,
        pinned_ids: &HashSet<String>,
        required_tags: &HashSet<SourceTag>,
        lang_filter: &str,
        search_query: &str,
        nsfw_enabled: bool,
    ) -> Vec<Arc<dyn Source>> {
        let q = search_query.trim().to_lowercase();
        let mut filtered: Vec<Arc<dyn Source>> = self
            .sources
            .iter()
            .filter(|s| {
                if !nsfw_enabled && s.is_nsfw() {
                    return false;
                }
                let match_lang = lang_filter == "ALL" || s.language().eq_ignore_ascii_case(lang_filter);
                let match_query = q.is_empty()
                    || s.name().to_lowercase().contains(&q)
                    || s.id().to_lowercase().contains(&q)
                    || s.tags().iter().any(|t| t.label().to_lowercase().contains(&q));
                let match_tags = required_tags.is_empty()
                    || required_tags.iter().all(|req| {
                        if *req == SourceTag::Nsfw {
                            s.is_nsfw()
                        } else {
                            s.tags().contains(req)
                        }
                    });
                match_lang && match_query && match_tags
            })
            .cloned()
            .collect();

        let latencies = self.latency_ms.read().clone();
        filtered.sort_by(|a, b| match sort_mode {
            CatalogSortMode::RecommendedOrder => a.priority_rank().cmp(&b.priority_rank()),
            CatalogSortMode::PinnedFirst => {
                let a_pin = pinned_ids.contains(a.id());
                let b_pin = pinned_ids.contains(b.id());
                b_pin
                    .cmp(&a_pin)
                    .then_with(|| a.priority_rank().cmp(&b.priority_rank()))
            }
            CatalogSortMode::FastestLatency => {
                let a_lat = latencies.get(a.id()).copied().unwrap_or(9999);
                let b_lat = latencies.get(b.id()).copied().unwrap_or(9999);
                a_lat
                    .cmp(&b_lat)
                    .then_with(|| a.priority_rank().cmp(&b.priority_rank()))
            }
            CatalogSortMode::Alphabetical => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
            CatalogSortMode::LanguageGroup => a
                .language()
                .cmp(b.language())
                .then_with(|| a.priority_rank().cmp(&b.priority_rank())),
        });

        filtered
    }

    pub async fn fetch_from_source(
        &self,
        source_id: &str,
        page: u32,
        latest: bool,
        query: Option<&str>,
        nsfw_enabled: bool,
    ) -> anyhow::Result<MangaPageResult> {
        let src = self
            .get(source_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown source {}", source_id))?;
        if !nsfw_enabled && src.is_nsfw() {
            anyhow::bail!("18+ NSFW source '{}' is hidden while Disable 18+ / NSFW is active.", src.name());
        }
        let start = Instant::now();
        let mut res = if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
            src.search(page, q).await
        } else if latest {
            src.latest(page).await
        } else {
            src.popular(page).await
        }?;
        self.record_latency(source_id, start.elapsed().as_millis() as u64);
        if !nsfw_enabled {
            res.items.retain(|m| !m.is_nsfw());
        }
        Ok(res)
    }

    pub async fn search_global(
        &self,
        query: &str,
        pinned_ids: &HashSet<String>,
        nsfw_enabled: bool,
    ) -> Vec<zibra_core::Manga> {
        let q = query.trim().to_string();
        if q.is_empty() {
            return Vec::new();
        }
        let targets: Vec<Arc<dyn Source>> = self
            .sources
            .iter()
            .filter(|s| (nsfw_enabled || !s.is_nsfw()) && (pinned_ids.contains(s.id()) || s.priority_rank() <= 12))
            .take(16)
            .cloned()
            .collect();

        let mut handles = Vec::new();
        for src in targets {
            let q_clone = q.clone();
            handles.push(tokio::spawn(async move {
                match tokio::time::timeout(Duration::from_secs(7), src.search(1, &q_clone)).await {
                    Ok(Ok(res)) => res.items,
                    _ => Vec::new(),
                }
            }));
        }

        let mut combined = Vec::new();
        let mut seen = HashSet::new();
        for h in handles {
            if let Ok(items) = h.await {
                for m in items {
                    if !nsfw_enabled && m.is_nsfw() {
                        continue;
                    }
                    if seen.insert(m.id.clone()) {
                        combined.push(m);
                    }
                }
            }
        }
        combined
    }
}
