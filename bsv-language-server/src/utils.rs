use lsp_types::Url;
use std::path::PathBuf;

#[allow(dead_code)]
pub fn uri_to_path(uri: &Url) -> Result<PathBuf, String> {
    uri.to_file_path()
        .map_err(|_| format!("Invalid file URI: {}", uri))
}

#[allow(dead_code)]
pub fn path_to_uri(path: &PathBuf) -> Result<Url, String> {
    Url::from_file_path(path)
        .map_err(|_| format!("Cannot convert path to URI: {:?}", path))
}

#[allow(dead_code)]
pub fn normalize_path(path: &str) -> String {
    path.replace("\\", "/")
}

pub fn get_line_content(text: &str, line: usize) -> Option<&str> {
    text.lines().nth(line)
}

#[allow(dead_code)]
pub fn is_bsv_file(path: &str) -> bool {
    path.ends_with(".bsv") || path.ends_with(".bs")
}
