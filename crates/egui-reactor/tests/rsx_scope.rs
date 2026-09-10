//! Plan 2.3 / test 3-6: `rsx!` gives every element its own scope, `key`
//! separates loop iterations, and an element removed by `if` unmounts.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{Counter, run_app};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use egui_reactor::prelude::*;

#[test]
fn two_elements_of_the_same_component_are_independent() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    <Counter initial={0}/>
                    <Counter initial={0}/>
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert_eq!(harness.query_all_by_label("count: 0").count(), 2);

    harness.get_all_by_label("+").next().unwrap().click();
    harness.run();
    assert_eq!(harness.query_all_by_label("count: 1").count(), 1);
    assert_eq!(harness.query_all_by_label("count: 0").count(), 1);
}

/// Run one pass and report how many id collisions it recorded.
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
fn a_keyed_loop_does_not_collide() {
    let n = collisions_of(|cx| {
        rsx! {
            for i in 0..3 {
                <Counter key={i} initial={0}/>
            }
        }
        .show(cx);
    });
    assert_eq!(n, 0);
}

#[test]
fn a_loop_without_key_collides() {
    let n = collisions_of(|cx| {
        rsx! {
            for _i in 0..3 {
                <Counter initial={0}/>
            }
        }
        .show(cx);
    });
    assert!(n > 0, "a loop without key= must be reported");
}

#[test]
fn an_element_removed_by_if_unmounts() {
    let show = Rc::new(Cell::new(true));
    let show_in_app = Rc::clone(&show);
    let mut harness = Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let show = show_in_app.get();
            run_app(ui, store, move |cx| {
                rsx! {
                    if show {
                        <Counter initial={0}/>
                    }
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    harness.get_by_label("+").click();
    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());

    show.set(false);
    harness.run();
    assert_eq!(harness.state().len(), 0);

    // Back again: a fresh mount, at the initial value.
    show.set(true);
    harness.run();
    assert!(harness.query_by_label("count: 0").is_some());
}
