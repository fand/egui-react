//! The `Url` source: one `ehttp` fetch per URL, then the bytes go into the
//! database and the chains are applied again.
//!
//! `ehttp::fetch` takes a callback and runs it on a thread of its own on
//! native and on the browser's event loop on wasm, so the source needs no
//! executor and the same code serves both targets. Until the bytes arrive the
//! entry is `Pending` and the chain draws with whatever is behind it, which
//! is why a chain keeps a bundled or generic fallback at its tail.
//!
//! skrifa reads TTF / OTF / TTC. WOFF and WOFF2 are compressed wrappers
//! around the same tables; `wuff` unwraps them behind the `woff2` feature,
//! because the Brotli tables it needs are not small and most self-hosted
//! fonts are plain TTF. Without the feature such a response is `Failed` with
//! a message that names the feature.

use std::sync::Arc;

use super::Fonts;
use super::resolve::{Loaded, SourceKey};

/// What the first four bytes say the file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Format {
    /// TrueType / OpenType / a collection: what skrifa and fontdb read as is.
    Sfnt,
    Woff,
    Woff2,
    Unknown,
}

pub(super) fn sniff(bytes: &[u8]) -> Format {
    match bytes.get(..4) {
        Some(b"\0\x01\0\0" | b"OTTO" | b"true" | b"ttcf") => Format::Sfnt,
        Some(b"wOFF") => Format::Woff,
        Some(b"wOF2") => Format::Woff2,
        _ => Format::Unknown,
    }
}

/// Bytes a server returned into bytes fontdb can load.
pub(super) fn decode(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    match sniff(&bytes) {
        Format::Sfnt => Ok(bytes),
        Format::Woff => decode_woff(&bytes, "WOFF"),
        Format::Woff2 => decode_woff(&bytes, "WOFF2"),
        Format::Unknown => Err(format!(
            "not a TTF / OTF / TTC / WOFF / WOFF2 file (starts with {:02x?}); is the URL right?",
            bytes.get(..4).unwrap_or(&bytes)
        )),
    }
}

#[cfg(feature = "woff2")]
fn decode_woff(bytes: &[u8], what: &str) -> Result<Vec<u8>, String> {
    let decoded = if what == "WOFF" {
        wuff::decompress_woff1(bytes)
    } else {
        wuff::decompress_woff2(bytes)
    };
    decoded.map_err(|err| format!("{what} decode failed: {err}"))
}

#[cfg(not(feature = "woff2"))]
fn decode_woff(_bytes: &[u8], what: &str) -> Result<Vec<u8>, String> {
    Err(format!(
        "{what} needs the `woff2` feature of egui-react-app (or serve the font as TTF / OTF)"
    ))
}

impl Fonts {
    /// Ask for `url` once; the answer goes through `url_done`.
    pub(super) fn start_fetch(&self, url: String) {
        let fonts = self.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| {
            let result = match result {
                Ok(response) if response.ok => decode(response.bytes),
                Ok(response) => Err(format!(
                    "HTTP {} {}",
                    response.status,
                    response.status_text.trim()
                )),
                Err(err) => Err(err),
            };
            fonts.url_done(url, result);
        });
    }

    /// A URL's bytes arrived (or did not). Runs on the fetch's thread on
    /// native and on the event loop on wasm; either way it takes the lock,
    /// which is why `Fonts` is a mutex and not a thread-local.
    fn url_done(&self, url: String, result: Result<Vec<u8>, String>) {
        let mut inner = self.lock();
        let loaded = match result {
            Ok(bytes) => {
                let ids = inner
                    .db
                    .load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
                Loaded::Faces {
                    ids: ids.to_vec(),
                    static_bytes: None,
                }
            }
            Err(reason) => {
                log::warn!("egui-react fonts: {url}: {reason}");
                Loaded::Failed(reason)
            }
        };
        inner.loaded.insert(SourceKey::Url(url), loaded);
        inner.reapply();
        inner.repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_reads_the_magic() {
        assert_eq!(sniff(epaint_default_fonts::HACK_REGULAR), Format::Sfnt);
        assert_eq!(sniff(b"OTTO...."), Format::Sfnt);
        assert_eq!(sniff(b"ttcf...."), Format::Sfnt);
        assert_eq!(sniff(b"wOFF...."), Format::Woff);
        assert_eq!(sniff(b"wOF2...."), Format::Woff2);
        assert_eq!(sniff(b"<html>"), Format::Unknown);
        assert_eq!(sniff(b"ab"), Format::Unknown);
    }

    #[test]
    fn decode_passes_sfnt_through_and_names_the_problem_otherwise() {
        let bytes = epaint_default_fonts::HACK_REGULAR.to_vec();
        assert_eq!(decode(bytes.clone()).unwrap(), bytes);
        let err = decode(b"<!doctype html>".to_vec()).unwrap_err();
        assert!(err.contains("not a TTF"), "{err}");
        // A WOFF2 header with nothing behind it: with the feature on it is a
        // decode error, without it the message names the feature; either
        // way an error, never a panic.
        let err = decode(b"wOF2\0\0\0\0".to_vec()).unwrap_err();
        assert!(err.contains("WOFF2"), "{err}");
    }
}
