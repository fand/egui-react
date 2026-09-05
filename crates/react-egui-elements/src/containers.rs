//! egui's own containers, wrapped so their children get a fresh [`Cx`].
//!
//! In taffy mode these behave as leaves: the container occupies one taffy node
//! and everything inside it is laid out by egui, not by taffy. That is the
//! documented escape hatch for places where flex layout is not wanted.

use react_egui::layout::ItemStyle;
use react_egui::prelude::*;

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
            panic!("react-egui: {s:?} is not a valid Side (expected left, right, top or bottom)")
        })
    }
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
            children.show(&mut cx);
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
                children.show(&mut cx);
            });
    });
}

/// A painted frame: background, border and inner margin.
#[component]
pub fn Frame(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    fill: Option<egui::Color32>,
    stroke: Option<egui::Stroke>,
    inner_margin: Option<f32>,
    corner_radius: Option<f32>,
    children: impl View,
) {
    let (store, scope) = (cx.store, cx.scope_id());
    cx.leaf(&style, move |ui| {
        let mut frame = egui::Frame::default();
        if let Some(fill) = fill {
            frame = frame.fill(fill);
        }
        if let Some(stroke) = stroke {
            frame = frame.stroke(stroke);
        }
        if let Some(margin) = inner_margin {
            frame = frame.inner_margin(margin as i8);
        }
        if let Some(radius) = corner_radius {
            frame = frame.corner_radius(radius as u8);
        }
        frame.show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            children.show(&mut cx);
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
    let (store, scope) = (cx.store, cx.scope_id());
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
        children.show(&mut cx);
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
    let (store, scope) = (cx.store, cx.scope_id());
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
            children.show(&mut cx);
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
    let (store, scope) = (cx.store, cx.scope_id());
    let show = move |ui: &mut egui::Ui| {
        egui::CentralPanel::default().show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            children.show(&mut cx);
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
    cx.leaf(&style, move |ui| {
        ui.vertical(move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            children.show(&mut cx);
        });
    });
}

/// egui's own horizontal layout, as an escape hatch from taffy.
#[component]
pub fn Horizontal(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    cx.leaf(&style, move |ui| {
        ui.horizontal(move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            children.show(&mut cx);
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
    cx.leaf(&style, move |ui| {
        let mut grid = egui::Grid::new(scope).striped(striped);
        if let Some(cols) = cols {
            grid = grid.num_columns(cols);
        }
        grid.show(ui, move |ui| {
            let mut cx = Cx::new(store, ui, scope);
            children.show(&mut cx);
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
