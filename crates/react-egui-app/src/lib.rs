//! The runner: `run(Options, |cx| rsx!{ <App/> })`.
//!
//! This is the only crate that knows about eframe. It owns the hook
//! [`Store`](react_egui::Store), opens the pass around the root view, wires
//! `use_persisted` to eframe's storage, and absorbs the difference between
//! native and wasm.

use react_egui::layout::{ContainerStyle, ItemStyle};
use react_egui::{Cx, Store, View};

/// The eframe storage key everything `use_persisted` holds is written under.
const STORAGE_KEY: &str = "react_egui";

/// The egui id of the root taffy container.
const ROOT_ID: &str = "react_egui_root";

/// The egui id of the root taffy container, as the runner builds it.
///
/// Exposed so tests and hand-written runners can reproduce the runner's frame.
pub fn root_id() -> egui::Id {
    egui::Id::new(ROOT_ID)
}

/// The taffy style of the root container: a column that fills the window.
///
/// `reserve_available_space` tells egui_taffy how much room there is, but it
/// leaves the root node's own `size` at `auto`, so taffy would size that node
/// by its content. Two things go wrong then. A `<View grow={1.0}
/// justify="center">` child finds no free space to grow into and nothing to be
/// centred in, so the app sits in the window's top-left corner. And a child
/// too wide to fit never has to shrink, because a content-sized parent simply
/// grows with it and there is no overflow to resolve — the row runs off the
/// right edge instead of `grow` and `flex-shrink` sharing out what there is.
///
/// So the width is fixed at 100%: a window is exactly as wide as it is, and
/// content that wants more belongs in a horizontal `ScrollArea`. The height is
/// only a *minimum* of 100%, so a column taller than the window still lays out
/// at its own height rather than being clipped.
///
/// Exposed for the same reason as [`root_id`].
pub fn root_style() -> react_egui::taffy::Style {
    ContainerStyle::default()
        .direction("column")
        .merge(&ItemStyle::default().w("100%").min_h("100%"))
}

/// How to run the app.
pub struct Options {
    /// The native window title. Ignored on wasm.
    pub title: String,
    /// How many passes egui may run for one frame.
    ///
    /// `egui_taffy` asks for another pass whenever a layout changes, and a
    /// `<View>` inside an egui container inside a `<View>` is a second taffy
    /// tree that only learns its new size in the pass after the outer one, so
    /// each level of nesting needs one more pass. The default is 3, one more
    /// than egui's own; deeper nesting settles on the next frame instead (the
    /// runner requests a repaint when a discard was refused).
    pub max_passes: usize,
    /// Whether `use_persisted` is saved to and loaded from eframe's storage.
    pub persist: bool,
    /// The id of the `<canvas>` element to attach to. wasm only.
    pub canvas_id: String,
    /// Everything else eframe accepts. Native only: wasm has no window.
    #[cfg(not(target_arch = "wasm32"))]
    pub native: eframe::NativeOptions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            title: String::from("react-egui"),
            max_passes: 3,
            persist: true,
            canvas_id: String::from("react_egui_canvas"),
            #[cfg(not(target_arch = "wasm32"))]
            native: eframe::NativeOptions::default(),
        }
    }
}

/// Run an app until the window closes.
///
/// ```ignore
/// fn main() -> eframe::Result {
///     react_egui_app::run(
///         Options {
///             title: String::from("counter"),
///             ..Default::default()
///         },
///         |_cx| rsx! { <App/> },
///     )
/// }
/// ```
///
/// `root` is called once per pass and must return a view that does not borrow
/// anything it created: a hook called *in* `root` would hand out a guard, and
/// the `rsx!` that reads it cannot outlive the closure body. Put the hooks in a
/// component and keep the root as `|_cx| rsx!{ <App/> }`; the `Cx` is there for
/// escape-hatch roots and is normally unused.
///
/// The root view is drawn inside a `CentralPanel` and a taffy container with
/// `direction: column` that reserves the whole window, so `<View>` children of
/// the root can use `grow` and `justify` right away.
pub fn run<V, F>(options: Options, root: F) -> eframe::Result
where
    V: View + 'static,
    F: FnMut(&mut Cx<'_, '_>) -> V + 'static,
{
    platform::run(options, root)
}

