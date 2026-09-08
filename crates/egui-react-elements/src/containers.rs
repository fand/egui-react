//! egui's own containers, wrapped so their children get a fresh [`Cx`].
//!
//! In taffy mode these behave as leaves: the container occupies one taffy node
//! and everything inside it is laid out by egui, not by taffy. That is the
//! documented escape hatch for places where flex layout is not wanted.
//!
//! Their look comes from the same `style` prop as everything else: the engine
//! paints the node's box (`bg` `border` `radius` `shadow` `opacity`) around the
//! egui container inside it. [`Frame`] is the exception in `Ui` mode, where
//! there is no node to paint and it builds an `egui::Frame` instead.
//!
//! Each of them re-enters with a fresh [`Cx`] around the `Ui` egui handed back.
//! They carry `cx.layout_id()` over that gap: inside a `<VirtualList>` row it is
//! the slot's id, and a `<View>` under one of these containers has to key its
//! taffy tree by the slot like any other, not by the row index in `scope`.

use egui_react::layout::{ItemStyle, Length, root_style};
use egui_react::prelude::*;

/// Which edge a [`Panel`] is docked to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Side {
    /// Docked to the left edge.
    #[default]
    Left,
    /// Docked to the right edge.
    Right,
    /// Docked to the top edge.
    Top,
    /// Docked to the bottom edge.
    Bottom,
}

impl Side {
    /// Parse the CSS-ish spelling, returning `None` if unknown.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

impl From<&str> for Side {
    /// # Panics
    /// Panics on an unknown spelling.
    fn from(s: &str) -> Self {
        Self::parse(s).unwrap_or_else(|| {
            panic!("egui-react: {s:?} is not a valid Side (expected left, right, top or bottom)")
        })
    }
}

/// Where an [`Overlay`] is pinned to the window.
///
/// Both spellings are accepted: the CSS-ish one (`"bottom-right"`, `"top"`,
/// `"center"`) and egui's `Align2` order (`"right-bottom"`, `"center-top"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Anchor {
    /// The top-left corner.
    #[default]
    TopLeft,
    /// The middle of the top edge.
    Top,
    /// The top-right corner.
    TopRight,
    /// The middle of the left edge.
    Left,
    /// The middle of the window.
    Center,
    /// The middle of the right edge.
    Right,
    /// The bottom-left corner.
    BottomLeft,
    /// The middle of the bottom edge.
    Bottom,
    /// The bottom-right corner.
    BottomRight,
}

impl Anchor {
    /// Parse either spelling, returning `None` if unknown.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "top-left" | "left-top" => Some(Self::TopLeft),
            "top" | "center-top" => Some(Self::Top),
            "top-right" | "right-top" => Some(Self::TopRight),
            "left" | "left-center" => Some(Self::Left),
            "center" | "center-center" => Some(Self::Center),
            "right" | "right-center" => Some(Self::Right),
            "bottom-left" | "left-bottom" => Some(Self::BottomLeft),
            "bottom" | "center-bottom" => Some(Self::Bottom),
            "bottom-right" | "right-bottom" => Some(Self::BottomRight),
            _ => None,
        }
    }

    /// The egui alignment this anchor pins to.
    pub fn to_align2(self) -> egui::Align2 {
        match self {
            Self::TopLeft => egui::Align2::LEFT_TOP,
            Self::Top => egui::Align2::CENTER_TOP,
            Self::TopRight => egui::Align2::RIGHT_TOP,
            Self::Left => egui::Align2::LEFT_CENTER,
            Self::Center => egui::Align2::CENTER_CENTER,
            Self::Right => egui::Align2::RIGHT_CENTER,
            Self::BottomLeft => egui::Align2::LEFT_BOTTOM,
            Self::Bottom => egui::Align2::CENTER_BOTTOM,
            Self::BottomRight => egui::Align2::RIGHT_BOTTOM,
        }
    }
}

