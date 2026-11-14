use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct SelfHostedServerSource {
    id: String,
    name: String,
    server_url: String,
    api_key: String,
    rank: u32,
    client: reqwest::Client,
}

impl SelfHostedServerSource {
    pub fn new_komga(server_url: impl Into<String>) -> Self {
        Self {
            id: "KOMGA".to_string(),
            name: "Komga Server".to_string(),
            server_url: server_url.into().trim_end_matches('/').to_string(),
            api_key: String::new(),
            rank: 50,
            client: reqwest::Client::new(),
        }
    }

    pub fn new_kavita(server_url: impl Into<String>) -> Self {
        Self {
            id: "KAVITA".to_string(),
            name: "Kavita Server".to_string(),
            server_url: server_url.into().trim_end_matches('/').to_string(),
            api_key: String::new(),
            rank: 51,
            client: reqwest::Client::new(),
        }
    }

    fn auth_headers(&self) -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("Accept".to_string(), "application/json".to_string());
        if !self.api_key.is_empty() {
            h.insert("X-API-Key".to_string(), self.api_key.clone());
            h.insert("x-api-key".to_string(), self.api_key.clone());
        }
        h
    }
}

#[async_trait]
impl Source for SelfHostedServerSource {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn language(&self) -> &str { "multi" }
    fn base_url(&self) -> &str { &self.server_url }
    fn priority_rank(&self) -> u32 { self.rank }
    fn tags(&self) -> &[SourceTag] {
        &[SourceTag::SelfHosted, SourceTag::OfficialApi, SourceTag::Manhwa, SourceTag::Manga]
    }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        let p = page.saturating_sub(1);
        let url = if self.id == "KOMGA" {
            format!("{}/api/v1/series?page={}&size=20&deleted=false", self.server_url, p)
        } else {
            format!("{}/api/Series/all?pageNumber={}&pageSize=20", self.server_url, page.max(1))
        };
        let mut req = self.client.get(&url);
        for (k, v) in self.auth_headers() {
            req = req.header(k, v);
        }
        let Ok(resp) = req.send().await else {
            return Ok(MangaPageResult { items: Vec::new(), has_next_page: false });
        };
        if !resp.status().is_success() {
            return Ok(MangaPageResult { items: Vec::new(), has_next_page: false });
        }
        let Ok(root) = resp.json::<Value>().await else {
            return Ok(MangaPageResult { items: Vec::new(), has_next_page: false });
        };
        let arr = root
            .get("content")
            .or_else(|| root.get("results"))
            .and_then(|v| v.as_array())
            .or_else(|| root.as_array())
            .cloned()
            .unwrap_or_default();
        let mut items = Vec::new();
        for raw in arr {
            let Some(sid) = raw.get("id").map(|v| v.to_string().trim_matches('"').to_string()) else { continue };
            let title = raw
                .get("metadata")
                .and_then(|m| m.get("title"))
                .or_else(|| raw.get("name"))
                .or_else(|| raw.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();
            let full_id = if self.id == "KOMGA" {
                format!("{}/api/v1/series/{}", self.server_url, sid)
            } else {
                format!("{}/api/Series/{}", self.server_url, sid)
            };
            let mut m = Manga::new(full_id.clone(), title, &self.id);
            m.cover_url = Some(format!("{}/thumbnail", full_id));
            items.push(m);
        }
        Ok(MangaPageResult {
            has_next_page: items.len() >= 20,
            items,
        })
    }
    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.popular(page).await
    }
    async fn search(&self, page: u32, _query: &str) -> anyhow::Result<MangaPageResult> {
        self.popular(page).await
    }
    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        Ok(manga.clone())
    }
    async fn chapters(&self, _manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        Ok(Vec::new())
    }
    async fn pages(&self, _chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        Ok(Vec::new())
    }
}
