use std::sync::Arc;
use std::time::Duration;

use image::DynamicImage;
use ratatui::style::Color;
use tokio::sync::mpsc;

use crate::action::{Action, CoverRenderRequest, CoverSurface};
use crate::api::{self, SongRow};
use crate::cover_cache::{CoverCache, PixelKeyInputs};
use crate::pixel::{self, PixelCover};

use super::{Effects, PREVIEW_CELLS};

#[derive(Clone)]
pub(super) struct CoverLoad {
    pub(super) request: CoverRenderRequest,
    pub(super) style: CoverStyle,
}

#[derive(Clone, Copy)]
pub(super) struct CoverStyle {
    pub(super) pixel: PixelStyle,
    pub(super) original: bool,
}

#[derive(Clone, Copy)]
pub(super) struct PixelStyle {
    pub(super) palette_mode: pixel::CoverPalette,
    pub(super) palette: &'static [(u8, u8, u8)],
    pub(super) background: Color,
    pub(super) detail_scale: f32,
    pub(super) detail: pixel::CoverDetail,
}

/// Several surfaces of the same artwork share one fetch: the URL downloads
/// once and every pending size is derived from the same bytes, so they
/// cannot double-download or fail independently.
pub(super) fn spawn_cover_loads(fx: &Effects, loads: Vec<CoverLoad>, pic_url: String) {
    let cache = fx.covers.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let mut pending = Vec::new();
        for load in loads {
            let pixel = if !load.style.original {
                if let Some(cache) = &cache {
                    read_pixel_cover(cache.clone(), &load).await
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(pixel) = pixel {
                send_cover(&actions, &load.request, EitherCover::Pixel(pixel));
            } else {
                pending.push(load);
            }
        }
        let Some(first) = pending.first() else { return };
        let cached = if let Some(cache) = &cache {
            read_original_cover(cache.clone(), first.request.source_key.clone()).await
        } else {
            None
        };
        if let Some(bytes) = cached {
            let processed = process_covers(cache.clone(), &pending, bytes, false).await;
            pending = pending
                .into_iter()
                .zip(processed)
                .filter_map(|(load, processed)| {
                    if let Some(processed) = processed {
                        send_cover(&actions, &load.request, processed);
                        None
                    } else {
                        Some(load)
                    }
                })
                .collect();
        }
        let Some(first) = pending.first() else { return };
        let bytes = match api::fetch_cover(&pic_url, first.request.source_edge).await {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(%error, "cover fetch failed");
                for load in pending {
                    send_cover_failure(&actions, &load.request);
                }
                return;
            }
        };
        let processed = process_covers(cache, &pending, bytes, true).await;
        for (load, processed) in pending.iter().zip(processed) {
            match processed {
                Some(processed) => send_cover(&actions, &load.request, processed),
                None => send_cover_failure(&actions, &load.request),
            }
        }
    });
}

fn send_cover_failure(actions: &mpsc::UnboundedSender<Action>, request: &CoverRenderRequest) {
    let _ = actions.send(Action::CoverLoadFailed {
        request: request.clone(),
    });
}

pub(super) fn spawn_selection_cover_lookup(
    fx: &Effects,
    generation: u64,
    load: Option<CoverLoad>,
    row: SongRow,
    neighbors: Vec<SongRow>,
    hot: bool,
) {
    let cache = fx.covers.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        let due_at = tokio::time::Instant::now() + Duration::from_millis(150);
        let local_ready = if hot {
            true
        } else if let Some(load) = load.as_ref() {
            send_cached_cover(cache, &actions, load).await
        } else {
            false
        };
        tokio::time::sleep_until(due_at).await;
        let _ = actions.send(Action::SelectionCoverDue {
            generation,
            row,
            neighbors,
            needs_network: load.is_some() && !local_ready,
        });
    });
}

pub(super) fn spawn_cover_download(fx: &Effects, load: CoverLoad, pic_url: String) {
    let cache = fx.covers.clone();
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        if send_cached_cover(cache.clone(), &actions, &load).await {
            return;
        }
        download_and_send_cover(cache, &actions, load, pic_url).await;
    });
}

pub(super) async fn send_cached_cover(
    cache: Option<Arc<CoverCache>>,
    actions: &mpsc::UnboundedSender<Action>,
    load: &CoverLoad,
) -> bool {
    let Some(cache) = cache else {
        return false;
    };
    let loaded = if load.style.original {
        let Some(bytes) = read_original_cover(cache.clone(), load.request.source_key.clone()).await
        else {
            return false;
        };
        process_cover(Some(cache), load, bytes, false).await
    } else {
        load_cached_pixel(cache, load).await.map(EitherCover::Pixel)
    };
    let Some(loaded) = loaded else {
        return false;
    };
    send_cover(actions, &load.request, loaded);
    true
}