impl From<&str> for Anchor {
    /// # Panics
    /// Panics on an unknown spelling.
    fn from(s: &str) -> Self {
        Self::parse(s).unwrap_or_else(|| {
            panic!(
                "egui-react: {s:?} is not a valid Anchor (expected top-left, top, top-right, \
                 left, center, right, bottom-left, bottom or bottom-right)"
            )
        })
    }
}

/// Which layer an [`Overlay`] is drawn in: [`egui::Order`], with `From<&str>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Order {
    /// Behind everything else.
    Background,
    /// Above the background, below the windows.
    Middle,
    /// Above the windows. The default.
    #[default]
    Foreground,
    /// Above everything, where tooltips live.
    Tooltip,
}

impl Order {
    /// Parse the CSS-ish spelling, returning `None` if unknown.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "background" => Some(Self::Background),
            "middle" => Some(Self::Middle),
            "foreground" => Some(Self::Foreground),
            "tooltip" => Some(Self::Tooltip),
            _ => None,
        }
    }

    /// The egui order this one names.
    pub fn to_egui(self) -> egui::Order {
        match self {
            Self::Background => egui::Order::Background,
            Self::Middle => egui::Order::Middle,
            Self::Foreground => egui::Order::Foreground,
            Self::Tooltip => egui::Order::Tooltip,
        }
    }
}

impl From<&str> for Order {
    /// # Panics
    /// Panics on an unknown spelling.
    fn from(s: &str) -> Self {
        Self::parse(s).unwrap_or_else(|| {
            panic!(
                "egui-react: {s:?} is not a valid Order (expected background, middle, foreground \
                 or tooltip)"
            )
        })
    }
}

