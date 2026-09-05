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

/// How to run the app.
pub struct Options {
    /// The native window title. Ignored on wasm.
    pub title: String,
    /// How many passes egui may run for one frame.
    ///
    /// `egui_taffy` asks for a second pass when the layout changes, so this has
    /// to be at least 2. The default is 2, which is also egui 0.36's own
    /// default; setting it explicitly pins the behaviour (5.3).
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
            max_passes: 2,
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
                let mut cx = Cx::new(store, ui, egui::Id::new(ROOT_ID));
                let view = root(&mut cx);
                let style = ContainerStyle::default()
                    .direction("column")
                    .merge(&ItemStyle::default());
                cx.root_container(egui::Id::new(ROOT_ID), style, |cx| view.show(cx));
            }
            // Every guard died with the component bodies above, so the sweep is
            // safe.
            store.end_pass();
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