async fn read_pixel_cover(cache: Arc<CoverCache>, load: &CoverLoad) -> Option<PixelCover> {
    let pixel_key = pixel_cache_key(load);
    match tokio::task::spawn_blocking(move || cache.get_pixel(&pixel_key)).await {
        Ok(Ok(cover)) => cover,
        Ok(Err(error)) => {
            tracing::warn!(%error, "pixel cover cache read failed");
            None
        }
        Err(error) => {
            tracing::warn!(%error, "pixel cover cache worker failed");
            None
        }
    }
}

async fn load_cached_pixel(cache: Arc<CoverCache>, load: &CoverLoad) -> Option<PixelCover> {
    if let Some(cover) = read_pixel_cover(cache.clone(), load).await {
        return Some(cover);
    }
    let bytes = read_original_cover(cache.clone(), load.request.source_key.clone()).await?;
    match process_cover(Some(cache), load, bytes, false).await? {
        EitherCover::Pixel(cover) => Some(cover),
        EitherCover::Original(_) => None,
    }
}

async fn read_original_cover(cache: Arc<CoverCache>, source_key: String) -> Option<Vec<u8>> {
    match tokio::task::spawn_blocking(move || cache.get_original(&source_key)).await {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            tracing::warn!(%error, "original cover cache read failed");
            None
        }
        Err(error) => {
            tracing::warn!(%error, "original cover cache worker failed");
            None
        }
    }
}

async fn download_and_send_cover(
    cache: Option<Arc<CoverCache>>,
    actions: &mpsc::UnboundedSender<Action>,
    load: CoverLoad,
    pic_url: String,
) {
    let bytes = match api::fetch_cover(&pic_url, load.request.source_edge).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, "cover fetch failed");
            return;
        }
    };
    let Some(processed) = process_cover(cache, &load, bytes, true).await else {
        return;
    };
    send_cover(actions, &load.request, processed);
}

/// Scale so the longest edge is exactly `edge`: smaller sources are blown up
/// (Resize::Fit never upscales), larger ones shrink to what the box can use.
fn fit_cover_edge(image: DynamicImage, edge: u32) -> DynamicImage {
    let longest = image.width().max(image.height());
    if longest == edge || edge == 0 {
        return image;
    }
    fit_cover_edge_ref(&image, edge)
}

pub(super) async fn process_cover(
    cache: Option<Arc<CoverCache>>,
    load: &CoverLoad,
    bytes: Vec<u8>,
    downloaded: bool,
) -> Option<EitherCover> {
    process_covers(cache, std::slice::from_ref(load), bytes, downloaded)
        .await
        .pop()
        .flatten()
}

async fn process_covers(
    cache: Option<Arc<CoverCache>>,
    loads: &[CoverLoad],
    bytes: Vec<u8>,
    downloaded: bool,
) -> Vec<Option<EitherCover>> {
    let count = loads.len();
    let loads = loads.to_vec();
    let processed = tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes)?;
        if let Some(first) = loads.first() {
            write_original_cover(&cache, downloaded, &first.request.source_key, &bytes);
        }
        let mut covers: Vec<Option<EitherCover>> = (0..loads.len()).map(|_| None).collect();
        for (index, load) in loads
            .iter()
            .enumerate()
            .filter(|(_, load)| !load.style.original)
        {
            let style = load.style.pixel;
            match pixel::from_image(
                &image,
                style.palette_mode,
                style.palette,
                style.background,
                load.request.cells,
                style.detail_scale,
                style.detail,
            ) {
                Ok(cover) => {
                    if let Some(cache) = &cache {
                        if let Err(error) = cache.put_pixel(&pixel_cache_key(load), &cover) {
                            tracing::warn!(%error, "pixel cover cache write failed");
                        }
                    }
                    covers[index] = Some(EitherCover::Pixel(cover));
                }
                Err(error) => tracing::warn!(%error, "pixel cover render failed"),
            }
        }
        // The bar must shrink the same upscaled source as the playing cover.
        let source_edge = loads
            .iter()
            .filter(|load| load.style.original && load.request.surface != CoverSurface::Selection)
            .map(|load| load.request.source_edge)
            .max();
        let image = if let Some(edge) =
            source_edge.filter(|edge| image.width().max(image.height()) < *edge)
        {
            image.resize(edge, edge, image::imageops::FilterType::CatmullRom)
        } else {
            image
        };
        // Moving the large image first would force a full-size clone for the bar.
        let mut image = Some(image);
        let mut originals: Vec<_> = loads
            .iter()
            .enumerate()
            .filter(|(_, load)| load.style.original)
            .collect();
        originals.sort_by_key(|(_, load)| load.request.surface == CoverSurface::Playing);
        let last = originals.len().saturating_sub(1);
        for (position, (index, load)) in originals.into_iter().enumerate() {
            let edge = load.request.display_edge;
            let original = if position == last {
                fit_cover_edge(image.take().unwrap(), edge)
            } else {
                fit_cover_edge_ref(image.as_ref().unwrap(), edge)
            };
            covers[index] = Some(EitherCover::Original(original));
        }
        Ok::<_, anyhow::Error>(covers)
    })
    .await;
    match processed {
        Ok(Ok(covers)) => covers,
        Ok(Err(error)) => {
            tracing::warn!(%error, "cover decode failed");
            (0..count).map(|_| None).collect()
        }
        Err(error) => {
            tracing::warn!(%error, "cover worker failed");
            (0..count).map(|_| None).collect()
        }
    }
}