/// A floating layer over the app: a sheet, a menu, a corner button.
///
/// Like a `<Window>` it is drawn in a layer of its own, so it takes no space
/// in the surrounding layout and can sit over anything. It has two modes.
///
/// **Sized** (`w` and/or `h` given): the size is known before anything is
/// drawn, so the overlay paints a sheet, takes the presses that land on it,
/// and roots a taffy tree of its own — a `<View>` inside is laid out against
/// the sheet, as the app root is against the window. A press on the sheet goes
/// nowhere: it never reaches the widgets underneath. The sheet is filled with
/// `fill`, or with the theme's `panel_fill`; `fill={egui::Color32::TRANSPARENT}`
/// opts out. A `Percent` length is a fraction of the window
/// (`egui::Context::content_rect`), and an axis that is not given is the
/// window's whole length on that axis.
///
/// **Unsized** (neither given): the overlay is as big as its children, paints
/// nothing unless `fill` is given, and lets every press it does not want
/// through. Put a `<View w=..>` inside to lay the children out.
///
/// `pos` places the overlay by hand and wins over `anchor`; with neither, egui
/// opens it in the top-left corner. `offset` moves an anchored overlay off its
/// corner (negative to come back in from the right or the bottom). `constrain`
/// (on by default) keeps it inside the window. `top` lifts it above the other
/// overlays every frame, which is what beats egui's own rule that the area
/// shown later is on top. Not drawing an `<Overlay>` unmounts its children,
/// as `open={false}` does for a `<Window>`.
#[component]
#[allow(clippy::too_many_arguments)]
pub fn Overlay(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(into)] anchor: Option<Anchor>,
    #[prop(default)] offset: (f32, f32),
    pos: Option<egui::Pos2>,
    #[prop(default, into)] order: Order,
    #[prop(default = true)] constrain: bool,
    #[prop(default)] top: bool,
    fill: Option<egui::Color32>,
    children: impl View,
) {
    // An `Area` is a layer of its own, so an invisible leaf `Ui` never reaches
    // it: a hidden one is not drawn at all, and its children unmount as when
    // `open` is false.
    if cx.is_hidden() {
        return;
    }
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    let ctx = cx.ctx().clone();
    let screen = ctx.content_rect();

    // Sized: the sheet is known before anything is drawn, so it can take the
    // presses and root a tree. `Px` as is, `Percent` of the window, an axis
    // that is not given is the window's.
    let sized = style.w.is_some() || style.h.is_some();
    let resolve = |len: Option<Length>, full: f32| match len {
        Some(Length::Px(v)) => v,
        Some(Length::Percent(p)) => p * full,
        Some(Length::Auto) | None => full,
    };
    let sheet_size = egui::vec2(
        resolve(style.w, screen.width()),
        resolve(style.h, screen.height()),
    );

    let mut area = egui::Area::new(scope)
        .order(order.to_egui())
        .constrain(constrain)
        // `Area::new` is movable by default; `anchor` / `fixed_pos` clear it,
        // a bare overlay has to.
        .movable(false);
    match (pos, anchor) {
        (Some(pos), _) => area = area.fixed_pos(pos),
        (None, Some(anchor)) => {
            area = area.anchor(anchor.to_align2(), egui::vec2(offset.0, offset.1));
        }
        (None, None) => {}
    }
    if sized {
        // Right on the first frame, before the sheet has been allocated once.
        area = area.default_size(sheet_size);
    }
    if top {
        // Every frame: egui keeps the areas of an `Order` in first-shown
        // order and a click brings one forward.
        ctx.move_to_top(area.layer());
    }
    area.show(&ctx, move |ui| {
        if sized {
            let sheet = egui::Rect::from_min_size(ui.max_rect().min, sheet_size);
            let fill = fill.unwrap_or_else(|| ui.visuals().panel_fill);
            ui.painter().rect_filled(sheet, 0.0, fill);
            // The whole sheet, before the children: it makes the area the
            // sheet's size (that is what stops a press reaching the layer
            // beneath) and a later widget in the same layer is on top of it,
            // so the children's buttons still get theirs.
            ui.allocate_rect(sheet, egui::Sense::click());
            let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(sheet));
            let mut cx = Cx::new(store, &mut ui, scope);
            cx.with_layout_id(layout, |cx| {
                cx.root_container(scope.with("overlay"), root_style(), |cx| children.show(cx));
            });
        } else {
            // The background is painted after the children, into a slot
            // claimed before them, because the area's rect is what they drew.
            let background = fill.map(|_| ui.painter().add(egui::Shape::Noop));
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
            if let (Some(idx), Some(fill)) = (background, fill) {
                ui.painter()
                    .set(idx, egui::Shape::rect_filled(ui.min_rect(), 0.0, fill));
            }
        }
    });
}

/// A scrollable region.
#[component]
pub fn ScrollArea(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default)] horizontal: bool,
    #[prop(default = true)] vertical: bool,
    max_h: Option<f32>,
    children: impl View,
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    // `leaf_fill`, not `leaf`: a scroll area fills whatever it is given and
    // reports that back as its content size, so a content-measured leaf would
    // pin it at its first-frame size forever. Inside a `<View>` it is sized by
    // taffy alone: `w` / `h`, `grow`, or the remaining space.
    cx.leaf_fill(&style, move |ui| {
        let mut area = egui::ScrollArea::new([horizontal, vertical]).id_salt(scope);
        if let Some(max_h) = max_h {
            area = area.max_height(max_h);
        }
        area.show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    });
}

/// A collapsing section with a clickable header.
#[component]
pub fn Collapsing(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    header: &str,
    #[prop(default)] default_open: bool,
    children: impl View,
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    cx.leaf(&style, move |ui| {
        // The header has no `wrap_mode` builder, so the mode goes on the leaf's
        // `Ui` (see the `widgets` module docs). The body puts it back: what the
        // children draw is the caller's business.
        let outer = ui.style().wrap_mode;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        egui::CollapsingHeader::new(header)
            .id_salt(scope)
            .default_open(default_open)
            .show(ui, move |ui| {
                ui.style_mut().wrap_mode = outer;
                let mut cx = Cx::new(store, ui, scope);
                cx.with_layout_id(layout, |cx| children.show(cx));
            });
    });
}

