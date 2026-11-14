use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;
use zibra_core::{Chapter, ChapterArchive, Manga, MangaPage, SourceTag};

pub struct LocalFolderSource {
    roots: Vec<PathBuf>,
}

impl LocalFolderSource {
    pub fn new(primary_root: PathBuf) -> Self {
        let mut roots = vec![primary_root];
        if let Some(home) = dirs::home_dir() {
            let zibra_dl = home.join(".zibra").join("downloads");
            let zibra_local = home.join(".zibra").join("local-library");
            let zeravrix_local = home.join(".zeravrix").join("local-library");
            let zeravrix_dl = home.join(".zeravrix").join("downloads");
            for p in [zibra_dl, zibra_local, zeravrix_local, zeravrix_dl] {
                if !roots.contains(&p) {
                    roots.push(p);
                }
            }
        }
        Self { roots }
    }

    fn scan_manga(&self, query: Option<&str>) -> Vec<Manga> {
        let q = query.unwrap_or("").trim().to_lowercase();
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for root in &self.roots {
            if !root.is_dir() {
                continue;
            }
            let Ok(entries) = fs::read_dir(root) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let title = entry.file_name().to_string_lossy().to_string();
                if title.starts_with('.') {
                    continue;
                }
                if !q.is_empty() && !title.to_lowercase().contains(&q) {
                    continue;
                }
                let id_str = path.to_string_lossy().to_string();
                if !seen.insert(id_str.clone()) {
                    continue;
                }
                let mut m = Manga::new(id_str, title, "LOCAL_FILES");
                m.description = format!("Offline local manga folder at {}", path.display());
                items.push(m);
            }
        }
        items
    }
}

#[async_trait]
impl Source for LocalFolderSource {
    fn id(&self) -> &str { "LOCAL_FILES" }
    fn name(&self) -> &str { "Local CBZ & Folders" }
    fn language(&self) -> &str { "multi" }
    fn base_url(&self) -> &str { "https://localhost" }
    fn priority_rank(&self) -> u32 { 49 }
    fn tags(&self) -> &[SourceTag] {
        &[SourceTag::LocalStorage, SourceTag::Manhwa, SourceTag::Manga, SourceTag::FastCdn, SourceTag::VerifiedWorking]
    }

    async fn popular(&self, _page: u32) -> anyhow::Result<MangaPageResult> {
        Ok(MangaPageResult {
            items: self.scan_manga(None),
            has_next_page: false,
        })
    }
    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.popular(page).await
    }
    async fn search(&self, _page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        Ok(MangaPageResult {
            items: self.scan_manga(Some(query)),
            has_next_page: false,
        })
    }
    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        Ok(manga.clone())
    }
    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        let dir = PathBuf::from(&manga.id);
        let mut chapters = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            paths.sort();
            for (idx, p) in paths.into_iter().enumerate() {
                let name = p
                    .file_stem()
                    .or_else(|| p.file_name())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("Chapter {}", idx + 1));
                if name.starts_with('.') {
                    continue;
                }
                if p.is_dir() || p.extension().and_then(|e| e.to_str()) == Some("cbz") {
                    chapters.push(Chapter::new(
                        p.to_string_lossy().to_string(),
                        manga.id.clone(),
                        name,
                        Some((idx + 1) as f32),
                        "LOCAL_FILES",
                    ));
                }
            }
        }
        Ok(chapters)
    }
    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let path = PathBuf::from(&chapter.id);
        let mut pages = Vec::new();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("cbz") {
            if let Ok(entries) = ChapterArchive::image_entries(&path) {
                for (idx, entry_name) in entries.into_iter().enumerate() {
                    let uri = format!("cbz://{}!{}", path.to_string_lossy(), entry_name);
                    pages.push(MangaPage::new(idx, uri, None));
                }
            }
        } else if path.is_dir() {
            if let Ok(entries) = fs::read_dir(&path) {
                let mut files: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && ChapterArchive::is_image_name(&p.to_string_lossy()))
                    .collect();
                files.sort();
                for (idx, f) in files.into_iter().enumerate() {
                    pages.push(MangaPage::new(idx, f.to_string_lossy().to_string(), None));
                }
            }
        }
        Ok(pages)
    }
}
