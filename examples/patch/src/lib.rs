//! A node editor that writes the shader it is previewing.
//!
//! The picture on the right is not drawn by the editor; it is drawn by a
//! fragment shader that the patch on the left *is*. Wire two nodes together
//! and a new WGSL program is generated, validated and compiled. Move a slider
//! and nothing is generated at all — four floats reach a uniform buffer and
//! the same program draws a different picture.
//!
//! **Two revisions, two memos.** That difference is the whole example, and it
//! is one decision made in one place: every message in `graph.rs` bumps either
//! `topology_rev` (it could change the program) or `param_rev` (it can only
//! change the numbers). The screen hangs two derivations off those counters —
//!
//! ```text
//! topology_rev ─▶ use_memo ─▶ WGSL + naga ─▶ source hash ─▶ pipeline (in prepare)
//! param_rev    ─▶ use_memo ─▶ [[f32; 4]; 32] ────────────▶ uniform buffer
//! ```
//!
//! — and a test (P-5) pins it: dragging a slider does not change one character
//! of the generated source, and rewiring does. Nothing watches for that; it
//! falls out of what the deps of each memo are.
//!
//! **Nodes at absolute positions.** `ItemStyle` has no `position: absolute`
//! and does not need one. The canvas is a single leaf: it allocates its
//! rectangle, and for each node opens a child `Ui` at that node's coordinates
//! and builds a `Cx` around it (the `escape-hatch` example's `Nested`, one
//! level up). Inside a node it is ordinary `<View>` flexbox again.
//!
//! The line that keys those nodes is written by hand here:
//!
//! ```ignore
//! cx.scope(node.id, |cx| rsx! { <NodeView node={node} .. /> }.show(cx));
//! ```
//!
//! which is exactly what `key={node.id}` does inside `rsx!` — mix the key into
//! the scope id. Because the key is the node's *identity* and the canvas is a
//! flat list, a node's own state (whether it is collapsed, which port the
//! pointer is over) survives deleting another node and survives `Raise`
//! reordering the vector. `board` needed `use_identity` for the same effect
//! because its cards sit inside columns and moving one changed its parent;
//! here nothing is in between, so the key alone is enough.
//!
//! The rest is the usual company: `use_reducer` (inside board's `use_undoable`)
//! owns the patch, `use_persisted` keeps it across restarts, `use_future` and
//! `<Suspense>` load the preset while the palette carries on, `provide_context`
//! hands the dispatcher, the drag session and the port map to the tree, and the
//! drag session itself is `board`'s `use_dnd` with a different payload type.
//!
//! No `Panel`, so the gallery can run it in a column of its own.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use board::hooks::{Dnd, Undoable, use_dnd, use_undoable};
use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

pub mod codegen;
pub mod gpu;
pub mod graph;
pub mod preset;

use codegen::{GenError, Generated};
use graph::{Graph, GrayMethod, Kind, MixMode, Msg, Node, NodeId, reduce};

pub const META: Meta = Meta {
    name: "patch",
    summary: "A node editor that generates, validates and previews its own WGSL shader.",
    hooks: &[
        "use_state",
        "use_reducer",
        "use_persisted",
        "use_memo",
        "use_future",
        "use_handle",
        "provide_context",
        "use_context",
        "#[hook]",
        "Cx::new",
        "Cx::leaf",
    ],
    elements: &[
        "View",
        "Text",
        "Button",
        "Canvas",
        "Suspense",
        "ScrollArea",
        "Frame",
        "Checkbox",
        "ComboBox",
        "Separator",
    ],
    source: include_str!("lib.rs"),
    plain: None,
};

/// One end of a wire: a node and one of its sockets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortRef {
    pub node: NodeId,
    pub port: Port,
}

/// Which socket. Inputs are numbered; there is only ever one output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Port {
    In(usize),
    Out,
}

/// Which edge of a node a socket sits on: inputs on the left, the output on
/// the right, the way the wires run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The drag session, `board`'s hook with a port at both ends: a port is picked
/// up and another port is where it lands.
pub type PortDnd = Dnd<PortRef, PortRef>;

/// What everything on screen sends to the reducer.
///
/// `Dispatch` is `Clone + Send + 'static`, which is what lets it travel by
/// context while a `State` guard could not (ARCHITECTURE 3.5).
pub type Actions = Dispatch<Undoable<Msg>>;

/// Where every port was drawn this frame.
///
/// A node's ports are laid out by flexbox, so the middle of a port circle is
/// not known until the node has been drawn — and the wires need all of them.
/// The nodes write into this as they go and the canvas reads it afterwards,
/// painting the wires into a shape slot it reserved before drawing anything.
///
/// The `Rc<RefCell<_>>` is not decoration. This is written on every frame, and
/// every write on `Handle` (`set`, `update`) asks for a repaint, which would
/// mean an app that never goes idle (ARCHITECTURE 5.6). `Handle::with` hands
/// out a `&T` without dirtying anything, so the scribbling happens inside the
/// value — the same shape, and for the same reason, as `board`'s `Dnd`.
#[derive(Clone, Default)]
pub struct Ports {
    seen: Rc<RefCell<HashMap<PortRef, egui::Pos2>>>,
}

impl Ports {
    /// Forget the last frame's positions.
    fn begin_frame(&self) {
        self.seen.borrow_mut().clear();
    }

    /// Report where a port is, from the node that just drew it.
    pub fn put(&self, port: PortRef, at: egui::Pos2) {
        self.seen.borrow_mut().insert(port, at);
    }

    /// Where a port is, or `None` if it has not been drawn yet.
    pub fn at(&self, port: PortRef) -> Option<egui::Pos2> {
        self.seen.borrow().get(&port).copied()
    }
}

/// The port map for the whole tree, cleared once per frame.
#[hook]
fn use_ports<'s>(cx: &mut Cx<'s, '_>) -> Handle<'s, Ports> {
    let ports = use_handle(cx, Ports::default);
    // `with`, not `update`: this runs every frame and `update` would ask for a
    // repaint every frame with it.
    ports.with(Ports::begin_frame);
    ports
}

/// The port map, or an unattached one outside a provider.
#[hook]
fn use_port_map(cx: &mut Cx) -> Ports {
    use_context::<Ports>(cx).map_or_else(Ports::default, |ports| ports.get())
}

/// The drag session, or an unattached one outside a provider.
#[hook]
fn use_drag(cx: &mut Cx) -> PortDnd {
    use_context::<PortDnd>(cx).map_or_else(PortDnd::new, |dnd| dnd.get())
}

/// The dispatcher, if this is drawn under one.
#[hook]
fn use_actions(cx: &mut Cx) -> Option<Actions> {
    use_context::<Actions>(cx).map(|actions| actions.get())
}

/// Send one message to the patch's reducer.
fn send(actions: &Option<Actions>, msg: Msg) {
    if let Some(actions) = actions {
        actions.send(Undoable::Do(msg));
    }
}