/// `egui::Frame` itself, as an escape hatch from taffy.
///
/// **Inside a tree, use `<View>` with paint.** Every element takes `bg`
/// `border` `radius` `shadow` `custom_shadow` `opacity` through `style`, and
/// the engine paints them on the node's own box, so a painted box costs no
/// element and no `Ui` of its own. This one is for egui-native children: over a
/// plain `Ui` it builds an `egui::Frame` from the same paint props and shows
/// them inside it, which is the one thing a `<View>` cannot do there (a plain
/// `Ui` has no rect to paint until its children have drawn).
///
/// The inner margin is `p` in points; a percentage padding needs a layout to
/// resolve against and is ignored here. Inside a tree this is a `<View>`
/// spelled with an extra `Ui`: the engine paints the box and the children are
/// laid out by egui, vertically, as in a `<Vertical>`.
#[component]
pub fn Frame(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();

    if cx.in_taffy() {
        // A leaf whose look the engine paints, from the node's rect.
        cx.leaf(&style, move |ui| {
            ui.vertical(move |ui| {
                let mut cx = Cx::new(store, ui, scope);
                cx.with_layout_id(layout, |cx| children.show(cx));
            });
        });
        return;
    }

    let paint = style.paint;
    let padding = style.padding_px();
    cx.leaf(&style, move |ui| {
        let mut frame = egui::Frame::new();
        if let Some(fill) = paint.bg {
            frame = frame.fill(fill);
        }
        if let Some(stroke) = paint.border {
            frame = frame.stroke(stroke);
        }
        if let Some([top, right, bottom, left]) = padding {
            frame = frame.inner_margin(egui::Margin {
                left: left as i8,
                right: right as i8,
                top: top as i8,
                bottom: bottom as i8,
            });
        }
        if let Some(radius) = paint.radius {
            frame = frame.corner_radius(radius as u8);
        }
        if let Some(shadow) = paint.custom_shadow {
            frame = frame.shadow(shadow);
        } else if paint.shadow {
            frame = frame.shadow(ui.visuals().window_shadow);
        }
        frame.show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    });
}

/// A floating window.
///
/// A window is anchored to the [`egui::Context`], not to the surrounding
/// layout, so it takes no space in its parent even inside a `<View>`. Setting
/// `open` to `false` stops drawing the children, which unmounts their hooks.
///
/// `default_pos` and `default_size` apply on the first frame only; after that
/// the window keeps wherever the user dragged it. Without a `default_pos` egui
/// opens it in the top-left corner, over whatever is there.
#[component]
#[allow(clippy::too_many_arguments)]
pub fn Window(
    cx: &mut Cx,
    title: &str,
    open: Option<&mut bool>,
    #[prop(default = true)] resizable: bool,
    default_pos: Option<egui::Pos2>,
    default_size: Option<egui::Vec2>,
    children: impl View,
) {
    // An `Area` is a layer of its own, so an invisible leaf `Ui` never reaches
    // it: a hidden one is not drawn at all, and its children unmount as when
    // `open` is false.
    if cx.is_hidden() {
        return;
    }
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    let ctx = cx.ctx().clone();
    let mut window = egui::Window::new(title).id(scope).resizable(resizable);
    if let Some(open) = open {
        window = window.open(open);
    }
    // Only the first frame: after that the window remembers where the user put
    // it, which is the whole point of a floating window.
    if let Some(pos) = default_pos {
        window = window.default_pos(pos);
    }
    if let Some(size) = default_size {
        window = window.default_size(size);
    }
    window.show(&ctx, move |ui| {
        let mut cx = Cx::new(store, ui, scope);
        cx.with_layout_id(layout, |cx| children.show(cx));
    });
}

