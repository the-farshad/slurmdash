//! Braille-fractional horizontal progress bars (btop aesthetic).
//!
//! Each Braille cell carries up to 8 sub-pixels of resolution, so a
//! 10-cell bar represents 80 distinct fill levels — substantially
//! smoother than the block (▰/▱) bars used in early phases. The empty
//! side is a faint dot baseline so the bar's full extent stays
//! visible even at 0% fill.

/// 9-step fill ramp: empty, then 1..7 dots, then full. Picked so each
/// step adds dots on the LEFT half first (matches L→R growth of the
/// bar).
const STEPS: [char; 9] = ['⠀', '⡀', '⡄', '⡆', '⡇', '⣇', '⣧', '⣷', '⣿'];

/// Build (filled, empty) strings that together span exactly `cells`
/// terminal columns. The filled side ends with a partial-fill glyph
/// when the percentage doesn't fall on an 8-dot boundary; the empty
/// side renders a faint `⣀` baseline so the track stays visible at
/// 0 %. Callers pick the colors when wrapping each in a Span.
pub fn bar_pair(pct: f64, cells: usize) -> (String, String) {
    let pct = pct.clamp(0.0, 1.0);
    let total_dots = (pct * cells as f64 * 8.0).round() as usize;
    let full_cells = total_dots / 8;
    let remainder = total_dots % 8;

    let mut filled = String::with_capacity(full_cells * 3 + 3);
    for _ in 0..full_cells {
        filled.push('⣿');
    }
    let partial = remainder > 0 && full_cells < cells;
    if partial {
        filled.push(STEPS[remainder]);
    }

    let used = full_cells + usize::from(partial);
    let empty_cells = cells.saturating_sub(used);
    let mut empty = String::with_capacity(empty_cells * 3);
    for _ in 0..empty_cells {
        empty.push('⣀');
    }

    (filled, empty)
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
        assert_eq!(f.chars().count() + e.chars().count(), 10);
        assert_eq!(f.chars().count(), 5);
    }

    #[test]
    fn fractional_pct_uses_partial_glyph() {
        // 33% of 10 cells × 8 dots/cell = 26.4 dots → round to 26
        // → 3 full cells + a 2-dot partial = 4 glyphs.
        let (f, _) = bar_pair(0.33, 10);
        assert_eq!(f.chars().count(), 4);
        let last = f.chars().last().unwrap();
        assert!(STEPS[1..8].contains(&last), "partial glyph, got {last:?}");
    }
}
