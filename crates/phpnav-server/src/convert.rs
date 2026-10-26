//! Conversion between file paths and LSP URIs.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_types::Uri;
use url::Url;

/// `file://` URI of an absolute path, percent-encoded.
pub fn path_to_uri(path: &Path) -> Uri {
    let url = Url::from_file_path(path).expect("path is absolute");
    Uri::from_str(url.as_str()).expect("a file URL is a valid URI")
}

/// Path of a `file://` URI; None for other schemes.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{path_to_uri, uri_to_path};

    #[test]
    fn uri_round_trip_with_space_and_accent() {
        let path = Path::new("/Users/x/Application Support/Città/Foo.php");

        let uri = path_to_uri(path);

        assert_eq!(
            uri.as_str(),
            "file:///Users/x/Application%20Support/Citt%C3%A0/Foo.php"
        );
        assert_eq!(uri_to_path(&uri).as_deref(), Some(path));
    }
}
