use crate::extractor::parse_chapter_number;
use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use serde_json::Value;
use url::Url;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

/// Mobile app host — the website host (e.g. meshmanga.com) redirects images to a decoy banner.
const APP_HOST: &str = "appswat.com";
const API_BASE: &str = "https://appswat.com/v2/api/v2";

/// Rewrite any website-host URL (including cached API responses or legacy saved IDs) to `appswat.com`.
pub fn to_app_host(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Ok(mut parsed) = Url::parse(trimmed) {
        let _ = parsed.set_scheme("https");
        let _ = parsed.set_host(Some(APP_HOST));
        return parsed.to_string();
    }
    trimmed.to_string()
}

fn parse_pages_from_json(root: &Value, referer: &str) -> Vec<MangaPage> {
    let Some(arr) = root
        .get("images")
        .or_else(|| root.get("pages"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };

    let mut entries: Vec<(i64, usize, String)> = arr
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let url = item
                .get("image")
                .or_else(|| item.get("url"))
                .and_then(|v| v.as_str())?;
            let order = item
                .get("order")
                .and_then(|v| v.as_i64())
                .unwrap_or(idx as i64);
            Some((order, idx, to_app_host(url)))
        })
        .collect();

    entries.sort_by_key(|(order, idx, _)| (*order, *idx));

    entries
        .into_iter()
        .enumerate()
        .map(|(i, (_, _, url))| MangaPage::new(i, url, Some(referer.to_string())))
        .collect()
}

pub struct MangaSwatSource {
    client: reqwest::Client,
    base_url: String,
    api_url: String,
}

impl MangaSwatSource {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: format!("https://{}", APP_HOST),
            api_url: API_BASE.to_string(),
        }
    }

    async fn api_get(&self, path_or_url: &str) -> anyhow::Result<Value> {
        let target = if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            to_app_host(path_or_url)
        } else {
            let rel = if let Some(idx) = path_or_url.find("/v2/api/v2/") {
                &path_or_url[idx + "/v2/api/v2".len()..]
            } else {
                path_or_url
            };
            format!(
                "{}/{}",
                self.api_url.trim_end_matches('/'),
                rel.trim_start_matches('/')
            )
        };

        let resp = self
            .client
            .get(&target)
            .header("Accept", "application/json")
            .header("Referer", format!("{}/", self.base_url))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HTTP {} from {}", resp.status(), target));
        }
        Ok(resp.json::<Value>().await?)
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
            .map(to_app_host);

        let full_id = format!("{}/series/{}/", self.api_url, id);
        let mut m = Manga::new(full_id, title, "MANGASWAT");
        m.cover_url = cover;
        m.is_manhwa = true;
        m.enrich_manhwa_tags();
        Some(m)
    }
}

#[async_trait]
impl Source for MangaSwatSource {
    fn id(&self) -> &str {
        "MANGASWAT"
    }
    fn name(&self) -> &str {
        "MangaSwat"
    }
    fn language(&self) -> &str {
        "ar"
    }
    fn base_url(&self) -> &str {
        &self.base_url
    }
    fn priority_rank(&self) -> u32 {
        2
    }
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
            .api_get(&format!(
                "/series/?order_by=-followers_count&page_size=100&page={}",
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

    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let root = self
            .api_get(&format!(
                "/series/releases/?page_size=100&page={}",
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

    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        let root = self
            .api_get(&format!(
                "/series/?search={}&page_size=100&page={}",
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
        if let Some(author) = root
            .get("author")
            .and_then(|a| a.get("name"))
            .and_then(|n| n.as_str())
        {
            updated.author = Some(author.to_string());
        }
        if let Some(status_id) = root
            .get("status")
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_i64())
        {
            updated.status = match status_id {
                79 => Some("Ongoing".to_string()),
                80 => Some("Completed".to_string()),
                81 => Some("On Hiatus".to_string()),
                _ => updated.status,
            };
        }
        let mut g = Vec::new();
        if let Some(type_name) = root
            .get("type")
            .and_then(|t| t.get("name"))
            .and_then(|n| n.as_str())
        {
            g.push(type_name.to_string());
        }
        if let Some(genres_arr) = root.get("genres").and_then(|v| v.as_array()) {
            for item in genres_arr {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    g.push(name.to_string());
                }
            }
        }
        if !g.is_empty() {
            updated.genres = g;
            updated.enrich_manhwa_tags();
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

        let mut list = Vec::new();
        for page in 1u32..=20 {
            let root = self
                .api_get(&format!(
                    "/chapters/?serie={}&order_by=-order&page_size=200&page={}",
                    series_id, page
                ))
                .await?;

            if let Some(arr) = root.get("results").and_then(|v| v.as_array()) {
                for raw in arr {
                    let Some(ch_id) = raw
                        .get("id")
                        .map(|v| v.to_string().trim_matches('"').to_string())
                    else {
                        continue;
                    };
                    let title = raw
                        .get("chapter")
                        .or_else(|| raw.get("title"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Chapter")
                        .to_string();
                    let full_url = format!("{}/chapters/{}/", self.api_url, ch_id);
                    let num = parse_chapter_number(&title);
                    let mut ch = Chapter::new(full_url, manga.id.clone(), title, num, "MANGASWAT");
                    if let Some(ts) = raw
                        .get("created_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    {
                        ch.upload_date = Some(ts.timestamp_millis());
                    }
                    list.push(ch);
                }
            }

            if root.get("next").and_then(|n| n.as_str()).is_none() {
                break;
            }
        }
        Ok(list)
    }

    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let root = self.api_get(&chapter.id).await?;
        Ok(parse_pages_from_json(&root, &format!("{}/", self.base_url)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rewrites_cached_website_host_to_appswat() {
        assert_eq!(
            to_app_host("https://meshmanga.com/v2/media/series/test/0001.webp"),
            "https://appswat.com/v2/media/series/test/0001.webp"
        );
        assert_eq!(
            to_app_host("http://swatmanhua.com/v2/api/v2/chapters/123/"),
            "https://appswat.com/v2/api/v2/chapters/123/"
        );
    }

    #[test]
    fn sorts_pages_by_order_and_rewrites_host() {
        let payload = json!({
            "images": [
                { "image": "https://meshmanga.com/v2/media/p3.webp", "order": 3 },
                { "image": "https://meshmanga.com/v2/media/p1.webp", "order": 1 },
                { "image": "https://appswat.com/v2/media/p2.webp", "order": 2 }
            ]
        });
        let pages = parse_pages_from_json(&payload, "https://appswat.com/");
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].image_url, "https://appswat.com/v2/media/p1.webp");
        assert_eq!(pages[1].image_url, "https://appswat.com/v2/media/p2.webp");
        assert_eq!(pages[2].image_url, "https://appswat.com/v2/media/p3.webp");
    }
}