/// A generated program, kept until a better one arrives.
///
/// The last one that compiled, not the last one that was asked for: a cycle or
/// a mistyped expression leaves this alone, so the preview keeps drawing and
/// the `wgsl` tab keeps showing what is actually on the GPU (test P-4).
#[derive(Clone)]
struct Program {
    wgsl: Arc<str>,
    hash: u64,
    slots: Vec<NodeId>,
}

/// What a node's header drag is doing.
#[derive(Clone, Copy, Debug)]
enum Drag {
    Start,
    By(egui::Vec2),
    End,
}

/// The width of a node box, in points. Fixed, so the canvas can place a node
/// before the node has drawn itself.
const NODE_W: f32 = 168.0;

/// How much room a node's child `Ui` is given. The node draws its own frame
/// and takes only the height it needs; this is the ceiling.
const NODE_MAX_H: f32 = 300.0;

/// The diameter of a port circle.
const PORT: f32 = 12.0;

/// How much bigger than the circle the drop target is, so a port can be aimed
/// at without precision.
const PORT_PAD: f32 = 5.0;

/// The height of the preview canvas.
const PREVIEW_H: f32 = 168.0;

/// What a node is about as tall as, for placing the view over the nodes.
/// The real height is only known once the node has drawn itself.
const NODE_H_NOMINAL: f32 = 150.0;

/// Where the canvas is looking. A patch point `p` is drawn at
/// `canvas.min + zoom * (pan + p)`.
///
/// `pan` is in patch units, so at `zoom == 1` the nodes are drawn exactly
/// where they would be with no layer transform at all, and none is set. That
/// is the state a test looks at: kittest reads widget rectangles in layer
/// coordinates, and at zoom 1 those are screen coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub pan: egui::Vec2,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub const ZOOM: std::ops::RangeInclusive<f32> = 0.25..=3.0;

    /// The layer transform: scale about the canvas's corner, so that at zoom
    /// 1 it is the identity whatever the pan.
    fn to_global(self, corner: egui::Pos2) -> egui::emath::TSTransform {
        use egui::emath::TSTransform;
        TSTransform::from_translation(corner.to_vec2())
            * TSTransform::from_scaling(self.zoom)
            * TSTransform::from_translation(-corner.to_vec2())
    }

    /// The patch point under a screen point.
    fn patch_at(self, corner: egui::Pos2, screen: egui::Pos2) -> egui::Pos2 {
        ((screen - corner) / self.zoom - self.pan).to_pos2()
    }

    /// Zoom by `factor` about a screen point, which stays where it is.
    fn zoomed(self, factor: f32, corner: egui::Pos2, about: egui::Pos2) -> Self {
        let zoom = (self.zoom * factor).clamp(*Self::ZOOM.start(), *Self::ZOOM.end());
        let under = self.patch_at(corner, about);
        Self {
            pan: (about - corner) / zoom - under.to_vec2(),
            zoom,
        }
    }

    /// Zoom 1, with the middle of the nodes in the middle of a canvas of
    /// `size`. An empty patch looks at its origin.
    fn home(nodes: &[Node], size: egui::Vec2) -> Self {
        let bounds = nodes.iter().fold(egui::Rect::NOTHING, |bounds, node| {
            bounds.union(egui::Rect::from_min_size(
                egui::pos2(node.pos[0], node.pos[1]),
                egui::vec2(NODE_W, NODE_H_NOMINAL),
            ))
        });
        let pan = if bounds.is_positive() {
            size / 2.0 - bounds.center().to_vec2()
        } else {
            egui::Vec2::ZERO
        };
        Self { pan, zoom: 1.0 }
    }
}

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        <PatchProvider>
            <PatchView/>
        </PatchProvider>
    }
}

/// Owns the drag session and the port map and publishes both.
///
/// A provider has to make what it provides: `provide_context` takes a `Handle`,
/// which borrows the store, and a prop may not name that lifetime
/// (ARCHITECTURE 6). The same shape as the `theme` example.
#[component(shares_ui)]
fn PatchProvider(cx: &mut Cx, children: impl View) {
    // One session for the whole tree: the port that is picked up and the port
    // it lands on are drawn by two different nodes.
    let dnd = use_dnd::<PortRef, PortRef>(cx);
    let ports = use_ports(cx);

    provide_context(cx, dnd, |cx| {
        provide_context(cx, ports, |cx| children.show(cx))
    });
}

/// The patch: the palette, the reducer behind it, and the two panes that wait
/// for the preset.
#[component]
fn PatchView(cx: &mut Cx) {
    // The reducer owns the patch and the persisted slot mirrors it, the way
    // `showcase` and `board` do. `use_persisted` first, so its value is there
    // to seed the history on the very first frame.
    let mut saved = use_persisted(cx, "patch/graph", Graph::empty);
    let (history, dispatch) = use_undoable(cx, reduce, || saved.clone());
    if *saved != history.present {
        *saved = history.present.clone();
    }
    // The `Dispatch` is what goes into the context; see [`Actions`].
    let actions = use_handle(cx, || dispatch.clone());

    // How many times the history has been walked, which is the one number
    // here that never goes backwards.
    //
    // A revision counter does: undo restores an older patch and its older
    // counter with it, so two different patches can carry the same
    // `topology_rev` — undo a rewire, then rewire differently, and the number
    // is the one it already was. A memo compares its deps with the *previous*
    // value, and in practice a frame is drawn between any two messages, so it
    // would see the intermediate revision and recompute anyway. That is a fact
    // about timing rather than a property of the deps, and it stops being true
    // the moment two messages land in one visit to the reducer. `(rev, epoch)`
    // is monotonic, so the question does not arise.
    let mut epoch = use_state(cx, || 0u64);
    let mut camera = use_state(cx, Camera::default);
    // How big the canvas is, reported by the canvas itself and written only
    // when it changes. "Recentre" is decided here, next to the camera, and
    // needs the one number the layout engine has and this component does not.
    let mut canvas = use_state(cx, egui::Vec2::default);

    // Read once: an element may not hold a shared borrow of a state *and* a
    // handler that writes it (ARCHITECTURE 3.7).
    let steps = *epoch;
    let view_at = *camera;
    let graph = &history.present;
    let full = graph.nodes.len() >= graph::MAX_NODES;
    let home = Camera::home(&graph.nodes, *canvas);
    // Where a new node lands: the first free place near the canvas's top-left
    // corner, in patch coordinates.
    let next_pos = graph.free_pos([
        -view_at.pan.x + 40.0 / view_at.zoom,
        -view_at.pan.y + 40.0 / view_at.zoom,
    ]);

    let view = rsx! {
        <View direction="row" grow={1.0} w="100%" h="100%" gap={8} p={8}>
            <Palette
                full={full}
                count={graph.nodes.len()}
                can_undo={history.can_undo()}
                can_redo={history.can_redo()}
                on_add={|kind: Kind| {
                    dispatch.send(Undoable::Do(Msg::AddNode { kind, pos: next_pos }));
                }}
                on_undo={|| {
                    *epoch += 1;
                    dispatch.send(Undoable::Undo);
                }}
                on_redo={|| {
                    *epoch += 1;
                    dispatch.send(Undoable::Redo);
                }}
                on_home={|| *camera = home}
            />
            // Only the canvas and the preview wait for the preset; the palette
            // above is drawn and usable while the future is pending.
            <Suspense fallback={view(|cx| {
                cx.leaf(&ItemStyle::default().grow(1.0), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    ui.label("loading the preset");
                });
            })}>
                <Stage
                    graph={graph}
                    epoch={steps}
                    camera={view_at}
                    on_camera={|next: Camera| *camera = next}
                    on_size={|size: egui::Vec2| {
                        // Every frame, so only a change may write.
                        if *canvas != size {
                            *canvas = size;
                        }
                    }}
                />
            </Suspense>
        </View>
    };
    provide_context(cx, actions, |cx| view.show(cx));
}

