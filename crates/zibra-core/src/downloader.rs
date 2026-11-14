use crate::archive::{ArchiveImage, ChapterArchive};
use crate::models::{Chapter, DownloadedChapter, Manga, MangaPage};
use chrono::Utc;
use parking_lot::RwLock;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    Queued,
    Downloading { completed_pages: usize, total_pages: usize },
    Completed { archive_path: PathBuf },
    Failed { error: String },
}

#[derive(Debug, Clone)]
pub struct DownloadJobState {
    pub manga: Manga,
    pub chapter: Chapter,
    pub status: DownloadStatus,
}

#[derive(Clone)]
pub struct DesktopDownloader {
    downloads_dir: PathBuf,
    client: reqwest::Client,
    jobs: Arc<RwLock<Vec<DownloadJobState>>>,
    semaphore: Arc<Semaphore>,
}

impl DesktopDownloader {
    pub fn new(app_dir: &Path, max_concurrent: usize) -> Self {
        let downloads_dir = app_dir.join("downloads");
        let _ = fs::create_dir_all(&downloads_dir);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();

        Self {
            downloads_dir,
            client,
            jobs: Arc::new(RwLock::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    pub fn downloads_dir(&self) -> &Path {
        &self.downloads_dir
    }

    pub fn jobs(&self) -> Vec<DownloadJobState> {
        self.jobs.read().clone()
    }

    pub fn list_downloaded(&self) -> Vec<DownloadedChapter> {
        let mut results = Vec::new();
        let Ok(manga_dirs) = fs::read_dir(&self.downloads_dir) else {
            return results;
        };
        for m_entry in manga_dirs.flatten() {
            if !m_entry.path().is_dir() {
                continue;
            }
            let manga_title = m_entry.file_name().to_string_lossy().to_string();
            if manga_title.starts_with('.') {
                continue;
            }
            if let Ok(ch_entries) = fs::read_dir(m_entry.path()) {
                for ch_entry in ch_entries.flatten() {
                    let path = ch_entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("cbz") {
                        let mut manga_id = manga_title.clone();
                        let mut chapter_id = path.to_string_lossy().to_string();
                        let mut ch_title = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Chapter".to_string());
                        let mut source_id = "LOCAL_FILES".to_string();
                        let mut cover_url = None;

                        if let Ok(manifest_bytes) = ChapterArchive::read_entry(&path, "zibra.json") {
                            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&manifest_bytes) {
                                if let Some(mid) = val.get("mangaId").and_then(|v| v.as_str()) {
                                    manga_id = mid.to_string();
                                }
                                if let Some(cid) = val.get("chapterId").and_then(|v| v.as_str()) {
                                    chapter_id = cid.to_string();
                                }
                                if let Some(ct) = val.get("chapterTitle").and_then(|v| v.as_str()) {
                                    ch_title = ct.to_string();
                                }
                                if let Some(sid) = val.get("sourceId").and_then(|v| v.as_str()) {
                                    source_id = sid.to_string();
                                }
                                if let Some(cov) = val.get("mangaCoverUrl").and_then(|v| v.as_str()) {
                                    cover_url = Some(cov.to_string());
                                }
                            }
                        }

                        let page_count = ChapterArchive::image_entries(&path)
                            .map(|v| v.len())
                            .unwrap_or(0);
                        let downloaded_at = ch_entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        results.push(DownloadedChapter {
                            manga_id,
                            source_id,
                            manga_title: manga_title.clone(),
                            manga_cover_url: cover_url,
                            chapter_id,
                            chapter_title: ch_title,
                            number: None,
                            directory: m_entry.path(),
                            page_count,
                            downloaded_at,
                            archive_path: Some(path),
                        });
                    }
                }
            }
        }
        results.sort_by(|a, b| b.downloaded_at.cmp(&a.downloaded_at));
        results
    }

    pub fn downloaded_for_manga(&self, manga_id: &str) -> Vec<DownloadedChapter> {
        self.list_downloaded()
            .into_iter()
            .filter(|d| d.manga_id == manga_id)
            .collect()
    }

    pub fn local_pages(&self, dl: &DownloadedChapter) -> Vec<MangaPage> {
        let Some(ref archive_path) = dl.archive_path else {
            return Vec::new();
        };
        let Ok(entries) = ChapterArchive::image_entries(archive_path) else {
            return Vec::new();
        };
        entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| {
                let uri = format!("cbz://{}!{}", archive_path.to_string_lossy(), entry);
                MangaPage::new(idx, uri, None)
            })
            .collect()
    }

    pub async fn download_chapter(
        &self,
        manga: Manga,
        chapter: Chapter,
        pages: Vec<MangaPage>,
    ) -> anyhow::Result<PathBuf> {
        if pages.is_empty() {
            anyhow::bail!("No pages returned for chapter {}", chapter.title);
        }

        {
            let mut jobs = self.jobs.write();
            jobs.retain(|j| j.chapter.id != chapter.id);
            jobs.push(DownloadJobState {
                manga: manga.clone(),
                chapter: chapter.clone(),
                status: DownloadStatus::Downloading {
                    completed_pages: 0,
                    total_pages: pages.len(),
                },
            });
        }

        let safe_manga = sanitize_filename(&manga.title);
        let safe_chapter = sanitize_filename(&chapter.title);
        let manga_dir = self.downloads_dir.join(&safe_manga);
        let temp_dir = manga_dir.join(format!(".tmp_{}", safe_chapter));
        fs::create_dir_all(&temp_dir)?;

        let total = pages.len();
        let mut archive_images = Vec::with_capacity(total);

        let completed_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut join_handles = Vec::with_capacity(total);

        for (idx, page) in pages.into_iter().enumerate() {
            let dl_self = self.clone();
            let sem = self.semaphore.clone();
            let temp_dir_clone = temp_dir.clone();
            let ch_id = chapter.id.clone();
            let counter = completed_counter.clone();

            join_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await?;
                let bytes = dl_self.fetch_page_with_retry(&page).await?;

                if is_html_or_challenge(&bytes) {
                    anyhow::bail!("Page {} returned an HTML challenge instead of image bytes", idx + 1);
                }

                let ext = detect_image_ext(&bytes);
                let entry_name = format!("{:04}.{}", idx + 1, ext);
                let file_path = temp_dir_clone.join(&entry_name);
                fs::write(&file_path, &bytes)?;

                let done = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                dl_self.update_status(
                    &ch_id,
                    DownloadStatus::Downloading {
                        completed_pages: done,
                        total_pages: total,
                    },
                );

                Ok::<(usize, ArchiveImage), anyhow::Error>((
                    idx,
                    ArchiveImage {
                        entry_name,
                        file_path,
                    },
                ))
            }));
        }

        let mut indexed_images = Vec::with_capacity(total);
        for handle in join_handles {
            match handle.await {
                Ok(Ok(pair)) => indexed_images.push(pair),
                Ok(Err(e)) => {
                    self.update_status(
                        &chapter.id,
                        DownloadStatus::Failed {
                            error: e.to_string(),
                        },
                    );
                    return Err(e);
                }
                Err(join_err) => {
                    let msg = format!("Download worker panicked: {}", join_err);
                    self.update_status(&chapter.id, DownloadStatus::Failed { error: msg.clone() });
                    anyhow::bail!(msg);
                }
            }
        }
        indexed_images.sort_by_key(|(idx, _)| *idx);
        for (_, img) in indexed_images {
            archive_images.push(img);
        }

        let target_cbz = manga_dir.join(format!("{}.cbz", safe_chapter));
        let comic_info = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo>\n  <Series>{}</Series>\n  <Title>{}</Title>\n  <PageCount>{}</PageCount>\n</ComicInfo>",
            xml_escape(&manga.title),
            xml_escape(&chapter.title),
            total
        );
        let manifest = serde_json::json!({
            "mangaId": manga.id,
            "sourceId": manga.source_id,
            "mangaTitle": manga.title,
            "mangaCoverUrl": manga.cover_url,
            "chapterId": chapter.id,
            "chapterTitle": chapter.title,
            "downloadedAt": Utc::now().timestamp_millis(),
        })
        .to_string();

        ChapterArchive::write(&target_cbz, &archive_images, Some(&comic_info), Some(&manifest))?;
        let _ = fs::remove_dir_all(&temp_dir);

        self.update_status(
            &chapter.id,
            DownloadStatus::Completed {
                archive_path: target_cbz.clone(),
            },
        );

        Ok(target_cbz)
    }

    async fn fetch_page_with_retry(&self, page: &MangaPage) -> anyhow::Result<Vec<u8>> {
        let mut last_err = None;
        let origin_referer = url::Url::parse(&page.image_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| format!("{}://{}/", u.scheme(), h)));

        let mut referer_candidates: Vec<Option<String>> = Vec::new();
        if let Some(ref r) = page.referer {
            if !r.trim().is_empty() {
                referer_candidates.push(Some(r.clone()));
            }
        }
        if let Some(orig) = origin_referer {
            if !referer_candidates.iter().any(|c| c.as_deref() == Some(orig.as_str())) {
                referer_candidates.push(Some(orig));
            }
        }
        referer_candidates.push(None);

        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
            for ref_opt in &referer_candidates {
                let mut req = self
                    .client
                    .get(&page.image_url)
                    .header(
                        "Accept",
                        "image/webp,image/png,image/jpeg,image/gif,image/avif,image/*,*/*;q=0.8",
                    )
                    .header("Accept-Language", "en-US,en;q=0.9,ar;q=0.8");
                for (k, v) in &page.headers {
                    if !k.eq_ignore_ascii_case("referer") {
                        req = req.header(k, v);
                    }
                }
                if let Some(ref r) = ref_opt {
                    req = req.header("Referer", r);
                }
                match req.send().await {
                    Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                        Ok(b) if b.len() >= 64 && !is_html_or_challenge(&b) => return Ok(b.to_vec()),
                        Ok(_) => last_err = Some(anyhow::anyhow!("Image payload too small or HTML challenge")),
                        Err(e) => last_err = Some(e.into()),
                    },
                    Ok(resp) => {
                        last_err = Some(anyhow::anyhow!("HTTP {} for {}", resp.status(), page.image_url));
                    }
                    Err(e) => last_err = Some(e.into()),
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Failed to download page")))
    }

    fn update_status(&self, chapter_id: &str, status: DownloadStatus) {
        let mut jobs = self.jobs.write();
        if let Some(job) = jobs.iter_mut().find(|j| j.chapter.id == chapter_id) {
            job.status = status;
        }
    }
}

pub fn is_html_or_challenge(bytes: &[u8]) -> bool {
    let head_len = bytes.len().min(512);
    let head = String::from_utf8_lossy(&bytes[..head_len]).trim_start().to_ascii_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html") || head.contains("cloudflare")
}

pub fn detect_image_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "webp"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else {
        "jpg"
    }
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.chars().take(100).collect()
    }
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
