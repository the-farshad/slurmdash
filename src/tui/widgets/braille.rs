//! Solid-Braille horizontal progress bars used by the History section
//! of the job-details view. Two glyphs only: `⣿` for filled cells and
//! `⣀` for the faint baseline track. No fractional / partial-fill
//! intermediate glyphs — every cell is either fully on or fully off
//! to match the dot aesthetic the user asked for verbatim.

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
