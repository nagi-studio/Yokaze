use anyhow::{ensure, Result};
use image::imageops::{resize, FilterType};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use serde::{Deserialize, Serialize};
use std::fmt;

type Rgb = (u8, u8, u8);

const BAYER_4X4: [[i16; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
const DITHER_RANGE: i16 = 14;
const VISIBLE_ALPHA: u8 = 128;
// Below this squared distance the undithered match is faithful enough;
// dithering there would only speckle smooth gradients (the dirty-logo bug).
const CLEAN_MATCH_SQ: i32 = 2800;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverDetail {
    #[default]
    Half,
    Quad,
    Sextant,
    Octant,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverPalette {
    #[default]
    Original,
    Theme,
}

impl CoverPalette {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Theme => "theme",
        }
    }
}

impl fmt::Display for CoverPalette {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl CoverDetail {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Half => "half",
            Self::Quad => "quad",
            Self::Sextant => "sextant",
            Self::Octant => "octant",
        }
    }

    const fn sample_size(self) -> (u32, u32) {
        match self {
            Self::Half => (1, 2),
            Self::Quad => (2, 2),
            Self::Sextant => (2, 3),
            Self::Octant => (2, 4),
        }
    }
}

impl fmt::Display for CoverDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelCell {
    pub glyph: char,
    pub fg: Color,
    pub bg: Color,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PixelCover {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<PixelCell>,
}

/// Built-in idle art: a procedural vinyl record quantized to the theme
/// palette — no bundled asset, and it recolors with the theme like covers.
pub fn vinyl(
    palette: &[Rgb],
    background: Color,
    cell_width: u16,
    cell_height: u16,
    detail: CoverDetail,
) -> PixelCover {
    let last = palette.len().saturating_sub(1);
    let disc = palette[1.min(last)];
    let ring = palette[last / 2];
    let shine = palette[last * 5 / 8];
    let label = palette[last.saturating_sub(1).max(1)];

    let (samples_x, samples_y) = detail.sample_size();
    let width_px = f64::from(cell_width);
    let height_px = f64::from(cell_height) * 2.0;
    let (cx, cy) = (width_px / 2.0, height_px / 2.0);
    let radius = width_px.min(height_px) / 2.0 - 0.5;

    let mut cells = Vec::with_capacity(cell_width as usize * cell_height as usize);
    let color_at = |x: u32, y: u32| -> Color {
        let sample_x = (f64::from(x) + 0.5) / f64::from(samples_x);
        let sample_y = (f64::from(y) + 0.5) * 2.0 / f64::from(samples_y);
        let (dx, dy) = (sample_x - cx, sample_y - cy);
        let r = (dx * dx + dy * dy).sqrt();
        if r > radius {
            return background;
        }
        if r > radius - 1.0 {
            return to_color(ring);
        }
        if r < radius * 0.08 {
            return background; // spindle hole
        }
        if r < radius * 0.32 {
            return to_color(label);
        }
        // Groove zone: dark disc, sparse lighter rings, one specular wedge.
        let angle = dy.atan2(dx);
        let in_shine = (-2.5..-1.9).contains(&angle) || (0.6..1.2).contains(&angle);
        if in_shine && r > radius * 0.45 {
            to_color(shine)
        } else if (r as u32).is_multiple_of(4) {
            to_color(ring)
        } else {
            to_color(disc)
        }
    };
    let candidates = color_candidates(palette, background);
    for cell_y in 0..u32::from(cell_height) {
        for cell_x in 0..u32::from(cell_width) {
            let samples = cell_samples(samples_x, samples_y, |sample_x, sample_y| {
                color_at(cell_x * samples_x + sample_x, cell_y * samples_y + sample_y)
            });
            cells.push(select_cell(&samples, detail, &candidates));
        }
    }
    PixelCover {
        width: cell_width,
        height: cell_height,
        cells,
    }
}

pub fn from_image_bytes(
    bytes: &[u8],
    palette_mode: CoverPalette,
    palette: &[Rgb],
    background: Color,
    cells: (u16, u16),
    detail_scale: f32,
    detail: CoverDetail,
) -> Result<PixelCover> {
    validate_image_inputs(palette_mode, palette, cells)?;
    render_image(
        image::load_from_memory(bytes)?.to_rgba8(),
        palette_mode,
        palette,
        background,
        cells,
        detail_scale,
        detail,
    )
}

pub fn from_image(
    image: &image::DynamicImage,
    palette_mode: CoverPalette,
    palette: &[Rgb],
    background: Color,
    cells: (u16, u16),
    detail_scale: f32,
    detail: CoverDetail,
) -> Result<PixelCover> {
    validate_image_inputs(palette_mode, palette, cells)?;
    render_image(
        image.to_rgba8(),
        palette_mode,
        palette,
        background,
        cells,
        detail_scale,
        detail,
    )
}

fn validate_image_inputs(
    palette_mode: CoverPalette,
    palette: &[Rgb],
    cells: (u16, u16),
) -> Result<()> {
    let (cell_width, cell_height) = cells;
    ensure!(
        palette_mode == CoverPalette::Original || !palette.is_empty(),
        "theme pixel cover palette cannot be empty"
    );
    ensure!(
        cell_width > 0 && cell_height > 0,
        "pixel cover dimensions must be non-zero"
    );

    Ok(())
}

fn render_image(
    mut source: image::RgbaImage,
    palette_mode: CoverPalette,
    palette: &[Rgb],
    background: Color,
    cells: (u16, u16),
    detail_scale: f32,
    detail: CoverDetail,
) -> Result<PixelCover> {
    let (cell_width, cell_height) = cells;
    for pixel in source.pixels_mut() {
        let alpha = pixel.0[3];
        pixel.0[0] = premultiply_channel(pixel.0[0], alpha);
        pixel.0[1] = premultiply_channel(pixel.0[1], alpha);
        pixel.0[2] = premultiply_channel(pixel.0[2], alpha);
    }
    let (samples_x, samples_y) = detail.sample_size();
    let target_width = u32::from(cell_width) * samples_x;
    let target_height = u32::from(cell_height) * samples_y;
    let detail_scale = detail_scale.clamp(0.5, 4.0);
    let detail_width = ((target_width as f32 * detail_scale).round() as u32).max(1);
    let detail_height = ((target_height as f32 * detail_scale).round() as u32).max(1);
    let (resized_width, resized_height) = fitted_dimensions(
        source.width(),
        source.height(),
        detail_width,
        detail_height,
        samples_x,
        samples_y,
    );
    let resized = resize(&source, resized_width, resized_height, FilterType::Triangle);
    let offset_x = (detail_width - resized_width) / 2;
    let offset_y = (detail_height - resized_height) / 2;

    let mut detail_pixels = vec![background; detail_width as usize * detail_height as usize];
    for y in 0..resized_height {
        for x in 0..resized_width {
            let [red, green, blue, alpha] = resized.get_pixel(x, y).0;
            let target_x = x + offset_x;
            let target_y = y + offset_y;
            let index = target_y as usize * detail_width as usize + target_x as usize;
            let Some((red, green, blue)) =
                flatten_premultiplied((red, green, blue), alpha, background)
            else {
                detail_pixels[index] = background;
                continue;
            };
            detail_pixels[index] = Color::Rgb(red, green, blue);
        }
    }

    let theme_candidates = color_candidates(palette, background);
    let mut cells = Vec::with_capacity(cell_width as usize * cell_height as usize);
    for cell_y in 0..cell_height {
        for cell_x in 0..cell_width {
            let samples = cell_samples(samples_x, samples_y, |sample_x, sample_y| {
                let x = u32::from(cell_x) * samples_x + sample_x;
                let y = u32::from(cell_y) * samples_y + sample_y;
                let color = sampled_color(
                    &detail_pixels,
                    detail_width,
                    detail_height,
                    x,
                    y,
                    target_width,
                    target_height,
                );
                render_sample(color, background, palette_mode, palette, x, y)
            });
            let original_candidates;
            let candidates = match palette_mode {
                CoverPalette::Original => {
                    original_candidates = sample_candidates(&samples, detail);
                    original_candidates.as_slice()
                }
                CoverPalette::Theme => theme_candidates.as_slice(),
            };
            cells.push(select_cell(&samples, detail, candidates));
        }
    }

    Ok(PixelCover {
        width: cell_width,
        height: cell_height,
        cells,
    })
}

fn render_sample(
    color: Color,
    background: Color,
    palette_mode: CoverPalette,
    palette: &[Rgb],
    x: u32,
    y: u32,
) -> Color {
    if color == background {
        return color;
    }
    let Color::Rgb(red, green, blue) = color else {
        return color;
    };
    let offset = bayer_offset(x, y);
    match palette_mode {
        CoverPalette::Original => Color::Rgb(
            apply_offset(red, offset),
            apply_offset(green, offset),
            apply_offset(blue, offset),
        ),
        CoverPalette::Theme => {
            let plain = nearest_color((red, green, blue), palette);
            if color_distance_sq((red, green, blue), plain) <= CLEAN_MATCH_SQ {
                to_color(plain)
            } else {
                to_color(nearest_color(
                    (
                        apply_offset(red, offset),
                        apply_offset(green, offset),
                        apply_offset(blue, offset),
                    ),
                    palette,
                ))
            }
        }
    }
}

fn sample_candidates(samples: &[Color; 8], detail: CoverDetail) -> Vec<Color> {
    let count = match detail {
        CoverDetail::Half => 2,
        CoverDetail::Quad => 4,
        CoverDetail::Sextant => 6,
        CoverDetail::Octant => 8,
    };
    let mut candidates = Vec::with_capacity(count);
    for &color in &samples[..count] {
        if !candidates.contains(&color) {
            candidates.push(color);
        }
    }
    candidates
}

fn premultiply_channel(channel: u8, alpha: u8) -> u8 {
    ((u32::from(channel) * u32::from(alpha) + 127) / 255) as u8
}

fn flatten_premultiplied(foreground: Rgb, alpha: u8, background: Color) -> Option<Rgb> {
    if alpha == 0 {
        return None;
    }
    if let Color::Rgb(red, green, blue) = background {
        return Some((
            composite_channel(foreground.0, red, alpha),
            composite_channel(foreground.1, green, alpha),
            composite_channel(foreground.2, blue, alpha),
        ));
    }
    (alpha >= VISIBLE_ALPHA).then(|| {
        (
            unpremultiply_channel(foreground.0, alpha),
            unpremultiply_channel(foreground.1, alpha),
            unpremultiply_channel(foreground.2, alpha),
        )
    })
}

fn composite_channel(foreground: u8, background: u8, alpha: u8) -> u8 {
    let alpha = u32::from(alpha);
    ((u32::from(foreground) * 255 + u32::from(background) * (255 - alpha) + 127) / 255).min(255)
        as u8
}

fn unpremultiply_channel(channel: u8, alpha: u8) -> u8 {
    ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
}

fn sampled_color(
    pixels: &[Color],
    source_width: u32,
    source_height: u32,
    x: u32,
    y: u32,
    target_width: u32,
    target_height: u32,
) -> Color {
    let source_x = (x * source_width / target_width).min(source_width - 1);
    let source_y = (y * source_height / target_height).min(source_height - 1);
    pixels[(source_y * source_width + source_x) as usize]
}

fn cell_samples(
    sample_width: u32,
    sample_height: u32,
    mut color_at: impl FnMut(u32, u32) -> Color,
) -> [Color; 8] {
    let mut samples = [Color::Reset; 8];
    for y in 0..sample_height {
        for x in 0..sample_width {
            samples[(y * sample_width + x) as usize] = color_at(x, y);
        }
    }
    samples
}

fn color_candidates(palette: &[Rgb], background: Color) -> Vec<Color> {
    let mut candidates = Vec::with_capacity(palette.len() + 1);
    for &rgb in palette {
        let color = to_color(rgb);
        if !candidates.contains(&color) {
            candidates.push(color);
        }
    }
    if !candidates.contains(&background) {
        candidates.push(background);
    }
    candidates
}

fn select_cell(samples: &[Color; 8], detail: CoverDetail, candidates: &[Color]) -> PixelCell {
    match detail {
        CoverDetail::Half => half_cell(samples[0], samples[1]),
        CoverDetail::Quad => select_pattern_cell(&samples[..4], 16, candidates, quadrant_glyph),
        CoverDetail::Sextant => select_pattern_cell(&samples[..6], 64, candidates, sextant_glyph),
        CoverDetail::Octant => select_pattern_cell(&samples[..8], 256, candidates, octant_glyph),
    }
}

fn half_cell(upper: Color, lower: Color) -> PixelCell {
    match (upper == Color::Reset, lower == Color::Reset) {
        (true, true) => PixelCell {
            glyph: ' ',
            fg: Color::Reset,
            bg: Color::Reset,
        },
        (false, true) => PixelCell {
            glyph: '▀',
            fg: upper,
            bg: Color::Reset,
        },
        (true, false) => PixelCell {
            glyph: '▄',
            fg: lower,
            bg: Color::Reset,
        },
        (false, false) => PixelCell {
            glyph: '▀',
            fg: upper,
            bg: lower,
        },
    }
}

fn select_pattern_cell(
    samples: &[Color],
    mask_count: u16,
    candidates: &[Color],
    glyph_for_mask: fn(u8) -> char,
) -> PixelCell {
    debug_assert!(!candidates.is_empty());
    let mut best = PixelCell {
        glyph: ' ',
        fg: candidates[0],
        bg: candidates[0],
    };
    let mut best_error = u64::MAX;

    for mask in 0..mask_count {
        let mask = mask as u8;
        let fg = best_partition_color(samples, mask, true, candidates);
        let bg = best_partition_color(samples, mask, false, candidates);
        let error = pattern_error(samples, mask, fg, bg);
        if error < best_error {
            best_error = error;
            let (mask, fg, bg) = if fg == Color::Reset && bg != Color::Reset {
                (mask ^ (mask_count - 1) as u8, bg, fg)
            } else {
                (mask, fg, bg)
            };
            best = PixelCell {
                glyph: glyph_for_mask(mask),
                fg,
                bg,
            };
        }
    }
    best
}

fn best_partition_color(
    samples: &[Color],
    mask: u8,
    foreground: bool,
    candidates: &[Color],
) -> Color {
    let mut best = candidates[0];
    let mut best_error = u64::MAX;
    for &candidate in candidates {
        let error = samples
            .iter()
            .enumerate()
            .filter(|(index, _)| (((mask >> index) & 1) != 0) == foreground)
            .map(|(_, &sample)| terminal_color_distance_sq(sample, candidate))
            .sum();
        if error < best_error {
            best = candidate;
            best_error = error;
        }
    }
    best
}

fn pattern_error(samples: &[Color], mask: u8, fg: Color, bg: Color) -> u64 {
    samples
        .iter()
        .enumerate()
        .map(|(index, &sample)| {
            let candidate = if ((mask >> index) & 1) != 0 { fg } else { bg };
            terminal_color_distance_sq(sample, candidate)
        })
        .sum()
}

fn terminal_color_distance_sq(a: Color, b: Color) -> u64 {
    if a == b {
        return 0;
    }
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            color_distance_sq((ar, ag, ab), (br, bg, bb)) as u64
        }
        _ => 3 * u64::from(u8::MAX).pow(2),
    }
}