/// The node palette, the history buttons and the counts.
///
/// Everything here belongs to the patch rather than to the palette, so the
/// palette owns none of it: it is handed what to show and reports what was
/// pressed.
#[component]
fn Palette(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    count: usize,
    full: bool,
    can_undo: bool,
    can_redo: bool,
    #[event] on_add: Kind,
    #[event] on_undo: (),
    #[event] on_redo: (),
    #[event] on_home: (),
) {
    let accent = look(cx.ctx()).accent;

    rsx! {
        <View style={style} direction="column" w={150.0} shrink={0.0} gap={6}>
            <Text size={20.0} strong color={accent}>"patch"</Text>
            <View direction="row" gap={4} w="100%">
                <Button enabled={can_undo} on_click={|| on_undo.emit(())}>"undo"</Button>
                <Button enabled={can_redo} on_click={|| on_redo.emit(())}>"redo"</Button>
            </View>
            <Separator/>
            <Text strong>"add"</Text>
            for kind in Kind::palette() {
                <Button
                    key={kind.name()}
                    w="100%"
                    enabled={!full}
                    on_click={|| on_add.emit(kind.clone())}
                >
                    {kind.name()}
                </Button>
            }
            <Separator/>
            <Text>{format!("{count} / {} nodes", graph::MAX_NODES)}</Text>
            <Button w="100%" on_click={|| on_home.emit(())}>"recentre"</Button>
            <Text size={11.0} wrap w="100%">
                "Drag a header to move a node, a port to wire one. Click a wired input to unplug it. Drag the background to pan, scroll or pinch to zoom."
            </Text>
        </View>
    }
}

/// The canvas and the preview: everything that needs the patch to exist.
///
/// The preset arrives through `use_future`, so this component returns early
/// until it is here and the `<Suspense>` above draws its fallback meanwhile.
/// The file is `include_str!`, so the wait is one frame — in a real editor it
/// would be a request to a patch library, and nothing else here would change.
#[component]
fn Stage(
    cx: &mut Cx,
    graph: &Graph,
    epoch: u64,
    camera: Camera,
    #[event] on_camera: Camera,
    #[event] on_size: egui::Vec2,
) {
    let loaded = use_future(cx, (), || async { preset::parse(preset::STARTER) });
    let Poll::Ready(preset) = loaded else {
        // Nothing to draw, and nothing to say about it: the nearest boundary
        // is what shows the fallback (ARCHITECTURE 5.8).
        return;
    };

    let actions = use_actions(cx);
    // Seeded once per mount, and only into a patch nobody has touched, so a
    // saved patch is never overwritten by the preset.
    let mut seeded = use_state(cx, || false);
    if !*seeded {
        *seeded = true;
        if graph.is_untouched() {
            send(&actions, Msg::Load(Box::new(preset.clone())));
        }
    }

    // Stage one: the program. Rebuilt when the topology changed, and not when
    // a parameter did — `generate` also runs the result through naga, so this
    // is where a bad expression is caught, on the CPU, with no device around.
    let generated: &Result<Generated, GenError> =
        use_memo(cx, (graph.topology_rev, epoch), || codegen::generate(graph));

    // The last program that compiled. A memo cannot do this job: it recomputes
    // when its deps change and has no notion of keeping the previous value
    // when the new one is an error.
    let mut program = use_state(cx, || None::<Program>);
    if let Ok(next) = generated
        && program.as_ref().map(|p| p.hash) != Some(next.hash)
    {
        *program = Some(Program {
            wgsl: Arc::from(next.wgsl.as_str()),
            hash: next.hash,
            slots: next.slots.clone(),
        });
    }
    let current = program.clone();
    let slots = current
        .as_ref()
        .map(|p| p.slots.clone())
        .unwrap_or_default();

    // Stage two: the numbers. This is the memo a slider moves, and it makes no
    // string and compiles nothing.
    let params: &[[f32; 4]; codegen::SLOTS] =
        use_memo(cx, (graph.param_rev, graph.topology_rev, epoch), || {
            codegen::pack_params(graph, &slots)
        });

    let error = generated.as_ref().err().map(|err| err.to_string());
    let mut selected = use_state(cx, || None::<NodeId>);
    let picked = *selected;

    rsx! {
        <View direction="column" grow={1.0} min_w={0.0} gap={4}>
            <PatchCanvas
                grow={1.0}
                h={0.0}
                w="100%"
                graph={graph}
                camera={camera}
                selected={&picked}
                on_select={|node: NodeId| *selected = Some(node)}
                on_camera={|next: Camera| on_camera.emit(next)}
                on_size={|size: egui::Vec2| on_size.emit(size)}
            />
        </View>
        <View direction="column" w={300.0} shrink={0.0} gap={6}>
            <Preview program={&current} params={params}/>
            <Inspector
                grow={1.0}
                h={0.0}
                graph={graph}
                selected={&picked}
                program={&current}
                error={&error}
            />
        </View>
    }
}

