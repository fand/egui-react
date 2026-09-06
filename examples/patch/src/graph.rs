//! The patch: nodes, the links between them, the messages that change them,
//! and the reducer.
//!
//! Nothing here knows about egui or about wgpu — a node's position is two
//! floats, not a `Pos2` — so the whole model can be built and reduced in a unit
//! test, and `codegen.rs` next door can turn it into a shader without either
//! side of the screen being involved.
//!
//! The two revision counters are the point of the file. Every message says, by
//! which counter it bumps, whether it can change the *program* or only the
//! *numbers the program reads*. That single distinction is what lets the UI
//! recompile a shader when a wire moves and not when a slider does.

use serde::{Deserialize, Serialize};

pub type NodeId = u64;

/// How many nodes a patch may hold.
///
/// The generated uniform block is `array<vec4<f32>, 32>` and every node that
/// ends up in the picture takes one slot of it, so this is a limit of the
/// shader, not of the editor.
pub const MAX_NODES: usize = 32;

/// How two pictures are combined by a [`Kind::Mix`] node.
///
/// The mode is baked into the generated WGSL rather than read from a uniform:
/// a different mode is a different expression, which is exactly what makes it
/// a topology change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MixMode {
    #[default]
    Mix,
    Add,
    Multiply,
    Screen,
    Difference,
}

impl MixMode {
    pub const ALL: [Self; 5] = [
        Self::Mix,
        Self::Add,
        Self::Multiply,
        Self::Screen,
        Self::Difference,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Mix => "mix",
            Self::Add => "add",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Difference => "difference",
        }
    }
}

/// How colour is flattened to grey. Baked in, for the same reason as
/// [`MixMode`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrayMethod {
    #[default]
    Luma,
    Average,
    Max,
}

impl GrayMethod {
    pub const ALL: [Self; 3] = [Self::Luma, Self::Average, Self::Max];

    pub fn name(self) -> &'static str {
        match self {
            Self::Luma => "luma",
            Self::Average => "average",
            Self::Max => "max",
        }
    }
}

/// What a node does. Everything that changes the *shape* of the generated
/// code is in here; everything that is only a number is in [`Node::params`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    /// A WGSL expression written by hand: the only source of a picture.
    Shader { src: String },
    /// Brightness, contrast, gamma.
    Level,
    /// Hue, saturation, value.
    Hsv,
    /// Colour to grey, by one of three formulas.
    Grayscale { method: GrayMethod },
    /// Moves, turns and scales the *uv* before asking its input for a colour.
    Transform,
    /// Two pictures, one blend.
    Mix { mode: MixMode },
    /// Flips the colour towards its opposite.
    Invert,
    /// Rounds each channel down to a few steps.
    Posterize,
    /// Reads one colour per square cell, like a big pixel.
    Pixelate,
    /// Repeats the picture across the frame.
    Tile,
    /// The end of the chain. Exactly one per patch.
    Output,
}

/// What the palette offers, in the order it lists them.
pub const DEFAULT_SRC: &str = "vec4<f32>(0.5 + 0.5 * sin(uv.x * 8.0 * p0 + t), 0.5 + 0.5 * sin(uv.y * 8.0 * p1 - t), p2, 1.0)";

impl Kind {
    /// One of every kind a user may add, ready to be pushed into a patch.
    ///
    /// `Output` is not here: a patch has exactly one and it comes with the
    /// empty patch.
    pub fn palette() -> [Self; 10] {
        [
            Self::Shader {
                src: String::from(DEFAULT_SRC),
            },
            Self::Level,
            Self::Hsv,
            Self::Grayscale {
                method: GrayMethod::Luma,
            },
            Self::Transform,
            Self::Mix { mode: MixMode::Mix },
            Self::Invert,
            Self::Posterize,
            Self::Pixelate,
            Self::Tile,
        ]
    }

