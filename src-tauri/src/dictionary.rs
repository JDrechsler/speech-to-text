use std::fs;

use crate::paths;

const MAX_TERM_LENGTH: usize = 50;

const TEMPLATE: &str = "# Your personal dictionary for Speech to Text. It lives only on this computer.
# Add one word or name per line, spelled exactly the way you want it written.
# Every engine gets these as spelling hints. Cloud engines receive them with each recording.
# Keep it under 50 entries: rare names and jargon help, common words do not.
# Lines starting with # are ignored.

";

pub fn ensure_file_exists() {
    let path = paths::dictionary_file();
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, TEMPLATE);
}

pub fn terms() -> Vec<String> {
    let Ok(content) = fs::read_to_string(paths::dictionary_file()) else {
        return Vec::new();
    };
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| line.chars().count() <= MAX_TERM_LENGTH)
        .map(String::from)
        .collect()
}