/// The patch canvas: the one place this example leaves the layout engine.
///
/// A leaf, because absolute positions are what a node graph is and taffy has
/// no opinion about them. Inside, for every node: a child `Ui` clipped to that
/// node's rectangle, a `Cx` built around it, and `cx.scope(node.id, ..)` to key
/// the node's hooks by the node rather than by where it is in the list.
#[component]
fn PatchCanvas(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    graph: &Graph,
    camera: Camera,
    // A real `Option`, so `&Option<T>`: a bare `Option<T>` prop is the
    // *optional* kind, whose setter takes the inner value (board 8.5).
    selected: &Option<NodeId>,
    #[event] on_select: NodeId,
    #[event] on_camera: Camera,
    #[event] on_size: egui::Vec2,
) {
    let dnd = use_drag(cx);
    let ports = use_port_map(cx);
    let actions = use_actions(cx);
    let look = look(cx.ctx());

    // A drag that ended on a port becomes exactly one message, which is what
    // makes it exactly one step of the undo history.
    if let Some((from, to)) = dnd.take_drop() {
        match (from.port, to.port) {
            (Port::Out, Port::In(port)) => send(
                &actions,
                Msg::Connect {
                    from: from.node,
                    to: to.node,
                    port,
                },
            ),
            // The same wire, drawn backwards.
            (Port::In(port), Port::Out) => send(
                &actions,
                Msg::Connect {
                    from: to.node,
                    to: from.node,
                    port,
                },
            ),
            _ => {}
        }
    }

    // Where a node is while it is being dragged. This one piece of node state
    // belongs to the canvas and not to the node: the rectangle a node is drawn
    // in has to be decided *before* the node is drawn, and the offset is part
    // of it. The node reports the gesture; the canvas keeps the position, and
    // sends one `MoveNode` when the pointer is let go rather than one per
    // frame (which would be one undo step per pixel).
    let mut drag = use_state(cx, || None::<(NodeId, egui::Vec2)>);
    let live = *drag;

    let (store, scope) = (cx.store, cx.scope_id());
    let carrying = dnd.carrying();
    let chosen = *selected;

    cx.leaf_fill(&style, move |ui| {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        on_size.emit(rect.size());
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, look.canvas);
        grid(painter, rect, camera, look);

        // The nodes go in a layer of their own, right above this one, and the
        // layer is what zooms: egui scales its shapes and maps the pointer
        // back, so nothing inside a node knows. The same shape as
        // `egui::Scene`, by hand, because the zoom here is driven by the
        // scroll wheel as well as the pinch and `Scene` reads the wheel as a
        // pan.
        let to_global = camera.to_global(rect.min);
        let layer = egui::LayerId::new(ui.layer_id().order, ui.id().with("patch"));
        ui.ctx().set_sublayer(ui.layer_id(), layer);
        let mut local = ui.new_child(
            egui::UiBuilder::new()
                .layer_id(layer)
                .max_rect(to_global.inverse() * rect)
                .sense(egui::Sense::click_and_drag()),
        );
        // The clip rect is the canvas, in layer coordinates: it is what keeps
        // a node panned past the edge from painting over the palette.
        local.set_clip_rect(to_global.inverse() * rect);
        ui.ctx().set_transform_layer(layer, to_global);
        let background = local.response();

        // The background is the only thing that pans. A node's header is a
        // widget on top of it and therefore wins the pointer. `drag_delta` is
        // already divided by the zoom, so this is in patch units.
        if background.dragged() {
            on_camera.emit(Camera {
                pan: camera.pan + background.drag_delta(),
                ..camera
            });
        }
        // The wheel and the pinch both zoom, about the pointer. Only while the
        // pointer is over the canvas: the inspector has a scroll area of its
        // own.
        if let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
            && rect.contains(pointer)
        {
            let (pinch, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
            let factor = pinch * (scroll.y * 0.002).exp();
            if factor != 1.0 {
                on_camera.emit(camera.zoomed(factor, rect.min, pointer));
            }
        }

        // Everything below draws into the zoomed layer.
        let ui = &mut local;
        let painter = ui.painter();
        // Reserved now, filled at the end: the wires are drawn from positions
        // the nodes themselves report, and they belong *under* the nodes.
        let wires = painter.add(egui::Shape::Noop);

        let origin = rect.min + camera.pan;
        for node in &graph.nodes {
            let mut at = origin + egui::vec2(node.pos[0], node.pos[1]);
            if let Some((id, offset)) = live
                && id == node.id
            {
                at += offset;
            }
            // The box a node is given. It draws its own frame inside and
            // takes only the height it needs; the clip rect above cuts off
            // whatever falls outside the canvas. Nothing is culled: a node that
            // is not drawn is a node whose hooks are swept, and panning a node
            // off the edge should not forget that it was collapsed.
            let node_rect = egui::Rect::from_min_size(at, egui::vec2(NODE_W, NODE_MAX_H));
            // `id_salt` by the node, so the child `Ui`'s id does not depend on
            // where the node is in `graph.nodes`. That is only half of it:
            // egui's *auto* ids inside still count from the parent's counter,
            // so `Raise` — sent by the first frame of a header drag — would
            // still move them. Anything in a node that has to keep its id
            // across a reorder (the header, the sockets) names its id itself.
            let builder = egui::UiBuilder::new().id_salt(node.id).max_rect(node_rect);
            ui.scope_builder(builder, |ui| {
                let mut cx = Cx::new(store, ui, scope);
                // `key={node.id}`, written out. The scope chain of a node's
                // hooks is canvas → node id → component, with nothing about
                // *where* the node is in it, which is what makes a node's own
                // state stay with the node.
                cx.scope(node.id, |cx| {
                    let view = rsx! {
                        <NodeView
                            node={node}
                            selected={chosen == Some(node.id)}
                            // Whether anything reads this node, so a dangling
                            // one looks dangling. The node cannot know; the
                            // canvas has the whole patch in front of it.
                            used={graph.nodes.iter().any(|other| {
                                other.inputs.contains(&Some(node.id))
                            })}
                            on_select={|| on_select.emit(node.id)}
                            on_drag={|phase: Drag| match phase {
                                Drag::Start => {
                                    *drag = Some((node.id, egui::Vec2::ZERO));
                                    send(&actions, Msg::Raise(node.id));
                                    on_select.emit(node.id);
                                }
                                Drag::By(delta) => {
                                    if let Some((id, offset)) = drag.as_mut()
                                        && *id == node.id
                                    {
                                        *offset += delta;
                                    }
                                }
                                Drag::End => {
                                    if let Some((id, offset)) = *drag
                                        && id == node.id
                                    {
                                        send(&actions, Msg::MoveNode {
                                            node: id,
                                            pos: [node.pos[0] + offset.x, node.pos[1] + offset.y],
                                        });
                                    }
                                    *drag = None;
                                }
                            }}
                        />
                    };
                    view.show(cx);
                });
            });
        }

        // Every node has now said where its ports are.
        let mut shapes = Vec::new();
        for node in &graph.nodes {
            for (port, from) in node.inputs.iter().enumerate() {
                let Some(from) = from else { continue };
                let ends = (
                    ports.at(PortRef {
                        node: *from,
                        port: Port::Out,
                    }),
                    ports.at(PortRef {
                        node: node.id,
                        port: Port::In(port),
                    }),
                );
                if let (Some(a), Some(b)) = ends {
                    shapes.push(wire(a, b, look.wire));
                }
            }
        }
        ui.painter().set(wires, egui::Shape::Vec(shapes));

        // The wire being dragged is drawn last, on top of everything. The
        // pointer is a screen position and the wire is drawn in the layer.
        if let (Some(held), Some(pointer)) = (carrying, dnd.pointer())
            && let Some(from) = ports.at(held)
        {
            ui.painter()
                .add(wire(from, to_global.inverse() * pointer, look.accent));
        }

        // The background answers to the pointer over the whole canvas, not
        // just where the nodes happen to be.
        ui.expand_to_include_rect(to_global.inverse() * rect);
    });
}

