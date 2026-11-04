use super::*;

fn load(surface: CoverSurface, original: bool) -> CoverLoad {
    let theme = crate::theme::Theme::db16();
    CoverLoad {
        request: CoverRenderRequest {
            request_id: 1,
            surface,
            generation: 1,
            cells: if surface == CoverSurface::Playing {
                (32, 16)
            } else {
                (6, 3)
            },
            style_revision: 0,
            song_id: 1,
            source_key: "batch-pixels".into(),
            source_edge: 128,
            display_edge: if surface == CoverSurface::Playing {
                128
            } else {
                48
            },
        },
        style: CoverStyle {
            original,
            pixel: PixelStyle {
                palette_mode: pixel::CoverPalette::Original,
                palette: theme.palette,
                background: theme.bg,
                detail_scale: 1.0,
                detail: pixel::CoverDetail::Half,
            },
        },
    }
}

fn artwork(width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([
            (x * 7) as u8,
            (y * 11) as u8,
            ((x + y) * 3) as u8,
            (x * 5 + y * 9) as u8,
        ])
    }));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[tokio::test]
async fn batched_pixel_surfaces_preserve_cells_and_partial_failure() {
    let bytes = artwork(31, 19);
    let mut loads = [
        load(CoverSurface::Playing, false),
        load(CoverSurface::Bar, false),
    ];
    for invalid_bar in [false, true] {
        if invalid_bar {
            loads[1].request.cells = (0, 0);
        }
        let covers = process_covers(None, &loads, bytes.clone(), false).await;
        for (load, actual) in loads.iter().zip(covers) {
            let style = load.style.pixel;
            let expected = pixel::from_image_bytes(
                &bytes,
                style.palette_mode,
                style.palette,
                style.background,
                load.request.cells,
                style.detail_scale,
                style.detail,
            );
            match (expected, actual) {
                (Ok(expected), Some(EitherCover::Pixel(actual))) => assert_eq!(actual, expected),
                (Err(_), None) => {}
                _ => panic!("surface output changed"),
            }
        }
    }
}