fn quadrant_glyph(mask: u8) -> char {
    const GLYPHS: [char; 16] = [
        ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
    ];
    GLYPHS[usize::from(mask)]
}

fn sextant_glyph(mask: u8) -> char {
    match mask {
        0 => ' ',
        21 => '▌',
        42 => '▐',
        63 => '█',
        _ => {
            let skipped = u32::from(mask > 21) + u32::from(mask > 42);
            char::from_u32(0x1fb00 + u32::from(mask) - 1 - skipped)
                .expect("sextant mask maps to Unicode")
        }
    }
}

fn octant_glyph(mask: u8) -> char {
    const LEGACY_GLYPHS: [(u8, char); 26] = [
        (0, ' '),
        (1, '\u{1cea8}'),
        (2, '\u{1ceab}'),
        (3, '\u{1fb82}'),
        (5, '▘'),
        (10, '▝'),
        (15, '▀'),
        (20, '\u{1fbe6}'),
        (40, '\u{1fbe7}'),
        (63, '\u{1fb85}'),
        (64, '\u{1cea3}'),
        (80, '▖'),
        (85, '▌'),
        (90, '▞'),
        (95, '▛'),
        (128, '\u{1cea0}'),
        (160, '▗'),
        (165, '▚'),
        (170, '▐'),
        (175, '▜'),
        (192, '▂'),
        (240, '▄'),
        (245, '▙'),
        (250, '▟'),
        (252, '▆'),
        (255, '█'),
    ];

    match LEGACY_GLYPHS.binary_search_by_key(&mask, |(legacy_mask, _)| *legacy_mask) {
        Ok(index) => LEGACY_GLYPHS[index].1,
        Err(skipped) => char::from_u32(0x1cd00 + u32::from(mask) - skipped as u32)
            .expect("octant mask maps to Unicode"),
    }
}

