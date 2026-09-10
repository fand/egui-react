//! The runner: `run(Options, |cx| rsx!{ <App/> })`.
//!
//! This is the only crate that knows about eframe. It owns the hook
//! [`Store`](egui_reactor::Store), opens the pass around the root view, wires
//! `use_persisted` to eframe's storage, and absorbs the difference between
//! native and wasm.

use egui_reactor::{Cx, Store, View};

pub mod a11y;
#[cfg(target_arch = "wasm32")]
pub mod accesskit_web;
pub mod fonts;

/// The eframe storage key everything `use_persisted` holds is written under.
///
/// Keeps the pre-rename name: the crate is now egui-reactor, but changing the
/// key would drop the `use_persisted` data already saved on the live site
/// (ARCHITECTURE 4, adr/app/0003).
const STORAGE_KEY: &str = "egui_react";

/// The egui id of the root taffy container.
///
/// Keeps the pre-rename name too, so the egui memory scoped under it (scroll
/// offsets and such) is not reset by the rename alone. That memory is keyed by
/// source position below this id and resets whenever those lines move, so this
/// is a courtesy, not a promise (adr/app/0003).
const ROOT_ID: &str = "egui_react_root";

/// The egui id of the root taffy container, as the runner builds it.
///
/// Exposed so tests and hand-written runners can reproduce the runner's frame.
pub fn root_id() -> egui::Id {
    egui::Id::new(ROOT_ID)
}

/// The taffy style of the root container, as [`egui_reactor::layout::root_style`].
///
/// Re-exported here so a hand-written runner needs one import.
pub use egui_reactor::layout::root_style;

/// What [`Options::setup`] holds: run once, when eframe is ready.
pub type Setup = Box<dyn FnOnce(&eframe::CreationContext<'_>)>;

/// How to run the app.
pub struct Options {
    /// The native window title. Ignored on wasm.
    pub title: String,
    /// How many passes egui may run for one frame.
    ///
    /// The layout engine asks for another pass when a node was created, removed
    /// or moved. A `<View>` inside an egui container inside a `<View>` is a
    /// second taffy tree that only learns its new size in the pass after the
    /// outer one, so each level of nesting can need one more pass. The default
    /// is 3, one more than egui's own; deeper nesting settles on the next frame
    /// instead (the runner requests a repaint when a discard was refused).
    pub max_passes: usize,
    /// Whether `use_persisted` is saved to and loaded from eframe's storage.
    pub persist: bool,
    /// The id of the `<canvas>` element to attach to. wasm only.
    pub canvas_id: String,
    /// Everything else eframe accepts. Native only: wasm has no window.
    #[cfg(not(target_arch = "wasm32"))]
    pub native: eframe::NativeOptions,
    /// Called once, as soon as eframe has a window and a render backend.
    ///
    /// This is where a wgpu pipeline is built and put into
    /// `cc.wgpu_render_state`'s `renderer.write().callback_resources`, so that
    /// a paint callback can find it again by type. egui's own `custom3d_wgpu`
    /// demo does the same thing in the same place.
    ///
    /// It is a hole in the runner rather than a hook or a context value on
    /// purpose: no wgpu type appears anywhere in the hook API, and an app that
    /// does not draw with wgpu never sees one. Available on native and on wasm.
    pub setup: Option<Setup>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            title: String::from("egui-reactor"),
            max_passes: 3,
            persist: true,
            canvas_id: String::from("egui_reactor_canvas"),
            #[cfg(not(target_arch = "wasm32"))]
            native: eframe::NativeOptions::default(),
            setup: None,
        }
    }
}

/// Run an app until the window closes.
///
/// ```ignore
/// fn main() -> eframe::Result {
///     egui_reactor_app::run(
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
    fn new(cc: &eframe::CreationContext<'_>, options: &mut Options, root: F) -> Self {
        // First, before anything is drawn: a paint callback added on frame one
        // has to find its pipeline already there.
        if let Some(setup) = options.setup.take() {
            setup(cc);
        }
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
    use egui_reactor::{Cx, View};

    pub(super) fn run<V, F>(options: Options, root: F) -> eframe::Result
    where
        V: View + 'static,
        F: FnMut(&mut Cx<'_, '_>) -> V + 'static,
    {
        // Only when the app installed no logger of its own. The default filter
        // is `error`, so nothing prints unless `RUST_LOG` asks for it;
        // `RUST_LOG=egui_reactor=debug` prints each layout discard and its cause.
        let _ = env_logger::try_init();
        let title = options.title.clone();
        let native = options.native.clone();
        let mut options = options;
        let mut root = Some(root);
        eframe::run_native(
            &title,
            native,
            Box::new(move |cc| {
                let root = root.take().expect("the app is created once");
                Ok(Box::new(ReactApp::new(cc, &mut options, root)))
            }),
        )
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{Options, ReactApp};
    use egui_reactor::{Cx, View};
    use wasm_bindgen::JsCast as _;

    pub(super) fn run<V, F>(options: Options, root: F) -> eframe::Result
    where
        V: View + 'static,
        F: FnMut(&mut Cx<'_, '_>) -> V + 'static,
    {
        // `log` to the browser console, as the eframe template does; `debug`
        // so that the layout engine's discard reasons show up there.
        eframe::WebLogger::init(log::LevelFilter::Debug).ok();
        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&options.canvas_id))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let Some(canvas) = canvas else {
            log::error!(
                "egui-reactor: no <canvas id=\"{}\"> in the document",
                options.canvas_id
            );
            return Ok(());
        };

        let mut options = options;
        let mut root = Some(root);
        wasm_bindgen_futures::spawn_local(async move {
            let result = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(move |cc| {
                        let root = root.take().expect("the app is created once");
                        Ok(Box::new(ReactApp::new(cc, &mut options, root)))
                    }),
                )
                .await;
            if let Err(err) = result {
                log::error!("egui-reactor: could not start the web runner: {err:?}");
            }
        });
        // The app keeps running in the browser's event loop.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Options;

    #[test]
    fn setup_is_off_by_default() {
        assert!(Options::default().setup.is_none());
    }
}
