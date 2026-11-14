use crate::engines::madara::MadaraEngine;
use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct HijalaSource {
    inner: MadaraEngine,
}

impl HijalaSource {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            inner: MadaraEngine::new(
                "HIJALA",
                "Hijala",
                "https://hijala.com",
                "ar",
                7,
                vec![SourceTag::Arabic, SourceTag::Manhwa, SourceTag::VerifiedWorking],
                client,
            ),
        }
    }
}

#[async_trait]
impl Source for HijalaSource {
    fn id(&self) -> &str { self.inner.id() }
    fn name(&self) -> &str { self.inner.name() }
    fn language(&self) -> &str { self.inner.language() }
    fn base_url(&self) -> &str { self.inner.base_url() }
    fn priority_rank(&self) -> u32 { self.inner.priority_rank() }
    fn tags(&self) -> &[SourceTag] { self.inner.tags() }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.inner.popular(page).await
    }
    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.inner.latest(page).await
    }
    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        self.inner.search(page, query).await
    }
    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        self.inner.details(manga).await
    }
    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        self.inner.chapters(manga).await
    }
    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        self.inner.pages(chapter).await
    }
}