/// The eframe application: the hook store plus the user's root closure.
struct ReactApp<V, F> {
    store: Store,
    root: F,
    persist: bool,
    _view: std::marker::PhantomData<fn() -> V>,
}

impl<V, F> ReactApp<V, F>
where
    V: View,
    F: FnMut(&mut Cx<'_, '_>) -> V,
{
    fn new(cc: &eframe::CreationContext<'_>, options: &Options, root: F) -> Self {
        let mut store = Store::new();
        if options.persist
            && let Some(storage) = cc.storage
            && let Some(json) = storage.get_string(STORAGE_KEY)
        {
            store.load_persisted(&json);
        }
        if let Some(max_passes) = std::num::NonZeroUsize::new(options.max_passes) {
            cc.egui_ctx.options_mut(|o| o.max_passes = max_passes);
        }
        Self {
            store,
            root,
            persist: options.persist,
            _view: std::marker::PhantomData,
        }
    }
}

impl<V, F> eframe::App for ReactApp<V, F>
where
    V: View + 'static,
    F: FnMut(&mut Cx<'_, '_>) -> V + 'static,
{
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // The `Ui` eframe hands out has no margin and no background, so the
        // panel is what makes text readable in light mode.
        egui::CentralPanel::default().show(ui, |ui| {
            // Split the borrows: the pass needs `&Store` while `root` is
            // `&mut`.
            let Self { store, root, .. } = self;
            store.begin_pass(ui.ctx());
            {
                let store: &Store = store;
                let mut cx = Cx::new(store, ui, root_id());
                let view = root(&mut cx);
                cx.root_container(root_id(), root_style(), |cx| view.show(cx));
            }
            // Every guard died with the component bodies above, so the sweep is
            // safe.
            store.end_pass();
            // A discard that egui refused (max_passes exhausted) would leave a
            // nested taffy tree drawn with its previous layout until the next
            // input; settle it on the next frame instead.
            let ctx = ui.ctx();
            if ctx.output(|o| o.requested_discard()) && !ctx.will_discard() {
                ctx.request_repaint();
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if self.persist {
            storage.set_string(STORAGE_KEY, self.store.save_persisted());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::{Options, ReactApp};
    use react_egui::{Cx, View};

    pub(super) fn run<V, F>(options: Options, root: F) -> eframe::Result
    where
        V: View + 'static,
        F: FnMut(&mut Cx<'_, '_>) -> V + 'static,
    {
        let title = options.title.clone();
        let native = options.native.clone();
        let mut root = Some(root);
        eframe::run_native(
            &title,
            native,
            Box::new(move |cc| {
                let root = root.take().expect("the app is created once");
                Ok(Box::new(ReactApp::new(cc, &options, root)))
            }),
        )
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{Options, ReactApp};
    use react_egui::{Cx, View};
    use wasm_bindgen::JsCast as _;

    pub(super) fn run<V, F>(options: Options, root: F) -> eframe::Result
    where
        V: View + 'static,
        F: FnMut(&mut Cx<'_, '_>) -> V + 'static,
    {
        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&options.canvas_id))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let Some(canvas) = canvas else {
            log::error!(
                "react-egui: no <canvas id=\"{}\"> in the document",
                options.canvas_id
            );
            return Ok(());
        };

        let mut root = Some(root);
        wasm_bindgen_futures::spawn_local(async move {
            let result = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(move |cc| {
                        let root = root.take().expect("the app is created once");
                        Ok(Box::new(ReactApp::new(cc, &options, root)))
                    }),
                )
                .await;
            if let Err(err) = result {
                log::error!("react-egui: could not start the web runner: {err:?}");
            }
        });
        // The app keeps running in the browser's event loop.
        Ok(())
    }
}
