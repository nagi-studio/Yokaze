use super::*;

fn original_state(font: FontSize) -> AppState {
    let mut state = AppState::new(&Config::default());
    state.config.cover_mode = CoverMode::Original;
    #[allow(deprecated)]
    let picker = Picker::from_fontsize(font);
    state.original_cover = Some(OriginalCover::new(
        picker.clone(),
        mpsc::unbounded_channel().0,
    ));
    state.bar_original_cover = Some(OriginalCover::new(picker, mpsc::unbounded_channel().0));
    state.active_row = Some(covered_row(1));
    state.generation = 1;
    state
}

fn cached_effects(directory: &TempDir) -> (Effects, mpsc::UnboundedReceiver<Action>) {
    let mut fx = effects(directory);
    let (actions, receiver) = mpsc::unbounded_channel();
    fx.actions = actions;
    let cache = CoverCache::new(directory.path().join("covers"), 256 * 1024 * 1024).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    DynamicImage::new_rgba8(32, 32)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    for edge in COVER_SOURCE_EDGES {
        cache
            .put_original(
                &CoverCache::original_key("https://example.test/1.jpg", edge),
                bytes.get_ref(),
            )
            .unwrap();
    }
    fx.covers = Some(Arc::new(cache));
    (fx, receiver)
}

async fn receive_pair(
    state: &mut AppState,
    fx: &Effects,
    rx: &mut mpsc::UnboundedReceiver<Action>,
) {
    let mut received = 0;
    while received < 2 {
        let action = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        if matches!(action, Action::CoverDecoded { .. }) {
            received += 1;
        } else {
            assert!(matches!(action, Action::IdleArtLoaded { .. }));
        }
        state.update(action, fx);
    }
}

#[tokio::test]
async fn original_resize_reuses_the_source_until_the_resolution_tier_changes() {
    let directory = tempfile::tempdir().unwrap();
    let (fx, mut rx) = cached_effects(&directory);
    let mut state = original_state(FontSize::new(24, 53));
    state.terminal_size = (124, 20);
    state.load_playing_cover(&fx, &state.active_row.clone().unwrap());
    receive_pair(&mut state, &fx, &mut rx).await;
    let initial_edge = state.cover_source_edge();

    state.update(
        Action::Resize {
            cols: 125,
            rows: 21,
        },
        &fx,
    );
    assert_eq!(state.cover_source_edge(), initial_edge);
    assert!(tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .is_err());

    state.update(
        Action::Resize {
            cols: 160,
            rows: 60,
        },
        &fx,
    );
    assert_ne!(state.cover_source_edge(), initial_edge);
    receive_pair(&mut state, &fx, &mut rx).await;
    assert!(state.playing_original_visible());
}

#[tokio::test]
async fn original_resize_reuses_the_hidden_bars_full_source_when_it_becomes_visible() {
    let directory = tempfile::tempdir().unwrap();
    let (fx, mut rx) = cached_effects(&directory);
    let mut state = original_state(FontSize::new(10, 20));
    state.terminal_size = (100, 6);
    state.load_playing_cover(&fx, &state.active_row.clone().unwrap());
    receive_pair(&mut state, &fx, &mut rx).await;
    let edge = state.cover_source_edge();

    state.update(
        Action::Resize {
            cols: 100,
            rows: 24,
        },
        &fx,
    );
    assert_eq!(state.cover_source_edge(), edge);
    // The hidden bar retained the full tier, so it already has enough pixels.
    assert!(tokio::time::timeout(Duration::from_millis(100), rx.recv())
        .await
        .is_err());
    assert!(state.bar_original_visible());
}

#[tokio::test]
async fn late_decodes_and_failures_cannot_replace_a_newer_cover_request() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    for surface in [
        CoverSurface::Playing,
        CoverSurface::Bar,
        CoverSurface::Selection,
    ] {
        let mut state = original_state(FontSize::new(10, 20));
        state.selected_cover.generation = state.generation;
        state.selected_original_cover = Some(OriginalCover::new(
            #[allow(deprecated)]
            Picker::from_fontsize(FontSize::new(10, 20)),
            mpsc::unbounded_channel().0,
        ));
        let mut stale = state.cover_request(surface, 1, 1, "stale", (40, 20));
        state.cover_requests.record(&mut stale);
        let mut current = stale.clone();
        current.source_edge = 2_048;
        current.display_edge = 640;
        state.cover_requests.record(&mut current);
        state.update(
            Action::CoverDecoded {
                request: current,
                image: DynamicImage::new_rgba8(640, 640),
            },
            &fx,
        );
        state.update(
            Action::CoverDecoded {
                request: stale.clone(),
                image: DynamicImage::new_rgba8(320, 320),
            },
            &fx,
        );
        state.update(Action::CoverLoadFailed { request: stale }, &fx);

        let cover = match surface {
            CoverSurface::Playing => state.original_cover.as_ref(),
            CoverSurface::Bar => state.bar_original_cover.as_ref(),
            CoverSurface::Selection => state.selected_original_cover.as_ref(),
        }
        .unwrap();
        assert_eq!(cover.generation(), Some(1));
        let size = cover
            .protocol()
            .size_for(ratatui_image::Resize::Fit(None), (100, 100).into())
            .unwrap();
        assert_eq!((size.width, size.height), (64, 32), "{surface:?}");
    }
}

