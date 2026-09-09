//! Every example has a page on the documentation site, and every page shows
//! its own source.
//!
//! The test lives here, next to [`gallery::EXAMPLES`], because this is where
//! the requirement appears: an example is added to the gallery by putting its
//! `Meta` in that list, and the site's sidebar and example pages are built from
//! the same set. A page that was never written, or one that was copied from a
//! neighbour and still includes the neighbour's code, then fails the build that
//! added the example rather than being noticed on the deployed site.
//!
//! `shell` is checked separately: it has a page (source only — a docked panel
//! carves up the window, so there is nothing for the embed to run) but it is
//! not in `EXAMPLES`, because the gallery cannot run it either.

use std::path::{Path, PathBuf};

use gallery::EXAMPLES;

/// The example pages, from this crate rather than from the current directory,
/// so the test does not care where cargo was run from.
fn pages() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/examples")
}

/// The page every example needs, and the container that puts its source on it.
fn assert_page(name: &str) {
    let page = pages().join(format!("{name}.md"));
    let markdown = std::fs::read_to_string(&page)
        .unwrap_or_else(|err| panic!("site/examples/{name}.md: {err}. Write the page for {name}."));
    let include = format!("example-source {name}");
    assert!(
        markdown.contains(&include),
        "site/examples/{name}.md does not contain `::: {include}`, so it shows another example's \
         code"
    );
}

/// Adding an example to the gallery without writing its page fails here.
#[test]
fn every_example_has_a_page() {
    for meta in EXAMPLES {
        assert_page(meta.name);
    }
}

/// The one example the gallery does not run still has a page.
#[test]
fn shell_has_a_page() {
    assert_page("shell");
}
