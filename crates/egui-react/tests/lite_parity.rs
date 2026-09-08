//! Plan E, section 4: the lite path lays a row out exactly where taffy does.
//!
//! A `<VirtualList>` row is laid out by `engine::lite` instead of by a taffy
//! tree (see `crates/egui-react/src/engine/lite.rs`). Two layout
//! implementations are only worth having if they agree, so every tree in the
//! corpus below is drawn twice — once through each path, with
//! `Store::force_taffy_rows` picking — and every node's rect is compared for
//! exact equality.
//!
//! Rects are read where they matter: a leaf records the `max_rect` of the `Ui`
//! it was given, which is the content rect of its node, and a `<Text>` records
//! the rect of the response it returns, which is the widget rect `egui::Label`
//! would have allocated. A container has no rect of its own on screen, so it is
//! covered by where its children land.
//!
//! Three frames per tree, because a widget leaf has to draw once before it can
//! say how big it is.

use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui_react::layout::{ContainerStyle, ItemStyle, Length};
use egui_react::prelude::*;

/// The rect the row is laid out into: what `<VirtualList>` hands a row.
const ROW_W: f32 = 300.0;
const ROW_H: f32 = 24.0;

/// Every rect one drawing of a tree produced, in draw order.
type Rects = Rc<RefCell<Vec<(&'static str, egui::Rect)>>>;

/// What a corpus case draws. The `&Rects` is where it records what it got.
type Case = fn(&mut Cx<'_, '_>, &Rects);

/// A leaf that draws a fixed-size widget and records the rect it was given.
///
/// Wrapped in a scope of its own, as every element is: that is what separates
/// two leaves that sit at the same child index under different containers.
fn leaf(
    cx: &mut Cx<'_, '_>,
    rects: &Rects,
    name: &'static str,
    style: &ItemStyle,
    size: egui::Vec2,
) {
    let rects = Rc::clone(rects);
    cx.scope(name, move |cx| {
        cx.leaf(style, move |ui| {
            rects.borrow_mut().push((name, ui.max_rect()));
            ui.allocate_space(size);
        });
    });
}

/// A `<Text>` that records the widget rect it registered.
fn text(cx: &mut Cx<'_, '_>, rects: &Rects, name: &'static str, style: &ItemStyle, body: &str) {
    cx.scope(name, |cx| {
        let response = cx.text(style, body.into(), false, Some(false));
        rects.borrow_mut().push((name, response.rect));
    });
}

/// A `<View>`, with the scope and the layout id an element gets.
fn view<R>(
    cx: &mut Cx<'_, '_>,
    name: &'static str,
    container: &ContainerStyle,
    item: &ItemStyle,
    f: impl FnOnce(&mut Cx<'_, '_>) -> R,
) -> R {
    cx.scope(name, |cx| {
        let id = cx.layout_id();
        cx.container(id, container, item, f)
    })
}

fn row() -> ContainerStyle {
    ContainerStyle::default().direction("row")
}

fn column() -> ContainerStyle {
    ContainerStyle::default().direction("column")
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

/// The shape `examples/list-10k` draws: a fixed index column, a growing name,
/// and a button.
fn list_10k_row(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(8.0).align("center");
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h(18.0),
        |cx| {
            text(cx, rects, "index", &ItemStyle::default().w(64.0), "#42");
            text(
                cx,
                rects,
                "name",
                &ItemStyle::default().grow(1.0),
                "row 42 delta",
            );
            leaf(
                cx,
                rects,
                "button",
                &ItemStyle::default(),
                egui::vec2(40.0, 16.0),
            );
        },
    );
}

/// Margins and paddings on both the container and its children.
fn margins_and_padding(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%").p(6.0),
        |cx| {
            leaf(
                cx,
                rects,
                "a",
                &ItemStyle::default().m(3.0).p(2.0),
                egui::vec2(30.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "b",
                &ItemStyle::default().ml(8.0).mt(2.0).pr(5.0).grow(1.0),
                egui::vec2(20.0, 12.0),
            );
        },
    );
}

/// Percentage widths, and a percentage padding, which resolves against the
/// container's inline size on both axes.
fn percentages(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row();
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%").px("5%"),
        |cx| {
            leaf(
                cx,
                rects,
                "half",
                &ItemStyle::default().w("50%"),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "quarter",
                &ItemStyle::default().w("25%").p("2%"),
                egui::vec2(10.0, 10.0),
            );
        },
    );
}

