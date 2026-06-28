/// Kind of diff row produced by the LCS line diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Equal,
    Delete,
    Insert,
}

/// A single aligned row in the side-by-side diff.
#[derive(Debug, Clone)]
pub struct DiffRow {
    pub kind: RowKind,
    pub left: Option<String>,
    pub right: Option<String>,
    pub left_num: Option<usize>,
    pub right_num: Option<usize>,
}

/// Result of matching a SEARCH block against the file.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub score: f32,
    pub file_start: usize,
    pub file_end: usize,
    pub rows: Vec<DiffRow>,
}

/// Classic LCS-based line diff.
fn lcs_diff(left: &[String], right: &[String]) -> Vec<(RowKind, Option<String>, Option<String>)> {
    let m = left.len();
    let n = right.len();

    // dp[i][j] = LCS length of left[0..i] and right[0..j]
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if left[i - 1] == right[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // backtrack to produce the diff
    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && left[i - 1] == right[j - 1] {
            result.push((
                RowKind::Equal,
                Some(left[i - 1].clone()),
                Some(right[j - 1].clone()),
            ));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            result.push((RowKind::Insert, None, Some(right[j - 1].clone())));
            j -= 1;
        } else {
            result.push((RowKind::Delete, Some(left[i - 1].clone()), None));
            i -= 1;
        }
    }

    result.reverse();
    result
}

/// Slide a window over `file` to find the region that best matches `search`.
///
/// Returns the match score (0–100), the line range in `file`, and the
/// aligned diff rows for side-by-side display.
pub fn find_best_match(search: &[String], file: &[String]) -> MatchResult {
    if search.is_empty() || file.is_empty() {
        return MatchResult {
            score: 0.0,
            file_start: 0,
            file_end: 0,
            rows: vec![],
        };
    }

    let search_len = search.len();

    // If the file is shorter than the search, just diff the whole file.
    if search_len > file.len() {
        let raw = lcs_diff(search, file);
        let matched = raw.iter().filter(|(k, _, _)| *k == RowKind::Equal).count();
        let score = (matched as f32 / search_len as f32) * 100.0;
        let rows = build_rows(&raw, 1, 1);
        return MatchResult {
            score,
            file_start: 0,
            file_end: file.len(),
            rows,
        };
    }

    // Try a small range of window sizes around the search length.
    let min_window = search_len.saturating_sub(2).max(1);
    let max_window = (search_len + 3).min(file.len());

    let mut best_score = -1.0_f32;
    let mut best_start = 0;
    let mut best_end = 0;
    let mut best_raw: Vec<(RowKind, Option<String>, Option<String>)> = Vec::new();

    for window_size in min_window..=max_window {
        for start in 0..=file.len().saturating_sub(window_size) {
            let window = &file[start..start + window_size];
            let raw = lcs_diff(search, window);

            let matched = raw.iter().filter(|(k, _, _)| *k == RowKind::Equal).count();
            let score = matched as f32 / search_len as f32;

            // penalise windows that are much larger than the search
            let extra = window_size.saturating_sub(search_len);
            let penalty = extra as f32 * 0.03;
            let adjusted = score - penalty;

            if adjusted > best_score {
                best_score = adjusted;
                best_start = start;
                best_end = start + window_size;
                best_raw = raw;
            }
        }
    }

    let rows = build_rows(&best_raw, 1, best_start + 1);
    let score = (best_score * 100.0).clamp(0.0, 100.0);

    MatchResult {
        score,
        file_start: best_start,
        file_end: best_end,
        rows,
    }
}

fn build_rows(
    raw: &[(RowKind, Option<String>, Option<String>)],
    left_start: usize,
    right_start: usize,
) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut ln = left_start;
    let mut rn = right_start;

    for (kind, left, right) in raw {
        let left_num = if left.is_some() {
            let n = Some(ln);
            ln += 1;
            n
        } else {
            None
        };
        let right_num = if right.is_some() {
            let n = Some(rn);
            rn += 1;
            n
        } else {
            None
        };

        rows.push(DiffRow {
            kind: *kind,
            left: left.clone(),
            right: right.clone(),
            left_num,
            right_num,
        });
    }

    rows
}