    /// The kind's name, which is also the stem of the names nodes get.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Shader { .. } => "shader",
            Self::Level => "level",
            Self::Hsv => "hsv",
            Self::Grayscale { .. } => "grayscale",
            Self::Transform => "transform",
            Self::Mix { .. } => "mix",
            Self::Invert => "invert",
            Self::Posterize => "posterize",
            Self::Pixelate => "pixelate",
            Self::Tile => "tile",
            Self::Output => "out",
        }
    }

    /// How many inputs the node accepts, 0 to 2.
    pub fn inputs(&self) -> usize {
        match self {
            Self::Shader { .. } => 0,
            Self::Mix { .. } => 2,
            _ => 1,
        }
    }

    /// What an input is called next to its socket.
    ///
    /// Only `Mix` needs more than "in": its two pictures are not
    /// interchangeable, so they are named after the blend that reads them.
    pub fn input_label(&self, port: usize) -> &'static str {
        match self {
            Self::Mix { .. } if port == 0 => "A",
            Self::Mix { .. } => "B",
            _ => "in",
        }
    }

    /// The parameters a new node of this kind starts with.
    pub fn defaults(&self) -> [f32; 4] {
        match self {
            Self::Shader { .. } => [1.0, 1.0, 0.5, 0.0],
            // brightness, contrast, gamma
            Self::Level => [0.0, 1.0, 1.0, 0.0],
            // hue shift, saturation, value
            Self::Hsv => [0.0, 1.0, 1.0, 0.0],
            // x, y, rotation, scale
            Self::Transform => [0.0, 0.0, 0.0, 1.0],
            // amount
            Self::Mix { .. } => [0.5, 0.0, 0.0, 0.0],
            // amount
            Self::Invert => [1.0, 0.0, 0.0, 0.0],
            // levels
            Self::Posterize => [4.0, 0.0, 0.0, 0.0],
            // cells
            Self::Pixelate => [16.0, 0.0, 0.0, 0.0],
            // x, y
            Self::Tile => [2.0, 2.0, 0.0, 0.0],
            Self::Grayscale { .. } | Self::Output => [0.0; 4],
        }
    }
}

/// One node: what it does, where it sits, what feeds it, and its four numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// The name the user sees. It is written into the generated WGSL as a
    /// comment, which is why renaming counts as a topology change.
    pub name: String,
    pub kind: Kind,
    /// Position in patch coordinates. Two floats rather than an `egui::Pos2`,
    /// so this file needs no egui and the JSON needs no feature of emath's.
    pub pos: [f32; 2],
    /// What feeds each input port. `None` is black.
    pub inputs: [Option<NodeId>; 2],
    /// One `vec4` of the uniform block. What the four floats mean is the
    /// [`Kind`]'s business, and the shader's.
    pub params: [f32; 4],
}

/// The whole patch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    /// Drawing order, which is also stacking order: [`Msg::Raise`] moves the
    /// node the user grabbed to the end.
    pub nodes: Vec<Node>,
    next_id: u64,
    /// Bumped by every change that could alter the generated WGSL.
    ///
    /// This is the deps of the memo that regenerates the shader. Rewiring,
    /// adding, deleting, renaming, editing a source or picking another blend
    /// mode all land here.
    pub topology_rev: u64,
    /// Bumped by changes that reach the picture only through the uniform
    /// buffer — a slider, a node dragged somewhere else, a node raised.
    pub param_rev: u64,
}

/// Everything that can change a patch. One message per user action.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    AddNode {
        kind: Kind,
        pos: [f32; 2],
    },
    RemoveNode(NodeId),
    MoveNode {
        node: NodeId,
        pos: [f32; 2],
    },
    Connect {
        from: NodeId,
        to: NodeId,
        port: usize,
    },
    Disconnect {
        to: NodeId,
        port: usize,
    },
    SetParam {
        node: NodeId,
        index: usize,
        value: f32,
    },
    SetShaderSrc {
        node: NodeId,
        src: String,
    },
    SetMode {
        node: NodeId,
        mode: MixMode,
    },
    SetGray {
        node: NodeId,
        method: GrayMethod,
    },
    Rename {
        node: NodeId,
        name: String,
    },
    /// Bring a node to the front of the drawing order.
    Raise(NodeId),
    /// Replace the whole patch with a loaded one.
    Load(Box<Graph>),
}

impl Graph {
    /// A patch with nothing in it but the one node every patch has.
    pub fn empty() -> Self {
        Self {
            nodes: vec![Node {
                id: 1,
                name: String::from("out1"),
                kind: Kind::Output,
                pos: [420.0, 150.0],
                inputs: [None, None],
                params: [0.0; 4],
            }],
            next_id: 2,
            topology_rev: 0,
            param_rev: 0,
        }
    }

    /// A patch built from parts, for the preset and for tests.
    ///
    /// Both counters start at zero; [`Msg::Load`] steps them past whatever the
    /// running patch was at when a preset like this is dropped in.
    pub fn from_parts(nodes: Vec<Node>, next_id: u64) -> Self {
        Self {
            nodes,
            next_id,
            topology_rev: 0,
            param_rev: 0,
        }
    }

