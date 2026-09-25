use limedl_core::types::BtPieceInfo;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::i18n::{self, Language};

/// Generate a dynamic piece map bitmap image and summary label from `BtPieceInfo` slice.
pub fn generate_piece_map_image(pieces: &[BtPieceInfo], lang: Language) -> (Image, String) {
    if pieces.is_empty() {
        let buf = SharedPixelBuffer::<Rgba8Pixel>::new(1, 1);
        return (Image::from_rgba8(buf), i18n::format_piece_map_summary(0, 0, 0.0, lang));
    }

    let total = pieces.len();
    let completed = pieces.iter().filter(|p| p.completed).count();
    let percent = if total > 0 {
        (completed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let summary_text = i18n::format_piece_map_summary(completed, total, percent, lang);

    let cols: u32 = if total > 2000 {
        64
    } else if total > 500 {
        48
    } else if total > 100 {
        32
    } else {
        24
    };

    let rows: u32 = (total as u32).div_ceil(cols).max(1);
    let cell_size: u32 = if rows > 40 { 6 } else if rows > 20 { 8 } else { 10 };
    let padding: u32 = 1;

    let width = cols * cell_size;
    let height = rows * cell_size;

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let slice = buffer.make_mut_slice();

    // Background color: #0f1114
    let bg_pixel = Rgba8Pixel { r: 15, g: 17, b: 20, a: 255 };
    slice.fill(bg_pixel);

    // Color definitions
    let completed_pixel = Rgba8Pixel { r: 132, g: 204, b: 22, a: 255 }; // #84cc16
    let pending_pixel = Rgba8Pixel { r: 38, g: 42, b: 49, a: 255 };    // #262a31

    for (idx, piece) in pieces.iter().enumerate() {
        let col = (idx as u32) % cols;
        let row = (idx as u32) / cols;

        let x_start = col * cell_size;
        let y_start = row * cell_size;
        let x_end = (x_start + cell_size).saturating_sub(padding).min(width);
        let y_end = (y_start + cell_size).saturating_sub(padding).min(height);

        let color = if piece.completed {
            completed_pixel
        } else {
            pending_pixel
        };

        for y in y_start..y_end {
            for x in x_start..x_end {
                let pixel_idx = (y * width + x) as usize;
                if pixel_idx < slice.len() {
                    slice[pixel_idx] = color;
                }
            }
        }
    }

    (Image::from_rgba8(buffer), summary_text)
}
