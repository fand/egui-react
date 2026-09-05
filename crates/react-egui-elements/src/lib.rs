//! The elements users write inside `rsx!`.
//!
//! Three families, all `#[component]` functions:
//!
//! - [`View`](view::View) and [`Text`](view::Text), the flex/grid primitives
//!   built on `egui_taffy`.
//! - Widgets ([`Button`](widgets::Button), [`TextEdit`](widgets::TextEdit), ..),
//!   which draw one egui widget through `cx.leaf`.
//! - Containers ([`ScrollArea`](containers::ScrollArea),
//!   [`Window`](containers::Window), ..), which wrap an egui container and
//!   re-enter the tree with the inner `Ui`.
//!
//! Every element accepts the layout attributes of
//! [`ItemStyle`](react_egui::ItemStyle) through a `style` prop, which `rsx!`
//! fills in from `w=` / `grow=` / `p=` and friends.

pub mod containers;
pub mod view;
pub mod widgets;

/// Every element and every generated event enum, in one `use`.
///
/// The event enums have to be in scope wherever `<Button on_click=../>` is
/// written, because that is the name the fused closure matches on.
pub mod prelude {
    pub use crate::containers::{
        CentralPanel, Collapsing, Frame, Grid, Horizontal, Panel, Row, ScrollArea, Side, Vertical,
        Window,
    };
    pub use crate::view::{Text, View};
    pub use crate::widgets::{
        Button, ButtonEvent, Checkbox, CheckboxEvent, ComboBox, ComboBoxEvent, Image, Label,
        Separator, Slider, SliderEvent, TextEdit, TextEditEvent,
    };
}
