//! Every example has a page on the documentation site, in both languages, and
//! every page shows its own source.
//!
//! The test lives here, next to [`gallery::EXAMPLES`], because this is where
//! the requirement appears: an example is added to the gallery by putting its
//! `Meta` in that list, and the site's sidebar and example pages are built from
//! the same set. A page that was never written, or one that was copied from a
//! neighbour and still includes the neighbour's code, then fails the build that
//! added the example rather than being noticed on the deployed site.
//!

use std::path::{Path, PathBuf};

use gallery::EXAMPLES;

/// One language's example pages: `site/examples` for English, `site/ja/examples`
/// for the translation. From this crate rather than from the current directory,
/// so the test does not care where cargo was run from.
fn pages(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../site")
        .join(dir)
}

/// The page every example needs, and the container that puts its source on it.
fn assert_page(dir: &str, name: &str) {
    let page = pages(dir).join(format!("{name}.md"));
    let markdown = std::fs::read_to_string(&page)
        .unwrap_or_else(|err| panic!("site/{dir}/{name}.md: {err}. Write the page for {name}."));
    let include = format!("example-source {name}");
    assert!(
        markdown.contains(&include),
        "site/{dir}/{name}.md does not contain `::: {include}`, so it shows another example's code"
    );
}

/// Adding an example to the gallery without writing its page fails here.
#[test]
fn every_example_has_a_page() {
    for meta in EXAMPLES {
        assert_page("examples", meta.name);
    }
}

/// The Japanese pages are the same set: a translation that is missing a page is
/// a hole in the sidebar, and one copied from a neighbour shows the wrong code.
#[test]
fn every_example_has_a_japanese_page() {
    for meta in EXAMPLES {
        assert_page("ja/examples", meta.name);
    }
}