/// Nested containers, three levels deep, with a column inside a row.
fn nested(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let outer = row().gap(6.0).align("center");
    view(
        cx,
        "root",
        &outer,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "left",
                &ItemStyle::default(),
                egui::vec2(24.0, 8.0),
            );
            let inner = column().gap(2.0);
            view(cx, "mid", &inner, &ItemStyle::default().grow(1.0), |cx| {
                text(cx, rects, "top", &ItemStyle::default(), "top line");
                let deepest = row().gap(3.0);
                view(cx, "deep", &deepest, &ItemStyle::default(), |cx| {
                    text(cx, rects, "deep-a", &ItemStyle::default(), "a");
                    text(cx, rects, "deep-b", &ItemStyle::default().grow(1.0), "b");
                });
            });
        },
    );
}

/// Every `justify` value that spreads items out, with room to spread them in.
fn justify_space_between(cx: &mut Cx<'_, '_>, rects: &Rects) {
    justify_case(cx, rects, "space-between")
}

fn justify_space_around(cx: &mut Cx<'_, '_>, rects: &Rects) {
    justify_case(cx, rects, "space-evenly")
}

fn justify_center_end(cx: &mut Cx<'_, '_>, rects: &Rects) {
    justify_case(cx, rects, "end")
}

fn justify_case(cx: &mut Cx<'_, '_>, rects: &Rects, justify: &str) {
    let style = row().justify(justify).gap(5.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "a",
                &ItemStyle::default(),
                egui::vec2(30.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "b",
                &ItemStyle::default(),
                egui::vec2(40.0, 12.0),
            );
            leaf(
                cx,
                rects,
                "c",
                &ItemStyle::default(),
                egui::vec2(20.0, 14.0),
            );
        },
    );
}

/// `align` and `align_self` on the cross axis, including the stretch default.
fn align_variants(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().align("center").gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "center",
                &ItemStyle::default(),
                egui::vec2(20.0, 8.0),
            );
            leaf(
                cx,
                rects,
                "start",
                &ItemStyle::default().align_self("start"),
                egui::vec2(20.0, 8.0),
            );
            leaf(
                cx,
                rects,
                "end",
                &ItemStyle::default().align_self("end"),
                egui::vec2(20.0, 8.0),
            );
            leaf(
                cx,
                rects,
                "stretch",
                &ItemStyle::default().align_self("stretch"),
                egui::vec2(20.0, 8.0),
            );
        },
    );
}

/// Items that do not fit, so they shrink, and one that refuses to.
fn shrinking(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w(120.0).h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "wide",
                &ItemStyle::default().w(100.0),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "wider",
                &ItemStyle::default().w(100.0).shrink(2.0),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "rigid",
                &ItemStyle::default().w(40.0).shrink(0.0),
                egui::vec2(10.0, 10.0),
            );
        },
    );
}

/// A hidden child holding a leaf and a `<Text>`: both paths draw the leaf into
/// an invisible `Ui` at the row's corner and register the text at
/// `Rect::NOTHING`.
fn hidden_in_row(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "before",
                &ItemStyle::default(),
                egui::vec2(20.0, 10.0),
            );
            let gone = ContainerStyle::default().display("none");
            view(cx, "gone", &gone, &ItemStyle::default().w(50.0), |cx| {
                leaf(
                    cx,
                    rects,
                    "hidden leaf",
                    &ItemStyle::default(),
                    egui::vec2(10.0, 10.0),
                );
                text(cx, rects, "hidden text", &ItemStyle::default(), "gone");
            });
            leaf(
                cx,
                rects,
                "after",
                &ItemStyle::default(),
                egui::vec2(20.0, 10.0),
            );
        },
    );
}

/// A `display: none` child, which takes no space and gets no box.
fn display_none(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "before",
                &ItemStyle::default(),
                egui::vec2(20.0, 10.0),
            );
            let gone = ContainerStyle::default().display("none");
            view(cx, "gone", &gone, &ItemStyle::default().w(50.0), |cx| {
                leaf(
                    cx,
                    rects,
                    "inside",
                    &ItemStyle::default(),
                    egui::vec2(10.0, 10.0),
                );
            });
            leaf(
                cx,
                rects,
                "after",
                &ItemStyle::default(),
                egui::vec2(20.0, 10.0),
            );
        },
    );
}

/// Minimum and maximum sizes clamping what grow and shrink would have done.
fn min_max_clamping(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "capped",
                &ItemStyle::default().grow(1.0).max_w(60.0),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "floored",
                &ItemStyle::default().grow(1.0).min_w(120.0).max_h(6.0),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "plain",
                &ItemStyle::default().grow(1.0),
                egui::vec2(10.0, 10.0),
            );
        },
    );
}

/// A column: the main axis is vertical, and the row's height is what bounds it.
fn column_direction(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = column().gap(2.0).align("start");
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            text(cx, rects, "one", &ItemStyle::default(), "first");
            text(cx, rects, "two", &ItemStyle::default().grow(1.0), "second");
            leaf(
                cx,
                rects,
                "three",
                &ItemStyle::default(),
                egui::vec2(30.0, 6.0),
            );
        },
    );
}

