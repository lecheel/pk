mod clipboard_utils;
mod constants;
mod editing;
mod git_ops;
mod git_panels;
mod help;
mod matching;
mod minimap;
mod palette;
mod split_view;
mod state;
mod status_bar;
mod toolbar;
mod types;

pub use state::MergeApp;

// Re-export palette for external use if needed
pub use palette::pal;