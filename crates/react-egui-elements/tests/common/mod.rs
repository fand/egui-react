//! The runner the element tests share, standing in for `react-egui-app`.
#![allow(dead_code)]

use react_egui::prelude::*;

/// One pass: `begin_pass` -> draw -> (all guards dropped) -> `end_pass`.
pub fn run_app(ui: &mut egui::Ui, store: &mut Store, app: impl FnOnce(&mut Cx<'_, '_>)) {
    store.begin_pass(ui.ctx());
    {
        let store: &Store = store;
        let mut cx = Cx::new(store, ui, egui::Id::new("root"));
        app(&mut cx);
    }
    store.end_pass();
}