impl Widget for &PixelCover {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let draw_width = area.width.min(self.width);
        let draw_height = area.height.min(self.height);
        let destination_x = area.x + area.width.saturating_sub(self.width) / 2;
        let destination_y = area.y + area.height.saturating_sub(self.height) / 2;

        for y in 0..draw_height {
            for x in 0..draw_width {
                let cover_cell =
                    self.cells[usize::from(y) * usize::from(self.width) + usize::from(x)];
                let Some(buffer_cell) = buf.cell_mut((destination_x + x, destination_y + y)) else {
                    continue;
                };
                buffer_cell
                    .set_char(cover_cell.glyph)
                    .set_fg(cover_cell.fg)
                    .set_bg(cover_cell.bg);
            }
        }
    }
}

fn fitted_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
    samples_x: u32,
    samples_y: u32,
) -> (u32, u32) {
    let source_width = u128::from(source_width);
    let source_height = u128::from(source_height);
    let target_width = u128::from(target_width);
    let target_height = u128::from(target_height);
    let horizontal_weight = u128::from(samples_x) * 2;
    let vertical_weight = u128::from(samples_y);

    if source_width * horizontal_weight * target_height
        >= source_height * vertical_weight * target_width
    {
        let denominator = source_width * horizontal_weight;
        let numerator = source_height * vertical_weight * target_width;
        let height = ((numerator + denominator / 2) / denominator).clamp(1, target_height);
        (target_width as u32, height as u32)
    } else {
        let denominator = source_height * vertical_weight;
        let numerator = source_width * horizontal_weight * target_height;
        let width = ((numerator + denominator / 2) / denominator).clamp(1, target_width);
        (width as u32, target_height as u32)
    }
}