/// One node: a header that is the drag handle, a row of sockets on the node's
/// two edges, and the controls for whatever kind of node it is.
///
/// `collapsed` and `hovered` belong to *this node*. Nothing above it knows
/// they exist, nothing has to make room for them when a node is added, and
/// nothing has to clean up after them when one is deleted — the pass-end sweep
/// does that.
#[component]
fn NodeView(
    cx: &mut Cx,
    node: &Node,
    selected: bool,
    used: bool,
    #[event] on_select: (),
    #[event] on_drag: Drag,
) {
    let actions = use_actions(cx);
    let look = look(cx.ctx());

    let mut collapsed = use_state(cx, || false);
    let mut hovered = use_state(cx, || None::<Port>);

    let open = !*collapsed;
    // The drag handle is one line of text tall, plus a little air. Taffy needs
    // the number before the strip is drawn, so it is asked for here.
    let head_h = cx.ui().text_style_height(&egui::TextStyle::Body) + 4.0;
    // What the pointer is over, in words. Node-local state doing a job: the
    // node is the only thing that knows which of its own ports is hot.
    let hint = match *hovered {
        Some(Port::Out) => String::from("out: drag to an input"),
        Some(Port::In(port)) if node.inputs[port].is_some() => {
            format!("{}: click to unplug", node.kind.input_label(port))
        }
        Some(Port::In(port)) => format!("{}: black", node.kind.input_label(port)),
        None => String::new(),
    };

    rsx! {
        <Frame
            fill={look.node}
            stroke={egui::Stroke::new(1.0, if selected { look.accent } else { look.edge })}
            corner_radius={6.0}
            // No inner margin: the sockets have to reach the node's edges, so
            // the padding goes on the rows that are not the port row.
            inner_margin={0.0}
        >
            <View direction="column" w="100%" gap={4} py={6}>
                <View direction="row" w="100%" gap={4} align="center" px={6}>
                    // The header strip is the drag handle, and one leaf is
                    // all a drag needs. `<Text>` would draw the same thing
                    // and hand back no `Response` (board 8.4).
                    //
                    // `leaf_fill`, so the strip is the whole width taffy
                    // gives it rather than the width of the name: a node is
                    // grabbed by its header, not by its title.
                    {view(|cx| {
                        let response = cx.leaf_fill(
                            &ItemStyle::default().grow(1.0).min_w(0.0).h(head_h),
                            |ui| {
                                // A named id, not an auto one: a drag lives
                                // as long as the widget keeps its id, and the
                                // first frame of this drag reorders the nodes
                                // (see `PatchCanvas`).
                                let (rect, _) = ui.allocate_exact_size(
                                    ui.available_size(),
                                    egui::Sense::hover(),
                                );
                                let response = ui.interact(
                                    rect,
                                    ui.id().with("header"),
                                    egui::Sense::click_and_drag(),
                                );
                                ui.painter().rect_filled(
                                    rect,
                                    3.0,
                                    look.edge.gamma_multiply(0.35),
                                );
                                let galley = egui::WidgetText::from(
                                    egui::RichText::new(node.name.as_str()).strong(),
                                )
                                .into_galley(
                                    ui,
                                    Some(egui::TextWrapMode::Truncate),
                                    (rect.width() - 8.0).max(0.0),
                                    egui::TextStyle::Body,
                                );
                                let at = egui::pos2(
                                    rect.left() + 4.0,
                                    rect.center().y - galley.size().y * 0.5,
                                );
                                ui.painter().galley(at, galley, ui.visuals().text_color());
                                // A painted title is not a widget, so the
                                // name has to be said out loud — it is what
                                // a screen reader, and every test here,
                                // looks the node up by.
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        node.name.as_str(),
                                    )
                                });
                                // What the pointer says it can do, and then
                                // that it is doing it.
                                if response.dragged() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    response
                                } else {
                                    response.on_hover_cursor(egui::CursorIcon::Grab)
                                }
                            },
                        );
                        if response.drag_started() {
                            on_drag.emit(Drag::Start);
                        }
                        // Only when it really moved: a write on every
                        // frame of a held pointer is a repaint on every
                        // frame of it.
                        if response.dragged() && response.drag_delta() != egui::Vec2::ZERO {
                            on_drag.emit(Drag::By(response.drag_delta()));
                        }
                        if response.drag_stopped() {
                            on_drag.emit(Drag::End);
                        }
                        if response.clicked() {
                            on_select.emit(());
                        }
                    })}

                    <SmallButton
                        label={format!("collapse {}", node.name).as_str()}
                        on_click={|| *collapsed = !*collapsed}
                    >
                        {if open { "-" } else { "+" }}
                    </SmallButton>
                </View>

                // The sockets sit on the node's own edges, where the wires
                // meet them: inputs down the left, the output on the right.
                <View direction="row" w="100%" justify="space-between" align="start">
                    // Enough room between the rows that two sockets are two
                    // targets: the area that answers to the pointer is wider
                    // than the circle.
                    <View direction="column" gap={6}>
                        for port in 0..node.kind.inputs() {
                            <View key={port} direction="row" gap={4} align="center">
                                <PortDot
                                    node={node.id}
                                    port={Port::In(port)}
                                    side={Side::Left}
                                    label={format!("{} in {port}", node.name).as_str()}
                                    connected={node.inputs[port].is_some()}
                                    on_hover={|over: bool| {
                                        set_hover(&mut hovered, Port::In(port), over)
                                    }}
                                />
                                <Text size={10.0}>{node.kind.input_label(port)}</Text>
                            </View>
                        }
                    </View>
                    // The output has none: it is the end of the chain.
                    if node.kind != Kind::Output {
                        <View direction="row" gap={4} align="center">
                            <Text size={10.0}>"out"</Text>
                            <PortDot
                                node={node.id}
                                port={Port::Out}
                                side={Side::Right}
                                label={format!("{} out", node.name).as_str()}
                                connected={used}
                                on_hover={|over: bool| set_hover(&mut hovered, Port::Out, over)}
                            />
                        </View>
                    }
                </View>

                if open {
                    <View direction="column" w="100%" gap={4} px={6}>
                        <NodeBody node={node}/>
                        <View direction="row" w="100%" gap={4} align="center">
                            <Text grow={1.0} size={10.0}>{node.kind.name()}</Text>
                            if node.kind != Kind::Output {
                                <SmallButton
                                    label={format!("delete {}", node.name).as_str()}
                                    on_click={|| send(&actions, Msg::RemoveNode(node.id))}
                                >"x"</SmallButton>
                            }
                        </View>
                    </View>
                }

                if !hint.is_empty() {
                    <Text size={10.0} px={6} color={look.accent}>{hint.as_str()}</Text>
                }
            </View>
        </Frame>
    }
}

