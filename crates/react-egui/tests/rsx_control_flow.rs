//! Plan 2.3 / test 3-2: `if` / `else if` / `else`, `for` with `key`, `match`
//! arms, fragments and `{expr}` inside `rsx!`.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{Counter, run_app};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use react_egui::prelude::*;

#[component]
fn Tag(cx: &mut Cx, text: &str) {
    cx.ui().label(text);
}

fn body(cx: &mut Cx<'_, '_>, n: i32) {
    let maybe: Option<&str> = (n == 7).then_some("option");
    rsx! {
        if n < 0 {
            <Tag text="negative"/>
        } else if n == 0 {
            <Tag text="zero"/>
        } else {
            <Tag text="positive"/>
        }
        <>
            "fragment a"
            "fragment b"
        </>
        for i in 0..3 {
            <Tag key={i} text={&format!("item {i}")}/>
        }
        match n {
            0 => "matched zero",
            1 | 2 => { <Tag text="matched small"/> }
            other if other > 5 => { <Tag text="matched big"/> }
            _ => {}
        }
        {maybe}
    }
    .show(cx);
}

fn harness_for(n: &Rc<Cell<i32>>) -> Harness<'static, Store> {
    let n = Rc::clone(n);
    Harness::new_ui_state(
        move |ui, store: &mut Store| {
            let n = n.get();
            run_app(ui, store, move |cx| body(cx, n));
        },
        Store::new(),
    )
}

#[test]
fn if_else_chain_picks_one_branch() {
    let n = Rc::new(Cell::new(0));
    let mut harness = harness_for(&n);

    harness.run();
    assert!(harness.query_by_label("zero").is_some());
    assert!(harness.query_by_label("negative").is_none());
    assert!(harness.query_by_label("positive").is_none());

    n.set(-1);
    harness.run();
    assert!(harness.query_by_label("negative").is_some());

    n.set(3);
    harness.run();
    assert!(harness.query_by_label("positive").is_some());
}

#[test]
fn fragments_and_loops_and_blocks() {
    let n = Rc::new(Cell::new(0));
    let mut harness = harness_for(&n);
    harness.run();

    assert!(harness.query_by_label("fragment a").is_some());
    assert!(harness.query_by_label("fragment b").is_some());
    for i in 0..3 {
        assert!(harness.query_by_label(&format!("item {i}")).is_some());
    }
    // `{maybe}` is `None` here, so nothing is drawn for it.
    assert!(harness.query_by_label("option").is_none());

    n.set(7);
    harness.run();
    assert!(harness.query_by_label("option").is_some());
}

#[test]
fn match_arms_cover_literals_guards_and_wildcards() {
    let n = Rc::new(Cell::new(0));
    let mut harness = harness_for(&n);

    harness.run();
    assert!(harness.query_by_label("matched zero").is_some());

    n.set(2);
    harness.run();
    assert!(harness.query_by_label("matched small").is_some());

    n.set(6);
    harness.run();
    assert!(harness.query_by_label("matched big").is_some());

    // The `_ => {}` arm draws nothing.
    n.set(4);
    harness.run();
    assert!(harness.query_by_label("matched zero").is_none());
    assert!(harness.query_by_label("matched small").is_none());
    assert!(harness.query_by_label("matched big").is_none());
}

#[test]
fn keyed_loop_items_keep_their_own_state() {
    let mut harness = Harness::new_ui_state(
        |ui, store: &mut Store| {
            run_app(ui, store, |cx| {
                rsx! {
                    for i in 0..2 {
                        <Counter key={i} initial={i * 10}/>
                    }
                }
                .show(cx);
            });
        },
        Store::new(),
    );

    harness.run();
    assert!(harness.query_by_label("count: 0").is_some());
    assert!(harness.query_by_label("count: 10").is_some());

    harness.get_all_by_label("+").next().unwrap().click();
    harness.run();
    assert!(harness.query_by_label("count: 1").is_some());
    assert!(harness.query_by_label("count: 10").is_some());
}
