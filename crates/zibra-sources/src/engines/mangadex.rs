use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use serde_json::Value;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct MangaDexSource {
    client: reqwest::Client,
    api_url: String,
}

impl MangaDexSource {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_url: "https://api.mangadex.org".to_string(),
        }
    }

    fn parse_manga_data(&self, data: &Value) -> Option<Manga> {
        let id = data.get("id")?.as_str()?.to_string();
        let attrs = data.get("attributes")?;
        let titles = attrs.get("title")?.as_object()?;
        let title = titles
            .get("en")
            .or_else(|| titles.get("ja-ro"))
            .or_else(|| titles.values().next())
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let desc = attrs
            .get("description")
            .and_then(|d| d.get("en"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let orig_lang = attrs.get("originalLanguage").and_then(|l| l.as_str()).unwrap_or("");
        let is_manhwa = orig_lang == "ko" || orig_lang == "zh";

        let mut cover_file = None;
        if let Some(rels) = data.get("relationships").and_then(|r| r.as_array()) {
            for rel in rels {
                if rel.get("type").and_then(|t| t.as_str()) == Some("cover_art") {
                    cover_file = rel
                        .get("attributes")
                        .and_then(|a| a.get("fileName"))
                        .and_then(|f| f.as_str())
                        .map(String::from);
                }
            }
        }

        let cover_url = cover_file.map(|f| format!("https://uploads.mangadex.org/covers/{}/{}", id, f));
        let mut m = Manga::new(id, title, "MANGADEX");
        m.cover_url = cover_url;
        m.description = desc;
        m.is_manhwa = is_manhwa;
        m.enrich_manhwa_tags();
        Some(m)
    }

    async fn get_json(&self, url: &str) -> anyhow::Result<Value> {
        let resp = self
            .client
            .get(url)
            .header("User-Agent", "Zibrax-App/2.2.0 (https://github.com/zeravrix/Project-Zibra)")
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("MangaDex returned HTTP {}: {}", status, text);
        }
        let root: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("MangaDex JSON error: {} (body len {})", e, text.len()))?;
        Ok(root)
    }

    async fn fetch_list(&self, url: &str) -> anyhow::Result<MangaPageResult> {
        let root = self.get_json(url).await?;
        let items: Vec<Manga> = root
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_manga_data(v)).collect())
            .unwrap_or_default();
        Ok(MangaPageResult {
            has_next_page: !items.is_empty(),
            items,
        })
    }
}

#[async_trait]
impl Source for MangaDexSource {
    fn id(&self) -> &str { "MANGADEX" }
    fn name(&self) -> &str { "MangaDex" }
    fn language(&self) -> &str { "en" }
    fn base_url(&self) -> &str { "https://mangadex.org" }
    fn priority_rank(&self) -> u32 { 1 }
    fn tags(&self) -> &[SourceTag] {
        &[
            SourceTag::English,
            SourceTag::MultiLang,
            SourceTag::Manhwa,
            SourceTag::Manga,
            SourceTag::OfficialApi,
            SourceTag::FastCdn,
            SourceTag::Popular,
            SourceTag::VerifiedWorking,
        ]
    }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let offset = (page.saturating_sub(1)) * 24;
        let url = format!(
            "{}/manga?limit=24&offset={}&includes[]=cover_art&contentRating[]=safe&contentRating[]=suggestive&order[followedCount]=desc",
            self.api_url, offset
        );
        self.fetch_list(&url).await
    }

    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let offset = (page.saturating_sub(1)) * 24;
        let url = format!(
            "{}/manga?limit=24&offset={}&includes[]=cover_art&contentRating[]=safe&contentRating[]=suggestive&order[latestUploadedChapter]=desc",
            self.api_url, offset
        );
        self.fetch_list(&url).await
    }

    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        let offset = (page.saturating_sub(1)) * 24;
        let url = format!(
            "{}/manga?limit=24&offset={}&title={}&includes[]=cover_art&contentRating[]=safe&contentRating[]=suggestive",
            self.api_url,
            offset,
            urlencoding::encode(query.trim())
        );
        self.fetch_list(&url).await
    }

    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        let url = format!("{}/manga/{}?includes[]=cover_art", self.api_url, manga.id);
        let root = self.get_json(&url).await?;
        if let Some(data) = root.get("data") {
            if let Some(m) = self.parse_manga_data(data) {
                return Ok(m);
            }
        }
        Ok(manga.clone())
    }

    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        let url = format!(
            "{}/manga/{}/feed?limit=200&translatedLanguage[]=en&translatedLanguage[]=ar&order[chapter]=desc",
            self.api_url, manga.id
        );
        let root = self.get_json(&url).await?;
        let mut list = Vec::new();
        if let Some(arr) = root.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                let Some(ch_id) = item.get("id").and_then(|v| v.as_str()) else { continue };
                let Some(attrs) = item.get("attributes") else { continue };
                let ch_num_str = attrs.get("chapter").and_then(|v| v.as_str()).unwrap_or("1");
                let ch_title = attrs.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let label = if ch_title.is_empty() {
                    format!("Chapter {}", ch_num_str)
                } else {
                    format!("Ch. {} - {}", ch_num_str, ch_title)
                };
                list.push(Chapter::new(
                    ch_id.to_string(),
                    manga.id.clone(),
                    label,
                    ch_num_str.parse::<f32>().ok(),
                    "MANGADEX",
                ));
            }
        }
        Ok(list)
    }

    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let url = format!("{}/at-home/server/{}", self.api_url, chapter.id);
        let root = self.get_json(&url).await?;
        let base_url = root
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing MangaDex at-home baseUrl"))?;
        let ch_obj = root
            .get("chapter")
            .ok_or_else(|| anyhow::anyhow!("Missing MangaDex chapter payload"))?;
        let hash = ch_obj.get("hash").and_then(|v| v.as_str()).unwrap_or("");
        let files = ch_obj
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut pages = Vec::new();
        for (idx, f) in files.iter().enumerate() {
            if let Some(fname) = f.as_str() {
                let img_url = format!("{}/data/{}/{}", base_url, hash, fname);
                pages.push(MangaPage::new(idx, img_url, Some("https://mangadex.org/".to_string())));
            }
        }
        Ok(pages)
    }
}