/// Remember which port the pointer is over, and only when the answer changed.
///
/// Every port reports on every frame, so writing unconditionally would dirty
/// the node's state sixty times a second (ARCHITECTURE 5.6).
fn set_hover(hovered: &mut State<'_, Option<Port>>, port: Port, over: bool) {
    if over {
        if **hovered != Some(port) {
            **hovered = Some(port);
        }
    } else if **hovered == Some(port) {
        **hovered = None;
    }
}

/// One socket: a circle to paint, a rectangle to drag from, and a rectangle to
/// drop on.
///
/// It reads the drag session and the port map out of the context rather than
/// being handed them, because every node has two or three of these and the
/// answer is the same for all of them.
#[component]
fn PortDot(
    cx: &mut Cx,
    node: NodeId,
    port: Port,
    // Which edge the circle straddles, which is what makes a wire arrive at
    // the node instead of somewhere inside it.
    side: Side,
    // What a screen reader — and a test — calls this socket. A painted circle
    // has no name of its own, so it is given one.
    label: &str,
    connected: bool,
    #[event] on_hover: bool,
) {
    let dnd = use_drag(cx);
    let ports = use_port_map(cx);
    let actions = use_actions(cx);
    let look = look(cx.ctx());
    let dragging = dnd.carrying().is_some();

    let (centre, hit, response, layer) = cx.leaf(&ItemStyle::default().shrink(0.0), |ui| {
        // The layout keeps `PORT` square of room, but the circle is drawn half
        // outside it: its middle is on the node's edge, so a wire ends where
        // the node does. Only the canvas clips, so painting past the node's
        // own rectangle is allowed.
        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(PORT), egui::Sense::hover());
        let centre = match side {
            Side::Left => rect.left_center(),
            Side::Right => rect.right_center(),
        };
        // The circle is small and a pointer is not precise, so what answers to
        // the pointer is a square around the circle rather than the space the
        // layout gave it.
        let hit = egui::Rect::from_center_size(centre, egui::Vec2::splat(PORT)).expand(PORT_PAD);
        // Named for the same reason as the header: the socket's id must not
        // depend on where its node is in the list.
        let response = ui.interact(hit, ui.id().with(port), egui::Sense::click_and_drag());
        let hot = response.hovered() || (dragging && response.contains_pointer());
        let fill = match (connected, hot) {
            (_, true) => look.accent,
            (true, false) => look.wire,
            (false, false) => look.canvas,
        };
        ui.painter()
            .circle(centre, PORT * 0.35, fill, egui::Stroke::new(1.0, look.wire));
        // Nothing about a painted circle reaches the accessibility tree unless
        // it is said out loud.
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        (centre, hit, response, ui.layer_id())
    });

    // Where the wires are drawn from, this frame.
    ports.put(PortRef { node, port }, centre);
    // Only worth offering during a drag; `slot` checks that for itself. The
    // session compares against the screen pointer, and this rectangle is in
    // the zoomed layer, so it is mapped out first.
    let to_global = cx
        .ctx()
        .layer_transform_to_global(layer)
        .unwrap_or(egui::emath::TSTransform::IDENTITY);
    dnd.slot(to_global * hit, PortRef { node, port });

    if response.drag_started() {
        dnd.pick_up(PortRef { node, port });
    }
    if response.clicked()
        && let Port::In(index) = port
        && connected
    {
        send(
            &actions,
            Msg::Disconnect {
                to: node,
                port: index,
            },
        );
    }
    on_hover.emit(response.hovered());
}

/// The controls inside a node, by kind.
///
/// The `match` is the polymorphism: every kind has its own component, and the
/// same components are used again by the inspector with `wide` turned on. A
/// node's box is narrow, so it gets drag values; the inspector has room for
/// sliders.
#[component]
fn NodeBody(cx: &mut Cx, node: &Node, #[prop(default)] wide: bool) {
    rsx! {
        match &node.kind {
            Kind::Shader { src } => { <ShaderParams node={node} src={src.as_str()} wide={wide}/> }
            Kind::Level => { <LevelParams node={node} wide={wide}/> }
            Kind::Hsv => { <HsvParams node={node} wide={wide}/> }
            Kind::Transform => { <TransformParams node={node} wide={wide}/> }
            Kind::Mix { mode } => { <MixParams node={node} mode={*mode} wide={wide}/> }
            Kind::Invert => { <InvertParams node={node} wide={wide}/> }
            Kind::Posterize => { <PosterizeParams node={node} wide={wide}/> }
            Kind::Pixelate => { <PixelateParams node={node} wide={wide}/> }
            Kind::Tile => { <TileParams node={node} wide={wide}/> }
            // Nothing to slide: the method is the whole node.
            Kind::Grayscale { method } => { <GrayParams node={node} method={*method}/> }
            Kind::Output => { <Text size={10.0}>"the picture"</Text> }
        }
    }
}

#[component]
fn ShaderParams(cx: &mut Cx, node: &Node, src: &str, wide: bool) {
    let actions = use_actions(cx);
    let id = node.id;

    rsx! {
        <View direction="column" w="100%" gap={2}>
            <SourceEdit
                src={src}
                rows={if wide { 6 } else { 3 }}
                on_change={|next: String| send(&actions, Msg::SetShaderSrc { node: id, src: next })}
            />
            <Text size={10.0}>"uv, t, p0, p1, p2, res"</Text>
            <Knob node={id} owner={node.name.as_str()} index={0} label="p0" value={node.params[0]} range={0.0..=4.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={1} label="p1" value={node.params[1]} range={0.0..=4.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={2} label="p2" value={node.params[2]} range={0.0..=1.0} wide={wide}/>
        </View>
    }
}

#[component]
fn LevelParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="bright" value={node.params[0]} range={-1.0..=1.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={1} label="contrast" value={node.params[1]} range={0.0..=4.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={2} label="gamma" value={node.params[2]} range={0.1..=4.0} wide={wide}/>
        </View>
    }
}

#[component]
fn HsvParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="hue" value={node.params[0]} range={0.0..=1.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={1} label="sat" value={node.params[1]} range={0.0..=2.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={2} label="val" value={node.params[2]} range={0.0..=2.0} wide={wide}/>
        </View>
    }
}

#[component]
fn TransformParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="x" value={node.params[0]} range={-1.0..=1.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={1} label="y" value={node.params[1]} range={-1.0..=1.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={2} label="turn" value={node.params[2]} range={-3.15..=3.15} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={3} label="scale" value={node.params[3]} range={0.1..=4.0} wide={wide}/>
        </View>
    }
}

#[component]
fn MixParams(cx: &mut Cx, node: &Node, mode: MixMode, wide: bool) {
    let actions = use_actions(cx);
    let id = node.id;
    let names: Vec<&str> = MixMode::ALL.iter().map(|mode| mode.name()).collect();
    let mut index = MixMode::ALL.iter().position(|m| *m == mode).unwrap_or(0);

    rsx! {
        <View direction="column" w="100%" gap={2}>
            // The mode is baked into the shader, so picking another one is a
            // recompile — the same message class as moving a wire.
            <ComboBox
                bind={&mut index}
                options={&names}
                on_change={|picked: usize| {
                    send(&actions, Msg::SetMode { node: id, mode: MixMode::ALL[picked] });
                }}
            />
            <Knob node={id} owner={node.name.as_str()} index={0} label="amount" value={node.params[0]} range={0.0..=1.0} wide={wide}/>
        </View>
    }
}

#[component]
fn InvertParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="amount" value={node.params[0]} range={0.0..=1.0} wide={wide}/>
        </View>
    }
}