/// Reversed directions, which walk the line the other way from the same edge.
fn row_reverse(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().direction("row-reverse").gap(5.0).justify("start");
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "a",
                &ItemStyle::default(),
                egui::vec2(20.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "b",
                &ItemStyle::default(),
                egui::vec2(30.0, 10.0),
            );
            text(cx, rects, "c", &ItemStyle::default(), "tail");
        },
    );
}

/// `basis`, in points and as a percentage, against a definite main size.
fn flex_basis(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            leaf(
                cx,
                rects,
                "points",
                &ItemStyle::default().basis(50.0).grow(1.0),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "percent",
                &ItemStyle::default().basis(Length::Percent(0.25)).grow(2.0),
                egui::vec2(10.0, 10.0),
            );
            leaf(
                cx,
                rects,
                "auto",
                &ItemStyle::default(),
                egui::vec2(25.0, 10.0),
            );
        },
    );
}

/// A container with no size of its own inside a row, so its main size comes
/// from what its children contribute.
fn intrinsic_container(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let outer = row().gap(4.0).justify("end");
    view(
        cx,
        "root",
        &outer,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            let inner = row().gap(3.0);
            view(cx, "inner", &inner, &ItemStyle::default().p(2.0), |cx| {
                text(cx, rects, "a", &ItemStyle::default(), "abc");
                leaf(cx, rects, "b", &ItemStyle::default(), egui::vec2(18.0, 9.0));
            });
            leaf(
                cx,
                rects,
                "tail",
                &ItemStyle::default(),
                egui::vec2(22.0, 11.0),
            );
        },
    );
}

/// A row whose text is wider than the space it has, so the automatic minimum
/// size of a flex item decides where the next item starts.
fn overflowing_text(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w(150.0).h("100%"),
        |cx| {
            text(
                cx,
                rects,
                "long",
                &ItemStyle::default().grow(1.0),
                "a very long label that does not fit into the row at all",
            );
            leaf(
                cx,
                rects,
                "after",
                &ItemStyle::default(),
                egui::vec2(30.0, 10.0),
            );
        },
    );
}

/// An empty container: no children, so it is sized like a leaf.
fn empty_container(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            let empty = column();
            view(cx, "empty", &empty, &ItemStyle::default().w(20.0), |_| {});
            leaf(
                cx,
                rects,
                "after",
                &ItemStyle::default(),
                egui::vec2(20.0, 10.0),
            );
        },
    );
}

/// A percentage `basis` inside a container whose main size is not definite: it
/// cannot be resolved, so the item is sized by its content instead. Plan E
/// lists this as a fallback case; it is in the subset because it falls out of
/// the same code as everything else, and this is the case that says so.
fn percent_basis_without_a_main_size(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let outer = column().gap(3.0);
    view(
        cx,
        "root",
        &outer,
        &ItemStyle::default().w("100%").h("100%"),
        |cx| {
            let inner = column().gap(2.0);
            view(cx, "inner", &inner, &ItemStyle::default(), |cx| {
                leaf(
                    cx,
                    rects,
                    "percent",
                    &ItemStyle::default().basis(Length::Percent(0.5)),
                    egui::vec2(20.0, 7.0),
                );
                text(cx, rects, "after", &ItemStyle::default(), "tail");
            });
            leaf(
                cx,
                rects,
                "below",
                &ItemStyle::default(),
                egui::vec2(20.0, 5.0),
            );
        },
    );
}

const CORPUS: &[(&str, Case)] = &[
    ("list_10k_row", list_10k_row),
    ("margins_and_padding", margins_and_padding),
    ("percentages", percentages),
    ("nested", nested),
    ("justify_space_between", justify_space_between),
    ("justify_space_evenly", justify_space_around),
    ("justify_end", justify_center_end),
    ("align_variants", align_variants),
    ("shrinking", shrinking),
    ("display_none", display_none),
    ("hidden_in_row", hidden_in_row),
    ("min_max_clamping", min_max_clamping),
    ("column_direction", column_direction),
    ("row_reverse", row_reverse),
    ("flex_basis", flex_basis),
    ("intrinsic_container", intrinsic_container),
    ("overflowing_text", overflowing_text),
    ("empty_container", empty_container),
    (
        "percent_basis_without_a_main_size",
        percent_basis_without_a_main_size,
    ),
];

// ---------------------------------------------------------------------------
// Running one case
// ---------------------------------------------------------------------------

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.options_mut(|o| o.max_passes = NonZeroUsize::new(4).unwrap());
    ctx
}

