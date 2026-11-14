use crate::extractor::parse_chapter_number;
use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use serde_json::Value;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct MangaSwatSource {
    client: reqwest::Client,
    base_url: String,
    api_urls: Vec<String>,
}

impl MangaSwatSource {
    pub fn new(client: reqwest::Client) -> Self {
        let base = "https://meshmanga.com".to_string();
        Self {
            client,
            api_urls: vec![
                "https://meshmanga.com/v2/api/v2".to_string(),
                "https://appswat.com/v2/api/v2".to_string(),
            ],
            base_url: base,
        }
    }

    async fn api_get(&self, path_or_url: &str) -> anyhow::Result<Value> {
        if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            let resp = self
                .client
                .get(path_or_url)
                .header("Accept", "application/json")
                .header("Referer", format!("{}/", self.base_url))
                .send()
                .await?;
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<Value>().await {
                    return Ok(v);
                }
            }
        }

        let rel = if let Some(idx) = path_or_url.find("/v2/api/v2/") {
            &path_or_url[idx + "/v2/api/v2".len()..]
        } else {
            path_or_url
        };

        let mut last_err = anyhow::anyhow!("MangaSwat API unreachable");
        for api_base in &self.api_urls {
            let target = format!("{}/{}", api_base.trim_end_matches('/'), rel.trim_start_matches('/'));
            match self
                .client
                .get(&target)
                .header("Accept", "application/json")
                .header("Referer", format!("{}/", self.base_url))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
                    Ok(v) => return Ok(v),
                    Err(e) => last_err = e.into(),
                },
                Ok(resp) => {
                    last_err = anyhow::anyhow!("HTTP {} from {}", resp.status(), target);
                }
                Err(e) => last_err = e.into(),
            }
        }
        Err(last_err)
    }

    fn parse_manga(&self, raw: &Value) -> Option<Manga> {
        let id = raw
            .get("id")
            .or_else(|| raw.get("serie_id"))
            .map(|v| v.to_string().trim_matches('"').to_string())?;
        let title = raw
            .get("title")
            .or_else(|| raw.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();
        let cover = raw
            .get("poster")
            .and_then(|p| {
                p.get("medium")
                    .or_else(|| p.get("large"))
                    .or_else(|| p.get("thumbnail"))
                    .and_then(|v| v.as_str())
                    .or_else(|| p.as_str())
            })
            .map(String::from);

        let full_id = format!("{}/series/{}/", self.api_urls[0], id);
        let mut m = Manga::new(full_id, title, "MANGASWAT");
        m.cover_url = cover;
        m.is_manhwa = true;
        m.enrich_manhwa_tags();
        Some(m)
    }
}

#[async_trait]
impl Source for MangaSwatSource {
    fn id(&self) -> &str { "MANGASWAT" }
    fn name(&self) -> &str { "MangaSwat (MeshManga)" }
    fn language(&self) -> &str { "ar" }
    fn base_url(&self) -> &str { &self.base_url }
    fn priority_rank(&self) -> u32 { 2 }
    fn tags(&self) -> &[SourceTag] {
        &[
            SourceTag::Arabic,
            SourceTag::Manhwa,
            SourceTag::Webtoon,
            SourceTag::OfficialApi,
            SourceTag::FastCdn,
            SourceTag::Popular,
            SourceTag::VerifiedWorking,
        ]
    }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let root = self
            .api_get(&format!("/series/?order_by=-followers_count&page={}", page.max(1)))
            .await?;
        let items: Vec<Manga> = root
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_manga(v)).collect())
            .unwrap_or_default();
        Ok(MangaPageResult {
            has_next_page: root.get("next").and_then(|n| n.as_str()).is_some(),
            items,
        })
    }

    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let root = self
            .api_get(&format!("/series/releases/?page={}", page.max(1)))
            .await?;
        let items: Vec<Manga> = root
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_manga(v)).collect())
            .unwrap_or_default();
        Ok(MangaPageResult {
            has_next_page: root.get("next").and_then(|n| n.as_str()).is_some(),
            items,
        })
    }

    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        let root = self
            .api_get(&format!(
                "/series/?search={}&page={}",
                urlencoding::encode(query.trim()),
                page.max(1)
            ))
            .await?;
        let items: Vec<Manga> = root
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_manga(v)).collect())
            .unwrap_or_default();
        Ok(MangaPageResult {
            has_next_page: root.get("next").and_then(|n| n.as_str()).is_some(),
            items,
        })
    }

    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        let root = self.api_get(&manga.id).await?;
        let mut updated = self.parse_manga(&root).unwrap_or_else(|| manga.clone());
        updated.description = root
            .get("story")
            .or_else(|| root.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if let Some(genres_arr) = root.get("genres").and_then(|v| v.as_array()) {
            let g: Vec<String> = genres_arr
                .iter()
                .filter_map(|item| item.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect();
            if !g.is_empty() {
                updated.genres = g;
                updated.enrich_manhwa_tags();
            }
        }
        Ok(updated)
    }

    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        let series_id = manga
            .id
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&manga.id);
        let root = self
            .api_get(&format!("/chapters/?serie={}&order_by=-order&page_size=200", series_id))
            .await?;

        let mut list = Vec::new();
        if let Some(arr) = root.get("results").and_then(|v| v.as_array()) {
            for raw in arr {
                let Some(ch_id) = raw.get("id").map(|v| v.to_string().trim_matches('"').to_string()) else { continue };
                let title = raw
                    .get("chapter")
                    .or_else(|| raw.get("title"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Chapter")
                    .to_string();
                let full_url = format!("{}/chapters/{}/", self.api_urls[0], ch_id);
                let num = parse_chapter_number(&title);
                list.push(Chapter::new(full_url, manga.id.clone(), title, num, "MANGASWAT"));
            }
        }
        Ok(list)
    }

    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let root = self.api_get(&chapter.id).await?;
        let mut pages = Vec::new();
        if let Some(arr) = root.get("images").or_else(|| root.get("pages")).and_then(|v| v.as_array()) {
            for (idx, item) in arr.iter().enumerate() {
                if let Some(url) = item.get("image").or_else(|| item.get("url")).and_then(|v| v.as_str()) {
                    pages.push(MangaPage::new(idx, url.trim(), Some(format!("{}/", self.base_url))));
                }
            }
        }
        Ok(pages)
    }
}