#[component]
fn PosterizeParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="levels" value={node.params[0]} range={2.0..=16.0} wide={wide}/>
        </View>
    }
}

#[component]
fn PixelateParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="cells" value={node.params[0]} range={2.0..=128.0} wide={wide}/>
        </View>
    }
}

#[component]
fn TileParams(cx: &mut Cx, node: &Node, wide: bool) {
    let id = node.id;
    rsx! {
        <View direction="column" w="100%" gap={2}>
            <Knob node={id} owner={node.name.as_str()} index={0} label="x" value={node.params[0]} range={1.0..=8.0} wide={wide}/>
            <Knob node={id} owner={node.name.as_str()} index={1} label="y" value={node.params[1]} range={1.0..=8.0} wide={wide}/>
        </View>
    }
}

#[component]
fn GrayParams(cx: &mut Cx, node: &Node, method: GrayMethod) {
    let actions = use_actions(cx);
    let id = node.id;
    let names: Vec<&str> = GrayMethod::ALL.iter().map(|method| method.name()).collect();
    let mut index = GrayMethod::ALL
        .iter()
        .position(|m| *m == method)
        .unwrap_or(0);

    rsx! {
        <ComboBox
            bind={&mut index}
            options={&names}
            on_change={|picked: usize| {
                send(&actions, Msg::SetGray { node: id, method: GrayMethod::ALL[picked] });
            }}
        />
    }
}

/// One parameter: a slider where there is room, a drag value where there is
/// not, and a `SetParam` when it moved.
///
/// The escape hatch rather than `<Slider>`, because the value being edited
/// lives in the reducer: a bound element would need a `&mut f32` that no one
/// here owns. The widget writes into a copy and the copy becomes a message.
#[component]
#[allow(clippy::too_many_arguments)]
fn Knob(
    cx: &mut Cx,
    node: NodeId,
    // The node's name, which is what tells eight "bright" knobs apart for
    // anyone who cannot see which box this one is in. The inspector draws the
    // name above its own copy, so there it is only "bright".
    owner: &str,
    index: usize,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    #[prop(default)] wide: bool,
) {
    let actions = use_actions(cx);
    let mut current = value;

    let changed = cx.leaf(&ItemStyle::default().w("100%"), |ui| {
        // A taffy leaf is measured from its first draw, which happens in a
        // zero-width `Ui`; a widget left to wrap would report one character
        // wide and stay that way (ARCHITECTURE 6).
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        if wide {
            ui.add(egui::Slider::new(&mut current, range).text(label))
                .changed()
        } else {
            let response = ui.add(
                egui::DragValue::new(&mut current)
                    .range(range)
                    .speed(0.01)
                    .prefix(format!("{label} ")),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::DragValue,
                    ui.is_enabled(),
                    format!("{owner} {label}"),
                )
            });
            response.changed()
        }
    });

    if changed {
        send(
            &actions,
            Msg::SetParam {
                node,
                index,
                value: current,
            },
        );
    }
}

/// The WGSL expression of a `Shader` node.
///
/// Hand-written rather than `<TextEdit>` because this one has to *report* the
/// text: the element's `bind` holds the only `&mut` to the string, so a
/// handler on the same element could not read it as well (ARCHITECTURE 3.7).
/// The buffer is a copy made each frame, which is what an immediate-mode text
/// field is happy with.
#[component]
fn SourceEdit(cx: &mut Cx, src: &str, rows: usize, #[event] on_change: String) {
    let mut text = src.to_owned();
    let changed = cx.leaf(&ItemStyle::default().w("100%"), |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut text)
                .desired_rows(rows)
                .desired_width(ui.available_width())
                .font(egui::TextStyle::Monospace),
        )
        .changed()
    });
    if changed {
        // Every keystroke regenerates and revalidates the program, which is
        // the point: type a bad expression and the message appears at once,
        // fix it and the picture comes back.
        on_change.emit(text);
    }
}

/// The picture, and the two things that reach it.
#[component]
fn Preview(cx: &mut Cx, program: &Option<Program>, params: &[[f32; 4]; codegen::SLOTS]) {
    let mut playing = use_state(cx, || true);

    // egui's own clock, in seconds since the app started. It stops moving when
    // nothing asks for a repaint, which is what makes `play` work.
    let time = cx.ui().input(|i| i.time) as f32;
    if *playing {
        cx.ctx().request_repaint();
    }
    let points_to_pixels = cx.ctx().pixels_per_point();
    let program = program.clone();
    let params = *params;

    rsx! {
        <View direction="column" w="100%" gap={4}>
            <Canvas
                w="100%"
                h={PREVIEW_H}
                paint={move |ui: &mut egui::Ui, rect: egui::Rect| {
                    let Some(program) = program else {
                        return;
                    };
                    let resolution = rect.size() * points_to_pixels;
                    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                        rect,
                        gpu::PatchCallback {
                            wgsl: program.wgsl.clone(),
                            source_hash: program.hash,
                            uniforms: gpu::Uniforms {
                                time,
                                pad: 0.0,
                                resolution: [resolution.x, resolution.y],
                                params,
                            },
                        },
                    ));
                }}
            />
            <View direction="row" gap={8} align="center">
                <Checkbox bind={playing.bind()} label="play"/>
                <Text>{format!("t {time:.1}")}</Text>
            </View>
        </View>
    }
}