/// Draw `case` for three frames and return the rects of the last one, relative
/// to the corner of the rect the row was given.
///
/// `taffy` picks the path. With it off the case must actually take the lite
/// path, or the comparison would be a tree against itself; that is asserted
/// below.
fn draw(case: Case, taffy: bool) -> Vec<(&'static str, egui::Rect)> {
    draw_with(case, taffy, !taffy)
}

fn draw_with(case: Case, taffy: bool, expect_lite: bool) -> Vec<(&'static str, egui::Rect)> {
    let ctx = context();
    let mut store = Store::new();
    store.force_taffy_rows(taffy);
    let mut last = Vec::new();

    for _ in 0..3 {
        let rects: Rects = Rc::new(RefCell::new(Vec::new()));
        let origin = Rc::new(RefCell::new(egui::Pos2::ZERO));
        {
            let rects = Rc::clone(&rects);
            let origin = Rc::clone(&origin);
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                rects.borrow_mut().clear();
                let mut root = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_size(
                            ui.max_rect().min,
                            egui::vec2(ROW_W, ROW_H),
                        ))
                        .layout(*ui.layout()),
                );
                *origin.borrow_mut() = root.max_rect().min;
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = &store;
                    let mut cx = Cx::new(store, &mut root, egui::Id::new("row"));
                    // What `<VirtualList>` does around a row: a fixed rect, and
                    // a layout id of the slot.
                    cx.with_layout_id(egui::Id::new("slot"), |cx| {
                        cx.with_root_size(egui::vec2(ROW_W, ROW_H), |cx| case(cx, &rects));
                    });
                }
                store.end_pass();
            });
            output.drop_without_applying_deltas();
        }
        if expect_lite {
            assert_eq!(
                store.lite_row_count(),
                1,
                "the case did not take the lite path",
            );
        }
        let origin = *origin.borrow();
        last = rects
            .borrow()
            .iter()
            .map(|(name, rect)| (*name, rect.translate(-origin.to_vec2())))
            .collect();
    }
    last
}

/// Do the two paths agree on one node's rect?
///
/// Plain equality, except that a hidden `<Text>` registers `Rect::NOTHING`,
/// and translating that by the row's origin gives NaN, which is not equal to
/// itself. Two rects that are both nothing are the same answer.
fn same_rect(a: &egui::Rect, b: &egui::Rect) -> bool {
    a == b || (a.any_nan() && b.any_nan())
}

#[test]
fn the_lite_path_and_taffy_agree_on_every_corpus_row() {
    let mut failures = Vec::new();
    for (name, case) in CORPUS {
        let lite = draw(*case, false);
        let taffy = draw(*case, true);
        if lite.len() != taffy.len() {
            failures.push(format!(
                "{name}: lite drew {} rects, taffy drew {}",
                lite.len(),
                taffy.len()
            ));
            continue;
        }
        for ((node, lite), (_, taffy)) in lite.iter().zip(taffy.iter()) {
            if !same_rect(lite, taffy) {
                failures.push(format!("{name}/{node}: lite {lite:?} != taffy {taffy:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Counts the fallback lines `engine::lite` logs, so the test can check that a
/// row says why it fell back, and says it once rather than once a frame.
struct FallbackLog;

static FALLBACKS: AtomicUsize = AtomicUsize::new(0);

impl log::Log for FallbackLog {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.args().to_string().contains("falls back") {
            FALLBACKS.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn flush(&self) {}
}

/// A row that the solver cannot lay out takes the taffy path from the pass it
/// is noticed in, and says so once.
#[test]
fn a_wrapping_row_falls_back_and_still_lays_out() {
    let _ = log::set_boxed_logger(Box::new(FallbackLog));
    log::set_max_level(log::LevelFilter::Debug);

    let before = FALLBACKS.load(Ordering::SeqCst);
    let rects = draw_with(wrapping_row, false, false);
    assert_eq!(
        FALLBACKS.load(Ordering::SeqCst) - before,
        1,
        "three frames of a row outside the subset should log the reason once",
    );

    let taffy = draw(wrapping_row, true);
    assert_eq!(
        rects, taffy,
        "a row that fell back should lay out exactly as it does on the taffy path",
    );
    assert!(
        rects.iter().any(|(_, rect)| rect.width() > 0.0),
        "the fallen-back row still has to lay out: {rects:?}",
    );
}

/// `wrap` is outside the subset, so this row goes to taffy.
fn wrapping_row(cx: &mut Cx<'_, '_>, rects: &Rects) {
    let style = row().wrap(true).gap(4.0);
    view(
        cx,
        "root",
        &style,
        &ItemStyle::default().w(60.0).h("100%"),
        |cx| {
            leaf(cx, rects, "a", &ItemStyle::default(), egui::vec2(40.0, 8.0));
            leaf(cx, rects, "b", &ItemStyle::default(), egui::vec2(40.0, 8.0));
        },
    );
}