/// A panel docked to one edge.
///
/// **Which edge**: a panel docks in the nearest enclosing egui `Ui` — the one
/// the current taffy tree was started in. Any `<View>`s between that `Ui` and
/// the panel are skipped. Under the runner that `Ui` is the window, which is
/// what "panels belong at the app root" has always meant, and it is why a
/// `<Panel>` written inside a `<View>` jumps out to the tree's edge rather than
/// taking a slice of the row it was written in. That is the consequence of
/// docking, not a bug: carving a taffy node would put the panel inside its own
/// little box, where four sibling panels would all draw at the same corner.
///
/// `shares_ui` for the same reason: a docked panel carves space out of the `Ui`
/// its siblings are drawn into, so it must not be given a child `Ui` of its
/// own. Without it, `<Panel/>` followed by `<CentralPanel/>` would stack
/// instead of docking.
#[component(shares_ui)]
pub fn Panel(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default, into)] side: Side,
    default_size: Option<f32>,
    #[prop(default = true)] resizable: bool,
    children: impl View,
) {
    // A docked panel draws into the tree's root `Ui`, not into the invisible
    // leaf `Ui` of a hidden view, so a hidden one is not drawn at all; its
    // children unmount as when a `<Window>` is closed.
    if cx.is_hidden() {
        return;
    }
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    let show = move |ui: &mut egui::Ui| {
        let mut panel = match side {
            Side::Left => egui::Panel::left(scope),
            Side::Right => egui::Panel::right(scope),
            Side::Top => egui::Panel::top(scope),
            Side::Bottom => egui::Panel::bottom(scope),
        }
        .resizable(resizable);
        if let Some(size) = default_size {
            panel = panel.default_size(size);
        }
        panel.show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    };

    if cx.in_taffy() {
        // The tree's own `Ui`, not a node of it. `style` is ignored here: a
        // docked panel's size is `default_size` and the drag handle, not a
        // taffy attribute.
        show(cx.ui());
    } else {
        cx.leaf(&style, show);
    }
}

/// The panel that takes whatever space the docked panels left over.
///
/// Docks in the same `Ui` as [`Panel`], for the same reasons.
#[component(shares_ui)]
pub fn CentralPanel(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    // A docked panel draws into the tree's root `Ui`, not into the invisible
    // leaf `Ui` of a hidden view, so a hidden one is not drawn at all; its
    // children unmount as when a `<Window>` is closed.
    if cx.is_hidden() {
        return;
    }
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    let show = move |ui: &mut egui::Ui| {
        egui::CentralPanel::default().show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    };

    if cx.in_taffy() {
        show(cx.ui());
    } else {
        cx.leaf(&style, show);
    }
}

/// egui's own vertical layout, as an escape hatch from taffy.
#[component]
pub fn Vertical(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    cx.leaf(&style, move |ui| {
        ui.vertical(move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    });
}

/// egui's own horizontal layout, as an escape hatch from taffy.
#[component]
pub fn Horizontal(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    cx.leaf(&style, move |ui| {
        ui.horizontal(move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    });
}

/// egui's own grid, whose rows are separated by [`Row`].
#[component]
pub fn Grid(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    cols: Option<usize>,
    #[prop(default)] striped: bool,
    children: impl View,
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    cx.leaf(&style, move |ui| {
        let mut grid = egui::Grid::new(scope).striped(striped);
        if let Some(cols) = cols {
            grid = grid.num_columns(cols);
        }
        grid.show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        });
    });
}

/// One row of a [`Grid`]: draws its children, then ends the row.
///
/// `shares_ui`, because `Ui::end_row` only reaches the grid it was called on;
/// an ordinary element would call it on a child `Ui` of its own.
#[component(shares_ui)]
pub fn Row(cx: &mut Cx, children: impl View) {
    children.show(cx);
    cx.ui().end_row();
}