/// The selected node's parameters, or the program, or the reason there is no
/// program.
///
/// The `wgsl` tab is a feature and a test surface at once: what it shows is
/// exactly the text that was compiled, so a test can read it and assert that a
/// slider did not change it (P-5).
#[component]
fn Inspector(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    graph: &Graph,
    selected: &Option<NodeId>,
    program: &Option<Program>,
    error: &Option<String>,
) {
    let look = look(cx.ctx());
    let mut code = use_state(cx, || false);
    let showing_code = *code;
    let node = selected.and_then(|id| graph.node(id));
    let source = program
        .as_ref()
        .map(|program| String::from(&*program.wgsl))
        .unwrap_or_default();
    let message = error.clone().unwrap_or_default();

    rsx! {
        <View style={style} direction="column" w="100%" gap={4}>
            <View direction="row" gap={4} align="center">
                <Tab label="params" active={!showing_code} on_click={|| *code = false}/>
                <Tab label="wgsl" active={showing_code} on_click={|| *code = true}/>
            </View>

            // The validation message, whichever tab is open: it is the reason
            // the picture stopped following the patch.
            if !message.is_empty() {
                <Text w="100%" wrap size={11.0} color={look.warn}>{message.as_str()}</Text>
            }

            <ScrollArea grow={1.0} h={0.0} horizontal>
                if showing_code {
                    {view(move |cx| {
                        // `leaf`, not `leaf_fill`: this is measured by its
                        // longest line, and the scroll area is what handles
                        // the overflow.
                        cx.leaf(&ItemStyle::default(), move |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                            ui.label(egui::RichText::new(source).monospace().size(10.0));
                        });
                    })}
                } else {
                    match node {
                        Some(node) => {
                            <View direction="column" w="100%" gap={6} pr={6}>
                                <Text strong size={16.0} color={look.accent}>{node.name.as_str()}</Text>
                                <Text size={11.0}>{node.kind.name()}</Text>
                                <Separator/>
                                <NodeBody node={node} wide/>
                            </View>
                        }
                        None => { <Text>"pick a node"</Text> }
                    }
                }
            </ScrollArea>
        </View>
    }
}

/// A tab, and the smallest example of the shape every component here has: take
/// a style, draw one thing, report the click. `react-egui-elements` has no
/// toggle, so this is the escape hatch one leaf deep.
#[component]
fn Tab(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    label: &str,
    active: bool,
    #[event] on_click: (),
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.selectable_label(active, label).clicked()
    });
    if clicked {
        on_click.emit(());
    }
}

/// A small flat button, for the things a node does to itself.
#[component]
fn SmallButton(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    // The name of the button, which is not the glyph on it: every node has a
    // "-" and an "x", and "x" tells a screen reader nothing.
    label: &str,
    #[event] on_click: (),
    children: impl Into<egui::WidgetText>,
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        let button = egui::Button::new(children)
            .small()
            .frame(false)
            .wrap_mode(egui::TextWrapMode::Extend);
        let response = ui.add(button);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        response.clicked()
    });
    if clicked {
        on_click.emit(());
    }
}

/// The colours the canvas paints with, taken from egui's own visuals so the
/// example follows whatever theme the gallery is in.
#[derive(Clone, Copy)]
struct Look {
    accent: egui::Color32,
    canvas: egui::Color32,
    node: egui::Color32,
    edge: egui::Color32,
    wire: egui::Color32,
    warn: egui::Color32,
}

fn look(ctx: &egui::Context) -> Look {
    let visuals = &ctx.style_of(ctx.theme()).visuals;
    Look {
        accent: if visuals.dark_mode {
            egui::Color32::from_rgb(0x7f, 0xd1, 0xb9)
        } else {
            egui::Color32::from_rgb(0x0f, 0x7a, 0x63)
        },
        canvas: visuals.extreme_bg_color,
        node: visuals.widgets.inactive.bg_fill,
        edge: visuals.widgets.noninteractive.bg_stroke.color,
        wire: visuals.widgets.active.bg_fill,
        warn: visuals.error_fg_color,
    }
}

/// The dots behind the patch, so panning is visible.
fn grid(painter: &egui::Painter, rect: egui::Rect, camera: Camera, look: Look) {
    // Drawn on the canvas, not in the zoomed layer, so the lines stay one
    // pixel wide; the spacing and the offset are what zoom.
    let step = 32.0 * camera.zoom;
    let stroke = egui::Stroke::new(1.0, look.edge.gamma_multiply(0.5));
    let start = |offset: f32| (offset * camera.zoom).rem_euclid(step);
    let mut x = rect.left() + start(camera.pan.x);
    while x < rect.right() {
        painter.vline(x, rect.y_range(), stroke);
        x += step;
    }
    let mut y = rect.top() + start(camera.pan.y);
    while y < rect.bottom() {
        painter.hline(rect.x_range(), y, stroke);
        y += step;
    }
}

/// A wire, as a cubic curve that leaves an output to the right and arrives at
/// an input from the left.
fn wire(from: egui::Pos2, to: egui::Pos2, color: egui::Color32) -> egui::Shape {
    let reach = ((to.x - from.x).abs() * 0.5).clamp(24.0, 90.0);
    egui::Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
        [
            from,
            from + egui::vec2(reach, 0.0),
            to - egui::vec2(reach, 0.0),
            to,
        ],
        false,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.5, color),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORNER: egui::Pos2 = egui::pos2(100.0, 20.0);

    /// Zooming about a point leaves that point over the same patch position.
    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        let camera = Camera {
            pan: egui::vec2(-30.0, 12.0),
            zoom: 1.0,
        };
        let about = egui::pos2(400.0, 300.0);
        let before = camera.patch_at(CORNER, about);
        let zoomed = camera.zoomed(2.0, CORNER, about);
        assert_eq!(zoomed.zoom, 2.0);
        let after = zoomed.patch_at(CORNER, about);
        assert!((after - before).length() < 1e-3, "{before:?} -> {after:?}");

        // And back again is where it started, pan included.
        let back = zoomed.zoomed(0.5, CORNER, about);
        assert_eq!(back.zoom, 1.0);
        assert!((back.pan - camera.pan).length() < 1e-3, "{back:?}");
    }

    #[test]
    fn zoom_is_clamped() {
        let camera = Camera::default();
        let about = egui::pos2(400.0, 300.0);
        assert_eq!(
            camera.zoomed(100.0, CORNER, about).zoom,
            *Camera::ZOOM.end()
        );
        assert_eq!(
            camera.zoomed(0.001, CORNER, about).zoom,
            *Camera::ZOOM.start()
        );
    }

    /// At zoom 1 the layer transform is the identity, whatever the pan: that
    /// is what lets a test read widget rectangles as screen positions.
    #[test]
    fn zoom_one_is_no_transform() {
        let camera = Camera {
            pan: egui::vec2(123.0, -45.0),
            zoom: 1.0,
        };
        assert_eq!(camera.to_global(CORNER), egui::emath::TSTransform::IDENTITY);
    }

    /// Home puts the middle of the nodes in the middle of the canvas.
    #[test]
    fn home_centres_the_nodes() {
        let node = |x: f32, y: f32| Node {
            id: 1,
            name: String::from("n"),
            kind: Kind::Level,
            pos: [x, y],
            params: [0.0; 4],
            inputs: [None, None],
        };
        let nodes = [node(0.0, 0.0), node(400.0, 200.0)];
        let size = egui::vec2(800.0, 600.0);
        let home = Camera::home(&nodes, size);
        assert_eq!(home.zoom, 1.0);
        // The nodes span 0..400+NODE_W by 0..200+NODE_H_NOMINAL.
        let centre = egui::pos2((400.0 + NODE_W) / 2.0, (200.0 + NODE_H_NOMINAL) / 2.0);
        let on_screen = (home.pan + centre.to_vec2()) * home.zoom;
        assert_eq!(on_screen, size / 2.0);

        assert_eq!(Camera::home(&[], size), Camera::default());
    }
}
