//! The patch, as one fragment shader.
//!
//! A pure function from [`Graph`] to a string, plus naga's opinion of that
//! string. There is no wgpu type in this file and no `egui` either, which is
//! what makes the tests at the bottom possible: a generated program can be
//! compared as text and validated on the CPU, with no device and no window.
//!
//! Every node in the picture becomes
//! `fn n<id>(uv: vec2<f32>) -> vec4<f32>`, and the fragment entry point calls
//! the one the output is wired to. Nothing is hoisted into a shared temporary:
//! a `Transform` changes the `uv` it passes down, so the same node called from
//! two places is genuinely two different pictures, and calling it twice is the
//! simple thing that is also the correct one.
//!
//! Parameters do not appear in the text at all — only the *slot number* a
//! node's four floats will be found at does. That is the whole reason a slider
//! costs nothing: the program that reads `U.p[3]` does not care what is in it.

use std::collections::HashMap;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::graph::{Graph, GrayMethod, Kind, MixMode, NodeId};

/// What the prelude gives the generated code. Concatenated in front of it,
/// because WGSL wants a declaration before its use and this half never changes.
const PRELUDE: &str = include_str!("prelude.wgsl");

/// The size of the uniform array in [`PRELUDE`]. Also [`crate::graph::MAX_NODES`].
pub const SLOTS: usize = 32;

/// What an unconnected input is worth.
const BLACK: &str = "vec4<f32>(0.0, 0.0, 0.0, 1.0)";

/// A generated program.
pub struct Generated {
    /// The whole WGSL source: prelude, node functions, entry point.
    pub wgsl: String,
    /// Which node owns which uniform slot: `slots[i]` is the node whose
    /// parameters go into `U.p[i]`. [`pack_params`] fills the array in this
    /// order and the generated code reads the same indices.
    pub slots: Vec<NodeId>,
    /// A hash of `wgsl`, counted once here so that the GPU side can compare
    /// two programs without comparing two strings every frame.
    pub hash: u64,
}

/// Why a patch could not be turned into a shader. All three are shown to the
/// user as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenError {
    /// A node feeds itself, directly or round a loop.
    Cycle(NodeId),
    /// More nodes than the uniform block has slots.
    TooManyNodes,
    /// naga refused the generated source. In practice this is a hand-written
    /// `Shader` expression that is not valid WGSL.
    Wgsl(String),
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle(id) => write!(f, "a cycle runs through node {id}: a patch is a DAG"),
            Self::TooManyNodes => write!(f, "more than {SLOTS} nodes: the uniform block is full"),
            Self::Wgsl(message) => write!(f, "{message}"),
        }
    }
}

/// Turn a patch into a shader, or say why not.
///
/// The order of the work matters: cycles are found before anything is written
/// (a recursive emitter would not return), and the finished text is validated
/// before it is handed on, so nothing that reaches wgpu has ever been anything
/// but a program naga accepted.
pub fn generate(graph: &Graph) -> Result<Generated, GenError> {
    check_cycles(graph)?;

    let order = evaluation_order(graph);
    if order.len() > SLOTS {
        return Err(GenError::TooManyNodes);
    }
    let slot_of: HashMap<NodeId, usize> = order
        .iter()
        .enumerate()
        .map(|(slot, id)| (*id, slot))
        .collect();

    let mut wgsl = String::from(PRELUDE);
    for id in &order {
        emit(&mut wgsl, graph, *id, slot_of[id]);
    }
    wgsl.push_str("\n@fragment\nfn fs_main(in: VertexOut) -> @location(0) vec4<f32> {\n");
    // Clip space is -1..1 with y up; a patch works in 0..1 with y down, the
    // way every texture does.
    wgsl.push_str("    let uv = vec2<f32>(in.uv.x, -in.uv.y) * 0.5 + vec2<f32>(0.5);\n");
    let root = graph.output().filter(|id| slot_of.contains_key(id));
    wgsl.push_str(&format!("    return {};\n}}\n", call(graph, root, "uv")));

    validate(&wgsl).map_err(GenError::Wgsl)?;

    let hash = hash_of(&wgsl);
    Ok(Generated {
        wgsl,
        slots: order,
        hash,
    })
}

