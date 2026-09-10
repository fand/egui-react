//! Every bound widget in one settings form, saved across restarts.
//!
//! A `bind` prop hands the widget `&mut` the state, so the widget writes into
//! it directly and nothing has to be copied back. That is also why `on_change`
//! cannot read the new value here: the widget already holds the only `&mut`,
//! and a handler on the same element that touched the same state would not
//! compile. So `on_change` carries whatever the widget can hand over — the new
//! `bool`, the new index — and the log is somewhere else entirely.

use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;
use example_meta::Meta;
use serde::{Deserialize, Serialize};

pub mod plain;

pub const META: Meta = Meta {
    name: "form",
    summary: "Every bound widget, a change log, and settings that survive a restart.",
    hooks: &["use_state", "use_persisted"],
    elements: &[
        "View",
        "Text",
        "TextEdit",
        "Checkbox",
        "Slider",
        "ComboBox",
        "Button",
        "Collapsing",
    ],
    source: include_str!("lib.rs"),
    plain: Some(include_str!("plain.rs")),
};

/// The options the theme dropdown offers. `ComboBox` binds an index, so this
/// is the list the index points into.
pub const THEMES: [&str; 3] = ["dark", "light", "system"];

/// Everything the form edits. `pub` so [`plain`] can edit the same thing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub name: String,
    pub notify: bool,
    pub autosave: bool,
    pub volume: u8,
    pub theme: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            name: String::from("anon"),
            notify: true,
            autosave: false,
            volume: 50,
            theme: 0,
        }
    }
}

impl Settings {
    /// One line saying what the settings are, so the form has something to
    /// show for itself (and a test has something to read).
    pub fn summary(&self) -> String {
        format!(
            "{}, {}, volume {}",
            self.name, THEMES[self.theme], self.volume
        )
    }
}

/// The width of the label column, so the widgets line up.
const LABEL: f32 = 90.0;

#[component]
pub fn App(cx: &mut Cx) {
    let mut settings = use_persisted(cx, "form/settings", Settings::default);
    let mut log = use_state(cx, Vec::<String>::new);

    let summary = settings.summary();
    let entries: Vec<(usize, String)> = log.iter().cloned().enumerate().rev().take(8).collect();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"settings"</Text>

            <Field label="name">
                <TextEdit
                    w={200.0}
                    bind={&mut settings.bind().name}
                    on_change={|| log.push(String::from("name edited"))}
                />
            </Field>

            <Field label="notify">
                <Checkbox
                    bind={&mut settings.bind().notify}
                    on_change={|on: bool| log.push(format!("notify = {on}"))}
                />
            </Field>

            <Field label="autosave">
                <Checkbox
                    bind={&mut settings.bind().autosave}
                    on_change={|on: bool| log.push(format!("autosave = {on}"))}
                />
            </Field>

            <Field label="volume">
                <Slider
                    bind={&mut settings.bind().volume}
                    range={0..=100}
                    on_change={|| log.push(String::from("volume changed"))}
                />
            </Field>

            <Field label="theme">
                <ComboBox
                    bind={&mut settings.bind().theme}
                    options={&THEMES}
                    on_change={|i: usize| log.push(format!("theme = {}", THEMES[i]))}
                />
            </Field>

            <Separator/>

            <View direction="row" gap={8} align="center" w="100%">
                <Text grow={1.0}>{summary}</Text>
                <Button on_click={|| {
                    *settings = Settings::default();
                    log.push(String::from("reset"));
                }}>"reset"</Button>
            </View>

            if !entries.is_empty() {
                <Collapsing header={&format!("log ({})", entries.len())}>
                    <View direction="column" gap={2}>
                        for (i, line) in entries.iter() {
                            <Text key={i}>{line.as_str()}</Text>
                        }
                    </View>
                </Collapsing>
            }
        </View>
    }
}

/// One labelled row. The label column has a width so the widgets line up;
/// without it every row would start wherever its own text ended.
#[component]
fn Field(cx: &mut Cx, label: &str, children: impl View) {
    rsx! {
        <View direction="row" gap={8} align="center" w="100%">
            <Text w={LABEL}>{label}</Text>
            {children}
        </View>
    }
}
