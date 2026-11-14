use image::{DynamicImage, GenericImageView};
use moka::sync::Cache;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use zibra_core::{is_html_or_challenge, MangaPage};

pub const MAX_TILE_HEIGHT: u32 = 2048;
pub const MAX_SAFE_IMAGE_BYTES: u64 = 256 * 1024 * 1024; // 256 MB guard against decompression bombs
pub const MAX_SAFE_IMAGE_WIDTH: usize = 16384;
pub const MAX_SAFE_IMAGE_HEIGHT: usize = 65536;

struct InFlightGuard {
    url: String,
    set: Arc<parking_lot::Mutex<HashSet<String>>>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.set.lock().remove(&self.url);
    }
}

#[derive(Debug, Clone)]
pub struct DecodedTile {
    pub tile_index: usize,
    pub width: u32,
    pub height: u32,
    pub y_offset: u32,
    pub rgba_bytes: Arc<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct SlicedPage {
    pub page_index: usize,
    pub total_width: u32,
    pub total_height: u32,
    pub tiles: Vec<DecodedTile>,
}

impl SlicedPage {
    pub fn byte_weight(&self) -> u32 {
        self.tiles
            .iter()
            .map(|t| t.rgba_bytes.len() as u32)
            .sum::<u32>()
            .max(1)
    }
}

#[derive(Clone)]
pub struct ReaderPagePipeline {
    client: reqwest::Client,
    ram_cache: Cache<String, Arc<SlicedPage>>,
    disk_cache_dir: PathBuf,
    in_flight: Arc<parking_lot::Mutex<HashSet<String>>>,
    decode_semaphore: Arc<tokio::sync::Semaphore>,
}

impl ReaderPagePipeline {
    pub fn new(app_dir: &Path, ram_budget_mb: usize) -> Self {
        let disk_cache_dir = app_dir.join("cache").join("pages");
        let _ = fs::create_dir_all(&disk_cache_dir);

        let max_bytes = (ram_budget_mb.max(64) as u64) * 1024 * 1024;
        let ram_cache = Cache::builder()
            .max_capacity(max_bytes)
            .weigher(|_k: &String, v: &Arc<SlicedPage>| -> u32 { v.byte_weight() })
            .time_to_idle(Duration::from_secs(600))
            .build();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();

        Self {
            client,
            ram_cache,
            disk_cache_dir,
            in_flight: Arc::new(parking_lot::Mutex::new(HashSet::new())),
            decode_semaphore: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }

    pub fn is_in_flight(&self, image_url: &str) -> bool {
        self.in_flight.lock().contains(image_url)
    }

    pub fn get_cached(&self, image_url: &str) -> Option<Arc<SlicedPage>> {
        self.ram_cache.get(image_url)
    }

    pub fn prune_except(&self, active_urls: &[String]) {
        let keep: std::collections::HashSet<&String> = active_urls.iter().collect();
        for (k, _) in self.ram_cache.iter() {
            if !keep.contains(&k.as_ref()) {
                self.ram_cache.invalidate(k.as_ref());
            }
        }
    }

    pub async fn load_and_slice_page(
        &self,
        page: &MangaPage,
        seam_trim_px: u32,
    ) -> anyhow::Result<Arc<SlicedPage>> {
        if let Some(cached) = self.ram_cache.get(&page.image_url) {
            return Ok(cached);
        }

        // Single-flight coordinator: join/skip if already in-flight
        let _guard = {
            let mut lock = self.in_flight.lock();
            if lock.contains(&page.image_url) {
                return Err(anyhow::anyhow!("Request already in flight for {}", page.image_url));
            }
            lock.insert(page.image_url.clone());
            InFlightGuard {
                url: page.image_url.clone(),
                set: self.in_flight.clone(),
            }
        };

        let raw_bytes = self.fetch_raw_bytes(page).await?;
        let page_index = page.index;
        let url_key = page.image_url.clone();

        // Check dimensions safely before full decompression (P0-02 checked arithmetic)
        if let Ok(size) = imagesize::blob_size(&raw_bytes) {
            let estimated_bytes = (size.width as u64)
                .saturating_mul(size.height as u64)
                .saturating_mul(4);
            if estimated_bytes > MAX_SAFE_IMAGE_BYTES
                || size.width > MAX_SAFE_IMAGE_WIDTH
                || size.height > MAX_SAFE_IMAGE_HEIGHT
            {
                anyhow::bail!(
                    "Decompression guard: image {}x{} exceeds memory safety limit",
                    size.width,
                    size.height
                );
            }
        }

        // Bounded CPU decode permits
        let permit = self.decode_semaphore.clone().acquire_owned().await?;

        let sliced = tokio::task::spawn_blocking(move || -> anyhow::Result<SlicedPage> {
            let _permit = permit;
            let img = image::load_from_memory(&raw_bytes)?;
            Ok(slice_dynamic_image(img, page_index, seam_trim_px))
        })
        .await??;

        let arc_sliced = Arc::new(sliced);
        self.ram_cache.insert(url_key, arc_sliced.clone());
        Ok(arc_sliced)
    }

    async fn fetch_raw_bytes(&self, page: &MangaPage) -> anyhow::Result<Vec<u8>> {
        let key_hash = hex::encode(Sha256::digest(page.image_url.as_bytes()));
        let disk_file = self.disk_cache_dir.join(format!("{}.bin", key_hash));

        if disk_file.is_file() {
            if let Ok(bytes) = fs::read(&disk_file) {
                if bytes.len() > 64 && !is_html_or_challenge(&bytes) {
                    return Ok(bytes);
                }
            }
        }

        let mut req = self.client.get(&page.image_url);
        if let Some(ref referer) = page.referer {
            req = req.header("Referer", referer);
        }
        for (k, v) in &page.headers {
            req = req.header(k, v);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("HTTP {} fetching page {}", resp.status(), page.image_url);
        }
        let bytes = resp.bytes().await?.to_vec();
        if is_html_or_challenge(&bytes) {
            anyhow::bail!("HTML response returned instead of image");
        }

        let _ = fs::write(&disk_file, &bytes);
        Ok(bytes)
    }
}

pub fn slice_dynamic_image(
    img: DynamicImage,
    page_index: usize,
    seam_trim_px: u32,
) -> SlicedPage {
    let (w, h) = img.dimensions();
    let trim = seam_trim_px.min(h / 8);
    let effective_y_start = trim;
    let effective_h = h.saturating_sub(trim * 2).max(1);

    let mut tiles = Vec::new();
    let mut y = 0u32;
    let mut tile_index = 0usize;

    while y < effective_h {
        let slice_h = (effective_h - y).min(MAX_TILE_HEIGHT);
        let cropped = img.crop_imm(0, effective_y_start + y, w, slice_h);
        let rgba = cropped.to_rgba8().into_raw();
        tiles.push(DecodedTile {
            tile_index,
            width: w,
            height: slice_h,
            y_offset: y,
            rgba_bytes: Arc::new(rgba),
        });
        y += slice_h;
        tile_index += 1;
    }

    SlicedPage {
        page_index,
        total_width: w,
        total_height: effective_h,
        tiles,
    }
}
