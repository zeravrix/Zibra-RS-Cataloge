use crate::extractor::extract_chapter_pages;
use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct DilarSource {
    client: reqwest::Client,
    base_url: String,
    client_pub_b64: String,
}

impl DilarSource {
    pub fn new(client: reqwest::Client) -> Self {
        let mut raw_key = vec![4u8];
        let h1 = Sha256::digest(b"zibra-dilar-secp256r1-x");
        let h2 = Sha256::digest(b"zibra-dilar-secp256r1-y");
        raw_key.extend_from_slice(&h1);
        raw_key.extend_from_slice(&h2);
        let client_pub_b64 = URL_SAFE_NO_PAD.encode(&raw_key);
        Self {
            client,
            base_url: "https://dilar.tube".to_string(),
            client_pub_b64,
        }
    }

    fn api_headers(&self) -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("Accept".to_string(), "application/json".to_string());
        h.insert("X-DH-Pub".to_string(), self.client_pub_b64.clone());
        h.insert("X-Crypto-Caps".to_string(), "1,2,3,4,5,6,7,8,9,10".to_string());
        h.insert("Referer".to_string(), format!("{}/", self.base_url));
        h
    }

    async fn api_get(&self, url: &str) -> anyhow::Result<Value> {
        let mut req = self.client.get(url);
        for (k, v) in self.api_headers() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Dilar returned HTTP {} at {}", resp.status(), url);
        }
        Ok(resp.json::<Value>().await?)
    }

    async fn api_post(&self, url: &str, body_json: &str) -> anyhow::Result<Value> {
        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body_json.to_string());
        for (k, v) in self.api_headers() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Dilar POST returned HTTP {} at {}", resp.status(), url);
        }
        Ok(resp.json::<Value>().await?)
    }

    fn parse_series(&self, raw: &Value) -> Option<Manga> {
        if raw
            .get("seriesType")
            .and_then(|s| s.get("title"))
            .and_then(|t| t.as_str())
            == Some("رواية")
        {
            return None;
        }
        let id = raw.get("id")?.to_string().trim_matches('"').to_string();
        let title = raw
            .get("title")
            .or_else(|| raw.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();
        let cover = raw
            .get("cover")
            .and_then(|c| c.as_str())
            .map(|c| {
                let stem = c.rsplit_once('.').map(|(s, _)| s).unwrap_or(c);
                format!("{}/uploads/manga/cover/{}/large_{}.webp", self.base_url, id, stem)
            });
        let summary = raw
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let genres: Vec<String> = raw
            .get("categories")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| g.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let full_id = format!("{}/series/{}/{}", self.base_url, id, title.replace('/', "-"));
        let mut m = Manga::new(full_id, title, "DILAR");
        m.cover_url = cover;
        m.description = summary;
        m.genres = genres;
        m.is_manhwa = true;
        m.enrich_manhwa_tags();
        Some(m)
    }
}

#[async_trait]
impl Source for DilarSource {
    fn id(&self) -> &str { "DILAR" }
    fn name(&self) -> &str { "Dilar" }
    fn language(&self) -> &str { "ar" }
    fn base_url(&self) -> &str { &self.base_url }
    fn priority_rank(&self) -> u32 { 7 }
    fn tags(&self) -> &[SourceTag] {
        &[
            SourceTag::Arabic,
            SourceTag::Manhwa,
            SourceTag::Webtoon,
            SourceTag::OfficialApi,
            SourceTag::EcdhEncrypted,
            SourceTag::VerifiedWorking,
        ]
    }