    /// Whether this is still the patch [`Graph::empty`] makes — the question
    /// the preset asks before it seeds anything (see `lib.rs`).
    pub fn is_untouched(&self) -> bool {
        self.nodes.len() == 1 && self.nodes[0].kind == Kind::Output
    }

    /// The first place near `start` where a new node does not land on top of
    /// one that is already there.
    ///
    /// Walks a coarse grid: across, then down. A canvas has no layout to ask,
    /// so "where does this go" is arithmetic, and it is here rather than in the
    /// UI because it is a question about the patch.
    pub fn free_pos(&self, start: [f32; 2]) -> [f32; 2] {
        const COLUMN: f32 = 190.0;
        const ROW: f32 = 150.0;
        let taken = |pos: [f32; 2]| {
            self.nodes.iter().any(|node| {
                (node.pos[0] - pos[0]).abs() < COLUMN - 14.0
                    && (node.pos[1] - pos[1]).abs() < ROW - 10.0
            })
        };

        let mut pos = start;
        for _ in 0..MAX_NODES * 2 {
            if !taken(pos) {
                return pos;
            }
            pos[0] += COLUMN;
            if pos[0] > start[0] + COLUMN * 3.0 {
                pos[0] = start[0];
                pos[1] += ROW;
            }
        }
        pos
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    /// The node the picture comes out of, if the patch still has one.
    pub fn output(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|node| node.kind == Kind::Output)
            .map(|node| node.id)
    }

    /// `level`, `level2`, `level3`: the first free name with this stem.
    fn unique_name(&self, stem: &str) -> String {
        let mut n = 1;
        loop {
            let name = if n == 1 {
                format!("{stem}1")
            } else {
                format!("{stem}{n}")
            };
            if !self.nodes.iter().any(|node| node.name == name) {
                return name;
            }
            n += 1;
        }
    }
}

