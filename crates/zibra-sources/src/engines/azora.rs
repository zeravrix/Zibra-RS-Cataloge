use crate::{MangaPageResult, Source};
use async_trait::async_trait;
use serde_json::Value;
use zibra_core::{Chapter, Manga, MangaPage, SourceTag};

pub struct AzoraSource {
    client: reqwest::Client,
    base_url: String,
    api_url: String,
}

impl AzoraSource {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            base_url: "https://azorafly.com".to_string(),
            api_url: "https://api.azorafly.com".to_string(),
        }
    }

    async fn api_get(&self, url: &str) -> anyhow::Result<Value> {
        let resp = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("Referer", format!("{}/", self.base_url))
            .send()
            .await?
            .json::<Value>()
            .await?;
        Ok(resp)
    }

    fn parse_manga(&self, raw: &Value) -> Option<Manga> {
        if raw.get("isNovel").and_then(|b| b.as_bool()) == Some(true) {
            return None;
        }
        let id = raw.get("id")?.to_string().trim_matches('"').to_string();
        let slug = raw.get("slug").and_then(|s| s.as_str()).unwrap_or(&id);
        let title = raw
            .get("postTitle")
            .or_else(|| raw.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled")
            .to_string();
        let cover = raw
            .get("featuredImage")
            .or_else(|| raw.get("thumbnailUrl"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let desc = raw.get("postContent").and_then(|v| v.as_str()).unwrap_or_default().to_string();

        let full_id = format!("{}/series/{}#{}", self.base_url, slug, id);
        let mut m = Manga::new(full_id, title, "AZORA");
        m.cover_url = cover;
        m.description = desc;
        m.is_manhwa = true;
        m.enrich_manhwa_tags();
        Some(m)
    }

    async fn search_page(&self, page: u32, query: &str, order_by: &str, dir: &str) -> anyhow::Result<MangaPageResult> {
        let url = format!(
            "{}/api/query?page={}&perPage=18&searchTerm={}&orderBy={}&orderDirection={}",
            self.api_url,
            page.max(1),
            urlencoding::encode(query.trim()),
            order_by,
            dir
        );
        let root = self.api_get(&url).await?;
        let arr = root
            .get("posts")
            .or_else(|| root.get("results"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let items: Vec<Manga> = arr.iter().filter_map(|v| self.parse_manga(v)).collect();
        Ok(MangaPageResult {
            has_next_page: !items.is_empty(),
            items,
        })
    }
}

#[async_trait]
impl Source for AzoraSource {
    fn id(&self) -> &str { "AZORA" }
    fn name(&self) -> &str { "Azora" }
    fn language(&self) -> &str { "ar" }
    fn base_url(&self) -> &str { &self.base_url }
    fn priority_rank(&self) -> u32 { 3 }
    fn tags(&self) -> &[SourceTag] {
        &[
            SourceTag::Arabic,
            SourceTag::Manhwa,
            SourceTag::Webtoon,
            SourceTag::OfficialApi,
            SourceTag::VerifiedWorking,
        ]
    }

    async fn popular(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.search_page(page, "", "totalViews", "desc").await
    }

    async fn latest(&self, page: u32) -> anyhow::Result<MangaPageResult> {
        self.search_page(page, "", "lastChapterAddedAt", "desc").await
    }

    async fn search(&self, page: u32, query: &str) -> anyhow::Result<MangaPageResult> {
        self.search_page(page, query, "postTitle", "asc").await
    }

    async fn details(&self, manga: &Manga) -> anyhow::Result<Manga> {
        let slug = manga
            .id
            .split("/series/")
            .nth(1)
            .and_then(|s| s.split('#').next())
            .unwrap_or(&manga.id);
        let root = self
            .api_get(&format!("{}/api/post?postSlug={}", self.api_url, urlencoding::encode(slug)))
            .await?;
        let post = root.get("post").unwrap_or(&root);
        Ok(self.parse_manga(post).unwrap_or_else(|| manga.clone()))
    }

    async fn chapters(&self, manga: &Manga) -> anyhow::Result<Vec<Chapter>> {
        let slug = manga
            .id
            .split("/series/")
            .nth(1)
            .and_then(|s| s.split('#').next())
            .unwrap_or(&manga.id);

        // Primary: Azora's dedicated chapter endpoint /api/post/chapters?postSlug=<slug>
        let mut ch_arr = match self
            .api_get(&format!("{}/api/post/chapters?postSlug={}", self.api_url, urlencoding::encode(slug)))
            .await
        {
            Ok(val) => {
                if let Some(arr) = val.as_array() {
                    arr.clone()
                } else if let Some(arr) = val.get("post").and_then(|p| p.get("chapters")).and_then(|c| c.as_array()) {
                    arr.clone()
                } else {
                    Vec::new()
                }
            }
            Err(_) => Vec::new(),
        };

        // Fallback 1: Legacy /api/post?postSlug=<slug>
        if ch_arr.is_empty() {
            if let Ok(root) = self
                .api_get(&format!("{}/api/post?postSlug={}", self.api_url, urlencoding::encode(slug)))
                .await
            {
                if let Some(arr) = root.get("post").and_then(|p| p.get("chapters")).and_then(|c| c.as_array()) {
                    ch_arr = arr.clone();
                }
            }
        }

        // Fallback 2: /api/chapters?postId=<id>
        if ch_arr.is_empty() {
            if let Some(post_id) = manga.id.split('#').nth(1) {
                if !post_id.is_empty() {
                    if let Ok(root) = self
                        .api_get(&format!("{}/api/chapters?postId={}", self.api_url, post_id))
                        .await
                    {
                        if let Some(arr) = root.get("post").and_then(|p| p.get("chapters")).and_then(|c| c.as_array()) {
                            ch_arr = arr.clone();
                        }
                    }
                }
            }
        }

        let mut list = Vec::new();
        for raw in ch_arr {
            let Some(ch_id) = raw.get("id").map(|v| v.to_string().trim_matches('"').to_string()) else { continue };
            let ch_slug = raw.get("slug").and_then(|s| s.as_str()).unwrap_or(&ch_id);
            let num_str = raw
                .get("number")
                .or_else(|| raw.get("chapter"))
                .map(|v| v.to_string().trim_matches('"').to_string())
                .unwrap_or_else(|| "0".to_string());
            let full_id = format!("{}/series/{}/{}#{}", self.base_url, slug, ch_slug, ch_id);
            list.push(Chapter::new(
                full_id,
                manga.id.clone(),
                format!("Chapter {}", num_str),
                num_str.parse::<f32>().ok(),
                "AZORA",
            ));
        }
        Ok(list)
    }

    async fn pages(&self, chapter: &Chapter) -> anyhow::Result<Vec<MangaPage>> {
        let ch_id = chapter.id.split('#').nth(1).unwrap_or(&chapter.id);
        let root = self
            .api_get(&format!("{}/api/chapter?chapterId={}", self.api_url, ch_id))
            .await?;
        let ch_obj = root.get("chapter").unwrap_or(&root);
        let imgs = ch_obj
            .get("images")
            .or_else(|| ch_obj.get("pages"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut pages = Vec::new();
        for (idx, item) in imgs.iter().enumerate() {
            if let Some(url) = item.get("url").or_else(|| item.get("image")).and_then(|v| v.as_str()) {
                pages.push(MangaPage::new(
                    idx,
                    url.replace(' ', "%20"),
                    Some(chapter.id.clone()),
                ));
            }
        }
        Ok(pages)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_azora_chapter_array_deserialization() {
        let json_data = r#"[
            {"id": 75278, "number": 1, "slug": "chapter-1", "title": null},
            {"id": 75279, "number": 2, "slug": "chapter-2", "title": "Second Chapter"}
        ]"#;
        let parsed: serde_json::Value = serde_json::from_str(json_data).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].get("id").unwrap().as_i64().unwrap(), 75278);
        assert_eq!(arr[0].get("number").unwrap().as_i64().unwrap(), 1);
    }
}
