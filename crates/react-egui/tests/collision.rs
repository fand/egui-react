//! ARCHITECTURE.md 10, item 6 and 3.4: two hooks that end up with the same id
//! are reported as a collision, and the two documented causes (a custom hook
//! without `#[hook]`, a loop without `key`) are both caught.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{run_app, use_counter_unscoped};
use egui_kittest::Harness;
use react_egui::prelude::*;

/// Run one pass of `app` and return how many collisions it recorded.
///
/// `begin_pass` clears the list, so it has to be read inside the pass.
fn collisions_of(app: impl Fn(&mut Cx<'_, '_>) + 'static) -> usize {
    let count = Rc::new(Cell::new(0usize));
    let count_in_app = Rc::clone(&count);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let count = Rc::clone(&count_in_app);
            run_app(ui, store, |cx| {
                app(cx);
                count.set(cx.store.collisions().len());
            });
        },
        Store::new(),
    );
    harness.run();
    count.get()
}

#[test]
fn custom_hook_without_hook_scope_collides() {
    let n = collisions_of(|cx| {
        // Both calls resolve to the one `use_state` line inside the helper.
        let a = *use_counter_unscoped(cx);
        let b = *use_counter_unscoped(cx);
        cx.ui.label(format!("{a} {b}"));
    });
    assert_eq!(n, 1);
}

#[test]
fn loop_without_key_collides() {
    let n = collisions_of(|cx| {
        for i in 0..3 {
            let v = *use_state(cx, || i);
            cx.ui.label(format!("{v}"));
        }
    });
    assert_eq!(n, 2);
}

#[test]
fn loop_with_scope_does_not_collide() {
    let n = collisions_of(|cx| {
        for i in 0..3 {
            cx.scope(i, |cx| {
                let v = *use_state(cx, || i);
                cx.ui.label(format!("{v}"));
            });
        }
    });
    assert_eq!(n, 0);
}

#[test]
#[should_panic(expected = "hook id collision")]
fn colliding_live_guards_panic_with_a_clear_message() {
    collisions_of(|cx| {
        // The first guard is still alive when the second call asks for the
        // same slot, so this cannot be recovered from.
        let a = use_counter_unscoped(cx);
        let b = use_counter_unscoped(cx);
        cx.ui.label(format!("{} {}", *a, *b));
    });
}
