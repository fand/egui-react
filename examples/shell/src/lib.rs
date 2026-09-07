//! An editor-shaped frame: docked panels, a floating window, and an editor in
//! what is left over.
//!
//! This is the one example the gallery does not run. A docked `<Panel>` carves
//! its space out of the `Ui` its siblings are drawn into, so it wants to be at
//! the root of an app, not inside a column of one. See tasks/examples/plan.md
//! section 8 for what happened when it was tried.
//!
//! Two rules, and they are the whole trick here:
//!
//! - Panels and `<CentralPanel>` are written as **siblings**, not nested. Each
//!   takes a bite out of what is left, in the order they appear, and the
//!   central panel gets the rest. Nesting them would put a panel inside a
//!   panel.
//! - A panel docks in the nearest enclosing egui `Ui`, which under the runner
//!   is the window. Any `<View>` in between is skipped, so it makes no
//!   difference whether the panels are at the root of `App` or further in.
//!
//! Everything inside a panel is drawn in plain `Ui` mode, so layout attributes
//! (`grow`, `justify`) do nothing there until a `<View>` starts a tree of its
//! own — as the toolbar and the inspector do.

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub const META: Meta = Meta {
    name: "shell",
    summary: "Docked panels, a floating window, and an editor in what is left.",
    hooks: &["use_state"],
    elements: &[
        "Panel",
        "CentralPanel",
        "Window",
        "Collapsing",
        "Frame",
        "ScrollArea",
        "View",
        "Text",
        "TextEdit",
        "Button",
    ],
    source: include_str!("lib.rs"),
    plain: None,
};

/// One file in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub folder: &'static str,
    pub name: &'static str,
    pub text: String,
}

impl File {
    fn new(folder: &'static str, name: &'static str, text: &str) -> Self {
        Self {
            folder,
            name,
            text: text.to_owned(),
        }
    }

    pub fn path(&self) -> String {
        format!("{}/{}", self.folder, self.name)
    }
}

/// The tree the shell opens with. Grouped by folder, in order.
pub fn initial_files() -> Vec<File> {
    vec![
        File::new(
            "src",
            "main.rs",
            "fn main() {\n    println!(\"hello\");\n}\n",
        ),
        File::new("src", "lib.rs", "pub mod shell;\npub mod tree;\n"),
        File::new("docs", "README.md", "# shell\n\nAn editor-shaped frame.\n"),
        File::new("tests", "basic.rs", "#[test]\nfn it_works() {}\n"),
    ]
}

/// The folders, in the order they first appear, with the file indices in each.
fn folders(files: &[File]) -> Vec<(&'static str, Vec<usize>)> {
    let mut out: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, file) in files.iter().enumerate() {
        match out.iter_mut().find(|(name, _)| *name == file.folder) {
            Some((_, indices)) => indices.push(i),
            None => out.push((file.folder, vec![i])),
        }
    }
    out
}

#[component]
pub fn App(cx: &mut Cx) {
    let mut files = use_state(cx, initial_files);
    let mut selected = use_state(cx, || 0usize);
    let mut log = use_state(cx, || vec![String::from("opened src/main.rs")]);
    let mut inspector = use_state(cx, || true);

    // Read what the panels need before any of them borrows the state.
    let tree = folders(&files);
    let names: Vec<String> = files.iter().map(|file| file.name.to_owned()).collect();
    let current = *selected;
    let path = files[current].path();
    let text = files[current].text.clone();
    let entries: Vec<(usize, String)> = log.iter().cloned().enumerate().rev().collect();

    rsx! {
        // Siblings, not nested. Top first, so it spans the full width; then the
        // sides; then the bottom; then whatever is left.
        <Panel side="top" default_size={30.0} resizable={false}>
            <View direction="row" gap={8} align="center">
                <Button on_click={|| log.push(format!("saved {path}"))}>"save"</Button>
                <Button on_click={|| *inspector = !*inspector}>"toggle inspector"</Button>
                <Text>{path.as_str()}</Text>
            </View>
        </Panel>

        <Panel side="left" default_size={170.0}>
            <Text strong>"files"</Text>
            for (folder, indices) in tree.iter() {
                <Collapsing key={folder} header={folder} default_open>
                    for i in indices.iter() {
                        // A selectable label rather than a `<Button>`: the tree
                        // shows which file is open, and elements have no
                        // element for that yet.
                        {view(|cx| {
                            let picked = cx.leaf(&ItemStyle::default(), |ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                ui.selectable_label(*i == current, names[*i].as_str())
                                    .clicked()
                            });
                            if picked {
                                *selected = *i;
                                log.push(format!("opened {}", files[*i].path()));
                            }
                        })}
                    }
                </Collapsing>
            }
        </Panel>

        <Panel side="bottom" default_size={110.0}>
            <Text strong>"log"</Text>
            <ScrollArea>
                for (i, line) in entries.iter() {
                    <Text key={i}>{line.as_str()}</Text>
                }
            </ScrollArea>
        </Panel>

        // The editor gets what the panels left. `<CentralPanel>` is what claims
        // it: without one the remaining space is simply not drawn into.
        <CentralPanel>
            // A `<View>` inside the panel, so the editor has a taffy tree to
            // grow in: `grow` means nothing in the panel's plain `Ui`.
            //
            // `w` and `h`, not just `grow`. A `<View>` with an `auto` size is
            // measured by its content, and the editor inside is measured by
            // *its* content, so the two would agree on a couple of characters
            // wide and stay there. The same rule as the runner's own root.
            <View direction="column" w="100%" h="100%">
                <TextEdit multiline grow={1.0} bind={&mut files.bind()[current].text}/>
            </View>
        </CentralPanel>

        // Anchored to the `egui::Context`, so it floats over all of the above
        // and takes no space in any of them.
        // `bind()`, not `&mut *inspector`: the window writes the flag only
        // when its close button is used, and `&mut *` would mark the state
        // changed on every frame and ask for another one, forever.
        // `default_pos`, or egui opens it over the toolbar.
        <Window
            title="inspector"
            open={inspector.bind()}
            default_pos={egui::pos2(560.0, 300.0)}
            default_size={egui::vec2(220.0, 90.0)}
        >
            <Frame inner_margin={4.0}>
                <View direction="column" gap={4}>
                    <Text strong>{path.as_str()}</Text>
                    <Text>{format!("{} lines", text.lines().count())}</Text>
                    <Text>{format!("{} characters", text.chars().count())}</Text>
                </View>
            </Frame>
        </Window>
    }
}
