//! Slurm hostlist expansion.
//!
//! Slurm uses a compact list syntax to name groups of compute nodes:
//!
//! - `nid001` — a single host
//! - `nid[001-003]` — a contiguous numeric range, zero-padded
//! - `nid[1,3,5]` — discrete values
//! - `nid[1-3,7-9]` — mixed ranges and singletons
//! - `nid001,nid007` — comma-separated singletons
//! - `node[a,b]` — non-numeric inside brackets passes through verbatim
//!
//! [`expand`] turns any of these into a flat `Vec<String>` of individual
//! host names. It returns an empty vec on `"(null)"` or empty input, so
//! callers can iterate without checking.

/// Expand a Slurm hostlist string into individual host names.
pub fn expand(input: &str) -> Vec<String> {
    let s = input.trim();
    if s.is_empty() || s == "(null)" || s == "n/a" || s == "N/A" {
        return Vec::new();
    }

    // Split at top-level commas (not inside brackets) into independent terms.
    let mut terms: Vec<String> = Vec::new();
    let mut depth = 0i32;
    let mut buf = String::new();
    for c in s.chars() {
        match c {
            '[' => {
                depth += 1;
                buf.push(c);
            }
            ']' => {
                depth = depth.saturating_sub(1);
                buf.push(c);
            }
            ',' if depth == 0 => {
                if !buf.is_empty() {
                    terms.push(std::mem::take(&mut buf));
                }
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() {
        terms.push(buf);
    }

    let mut out = Vec::new();
    for term in terms {
        expand_term(&term, &mut out);
    }
    out
}

fn expand_term(term: &str, out: &mut Vec<String>) {
    let Some(open) = term.find('[') else {
        out.push(term.to_string());
        return;
    };
    let Some(close) = term[open..].find(']').map(|c| open + c) else {
        // Unbalanced — emit verbatim.
        out.push(term.to_string());
        return;
    };
    let prefix = &term[..open];
    let suffix = &term[close + 1..];
    let inside = &term[open + 1..close];

    for chunk in inside.split(',') {
        if let Some(dash) = chunk.find('-') {
            let (start_s, end_s) = (chunk[..dash].trim(), chunk[dash + 1..].trim());
            if let (Ok(start), Ok(end)) = (start_s.parse::<u64>(), end_s.parse::<u64>()) {
                let width = start_s.len();
                for n in start..=end {
                    out.push(format!("{prefix}{n:0width$}{suffix}"));
                }
                continue;
            }
        }
        if let Ok(n) = chunk.trim().parse::<u64>() {
            let width = chunk.trim().len();
            out.push(format!("{prefix}{n:0width$}{suffix}"));
        } else if !chunk.is_empty() {
            // Non-numeric brackets — pass through.
            out.push(format!("{prefix}{}{suffix}", chunk.trim()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single() {
        assert_eq!(expand("nid001"), vec!["nid001"]);
    }

    #[test]
    fn contiguous_range() {
        assert_eq!(expand("nid[001-003]"), vec!["nid001", "nid002", "nid003"]);
    }

    #[test]
    fn discrete_singletons() {
        assert_eq!(expand("nid[1,3,5]"), vec!["nid1", "nid3", "nid5"]);
    }

    #[test]
    fn mixed_ranges() {
        assert_eq!(
            expand("nid[1-3,7-9]"),
            vec!["nid1", "nid2", "nid3", "nid7", "nid8", "nid9"]
        );
    }

    #[test]
    fn comma_separated_terms() {
        assert_eq!(expand("nid001,nid007"), vec!["nid001", "nid007"]);
    }

    #[test]
    fn mixed_terms() {
        assert_eq!(
            expand("nid[001-002],gpu05"),
            vec!["nid001", "nid002", "gpu05"]
        );
    }

    #[test]
    fn null_and_empty() {
        assert!(expand("").is_empty());
        assert!(expand("(null)").is_empty());
        assert!(expand("n/a").is_empty());
    }
}