#[tokio::test]
async fn a_font_refresh_rejects_a_decode_even_when_its_edges_did_not_change() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = original_state(FontSize::new(10, 20));
    let mut stale = state.cover_request(CoverSurface::Playing, 1, 1, "font", (40, 20));
    state.cover_requests.record(&mut stale);
    state.refresh_graphics_geometry(FontSize::new(10, 21), &fx);
    state.update(
        Action::CoverDecoded {
            request: stale,
            image: DynamicImage::new_rgba8(320, 320),
        },
        &fx,
    );
    assert!(!state.playing_original_visible());
}

#[tokio::test]
async fn original_preview_uses_the_neighbors_prefetched_file_at_each_font_scale() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for font in [FontSize::new(24, 53), FontSize::new(60, 120)] {
        let directory = tempfile::tempdir().unwrap();
        let (fx, mut rx) = cached_effects(&directory);
        let mut state = original_state(font);
        state.terminal_size = (160, 60);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/cover.jpg", listener.local_addr().unwrap());
        let mut neighbor = row(2);
        neighbor.pic_url = Some(url.clone());
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let length = connection.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..length]).into_owned();
            let mut bytes = std::io::Cursor::new(Vec::new());
            DynamicImage::new_rgba8(32, 32)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.get_ref().len()
            );
            connection.write_all(header.as_bytes()).await.unwrap();
            connection.write_all(bytes.get_ref()).await.unwrap();
            request
        });
        let style = CoverStyle {
            pixel: state.pixel_style(),
            original: true,
        };
        spawn_cover_prefetch(
            &fx,
            1,
            0,
            vec![neighbor],
            style,
            state.selection_source_edge(),
        );
        let request = state.cover_request(CoverSurface::Selection, 1, 2, &url, PREVIEW_CELLS);
        tokio::time::timeout(Duration::from_secs(5), async {
            while fx
                .covers
                .as_ref()
                .unwrap()
                .get_original(&request.source_key)
                .unwrap()
                .is_none()
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let wire = server.await.unwrap();
        assert!(
            wire.contains(&format!("{}y{}", request.source_edge, request.source_edge)),
            "{wire}"
        );
        assert!(
            send_cached_cover(
                fx.covers.clone(),
                &fx.actions,
                &CoverLoad { request, style }
            )
            .await
        );
        assert!(matches!(
            rx.recv().await.unwrap(),
            Action::CoverDecoded { .. }
        ));
    }
}

#[tokio::test]
async fn switching_to_pixel_releases_original_art_and_keeps_the_picker() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = original_state(FontSize::new(10, 20));
    state.active_row = None;
    let make_cover = || {
        let mut cover = OriginalCover::buffered(
            Picker::halfblocks(),
            mpsc::unbounded_channel().0,
            mpsc::unbounded_channel().0,
        );
        cover.replace(1, DynamicImage::new_rgba8(64, 64));
        cover.replace(2, DynamicImage::new_rgba8(64, 64));
        cover
    };
    state.original_cover = Some(make_cover());
    state.selected_original_cover = Some(make_cover());
    state.bar_original_cover = Some(make_cover());
    state.config.cover_mode = CoverMode::Pixel;
    state.refresh_pixel_art(&fx);
    for cover in [
        &state.original_cover,
        &state.selected_original_cover,
        &state.bar_original_cover,
    ]
    .into_iter()
    .flatten()
    {
        assert!(cover.generation().is_none());
        assert!(!cover.has_pending());
        assert!(cover.protocol().protocol_type().is_none());
        assert_eq!(cover.picker.protocol_type(), ProtocolType::Halfblocks);
    }
}

#[tokio::test]
async fn a_failed_original_load_can_retry_on_a_resize_in_the_same_tier() {
    let directory = tempfile::tempdir().unwrap();
    let (fx, mut rx) = cached_effects(&directory);
    let mut state = original_state(FontSize::new(10, 20));
    state.terminal_size = (100, 24);
    state.load_playing_cover(&fx, &state.active_row.clone().unwrap());
    receive_pair(&mut state, &fx, &mut rx).await;
    let mut request = state.cover_request(
        CoverSurface::Playing,
        1,
        1,
        "https://example.test/1.jpg",
        state.desired_cover_cells(),
    );
    state.cover_requests.record(&mut request);
    state.update(Action::CoverLoadFailed { request }, &fx);
    assert!(!state.playing_original_visible());
    let edge = state.cover_source_edge();
    state.update(
        Action::Resize {
            cols: 101,
            rows: 24,
        },
        &fx,
    );
    assert_eq!(state.cover_source_edge(), edge);
    receive_pair(&mut state, &fx, &mut rx).await;
    assert!(state.playing_original_visible());
}

#[tokio::test]
async fn a_font_change_without_a_resize_resizes_the_cover_column() {
    let directory = tempfile::tempdir().unwrap();
    let fx = effects(&directory);
    let mut state = original_state(FontSize::new(8, 17));
    state.set_terminal_size_for_test(193, 79);
    state.ensure_placeholder();
    let before = state
        .placeholder
        .as_ref()
        .map(|art| (art.width, art.height));
    state.refresh_graphics_geometry(FontSize::new(19, 42), &fx);
    let desired = state.desired_cover_cells();
    assert_ne!(before, Some(desired));
    assert_eq!(
        state
            .placeholder
            .as_ref()
            .map(|art| (art.width, art.height)),
        Some(desired)
    );
}

#[test]
fn the_cover_box_is_sized_for_now_playing_from_any_view() {
    let mut state = original_state(FontSize::new(8, 17));
    state.set_terminal_size_for_test(193, 79);
    state.view = View::NowPlaying;
    let now_playing = state.desired_cover_cells();
    state.view = View::Library;
    assert_eq!(state.desired_cover_cells(), now_playing);
}