fn fit_cover_edge_ref(image: &DynamicImage, edge: u32) -> DynamicImage {
    let longest = image.width().max(image.height());
    if longest == edge || edge == 0 {
        return image.clone();
    }
    let filter = if longest < edge {
        image::imageops::FilterType::CatmullRom
    } else {
        image::imageops::FilterType::Triangle
    };
    image.resize(edge, edge, filter)
}

fn send_cover(
    actions: &mpsc::UnboundedSender<Action>,
    request: &CoverRenderRequest,
    cover: EitherCover,
) {
    let action = match cover {
        EitherCover::Pixel(cover) => Action::CoverLoaded {
            request: request.clone(),
            cover,
        },
        EitherCover::Original(image) => Action::CoverDecoded {
            request: request.clone(),
            image,
        },
    };
    let _ = actions.send(action);
}

fn pixel_cache_key(load: &CoverLoad) -> String {
    CoverCache::pixel_key(PixelKeyInputs {
        song_id: load.request.song_id,
        original_key: &load.request.source_key,
        cells: load.request.cells,
        detail_scale: load.style.pixel.detail_scale,
        detail: load.style.pixel.detail,
        palette_mode: load.style.pixel.palette_mode,
        background: load.style.pixel.background,
        palette: load.style.pixel.palette,
    })
}

pub(super) enum EitherCover {
    Pixel(PixelCover),
    Original(DynamicImage),
}

fn write_original_cover(
    cache: &Option<Arc<CoverCache>>,
    downloaded: bool,
    source_key: &str,
    bytes: &[u8],
) {
    if !downloaded {
        return;
    }
    if let Some(cache) = cache {
        if let Err(error) = cache.put_original(source_key, bytes) {
            tracing::warn!(%error, "original cover cache write failed");
        }
    }
}

pub(super) fn spawn_cover_prefetch(
    fx: &Effects,
    generation: u64,
    style_revision: u64,
    rows: Vec<SongRow>,
    style: CoverStyle,
    source_edge: u32,
) {
    let Some(cache) = fx.covers.clone() else {
        return;
    };
    let actions = fx.actions.clone();
    tokio::spawn(async move {
        for row in rows {
            let Some(pic_url) = row.pic_url.as_deref() else {
                continue;
            };
            let load = CoverLoad {
                request: CoverRenderRequest {
                    request_id: 0,
                    surface: CoverSurface::Selection,
                    generation,
                    cells: PREVIEW_CELLS,
                    style_revision,
                    song_id: row.id,
                    source_key: CoverCache::original_key(pic_url, source_edge),
                    source_edge,
                    display_edge: source_edge,
                },
                style,
            };
            if style.original {
                if read_original_cover(cache.clone(), load.request.source_key.clone())
                    .await
                    .is_some()
                {
                    continue;
                }
                let Ok(bytes) = api::fetch_cover(pic_url, load.request.source_edge).await else {
                    continue;
                };
                // Warm the disk only: decoding neighbours here filled the
                // decoded LRU with full-size RGBA for rows never opened.
                let cache = Some(cache.clone());
                let key = load.request.source_key.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    write_original_cover(&cache, true, &key, &bytes);
                })
                .await;
                continue;
            }
            let pixel_key = pixel_cache_key(&load);
            let cover = match load_cached_pixel(cache.clone(), &load).await {
                Some(cover) => cover,
                None => {
                    let Ok(bytes) = api::fetch_cover(pic_url, load.request.source_edge).await
                    else {
                        continue;
                    };
                    let Some(EitherCover::Pixel(cover)) =
                        process_cover(Some(cache.clone()), &load, bytes, true).await
                    else {
                        continue;
                    };
                    cover
                }
            };
            let _ = actions.send(Action::SelectionCoverWarmed {
                generation,
                style_revision,
                pixel_key,
                cover,
            });
        }
    });
}

#[cfg(test)]
mod tests;
