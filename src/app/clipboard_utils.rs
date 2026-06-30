use crate::patch::PatchHunk;

#[cfg(not(target_arch = "wasm32"))]
pub fn get_clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_text().ok())
}

#[cfg(target_arch = "wasm32")]
pub fn get_clipboard_text() -> Option<String> {
    None
}

pub fn parse_clipboard_patch(pasted: &str) -> Vec<PatchHunk> {
    crate::patch::parse_patches(pasted)
}
