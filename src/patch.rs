/// One SEARCH/REPLACE hunk extracted from a patch file.
#[derive(Debug, Clone)]
pub struct PatchHunk {
    pub filename: String,
    pub search: Vec<String>,
    pub replace: Vec<String>,
}

/// Parse all `<patch>` blocks from the given text.
///
/// Expected format per hunk:
/// ```text
/// filename <path>
/// <<<<<<< SEARCH
/// ...search lines...
/// =======
/// ...replace lines...
/// >>>>>>> REPLACE
/// ```
pub fn parse_patches(content: &str) -> Vec<PatchHunk> {
    let mut hunks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("filename ") {
            let filename = trimmed["filename ".len()..].trim().to_string();
            i += 1;

            // advance to SEARCH marker
            while i < lines.len() && !lines[i].contains("<<<<<<< SEARCH") {
                i += 1;
            }
            if i >= lines.len() {
                break;
            }
            i += 1; // skip the SEARCH marker line

            // collect search lines
            let mut search = Vec::new();
            while i < lines.len() && !lines[i].trim().starts_with("=======") {
                search.push(lines[i].to_string());
                i += 1;
            }
            if i >= lines.len() {
                break;
            }
            i += 1; // skip the ======= marker line

            // collect replace lines
            let mut replace = Vec::new();
            while i < lines.len() && !lines[i].contains(">>>>>>> REPLACE") {
                replace.push(lines[i].to_string());
                i += 1;
            }

            hunks.push(PatchHunk {
                filename,
                search,
                replace,
            });
        }

        i += 1;
    }

    hunks
}