    async fn popular(&self, _page: u32) -> anyhow::Result<MangaPageResult> {
        let root = self.api_get(&format!("{}/api/rankings", self.base_url)).await?;
        let items: Vec<Manga> = root
            .get("topSeries")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_series(v)).collect())
            .unwrap_or_default();
        Ok(MangaPageResult {
            has_next_page: false,
            items,
        })
    }

    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let p = page.max(1);
        let root = self
            .api_get(&format!("{}/api/series?page={}", self.base_url, p))
            .await?;
        let items: Vec<Manga> = root
            .get("series")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_series(v)).collect())
            .unwrap_or_default();
        let cur = root.get("currentPage").and_then(|v| v.as_u64()).unwrap_or(p as u64);
        let tot = root.get("totalPages").and_then(|v| v.as_u64()).unwrap_or(cur);
        Ok(MangaPageResult {
            has_next_page: cur < tot,
            items,
        })
    }

    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        let body = serde_json::json!({
            "query": query.trim(),
            "page": page.max(1),
        })
        .to_string();
        let root = self
            .api_post(&format!("{}/api/search/filter", self.base_url), &body)
            .await?;
        let items: Vec<Manga> = root
            .get("rows")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| self.parse_series(v)).collect())
            .unwrap_or_default();
        Ok(MangaPageResult {
            has_next_page: items.len() >= 20,
            items,
        })
    }

    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        let series_id = manga
            .id
            .split("/series/")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or(&manga.id);
        let root = self
            .api_get(&format!("{}/api/series/{}", self.base_url, series_id))
            .await?;
        Ok(self.parse_series(&root).unwrap_or_else(|| manga.clone()))
    }

    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        let series_id = manga
            .id
            .split("/series/")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or(&manga.id);
        let root = self
            .api_get(&format!("{}/api/series/{}/chapters", self.base_url, series_id))
            .await?;
        let mut list = Vec::new();
        if let Some(ch_arr) = root.get("chapters").and_then(|v| v.as_array()) {
            for ch in ch_arr {
                let ch_num_str = ch
                    .get("chapter")
                    .map(|v| v.to_string().trim_matches('"').to_string())
                    .unwrap_or_else(|| "0".to_string());
                let title_opt = ch.get("title").and_then(|t| t.as_str()).unwrap_or("");
                if let Some(rels) = ch.get("releases").and_then(|r| r.as_array()) {
                    for rel in rels {
                        let Some(rel_id) = rel.get("id").map(|v| v.to_string().trim_matches('"').to_string()) else {
                            continue;
                        };
                        let label = if title_opt.is_empty() {
                            format!("Chapter {}", ch_num_str)
                        } else {
                            format!("Ch. {} - {}", ch_num_str, title_opt)
                        };
                        let full_id = format!("{}/series/{}/{}#{}", self.base_url, series_id, ch_num_str, rel_id);
                        let num = ch_num_str.trim_end_matches(".00").parse::<f32>().ok();
                        list.push(Chapter::new(full_id, manga.id.clone(), label, num, "DILAR"));
                    }
                }
            }
        }
        Ok(list)
    }

    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let release_id = chapter
            .id
            .split('#')
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or(&chapter.id);
        let chapter_api = format!("{}/api/chapters/{}", self.base_url, release_id);
        if let Ok(root) = self.api_get(&chapter_api).await {
            if let Some(storage_key) = root.get("storage_key").and_then(|s| s.as_str()) {
                if let Some(arr) = root.get("pages").and_then(|p| p.as_array()) {
                    let mut pages = Vec::new();
                    for (idx, item) in arr.iter().enumerate() {
                        if let Some(rel_path) = item.get("url").and_then(|u| u.as_str()) {
                            let img_url = format!(
                                "{}/uploads/releases/{}/hq/{}",
                                self.base_url, storage_key, rel_path
                            );
                            pages.push(
                                MangaPage::new(idx, img_url, Some(format!("{}/", self.base_url)))
                                    .with_headers(self.api_headers()),
                            );
                        }
                    }
                    if !pages.is_empty() {
                        return Ok(pages);
                    }
                }
            }
        }

        let clean_url = chapter.id.split('#').next().unwrap_or(&chapter.id);
        let resp = self
            .client
            .get(clean_url)
            .header("Referer", format!("{}/", self.base_url))
            .send()
            .await?;
        let html = resp.text().await?;
        Ok(extract_chapter_pages(
            &html,
            clean_url,
            ".reading-content img, .chapter-images img, img",
            &self.api_headers(),
        ))
    }
}
