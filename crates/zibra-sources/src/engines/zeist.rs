use crate::extractor::{extract_chapter_pages, parse_chapter_number};
use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct ZeistEngine {
    id: String,
    name: String,
    base_url: String,
    lang: String,
    rank: u32,
    tags: Vec<SourceTag>,
    client: reqwest::Client,
}

impl ZeistEngine {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        lang: impl Into<String>,
        rank: u32,
        tags: Vec<SourceTag>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            lang: lang.into(),
            rank,
            tags,
            client,
        }
    }

    async fn fetch_feed(&self, url: &str) -> anyhow::Result<Value> {
        let resp = self
            .client
            .get(url)
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Accept-Language", "en-US,en;q=0.9,ar;q=0.8")
            .header("Referer", format!("{}/", self.base_url))
            .header("sec-ch-ua", "\"Google Chrome\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("{} returned HTTP {} at {}", self.name, resp.status(), url);
        }
        Ok(resp.json::<Value>().await?)
    }

    fn parse_entries(&self, root: &Value) -> Vec<Manga> {
        let mut items = Vec::new();
        let Some(entries) = root
            .get("feed")
            .and_then(|f| f.get("entry"))
            .and_then(|e| e.as_array())
        else {
            return items;
        };

        for entry in entries {
            let title = entry
                .get("title")
                .and_then(|t| t.get("$t"))
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .trim()
                .to_string();
            let mut alt_url = None;
            if let Some(links) = entry.get("link").and_then(|l| l.as_array()) {
                for l in links {
                    if l.get("rel").and_then(|r| r.as_str()) == Some("alternate") {
                        alt_url = l.get("href").and_then(|h| h.as_str()).map(String::from);
                        break;
                    }
                }
            }
            let Some(url) = alt_url else { continue };
            let cover = entry
                .get("media$thumbnail")
                .and_then(|m| m.get("url"))
                .and_then(|u| u.as_str())
                .map(|u| u.replace("/s72-c/", "/w400/").replace("=s72-c", "=w400"));

            let mut manga = Manga::new(url, title, &self.id);
            manga.cover_url = cover;
            manga.enrich_manhwa_tags();
            items.push(manga);
        }
        items
    }
}

#[async_trait]
impl Source for ZeistEngine {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn language(&self) -> &str { &self.lang }
    fn base_url(&self) -> &str { &self.base_url }
    fn priority_rank(&self) -> u32 { self.rank }
    fn tags(&self) -> &[SourceTag] { &self.tags }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.latest(page).await
    }

    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let start = ((page.max(1) - 1) * 20) + 1;
        let url = format!(
            "{}/feeds/posts/default/-/Series?alt=json&start-index={}&max-results=20",
            self.base_url, start
        );
        let root = match self.fetch_feed(&url).await {
            Ok(v) => v,
            Err(_) => {
                let fallback = format!(
                    "{}/feeds/posts/default?alt=json&start-index={}&max-results=20",
                    self.base_url, start
                );
                self.fetch_feed(&fallback).await?
            }
        };
        let items = self.parse_entries(&root);
        Ok(MangaPageResult {
            has_next_page: !items.is_empty(),
            items,
        })
    }

    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        let start = ((page.max(1) - 1) * 20) + 1;
        let url = format!(
            "{}/feeds/posts/default?alt=json&q={}&start-index={}&max-results=20",
            self.base_url,
            urlencoding::encode(query.trim()),
            start
        );
        let root = self.fetch_feed(&url).await?;
        let items = self.parse_entries(&root);
        Ok(MangaPageResult {
            has_next_page: !items.is_empty(),
            items,
        })
    }

    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        Ok(manga.clone())
    }

    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        let url = format!(
            "{}/feeds/posts/default/-/{}?alt=json&max-results=500",
            self.base_url,
            urlencoding::encode(&manga.title)
        );
        let mut list = Vec::new();
        if let Ok(root) = self.fetch_feed(&url).await {
            if let Some(entries) = root
                .get("feed")
                .and_then(|f| f.get("entry"))
                .and_then(|e| e.as_array())
            {
                for entry in entries {
                    let title = entry
                        .get("title")
                        .and_then(|t| t.get("$t"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Chapter")
                        .trim()
                        .to_string();
                    let mut href = None;
                    if let Some(links) = entry.get("link").and_then(|l| l.as_array()) {
                        for l in links {
                            if l.get("rel").and_then(|r| r.as_str()) == Some("alternate") {
                                href = l.get("href").and_then(|h| h.as_str()).map(String::from);
                                break;
                            }
                        }
                    }
                    if let Some(ch_url) = href {
                        if ch_url != manga.id {
                            let num = parse_chapter_number(&title);
                            list.push(Chapter::new(ch_url, manga.id.clone(), title, num, &self.id));
                        }
                    }
                }
            }
        }
        Ok(list)
    }

    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let resp = self
            .client
            .get(&chapter.id)
            .header("Referer", format!("{}/", self.base_url))
            .send()
            .await?;
        let html = resp.text().await?;
        let mut headers = HashMap::new();
        headers.insert("Referer".to_string(), chapter.id.clone());
        Ok(extract_chapter_pages(
            &html,
            &chapter.id,
            ".post-body img, #reader img, .separator img, article img",
            &headers,
        ))
    }
}