fn color_distance_sq(a: Rgb, b: Rgb) -> i32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    dr * dr + dg * dg + db * db
}

fn bayer_offset(x: u32, y: u32) -> i16 {
    let threshold = BAYER_4X4[y as usize % 4][x as usize % 4];
    let numerator = (threshold * 2 + 1 - 16) * DITHER_RANGE;
    if numerator >= 0 {
        (numerator + 16) / 32
    } else {
        (numerator - 16) / 32
    }
}

fn apply_offset(channel: u8, offset: i16) -> u8 {
    (i16::from(channel) + offset).clamp(0, 255) as u8
}

fn nearest_color(color: Rgb, palette: &[Rgb]) -> Rgb {
    let mut nearest = palette[0];
    let mut shortest_distance = u32::MAX;
    for &candidate in palette {
        let red = i32::from(color.0) - i32::from(candidate.0);
        let green = i32::from(color.1) - i32::from(candidate.1);
        let blue = i32::from(color.2) - i32::from(candidate.2);
        let distance = (red * red + green * green + blue * blue) as u32;
        if distance < shortest_distance {
            nearest = candidate;
            shortest_distance = distance;
        }
    }
    nearest
}

fn to_color((red, green, blue): Rgb) -> Color {
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{
        DynamicImage, ImageFormat, Rgb as ImageRgb, RgbImage, Rgba as ImageRgba, RgbaImage,
    };
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::widgets::Widget;

    use super::{
        fitted_dimensions, from_image_bytes, octant_glyph, select_cell, sextant_glyph, CoverDetail,
        CoverPalette, PixelCell, PixelCover, Rgb,
    };

    const BLACK: Rgb = (0, 0, 0);
    const WHITE: Rgb = (255, 255, 255);

    #[test]
    fn quantizes_a_two_by_two_image_to_known_colors() {
        let bytes = png_bytes(2, 2, &[(255, 0, 0), (0, 255, 0), (0, 0, 255), WHITE]);
        let palette = [(255, 0, 0), (0, 255, 0), (0, 0, 255), WHITE];

        let cover = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &palette,
            Color::Black,
            (2, 1),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(
            cover.cells,
            vec![
                PixelCell {
                    glyph: '▀',
                    fg: Color::Rgb(255, 0, 0),
                    bg: Color::Rgb(0, 0, 255),
                },
                PixelCell {
                    glyph: '▀',
                    fg: Color::Rgb(0, 255, 0),
                    bg: Color::Rgb(255, 255, 255),
                },
            ]
        );
    }

    #[test]
    fn original_palette_keeps_truecolor_and_applies_bayer() {
        let bytes = png_bytes(1, 2, &[(100, 150, 200); 2]);

        let original = from_image_bytes(
            &bytes,
            CoverPalette::Original,
            &[],
            Color::Reset,
            (1, 1),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();
        let theme = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &[BLACK, WHITE],
            Color::Reset,
            (1, 1),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(
            original.cells,
            vec![PixelCell {
                glyph: '▀',
                fg: Color::Rgb(93, 143, 193),
                bg: Color::Rgb(104, 154, 204),
            }]
        );
        assert_ne!(original, theme);
        assert!(theme.cells.iter().all(|cell| {
            [cell.fg, cell.bg]
                .into_iter()
                .all(|color| matches!(color, Color::Rgb(0, 0, 0) | Color::Rgb(255, 255, 255)))
        }));
    }

    #[test]
    fn four_x_detail_keeps_multiple_final_bayer_phases() {
        let bytes = png_bytes(4, 4, &[(100, 150, 200); 16]);

        let cover = from_image_bytes(
            &bytes,
            CoverPalette::Original,
            &[],
            Color::Reset,
            (4, 2),
            4.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(cover.cells[0].fg, Color::Rgb(93, 143, 193));
        assert_eq!(cover.cells[3].fg, Color::Rgb(102, 152, 202));
        assert_ne!(cover.cells[0].fg, cover.cells[3].fg);
    }

    #[test]
    fn original_palette_preserves_transparent_reset_samples() {
        let bytes = rgba_png_bytes(1, 2, &[(255, 0, 0, 255), (0, 0, 0, 0)]);

        let cover = from_image_bytes(
            &bytes,
            CoverPalette::Original,
            &[],
            Color::Reset,
            (1, 1),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(cover.cells[0].glyph, '▀');
        assert!(matches!(cover.cells[0].fg, Color::Rgb(248, 0, 0)));
        assert_eq!(cover.cells[0].bg, Color::Reset);
    }

    #[test]
    fn bayer_dithering_is_deterministic() {
        let bytes = png_bytes(4, 4, &[(128, 128, 128); 16]);
        let palette = [BLACK, WHITE];

        let first = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &palette,
            Color::Black,
            (4, 2),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();
        let second = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &palette,
            Color::Black,
            (4, 2),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(first, second);
        assert_eq!((first.width, first.height), (4, 2));
        assert_eq!(first.cells.len(), 8);
        assert!(first.cells.iter().all(|cell| {
            [cell.fg, cell.bg]
                .into_iter()
                .all(|color| matches!(color, Color::Rgb(0, 0, 0) | Color::Rgb(255, 255, 255)))
        }));
    }

    #[test]
    fn fills_non_square_image_letterbox_with_theme_background() {
        let bytes = png_bytes(4, 2, &[(255, 0, 0); 8]);
        let red = (255, 0, 0);
        let background = Color::Rgb(12, 34, 56);

        let cover = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &[BLACK, red],
            background,
            (4, 2),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(
            cover.cells,
            [
                vec![
                    PixelCell {
                        glyph: '▀',
                        fg: background,
                        bg: Color::Rgb(255, 0, 0),
                    };
                    4
                ],
                vec![
                    PixelCell {
                        glyph: '▀',
                        fg: Color::Rgb(255, 0, 0),
                        bg: background,
                    };
                    4
                ],
            ]
            .concat()
        );
    }

    #[test]
    fn transparent_pixels_preserve_the_terminal_background() {
        let red = (255, 0, 0);
        let bytes = rgba_png_bytes(1, 2, &[(0, 0, 0, 0), (255, 0, 0, 255)]);

        let cover = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &[BLACK, red],
            Color::Reset,
            (1, 1),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(
            cover.cells,
            vec![PixelCell {
                glyph: '▄',
                fg: Color::Rgb(255, 0, 0),
                bg: Color::Reset,
            }]
        );
    }

    #[test]
    fn resizing_a_transparent_edge_keeps_its_visible_color() {
        let red = (255, 0, 0);
        let bytes = rgba_png_bytes(2, 1, &[(255, 0, 0, 255), (0, 0, 0, 0)]);

        let cover = from_image_bytes(
            &bytes,
            CoverPalette::Theme,
            &[BLACK, (128, 0, 0), red],
            Color::Reset,
            (1, 1),
            1.0,
            CoverDetail::Half,
        )
        .unwrap();

        assert_eq!(cover.cells[0].fg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn detail_scale_never_changes_the_final_cell_footprint() {
        let bytes = png_bytes(8, 8, &[(128, 128, 128); 64]);
        let palette = [BLACK, WHITE];

        for scale in [0.5, 1.0, 2.0, 4.0] {
            let cover = from_image_bytes(
                &bytes,
                CoverPalette::Theme,
                &palette,
                Color::Black,
                (7, 3),
                scale,
                CoverDetail::Half,
            )
            .unwrap();
            assert_eq!((cover.width, cover.height), (7, 3));
            assert_eq!(cover.cells.len(), 21);
        }
    }

    #[test]
    fn detail_scale_is_clamped_to_four() {
        let pixels = (0..64)
            .map(|index| {
                let value = (index * 4) as u8;
                (value, 255 - value, value / 2)
            })
            .collect::<Vec<_>>();
        let bytes = png_bytes(8, 8, &pixels);

        let at_limit = from_image_bytes(
            &bytes,
            CoverPalette::Original,
            &[],
            Color::Reset,
            (3, 2),
            4.0,
            CoverDetail::Quad,
        )
        .unwrap();
        let beyond_limit = from_image_bytes(
            &bytes,
            CoverPalette::Original,
            &[],
            Color::Reset,
            (3, 2),
            99.0,
            CoverDetail::Quad,
        )
        .unwrap();

        assert_eq!(at_limit, beyond_limit);
    }

    #[test]
    fn every_detail_fills_the_same_square_physical_canvas() {
        for detail in [
            CoverDetail::Half,
            CoverDetail::Quad,
            CoverDetail::Sextant,
            CoverDetail::Octant,
        ] {
            let (samples_x, samples_y) = detail.sample_size();
            let target = (26 * samples_x, 13 * samples_y);

            assert_eq!(
                fitted_dimensions(100, 100, target.0, target.1, samples_x, samples_y),
                target,
                "{detail}"
            );
        }
    }

    #[test]
    fn quad_selects_the_exact_quadrant_and_color_pair() {
        let red = Color::Rgb(255, 0, 0);
        let blue = Color::Rgb(0, 0, 255);
        let samples = [
            red,
            blue,
            blue,
            blue,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
        ];

        let cell = select_cell(&samples, CoverDetail::Quad, &[red, blue]);

        assert_eq!(
            cell,
            PixelCell {
                glyph: '▘',
                fg: red,
                bg: blue,
            }
        );
    }

    #[test]
    fn sextant_selects_the_exact_sextant_and_color_pair() {
        let red = Color::Rgb(255, 0, 0);
        let blue = Color::Rgb(0, 0, 255);
        let samples = [
            red,
            blue,
            blue,
            blue,
            blue,
            blue,
            Color::Reset,
            Color::Reset,
        ];

        let cell = select_cell(&samples, CoverDetail::Sextant, &[red, blue]);

        assert_eq!(
            cell,
            PixelCell {
                glyph: '🬀',
                fg: red,
                bg: blue,
            }
        );
    }

    #[test]
    fn octant_selects_the_exact_octant_and_color_pair() {
        let red = Color::Rgb(255, 0, 0);
        let blue = Color::Rgb(0, 0, 255);
        let samples = [blue, blue, red, blue, blue, blue, blue, blue];

        let cell = select_cell(&samples, CoverDetail::Octant, &[red, blue]);

        assert_eq!(
            cell,
            PixelCell {
                glyph: '\u{1cd00}',
                fg: red,
                bg: blue,
            }
        );
    }

    #[test]
    fn octant_maps_unicode_16_and_legacy_masks_exactly() {
        let legacy = [
            (0, ' '),
            (1, '\u{1cea8}'),
            (2, '\u{1ceab}'),
            (3, '\u{1fb82}'),
            (5, '▘'),
            (10, '▝'),
            (15, '▀'),
            (20, '\u{1fbe6}'),
            (40, '\u{1fbe7}'),
            (63, '\u{1fb85}'),
            (64, '\u{1cea3}'),
            (80, '▖'),
            (85, '▌'),
            (90, '▞'),
            (95, '▛'),
            (128, '\u{1cea0}'),
            (160, '▗'),
            (165, '▚'),
            (170, '▐'),
            (175, '▜'),
            (192, '▂'),
            (240, '▄'),
            (245, '▙'),
            (250, '▟'),
            (252, '▆'),
            (255, '█'),
        ];
        for (mask, glyph) in legacy {
            assert_eq!(octant_glyph(mask), glyph, "mask {mask}");
        }

        let mut codepoint = 0x1cd00;
        for mask in 0..=u8::MAX {
            if legacy.iter().any(|(legacy_mask, _)| *legacy_mask == mask) {
                continue;
            }
            assert_eq!(octant_glyph(mask) as u32, codepoint, "mask {mask}");
            codepoint += 1;
        }
        assert_eq!(codepoint, 0x1cde6);
    }

    #[test]
    fn quad_keeps_transparency_in_the_background_color() {
        let red = Color::Rgb(255, 0, 0);
        let samples = [
            Color::Reset,
            red,
            red,
            red,
            Color::Reset,
            Color::Reset,
            Color::Reset,
            Color::Reset,
        ];

        let cell = select_cell(&samples, CoverDetail::Quad, &[red, Color::Reset]);

        assert_eq!(
            cell,
            PixelCell {
                glyph: '▟',
                fg: red,
                bg: Color::Reset,
            }
        );
    }

    #[test]
    fn sextant_keeps_transparency_in_the_background_color() {
        let red = Color::Rgb(255, 0, 0);
        let samples = [
            Color::Reset,
            red,
            red,
            red,
            red,
            red,
            Color::Reset,
            Color::Reset,
        ];

        let cell = select_cell(&samples, CoverDetail::Sextant, &[red, Color::Reset]);

        assert_eq!(cell.fg, red);
        assert_eq!(cell.bg, Color::Reset);
        assert_eq!(cell.glyph, sextant_glyph(62));
    }

    #[test]
    fn octant_keeps_transparency_in_the_background_color() {
        let red = Color::Rgb(255, 0, 0);
        let samples = [Color::Reset, red, red, red, red, red, red, red];

        let cell = select_cell(&samples, CoverDetail::Octant, &[red, Color::Reset]);

        assert_eq!(cell.fg, red);
        assert_eq!(cell.bg, Color::Reset);
        assert_eq!(cell.glyph, octant_glyph(254));
    }

    #[test]
    fn sextant_uses_legacy_blocks_for_full_columns() {
        let red = Color::Rgb(255, 0, 0);
        let blue = Color::Rgb(0, 0, 255);
        let samples = [red, blue, red, blue, red, blue, Color::Reset, Color::Reset];

        let cell = select_cell(&samples, CoverDetail::Sextant, &[red, blue]);

        assert_eq!(
            cell,
            PixelCell {
                glyph: '▌',
                fg: red,
                bg: blue,
            }
        );
    }

    #[test]
    fn widget_writes_centered_half_blocks_with_rgb_colors() {
        let cover = PixelCover {
            width: 2,
            height: 1,
            cells: vec![
                PixelCell {
                    glyph: '▀',
                    fg: Color::Rgb(1, 2, 3),
                    bg: Color::Rgb(4, 5, 6),
                },
                PixelCell {
                    glyph: '▀',
                    fg: Color::Rgb(7, 8, 9),
                    bg: Color::Rgb(10, 11, 12),
                },
            ],
        };
        let mut buffer = Buffer::with_lines(["....", "....", "...."]);

        (&cover).render(Rect::new(0, 0, 4, 3), &mut buffer);

        let first = buffer.cell((1, 1)).unwrap();
        assert_eq!(first.symbol(), "▀");
        assert_eq!(first.fg, Color::Rgb(1, 2, 3));
        assert_eq!(first.bg, Color::Rgb(4, 5, 6));
        let second = buffer.cell((2, 1)).unwrap();
        assert_eq!(second.symbol(), "▀");
        assert_eq!(second.fg, Color::Rgb(7, 8, 9));
        assert_eq!(second.bg, Color::Rgb(10, 11, 12));
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), ".");
    }

    #[test]
    fn widget_preserves_transparent_half_cells() {
        let red = Color::Rgb(255, 0, 0);
        let cover = PixelCover {
            width: 3,
            height: 1,
            cells: vec![
                PixelCell {
                    glyph: ' ',
                    fg: Color::Reset,
                    bg: Color::Reset,
                },
                PixelCell {
                    glyph: '▀',
                    fg: red,
                    bg: Color::Reset,
                },
                PixelCell {
                    glyph: '▄',
                    fg: red,
                    bg: Color::Reset,
                },
            ],
        };
        let mut buffer = Buffer::with_lines(["..."]);

        (&cover).render(Rect::new(0, 0, 3, 1), &mut buffer);

        let empty = buffer.cell((0, 0)).unwrap();
        assert_eq!(empty.symbol(), " ");
        assert_eq!((empty.fg, empty.bg), (Color::Reset, Color::Reset));

        let upper = buffer.cell((1, 0)).unwrap();
        assert_eq!(upper.symbol(), "▀");
        assert_eq!((upper.fg, upper.bg), (red, Color::Reset));

        let lower = buffer.cell((2, 0)).unwrap();
        assert_eq!(lower.symbol(), "▄");
        assert_eq!((lower.fg, lower.bg), (red, Color::Reset));
    }

    #[test]
    fn widget_clips_when_the_area_is_smaller_than_the_cover() {
        let cover = PixelCover {
            width: 2,
            height: 1,
            cells: vec![
                PixelCell {
                    glyph: '▀',
                    fg: Color::Rgb(1, 2, 3),
                    bg: Color::Rgb(4, 5, 6),
                },
                PixelCell {
                    glyph: '▀',
                    fg: Color::Rgb(7, 8, 9),
                    bg: Color::Rgb(10, 11, 12),
                },
            ],
        };
        let mut buffer = Buffer::with_lines(["."]);

        (&cover).render(Rect::new(0, 0, 1, 1), &mut buffer);

        let cell = buffer.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(1, 2, 3));
        assert_eq!(cell.bg, Color::Rgb(4, 5, 6));
    }

    fn png_bytes(width: u32, height: u32, pixels: &[Rgb]) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            let (red, green, blue) = pixels[(y * width + x) as usize];
            ImageRgb([red, green, blue])
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn rgba_png_bytes(width: u32, height: u32, pixels: &[(u8, u8, u8, u8)]) -> Vec<u8> {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            let (red, green, blue, alpha) = pixels[(y * width + x) as usize];
            ImageRgba([red, green, blue, alpha])
        });
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }
}
