use std::path::{Path, PathBuf};

use tokio::fs::read;
use tower_lsp::{jsonrpc::Error, lsp_types::Url};

pub async fn read_file(path: &Path) -> Result<String, String> {
    let raw_bytes = read(path).await;
    let raw_bytes = match raw_bytes {
        Err(e) => return Err(format!("bad filesystem read: {:?}", e)),
        Ok(bytes) => bytes,
    };

    match String::from_utf8(raw_bytes) {
        Ok(res) => Ok(res),
        Err(e) => return Err(format!("couldn't read file as utf-8: {:?}", e)),
    }
}

pub fn uri_to_path(uri: &Url) -> Result<PathBuf, Error> {
    match uri.to_file_path() {
        Err(_) => Err(Error::invalid_params(
            "language server needs to run locally",
        )),
        Ok(path) => Ok(path),
    }
}