/// The parameters of every node in the picture, in slot order.
///
/// The other half of the contract [`Generated::slots`] describes: this array
/// goes into the uniform buffer as it is, and `U.p[i]` in the generated source
/// is `slots[i]`'s four floats.
pub fn pack_params(graph: &Graph, slots: &[NodeId]) -> [[f32; 4]; SLOTS] {
    let mut packed = [[0.0f32; 4]; SLOTS];
    for (slot, id) in slots.iter().enumerate().take(SLOTS) {
        if let Some(node) = graph.node(*id) {
            packed[slot] = node.params;
        }
    }
    packed
}

/// Ask naga whether this is a WGSL program.
///
/// The front end and the validator both run happily on the CPU, so this is
/// called from inside a `use_memo` and its message goes straight into the
/// inspector. A hand-written expression that does not compile stops here,
/// several steps before a device would have had to complain about it.
pub fn validate(wgsl: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(wgsl).map_err(|err| err.emit_to_string(wgsl))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|err| err.emit_to_string(wgsl))?;
    Ok(())
}

fn hash_of(wgsl: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    wgsl.hash(&mut hasher);
    hasher.finish()
}

/// A depth-first walk over the whole graph, looking for a back edge.
///
/// The whole graph and not only what the output can reach: a loop off to one
/// side is still a mistake, and reporting it where it is made is friendlier
/// than reporting it later, when the user finally wires it up.
fn check_cycles(graph: &Graph) -> Result<(), GenError> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        /// On the current path.
        Open,
        /// Finished, and known to be free of cycles.
        Done,
    }

    fn walk(graph: &Graph, id: NodeId, marks: &mut HashMap<NodeId, Mark>) -> Result<(), GenError> {
        match marks.get(&id) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Open) => return Err(GenError::Cycle(id)),
            None => {}
        }
        marks.insert(id, Mark::Open);
        if let Some(node) = graph.node(id) {
            for input in node.inputs.iter().flatten() {
                walk(graph, *input, marks)?;
            }
        }
        marks.insert(id, Mark::Done);
        Ok(())
    }

    let mut marks = HashMap::new();
    for node in &graph.nodes {
        walk(graph, node.id, &mut marks)?;
    }
    Ok(())
}

/// The nodes the output depends on, each one after everything it needs.
///
/// Only these are written out: a node nothing reaches costs no code and no
/// uniform slot. Call [`check_cycles`] first — this walk assumes it terminates.
fn evaluation_order(graph: &Graph) -> Vec<NodeId> {
    fn walk(graph: &Graph, id: NodeId, order: &mut Vec<NodeId>) {
        if order.contains(&id) {
            return;
        }
        let Some(node) = graph.node(id) else {
            return;
        };
        for input in node.inputs.iter().flatten() {
            walk(graph, *input, order);
        }
        order.push(id);
    }

    let mut order = Vec::new();
    if let Some(output) = graph.output() {
        walk(graph, output, &mut order);
    }
    order
}

/// The expression for "the colour of `input` at `uv`", or black.
fn call(graph: &Graph, input: Option<NodeId>, uv: &str) -> String {
    match input {
        Some(id) if graph.node(id).is_some() => format!("n{id}({uv})"),
        _ => String::from(BLACK),
    }
}

