//! The elements users write inside `rsx!`.
//!
//! Three families, all `#[component]` functions:
//!
//! - [`View`](view::View) and [`Text`](view::Text), the flex/grid primitives
//!   built on egui-react's own layout engine over taffy (`cx.container`,
//!   `cx.text`).
//! - Widgets ([`Button`](widgets::Button), [`TextEdit`](widgets::TextEdit), ..),
//!   which draw one egui widget through `cx.leaf`.
//! - Containers ([`ScrollArea`](containers::ScrollArea),
//!   [`Window`](containers::Window), ..), which wrap an egui container and
//!   re-enter the tree with the inner `Ui`.
//!
//! Plus [`Suspense`](suspense::Suspense), which draws a fallback until the
//! `use_future`s below it are ready, and [`Canvas`](canvas::Canvas), a leaf
//! taffy sizes and the caller paints.
//!
//! Every element accepts the layout attributes of
//! [`ItemStyle`](egui_react::ItemStyle) through a `style` prop, which `rsx!`
//! fills in from `w=` / `grow=` / `p=` and friends.

pub mod canvas;
pub mod containers;
pub mod suspense;
pub mod view;
pub mod virtual_list;
pub mod widgets;

/// Every element and every generated event enum, in one `use`.
///
/// The event enums have to be in scope wherever `<Button on_click=../>` is
/// written, because that is the name the fused closure matches on.
pub mod prelude {
    pub use crate::canvas::{Canvas, CanvasEvent};
    pub use crate::containers::{
        Anchor, CentralPanel, Collapsing, Frame, Grid, Horizontal, Order, Overlay, Panel, Row,
        ScrollArea, Side, Vertical, Window,
    };
    pub use crate::suspense::Suspense;
    pub use crate::view::{Text, View};
    pub use crate::virtual_list::VirtualList;
    pub use crate::widgets::{
        Button, ButtonEvent, Checkbox, CheckboxEvent, ComboBox, ComboBoxEvent, Image, Label,
        Separator, Slider, SliderEvent, TextEdit, TextEditEvent,
    };
}
