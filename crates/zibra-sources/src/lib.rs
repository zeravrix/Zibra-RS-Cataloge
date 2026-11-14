pub mod catalog;
pub mod engines;
pub mod extractor;
pub mod registry;

use async_trait::async_trait;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub use registry::LiveSourceRegistry;

#[derive(Debug, Clone)]
pub struct MangaPageResult {
    pub items: Vec<Manga>,
    pub has_next_page: bool,
}

#[async_trait]
pub trait Source: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn language(&self) -> &str;
    fn base_url(&self) -> &str;
    fn priority_rank(&self) -> u32;
    fn tags(&self) -> &[SourceTag];

    fn icon_url(&self) -> String {
        let host = url::Url::parse(self.base_url())
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| format!("{}.com", self.name().to_lowercase().replace(' ', "")));
        format!("https://www.google.com/s2/favicons?domain={}&sz=128", host)
    }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult>;
    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult>;
    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult>;
    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga>;
    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>>;
    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>>;
}
