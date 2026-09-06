//! Plan a11y A-3: no example grows a new widget that assistive technology
//! cannot name.
//!
//! A widget with no name is read out as its role and nothing else ("button",
//! "check box"), which is the one thing a screen reader cannot recover from.
//! Every example is drawn once and every focusable node in its accessibility
//! tree is checked for a name. The ones that have none today are listed in
//! [`KNOWN_UNNAMED`] with the reason; the list is compared exactly, so it can
//! only shrink — a new unnamed widget turns this red, and so does fixing one
//! without crossing it off.

use egui_kittest::Harness;
use egui_kittest::kittest::NodeT as _;
use gallery::{App, EXAMPLES};
use egui_react::prelude::*;
use egui_react_app::{root_id, root_style};

/// Examples this test does not run.
///
/// `fetch` starts a real HTTP request the moment it is drawn (`use_future` +
/// `ehttp`), which has no business in a unit test. The same reason keeps it out
/// of `gallery.rs`.
const SKIP: &[&str] = &["fetch"];

/// The focusable nodes that have no name today, as `"<example>: <role>"` in
/// the order the tree is walked.
///
/// None of these can be named through the elements as they stand:
///
/// - `TextInput`: `egui::TextEdit` writes no label into its AccessKit node at
///   all. `hint_text` goes to `PlatformOutput` for the web screen reader, not
///   to the node, and `<TextEdit>` has no name prop. Naming these needs the
///   same treatment `<Button label>` got (`accesskit_node_builder`), or
///   `Response::labelled_by` pointing at the label beside the field.
/// - `form`'s `CheckBox` / `Slider` / `SpinButton` / `ComboBox`: the form names
///   them with a label column (`<Field>`), which AccessKit expresses as a
///   `labelled_by` relation that the elements do not expose. Their own `label`
///   props draw the text, which would put the name on screen twice and desync
///   the plain egui twin that the gallery renders beside it.
/// - `escape-hatch`'s `ColorWell`: a raw `ui.color_edit_button_srgba`, which
///   egui leaves unnamed.
/// - `shader`'s `Unknown`: the `<Canvas>` leaf, which senses drags. It is a
///   drawing surface, not a control; naming it needs a story for canvases in
///   general (`docs/tasks/a11y/`).
/// - `patch`'s `GenericContainer`: an `<ScrollArea>` whose content overflows.
///   egui makes such a region focusable so it can be scrolled from the
///   keyboard, and writes no name for it; `<ScrollArea>` has no name prop.
/// - `patch`'s `MultilineTextInput`: the WGSL source fields, a hand-written
///   `egui::TextEdit::multiline` leaf — the same gap as the single-line ones.
/// - `patch`'s `ComboBox`: `<ComboBox>` names itself from its `label` prop,
///   but that prop also *draws* the label beside the box, which is a change to
///   the picture rather than to the tree.
const KNOWN_UNNAMED: &[&str] = &[
    "showcase: TextInput",
    "board: TextInput",
    "patch: GenericContainer",
    "patch: MultilineTextInput",
    "patch: MultilineTextInput",
    "patch: ComboBox",
    "patch: ComboBox",
    "todo: TextInput",
    "form: TextInput",
    "form: CheckBox",
    "form: CheckBox",
    "form: Slider",
    "form: SpinButton",
    "form: ComboBox",
    "custom-hook: TextInput",
    "custom-hook: TextInput",
    "escape-hatch: ColorWell",
    "shader: Unknown",
    "list-10k: TextInput",
];

/// The same window as `gallery.rs`: a `ScrollArea` culls what it does not
/// draw, and a culled widget is not in the tree to be checked.
const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 1000.0;

fn harness<'a>(start: &'static str) -> Harness<'a, Store> {
    Harness::builder()
        .with_size(egui::vec2(WIDTH, HEIGHT))
        .build_ui_state(
            move |ui: &mut egui::Ui, store: &mut Store| {
                store.begin_pass(ui.ctx());
                {
                    let store: &Store = store;
                    let mut cx = Cx::new(store, ui, root_id());
                    let view = rsx! { <App start={start}/> };
                    cx.root_container(root_id(), root_style(), |cx| view.show(cx));
                }
                store.end_pass();
            },
            Store::new(),
        )
}

#[test]
fn every_focusable_widget_has_a_name() {
    let mut unnamed = Vec::new();
    let mut where_ = Vec::new();

    for meta in EXAMPLES {
        if SKIP.contains(&meta.name) {
            continue;
        }
        let mut harness = harness(meta.name);
        // `run_steps`, not `run`: an animated example (`shader`, `clock`) asks
        // for a repaint every frame and `run` gives up when it never settles.
        // Two passes is what the rest of the gallery tests take — one to lay
        // out, one to draw at the sizes taffy worked out.
        harness.run_steps(2);

        for node in harness.root().children_recursive() {
            let node = node.accesskit_node();
            if !node.data().supports_action(egui::accesskit::Action::Focus) {
                continue;
            }
            if node.label().is_some_and(|label| !label.trim().is_empty()) {
                continue;
            }
            unnamed.push(format!("{}: {:?}", meta.name, node.role()));
            where_.push(format!(
                "{}: {:?} at {:?}",
                meta.name,
                node.role(),
                node.bounding_box(),
            ));
        }
    }

    assert_eq!(
        unnamed,
        KNOWN_UNNAMED,
        "the focusable widgets with no name are not the ones this test knows \
         about.\n\nFound:\n{}\n\nA widget a screen reader cannot name is read \
         out as its role and nothing else. Pass `label` (or `alt` on an \
         `<Image>`), or, if the widget really cannot be named yet, add it to \
         `KNOWN_UNNAMED` with the reason.",
        where_.join("\n"),
    );
}
