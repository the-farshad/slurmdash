//! Braille dot graphics used by the History views.
//!
//! Two routines:
//! - [`bar_pair`] — solid horizontal progress bar (⣿/⣀, whole-cell
//!   rounding) used by the per-job History block in the details view.
//! - [`vertical_spark`] — vertical-amplitude sparkline that packs two
//!   consecutive samples into each cell using the left + right dot
//!   columns of the Braille Patterns block. Used by the "History
//!   (last N samples)" panel at the top of the Dashboard and the
//!   Statistics view.

/// Build (filled, empty) strings that together span exactly `cells`
/// terminal columns. Cells round to whole `⣿` or `⣀` glyphs, so
/// 50% of 10 cells is exactly `⣿⣿⣿⣿⣿⣀⣀⣀⣀⣀`.
pub fn bar_pair(pct: f64, cells: usize) -> (String, String) {
    let pct = pct.clamp(0.0, 1.0);
    let full_cells = (pct * cells as f64).round() as usize;
    let full_cells = full_cells.min(cells);
    let empty_cells = cells - full_cells;

    let filled: String = "⣿".repeat(full_cells);
    let empty: String = "⣀".repeat(empty_cells);
    (filled, empty)
}

/// Render `samples` (each in 0.0..=1.0) as a Braille-dot vertical
/// sparkline `cells` characters wide. Each character cell holds two
/// consecutive samples:
///   - LEFT dot column = older sample,
///   - RIGHT dot column = newer sample.
///
/// Each column has four vertical levels (0..=4 dots stacked from the
/// bottom). The result reads as a chunky dot histogram of the last
/// `2 * cells` samples.
///
/// If we have fewer samples than slots, the leading slots are left
/// blank (older "no data" stays as the empty cell `⠀`).
pub fn vertical_spark(samples: &[f32], cells: usize) -> String {
    if cells == 0 {
        return String::new();
    }
    // Dot-bit patterns for the LEFT dot column (dots 7,3,2,1 stacked
    // from bottom to top) and the RIGHT dot column (dots 8,6,5,4).
    // Each character is U+2800 + (left_bits | right_bits).
    const LEFT_BITS: [u32; 5] = [
        0,    // 0 dots
        0x40, // dot 7
        0x44, // dots 7+3
        0x46, // dots 7+3+2
        0x47, // dots 7+3+2+1
    ];
    const RIGHT_BITS: [u32; 5] = [
        0,    // 0 dots
        0x80, // dot 8
        0xA0, // dots 8+6
        0xB0, // dots 8+6+5
        0xB8, // dots 8+6+5+4
    ];

    let total_slots = cells * 2;
    let pad = total_slots.saturating_sub(samples.len());
    let visible_start = samples.len().saturating_sub(total_slots - pad);

    let mut slots: Vec<u32> = vec![0; pad];
    slots.extend(
        samples[visible_start..]
            .iter()
            .map(|v| ((v.clamp(0.0, 1.0) * 4.0).round() as u32).min(4)),
    );

    let mut out = String::with_capacity(cells * 3);
    for pair in slots.chunks(2) {
        let l = pair[0] as usize;
        let r = *pair.get(1).unwrap_or(&0) as usize;
        let bits = LEFT_BITS[l] | RIGHT_BITS[r];
        let ch = char::from_u32(0x2800 + bits).unwrap_or('⠀');
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_pct_is_all_track() {
        let (f, e) = bar_pair(0.0, 10);
        assert!(f.is_empty());
        assert_eq!(e.chars().count(), 10);
        assert!(e.chars().all(|c| c == '⣀'));
    }

    #[test]
    fn full_pct_is_all_filled() {
        let (f, e) = bar_pair(1.0, 10);
        assert_eq!(f.chars().count(), 10);
        assert!(e.is_empty());
        assert!(f.chars().all(|c| c == '⣿'));
    }

    #[test]
    fn half_pct_splits_in_middle() {
        let (f, e) = bar_pair(0.5, 10);
        assert_eq!(f.chars().count(), 5);
        assert_eq!(e.chars().count(), 5);
        assert!(f.chars().all(|c| c == '⣿'));
        assert!(e.chars().all(|c| c == '⣀'));
    }

    #[test]
    fn cells_never_overflow_total_width() {
        // Even a value that rounds up to >cells stays clamped.
        let (f, e) = bar_pair(1.5, 8);
        assert_eq!(f.chars().count(), 8);
        assert!(e.is_empty());
    }
}
