//! A todo list: `use_reducer` for the messages, `use_persisted` for the data.
//!
//! Quit and restart it and the list is still there (native; the value is kept
//! in eframe's storage under the `"react_egui"` key).

use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;
use serde::{Deserialize, Serialize};

pub mod plain;

pub const META: Meta = Meta {
    name: "todo",
    summary: "A reducer drives the list; `use_persisted` keeps it across restarts.",
    hooks: &["use_state", "use_persisted", "use_reducer"],
    elements: &[
        "View",
        "Text",
        "TextEdit",
        "Checkbox",
        "Button",
        "Separator",
        "Collapsing",
    ],
    source: include_str!("lib.rs"),
    plain: Some(include_str!("plain.rs")),
};

/// One item. `pub` so [`plain`] can share it: the data model is the same, only
/// the code around it differs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    pub text: String,
    pub done: bool,
}

/// Everything that can change the list.
enum Msg {
    Add(String),
    Toggle(usize),
    Remove(usize),
    ClearDone,
}

fn reduce(todos: &mut Vec<Todo>, msg: Msg) {
    match msg {
        Msg::Add(text) => todos.push(Todo { text, done: false }),
        Msg::Toggle(i) => {
            if let Some(todo) = todos.get_mut(i) {
                todo.done = !todo.done;
            }
        }
        Msg::Remove(i) => {
            if i < todos.len() {
                todos.remove(i);
            }
        }
        Msg::ClearDone => todos.retain(|todo| !todo.done),
    }
}

#[component]
pub fn App(cx: &mut Cx) {
    // The data lives in storage; the reducer is what changes it. The key is
    // prefixed with the example name because the gallery runs every example
    // against one storage slot.
    let mut saved = use_persisted(cx, "todo/todos", Vec::<Todo>::new);
    let mut draft = use_state(cx, String::new);

    let (todos, dispatch) = use_reducer(
        cx,
        |state: &mut Vec<Todo>, msg| reduce(state, msg),
        || saved.clone(),
    );
    // Mirror the reducer's state into the persisted slot; both are cheap Vecs
    // and this keeps the storage copy up to date without a second reducer.
    if *saved != *todos {
        *saved = todos.clone();
    }

    let remaining = todos.iter().filter(|todo| !todo.done).count();
    let done: Vec<(usize, String)> = todos
        .iter()
        .enumerate()
        .filter(|(_, todo)| todo.done)
        .map(|(i, todo)| (i, todo.text.clone()))
        .collect();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"todo"</Text>

            <View direction="row" gap={8} align="center">
                // The field owns the `&mut String`, so `on_submit` gets the
                // text as its payload and `clear_on_submit` empties the field.
                <TextEdit
                    grow={1.0}
                    bind={draft.bind()}
                    hint="what needs doing?"
                    clear_on_submit
                    on_submit={|text: String| {
                        let text = text.trim().to_owned();
                        if !text.is_empty() {
                            dispatch.send(Msg::Add(text));
                        }
                    }}
                />
                <Text>{format!("{remaining} left")}</Text>
            </View>

            <Separator/>

            for (i, todo) in todos.iter().enumerate().filter(|(_, t)| !t.done) {
                <View key={i} direction="row" gap={8} align="center">
                    // `todos` is borrowed by the loop, so the checkbox binds to
                    // a scratch copy and the real change goes through the
                    // `Dispatch`, which lands on the next frame.
                    <Checkbox
                        bind={&mut { todo.done }}
                        on_change={|_: bool| dispatch.send(Msg::Toggle(i))}
                    />
                    <Text grow={1.0}>{todo.text.as_str()}</Text>
                    // `label` is what a screen reader says; "x" is what is drawn.
                    <Button label="remove" on_click={|| dispatch.send(Msg::Remove(i))}>"x"</Button>
                </View>
            }

            if !done.is_empty() {
                <Collapsing header={&format!("done ({})", done.len())}>
                    <View direction="column" gap={4}>
                        for (i, text) in done.iter() {
                            <View key={i} direction="row" gap={8} align="center">
                                <Text grow={1.0}>{text.as_str()}</Text>
                                <Button on_click={|| dispatch.send(Msg::Toggle(*i))}>"undo"</Button>
                            </View>
                        }
                        <Button on_click={|| dispatch.send(Msg::ClearDone)}>"clear done"</Button>
                    </View>
                </Collapsing>
            }
        </View>
    }
}