/// One node's function.
fn emit(wgsl: &mut String, graph: &Graph, id: NodeId, slot: usize) {
    let node = graph.node(id).expect("emitting a node that is there");
    let input = |port: usize, uv: &str| call(graph, node.inputs[port], uv);

    // The name is a comment and nothing more, which is exactly why renaming a
    // node counts as a change to the program: the text really is different.
    wgsl.push_str(&format!("\n// {} ({})\n", node.name, node.kind.name()));
    wgsl.push_str(&format!(
        "fn n{id}(uv: vec2<f32>) -> vec4<f32> {{\n    let p = U.p[{slot}];\n"
    ));

    let body = match &node.kind {
        Kind::Shader { src } => format!(
            "    let t = U.time;\n\
             \x20   let res = U.resolution;\n\
             \x20   let p0 = p.x;\n\
             \x20   let p1 = p.y;\n\
             \x20   let p2 = p.z;\n\
             \x20   return {src};\n"
        ),
        Kind::Level => format!(
            "    let src = {};\n\
             \x20   var c = (src.rgb - vec3<f32>(0.5)) * p.y + vec3<f32>(0.5 + p.x);\n\
             \x20   c = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / max(p.z, 0.001)));\n\
             \x20   return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), src.a);\n",
            input(0, "uv")
        ),
        Kind::Hsv => format!(
            "    let src = {};\n\
             \x20   let hsv = rgb2hsv(src.rgb);\n\
             \x20   let shifted = vec3<f32>(\n\
             \x20       fract(hsv.x + p.x),\n\
             \x20       clamp(hsv.y * p.y, 0.0, 1.0),\n\
             \x20       clamp(hsv.z * p.z, 0.0, 1.0),\n\
             \x20   );\n\
             \x20   return vec4<f32>(hsv2rgb(shifted), src.a);\n",
            input(0, "uv")
        ),
        Kind::Grayscale { method } => {
            let grey = match method {
                GrayMethod::Luma => "dot(src.rgb, vec3<f32>(0.2126, 0.7152, 0.0722))",
                GrayMethod::Average => "(src.r + src.g + src.b) / 3.0",
                GrayMethod::Max => "max(src.r, max(src.g, src.b))",
            };
            format!(
                "    let src = {};\n\
                 \x20   let g = {grey};\n\
                 \x20   return vec4<f32>(vec3<f32>(g), src.a);\n",
                input(0, "uv")
            )
        }
        Kind::Transform => format!(
            "    var q = uv - vec2<f32>(0.5);\n\
             \x20   let ca = cos(p.z);\n\
             \x20   let sa = sin(p.z);\n\
             \x20   q = vec2<f32>(q.x * ca - q.y * sa, q.x * sa + q.y * ca);\n\
             \x20   q = q / max(p.w, 0.001);\n\
             \x20   q = q + vec2<f32>(0.5) - vec2<f32>(p.x, p.y);\n\
             \x20   return {};\n",
            // The one node that does not pass `uv` down unchanged, and the
            // reason a shared subexpression would be wrong here.
            input(0, "q")
        ),
        Kind::Mix { mode } => {
            let blend = match mode {
                MixMode::Mix => "mix(a.rgb, b.rgb, k)",
                MixMode::Add => "a.rgb + b.rgb * k",
                MixMode::Multiply => "mix(a.rgb, a.rgb * b.rgb, k)",
                MixMode::Screen => {
                    "mix(a.rgb, vec3<f32>(1.0) - (vec3<f32>(1.0) - a.rgb) * \
                     (vec3<f32>(1.0) - b.rgb), k)"
                }
                MixMode::Difference => "mix(a.rgb, abs(a.rgb - b.rgb), k)",
            };
            format!(
                "    let a = {};\n\
                 \x20   let b = {};\n\
                 \x20   let k = clamp(p.x, 0.0, 1.0);\n\
                 \x20   return vec4<f32>(clamp({blend}, vec3<f32>(0.0), vec3<f32>(1.0)), a.a);\n",
                input(0, "uv"),
                input(1, "uv")
            )
        }
        Kind::Invert => format!(
            "    let src = {};\n\
             \x20   return vec4<f32>(\n\
             \x20       mix(src.rgb, vec3<f32>(1.0) - src.rgb, clamp(p.x, 0.0, 1.0)),\n\
             \x20       src.a,\n\
             \x20   );\n",
            input(0, "uv")
        ),
        Kind::Posterize => format!(
            "    let src = {};\n\
             \x20   let n = max(floor(p.x), 1.0);\n\
             \x20   return vec4<f32>(floor(src.rgb * n) / n, src.a);\n",
            input(0, "uv")
        ),
        // Another node that rewrites the `uv` it passes down. The `uv` here is
        // 0..1 across the frame (`fs_main` converts), so `n` is cells across
        // the whole width, and the half-cell shift samples the cell's centre.
        Kind::Pixelate => format!(
            "    let n = max(floor(p.x), 1.0);\n\
             \x20   let q = floor(uv * n) / n + vec2<f32>(0.5 / n);\n\
             \x20   return {};\n",
            input(0, "q")
        ),
        // `fract` of a scaled 0..1 uv: the picture starts again at every whole
        // step, p.x times across and p.y times down.
        Kind::Tile => format!(
            "    let q = fract(uv * vec2<f32>(p.x, p.y));\n\
             \x20   return {};\n",
            input(0, "q")
        ),
        Kind::Output => format!("    return {};\n", input(0, "uv")),
    };

    wgsl.push_str(&body);
    wgsl.push_str("}\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{DEFAULT_SRC, Msg, Node, reduce};

    /// Build a patch by hand, so a test says what it means in one place.
    ///
    /// Every node is wired to the one before it and the last to the output,
    /// unless the test rewires it afterwards.
    fn chain(kinds: &[Kind]) -> Graph {
        let mut graph = Graph::empty();
        let mut previous = None;
        for kind in kinds {
            reduce(
                &mut graph,
                Msg::AddNode {
                    kind: kind.clone(),
                    pos: [0.0, 0.0],
                },
            );
            let id = graph.nodes.last().expect("just added").id;
            if let Some(from) = previous {
                reduce(
                    &mut graph,
                    Msg::Connect {
                        from,
                        to: id,
                        port: 0,
                    },
                );
            }
            previous = Some(id);
        }
        if let Some(from) = previous {
            let out = graph.output().expect("an output");
            reduce(
                &mut graph,
                Msg::Connect {
                    from,
                    to: out,
                    port: 0,
                },
            );
        }
        graph
    }

    fn source() -> Kind {
        Kind::Shader {
            src: String::from(DEFAULT_SRC),
        }
    }

    /// Generating is also validating, so every one of these asserts that naga
    /// accepted what came out.
    fn generated(graph: &Graph) -> Generated {
        generate(graph).unwrap_or_else(|err| panic!("the patch did not generate: {err}"))
    }

    #[test]
    fn one_of_every_kind_generates_and_validates() {
        for kind in Kind::palette() {
            let graph = chain(&[source(), kind.clone()]);
            let wgsl = generated(&graph).wgsl;
            assert!(
                wgsl.contains(&format!("({})", kind.name())),
                "{} is not in its own program:\n{wgsl}",
                kind.name()
            );
        }
    }

    #[test]
    fn an_empty_patch_is_black() {
        let graph = Graph::empty();
        let wgsl = generated(&graph).wgsl;
        // The output is still emitted; what it returns is black.
        assert!(wgsl.contains("fn n1(uv: vec2<f32>) -> vec4<f32> {"));
        assert!(wgsl.contains(&format!("    return {BLACK};")));
    }

    #[test]
    fn every_blend_mode_and_grey_method_is_baked_in() {
        for mode in MixMode::ALL {
            let mut graph = chain(&[source(), Kind::Mix { mode }]);
            // The mix's second input is the source as well, which is also the
            // smallest DAG with a shared node in it.
            reduce(
                &mut graph,
                Msg::Connect {
                    from: 2,
                    to: 3,
                    port: 1,
                },
            );
            let wgsl = generated(&graph).wgsl;
            assert_eq!(
                wgsl.matches("n2(uv)").count(),
                2,
                "a node feeding two inputs is called twice"
            );
            assert!(wgsl.contains("let k = clamp(p.x, 0.0, 1.0);"), "{mode:?}");
        }

        for method in GrayMethod::ALL {
            let graph = chain(&[source(), Kind::Grayscale { method }]);
            let wgsl = generated(&graph).wgsl;
            assert!(wgsl.contains("let g = "), "{method:?}");
        }
    }

    #[test]
    fn invert_and_posterize_work_on_the_colour() {
        let wgsl = generated(&chain(&[source(), Kind::Invert])).wgsl;
        assert!(
            wgsl.contains("mix(src.rgb, vec3<f32>(1.0) - src.rgb"),
            "{wgsl}"
        );

        let wgsl = generated(&chain(&[source(), Kind::Posterize])).wgsl;
        assert!(wgsl.contains("floor(src.rgb * n) / n"), "{wgsl}");
    }

    /// Both of these rewrite the `uv`, so they call their input with `q`.
    #[test]
    fn pixelate_and_tile_work_on_the_uv() {
        let wgsl = generated(&chain(&[source(), Kind::Pixelate])).wgsl;
        assert!(wgsl.contains("floor(uv * n) / n"), "{wgsl}");
        assert!(wgsl.contains("return n2(q);"), "{wgsl}");

        let wgsl = generated(&chain(&[source(), Kind::Tile])).wgsl;
        assert!(wgsl.contains("fract(uv * vec2<f32>(p.x, p.y))"), "{wgsl}");
        assert!(wgsl.contains("return n2(q);"), "{wgsl}");
    }

    /// Nested transforms are why nothing is hoisted: each one calls its input
    /// with a `uv` of its own.
    #[test]
    fn nested_transforms_pass_their_own_uv_down() {
        let graph = chain(&[source(), Kind::Transform, Kind::Transform]);
        let wgsl = generated(&graph).wgsl;
        assert_eq!(wgsl.matches("return n2(q);").count(), 1);
        assert_eq!(wgsl.matches("return n3(q);").count(), 1);
    }

    /// The order is the point: WGSL is read top to bottom.
    #[test]
    fn a_node_is_declared_before_it_is_called() {
        let graph = chain(&[source(), Kind::Level, Kind::Hsv]);
        let generated = generated(&graph);
        let declared = |id: u64| {
            generated
                .wgsl
                .find(&format!("fn n{id}(uv"))
                .expect("declared")
        };
        assert!(declared(2) < declared(3));
        assert!(declared(3) < declared(4));
        assert!(declared(4) < declared(1));
        assert_eq!(generated.slots, vec![2, 3, 4, 1]);
    }

    #[test]
    fn unreachable_nodes_cost_nothing() {
        let mut graph = chain(&[source(), Kind::Level]);
        reduce(
            &mut graph,
            Msg::AddNode {
                kind: Kind::Hsv,
                pos: [0.0, 0.0],
            },
        );
        let generated = generated(&graph);
        assert!(!generated.wgsl.contains("fn n4("));
        assert!(!generated.slots.contains(&4));
    }

    #[test]
    fn a_cycle_is_refused() {
        let mut graph = chain(&[source(), Kind::Level, Kind::Hsv]);
        // The hsv node feeds the level node that feeds it.
        reduce(
            &mut graph,
            Msg::Connect {
                from: 4,
                to: 3,
                port: 0,
            },
        );
        assert!(matches!(generate(&graph), Err(GenError::Cycle(_))));

        // And so is a node wired to itself.
        let mut graph = chain(&[source(), Kind::Level]);
        reduce(
            &mut graph,
            Msg::Connect {
                from: 3,
                to: 3,
                port: 0,
            },
        );
        assert!(matches!(generate(&graph), Err(GenError::Cycle(3))));
    }

    #[test]
    fn a_broken_expression_is_a_message_and_not_a_pipeline() {
        let mut graph = chain(&[source()]);
        reduce(
            &mut graph,
            Msg::SetShaderSrc {
                node: 2,
                src: String::from("vec4<f32>(nope)"),
            },
        );
        let Err(GenError::Wgsl(message)) = generate(&graph) else {
            panic!("naga accepted an undefined identifier");
        };
        assert!(message.contains("nope"), "{message}");
    }

    #[test]
    fn a_full_uniform_block_is_an_error_and_not_a_wrong_picture() {
        let mut graph = Graph::empty();
        // Straight past the limit, bypassing `reduce`'s own cap: the check in
        // `generate` is what protects the shader.
        let mut previous = None;
        for id in 2..2 + SLOTS as u64 {
            graph.nodes.push(Node {
                id,
                name: format!("level{id}"),
                kind: Kind::Level,
                pos: [0.0, 0.0],
                inputs: [previous, None],
                params: Kind::Level.defaults(),
            });
            previous = Some(id);
        }
        graph.nodes[0].inputs[0] = previous;
        assert!(matches!(generate(&graph), Err(GenError::TooManyNodes)));
    }

    #[test]
    fn the_hash_follows_the_text() {
        let graph = chain(&[source(), Kind::Level]);
        let first = generated(&graph);

        let mut renamed = graph.clone();
        reduce(
            &mut renamed,
            Msg::Rename {
                node: 3,
                name: String::from("bright"),
            },
        );
        assert_ne!(generated(&renamed).hash, first.hash);

        let mut nudged = graph.clone();
        reduce(
            &mut nudged,
            Msg::SetParam {
                node: 3,
                index: 0,
                value: 0.7,
            },
        );
        let nudged = generated(&nudged);
        assert_eq!(nudged.wgsl, first.wgsl, "a parameter is not in the text");
        assert_eq!(nudged.hash, first.hash);
    }

    #[test]
    fn parameters_are_packed_into_their_slots() {
        let mut graph = chain(&[source(), Kind::Level]);
        reduce(
            &mut graph,
            Msg::SetParam {
                node: 3,
                index: 0,
                value: 0.25,
            },
        );
        let generated = generated(&graph);
        let packed = pack_params(&graph, &generated.slots);
        let slot = generated
            .slots
            .iter()
            .position(|id| *id == 3)
            .expect("the level node is in the picture");
        assert_eq!(packed[slot][0], 0.25);
        assert!(generated.wgsl.contains(&format!("let p = U.p[{slot}];")));
    }
}