/// Apply one message.
///
/// Which counter a message bumps *is* the specification of this example: a
/// message on the topology side can change the generated program, a message on
/// the parameter side can only change what the program reads. `graph.rs` never
/// says the word "shader" — it only sorts the messages — and `codegen.rs` never
/// hears about revisions.
///
/// A message that changes nothing bumps nothing, so no undo step is made for
/// it (the history in `use_undoable` compares before and after).
pub fn reduce(graph: &mut Graph, msg: Msg) {
    match msg {
        Msg::AddNode { kind, pos } => {
            if graph.nodes.len() >= MAX_NODES {
                return;
            }
            let id = graph.next_id;
            graph.next_id += 1;
            let name = graph.unique_name(kind.name());
            let params = kind.defaults();
            graph.nodes.push(Node {
                id,
                name,
                kind,
                pos,
                inputs: [None, None],
                params,
            });
            graph.topology_rev += 1;
        }

        Msg::RemoveNode(id) => {
            // The output is the one node that cannot go: there would be
            // nothing to generate from.
            if graph.node(id).map(|node| &node.kind) == Some(&Kind::Output) {
                return;
            }
            let before = graph.nodes.len();
            graph.nodes.retain(|node| node.id != id);
            if graph.nodes.len() == before {
                return;
            }
            for node in &mut graph.nodes {
                for input in &mut node.inputs {
                    if *input == Some(id) {
                        *input = None;
                    }
                }
            }
            graph.topology_rev += 1;
        }

        Msg::MoveNode { node, pos } => {
            if let Some(node) = graph.node_mut(node)
                && node.pos != pos
            {
                node.pos = pos;
                graph.param_rev += 1;
            }
        }

        Msg::Connect { from, to, port } => {
            if graph.node(from).is_none() {
                return;
            }
            let Some(node) = graph.node_mut(to) else {
                return;
            };
            if port >= node.kind.inputs() || node.inputs[port] == Some(from) {
                return;
            }
            node.inputs[port] = Some(from);
            // A cycle is allowed to exist. `codegen` reports it and the
            // preview keeps the last program that worked, which is a kinder
            // answer than refusing the wire the user just drew.
            graph.topology_rev += 1;
        }

        Msg::Disconnect { to, port } => {
            if let Some(node) = graph.node_mut(to)
                && port < node.inputs.len()
                && node.inputs[port].is_some()
            {
                node.inputs[port] = None;
                graph.topology_rev += 1;
            }
        }

        Msg::SetParam { node, index, value } => {
            if let Some(node) = graph.node_mut(node)
                && index < 4
                && node.params[index] != value
            {
                node.params[index] = value;
                graph.param_rev += 1;
            }
        }

        Msg::SetShaderSrc { node, src } => {
            if let Some(node) = graph.node_mut(node)
                && let Kind::Shader { src: current } = &mut node.kind
                && *current != src
            {
                *current = src;
                graph.topology_rev += 1;
            }
        }

        Msg::SetMode { node, mode } => {
            if let Some(node) = graph.node_mut(node)
                && let Kind::Mix { mode: current } = &mut node.kind
                && *current != mode
            {
                *current = mode;
                graph.topology_rev += 1;
            }
        }

        Msg::SetGray { node, method } => {
            if let Some(node) = graph.node_mut(node)
                && let Kind::Grayscale { method: current } = &mut node.kind
                && *current != method
            {
                *current = method;
                graph.topology_rev += 1;
            }
        }

        Msg::Rename { node, name } => {
            let name = name.trim().to_owned();
            if name.is_empty() {
                return;
            }
            if let Some(node) = graph.node_mut(node)
                && node.name != name
            {
                node.name = name;
                // The name is a comment in the generated WGSL, so this really
                // is a change to the program.
                graph.topology_rev += 1;
            }
        }

        Msg::Raise(id) => {
            let Some(index) = graph.nodes.iter().position(|node| node.id == id) else {
                return;
            };
            if index + 1 == graph.nodes.len() {
                return;
            }
            let node = graph.nodes.remove(index);
            graph.nodes.push(node);
            graph.param_rev += 1;
        }

        Msg::Load(loaded) => {
            // Both counters step past whatever the loaded patch was saved
            // with, so a memo can never mistake the new patch for the old one
            // at the same revision.
            let topology = graph.topology_rev.max(loaded.topology_rev) + 1;
            let param = graph.param_rev.max(loaded.param_rev) + 1;
            *graph = *loaded;
            graph.topology_rev = topology;
            graph.param_rev = param;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A patch with one shader feeding the output, for the tests below.
    fn patch() -> Graph {
        let mut graph = Graph::empty();
        reduce(
            &mut graph,
            Msg::AddNode {
                kind: Kind::Shader {
                    src: String::from(DEFAULT_SRC),
                },
                pos: [0.0, 0.0],
            },
        );
        reduce(
            &mut graph,
            Msg::AddNode {
                kind: Kind::Level,
                pos: [0.0, 0.0],
            },
        );
        graph
    }

    /// Which side a message falls on, as a pair of "did it move".
    fn revs(graph: &Graph, msg: Msg) -> (bool, bool) {
        let mut after = graph.clone();
        reduce(&mut after, msg);
        (
            after.topology_rev != graph.topology_rev,
            after.param_rev != graph.param_rev,
        )
    }

    /// The table the whole example rests on: one line per message.
    #[test]
    fn every_message_picks_one_side() {
        let graph = patch();
        let (shader, level, out) = (2, 3, 1);

        const TOPOLOGY: (bool, bool) = (true, false);
        const PARAMS: (bool, bool) = (false, true);

        assert_eq!(
            revs(
                &graph,
                Msg::AddNode {
                    kind: Kind::Hsv,
                    pos: [0.0, 0.0]
                }
            ),
            TOPOLOGY
        );
        assert_eq!(revs(&graph, Msg::RemoveNode(level)), TOPOLOGY);
        assert_eq!(
            revs(
                &graph,
                Msg::Connect {
                    from: shader,
                    to: level,
                    port: 0
                }
            ),
            TOPOLOGY
        );
        assert_eq!(
            revs(
                &graph,
                Msg::SetShaderSrc {
                    node: shader,
                    src: String::from("vec4<f32>(1.0)")
                }
            ),
            TOPOLOGY
        );
        assert_eq!(
            revs(
                &graph,
                Msg::Rename {
                    node: level,
                    name: String::from("bright")
                }
            ),
            TOPOLOGY
        );
        assert_eq!(
            revs(
                &graph,
                Msg::MoveNode {
                    node: level,
                    pos: [10.0, 10.0]
                }
            ),
            PARAMS
        );
        assert_eq!(
            revs(
                &graph,
                Msg::SetParam {
                    node: level,
                    index: 0,
                    value: 0.5
                }
            ),
            PARAMS
        );
        assert_eq!(revs(&graph, Msg::Raise(shader)), PARAMS);

        // Disconnect and the two mode setters need something to act on.
        let mut wired = graph.clone();
        reduce(
            &mut wired,
            Msg::Connect {
                from: shader,
                to: out,
                port: 0,
            },
        );
        assert_eq!(revs(&wired, Msg::Disconnect { to: out, port: 0 }), TOPOLOGY);

        let mut mixed = graph.clone();
        reduce(
            &mut mixed,
            Msg::AddNode {
                kind: Kind::Mix { mode: MixMode::Mix },
                pos: [0.0, 0.0],
            },
        );
        assert_eq!(
            revs(
                &mixed,
                Msg::SetMode {
                    node: 4,
                    mode: MixMode::Screen
                }
            ),
            TOPOLOGY
        );

        let mut grey = graph.clone();
        reduce(
            &mut grey,
            Msg::AddNode {
                kind: Kind::Grayscale {
                    method: GrayMethod::Luma,
                },
                pos: [0.0, 0.0],
            },
        );
        assert_eq!(
            revs(
                &grey,
                Msg::SetGray {
                    node: 4,
                    method: GrayMethod::Max
                }
            ),
            TOPOLOGY
        );

        // A load moves both, and past the loaded patch's own counters.
        let (topology, param) = (graph.topology_rev, graph.param_rev);
        let mut loaded = graph.clone();
        reduce(&mut loaded, Msg::Load(Box::new(Graph::empty())));
        assert!(loaded.topology_rev > topology && loaded.param_rev > param);
    }

    /// A message that changes nothing is not an undo step.
    #[test]
    fn a_no_op_moves_neither_counter() {
        let graph = patch();
        assert_eq!(revs(&graph, Msg::RemoveNode(99)), (false, false));
        assert_eq!(
            revs(
                &graph,
                Msg::SetParam {
                    node: 3,
                    index: 0,
                    value: 0.0
                }
            ),
            (false, false)
        );
        assert_eq!(
            revs(
                &graph,
                Msg::MoveNode {
                    node: 3,
                    pos: graph.node(3).unwrap().pos
                }
            ),
            (false, false)
        );
        // The output cannot be deleted, and a shader has no second input.
        assert_eq!(revs(&graph, Msg::RemoveNode(1)), (false, false));
        assert_eq!(
            revs(
                &graph,
                Msg::Connect {
                    from: 3,
                    to: 2,
                    port: 0
                }
            ),
            (false, false)
        );
    }

    #[test]
    fn deleting_a_node_unplugs_it() {
        let mut graph = patch();
        reduce(
            &mut graph,
            Msg::Connect {
                from: 2,
                to: 3,
                port: 0,
            },
        );
        reduce(
            &mut graph,
            Msg::Connect {
                from: 3,
                to: 1,
                port: 0,
            },
        );
        reduce(&mut graph, Msg::RemoveNode(3));

        assert_eq!(graph.node(1).unwrap().inputs, [None, None]);
        assert!(graph.node(3).is_none());
    }

    #[test]
    fn raising_a_node_moves_it_to_the_end() {
        let mut graph = patch();
        reduce(&mut graph, Msg::Raise(1));
        let order: Vec<NodeId> = graph.nodes.iter().map(|node| node.id).collect();
        assert_eq!(order, vec![2, 3, 1]);
    }

    /// A new node goes somewhere free, so it never lands exactly on another.
    #[test]
    fn a_new_node_gets_a_place_of_its_own() {
        let mut graph = Graph::empty();
        graph.nodes[0].pos = [40.0, 40.0];
        let first = graph.free_pos([40.0, 40.0]);
        assert_ne!(first, [40.0, 40.0]);

        reduce(
            &mut graph,
            Msg::AddNode {
                kind: Kind::Level,
                pos: first,
            },
        );
        assert_ne!(graph.free_pos([40.0, 40.0]), first);
    }

    #[test]
    fn names_do_not_repeat() {
        let mut graph = Graph::empty();
        for _ in 0..3 {
            reduce(
                &mut graph,
                Msg::AddNode {
                    kind: Kind::Level,
                    pos: [0.0, 0.0],
                },
            );
        }
        let names: Vec<&str> = graph.nodes.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, vec!["out1", "level1", "level2", "level3"]);
    }

    #[test]
    fn the_patch_never_grows_past_the_uniform_block() {
        let mut graph = Graph::empty();
        for _ in 0..MAX_NODES + 4 {
            reduce(
                &mut graph,
                Msg::AddNode {
                    kind: Kind::Level,
                    pos: [0.0, 0.0],
                },
            );
        }
        assert_eq!(graph.nodes.len(), MAX_NODES);
    }
}
