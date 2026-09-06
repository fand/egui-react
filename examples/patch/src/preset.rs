//! The patch the example opens with, as a file that is read at runtime.
//!
//! It is `include_str!`, so "reading" it takes no time at all — but it is read
//! through `use_future` all the same, because what the example wants to show
//! is a `<Suspense>` boundary around part of a screen. Swap this for an
//! `ehttp` request to a patch library and nothing above it changes; that is
//! the point of putting the parse behind a future rather than in a `const`.

use crate::graph::{Graph, GrayMethod, Kind, MixMode, Node};

/// The starter patch, as JSON. Generated from [`starter`] and kept in step
/// with it by the test at the bottom of this file.
pub const STARTER: &str = include_str!("preset.json");

/// Parse a saved patch, falling back to an empty one.
///
/// A patch that cannot be read is a warning and an empty canvas, never a
/// panic: this runs inside a future, and in a browser at that.
pub fn parse(json: &str) -> Graph {
    serde_json::from_str(json).unwrap_or_else(|err| {
        log::warn!("patch: could not read the preset ({err}), starting empty");
        Graph::empty()
    })
}

/// Two sources — a plasma and a moiré — put through a few effects and
/// screened together, so that the first thing on screen has something in every
/// node to look at.
pub fn starter() -> Graph {
    let node = |id, name: &str, kind: Kind, pos: [f32; 2], inputs, params| Node {
        id,
        name: String::from(name),
        kind,
        pos,
        inputs,
        params,
    };

    Graph::from_parts(
        vec![
            node(
                1,
                "out1",
                Kind::Output,
                [600.0, 170.0],
                [Some(7), None],
                [0.0; 4],
            ),
            node(
                2,
                "shader1",
                Kind::Shader {
                    src: String::from(
                        "vec4<f32>(0.5 + 0.5 * sin(uv.x * 8.0 * p0 + t), \
                         0.5 + 0.5 * sin(uv.y * 8.0 * p1 - t), p2, 1.0)",
                    ),
                },
                [20.0, 30.0],
                [None, None],
                [1.0, 1.0, 0.6, 0.0],
            ),
            node(
                3,
                "transform1",
                Kind::Transform,
                [210.0, 30.0],
                [Some(2), None],
                [0.0, 0.0, 0.4, 1.3],
            ),
            node(
                4,
                "level1",
                Kind::Level,
                [400.0, 30.0],
                [Some(3), None],
                [0.05, 1.4, 1.0, 0.0],
            ),
            node(
                5,
                "shader2",
                Kind::Shader {
                    src: String::from(
                        "vec4<f32>(vec3<f32>(0.5 + 0.5 * \
                         sin((uv.x + uv.y) * 24.0 * p0 - t * 1.5)), 1.0)",
                    ),
                },
                [20.0, 250.0],
                [None, None],
                [1.0, 1.0, 0.5, 0.0],
            ),
            node(
                6,
                "grayscale1",
                Kind::Grayscale {
                    method: GrayMethod::Luma,
                },
                [210.0, 250.0],
                [Some(5), None],
                [0.0; 4],
            ),
            node(
                7,
                "mix1",
                Kind::Mix {
                    mode: MixMode::Screen,
                },
                [400.0, 250.0],
                [Some(4), Some(6)],
                [0.6, 0.0, 0.0, 0.0],
            ),
        ],
        8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JSON file and the function above are two spellings of one patch. If
    /// this fails, `starter()` was edited and `preset.json` was not:
    /// `serde_json::to_string_pretty(&starter())` is the file.
    #[test]
    fn the_json_is_the_starter_patch() {
        assert_eq!(parse(STARTER), starter());
    }

    /// Whatever the preset is, it has to be a patch that compiles — otherwise
    /// the example opens on an error message.
    #[test]
    fn the_starter_patch_generates_a_shader() {
        let generated = crate::codegen::generate(&starter()).expect("the preset generates");
        assert_eq!(generated.slots.len(), starter().nodes.len());
    }

    /// A file that is not JSON is a warning and an empty patch.
    #[test]
    fn a_broken_preset_is_not_a_panic() {
        assert_eq!(parse("{ not json"), Graph::empty());
    }
}

#[cfg(test)]
mod show {
    #[test]
    fn print() {
        println!(
            "{}",
            crate::codegen::generate(&super::starter()).unwrap().wgsl
        );
    }
}
