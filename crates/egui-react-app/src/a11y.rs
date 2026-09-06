//! Web accessibility: the egui plugin that drives the DOM mirror.
//!
//! egui builds an AccessKit tree for every pass and puts the diff in
//! `PlatformOutput::accesskit_update`. On native, egui-winit hands that to an
//! AccessKit adapter and the OS reads it. On the web, eframe throws it away
//! (`accesskit_update: _, // not currently implemented`), so nothing an app
//! draws reaches a screen reader.
//!
//! [`WebA11y`] picks the tree up before eframe sees it, using egui 0.36's
//! [`egui::Plugin`] hooks, and hands it to [`accesskit_web::Adapter`], which
//! mirrors it into hidden DOM elements over the canvas. Nothing here needs a
//! forked eframe.
//!
//! On any target but wasm32 this is an empty plugin, so an app can register it
//! unconditionally.
//!
//! ```ignore
//! Options {
//!     setup: Some(Box::new(|cc| {
//!         cc.egui_ctx.add_plugin(egui_react_app::a11y::WebA11y::new("egui_react_canvas"));
//!     })),
//!     ..Default::default()
//! }
//! ```

/// Mirrors the app's accessibility tree into the DOM, on the web.
///
/// Register it with `Context::add_plugin`, from [`crate::Options::setup`],
/// where eframe already has its canvas. Registering it turns AccessKit on
/// (`Context::enable_accesskit`), which costs one `accesskit::Node` per widget
/// per pass, so an app that does not want to pay that should not register it.
///
/// The plugin itself is just a key: `egui::Plugin` is `Send + Sync` and DOM
/// handles are neither, so everything that touches the document lives in a
/// thread-local registry. wasm is single-threaded, so nothing is lost.
pub struct WebA11y {
    #[cfg(target_arch = "wasm32")]
    key: usize,
}

impl WebA11y {
    /// Mirror the app drawn on the `<canvas>` with this id.
    ///
    /// That is [`crate::Options::canvas_id`], which is what the runner passes
    /// to eframe.
    pub fn new(canvas_id: impl Into<String>) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                key: web::register(canvas_id.into()),
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _: String = canvas_id.into();
            Self {}
        }
    }
}

impl egui::Plugin for WebA11y {
    fn debug_name(&self) -> &'static str {
        "egui_react_web_a11y"
    }

    #[cfg(target_arch = "wasm32")]
    fn setup(&mut self, ctx: &egui::Context) {
        // egui builds no AccessKit tree until someone asks for one, on every
        // platform including this one.
        ctx.enable_accesskit();
        web::with_mirror(self.key, |mirror| mirror.attach(ctx));
    }

    #[cfg(target_arch = "wasm32")]
    fn output_hook(&mut self, ctx: &egui::Context, output: &mut egui::FullOutput) {
        let platform_output = &mut output.platform_output;

        // `output_hook` runs once per *pass*, and egui runs a pass again from
        // the top when it is discarded; egui-react's taffy layout normally
        // takes two passes (ARCHITECTURE 5.3). The tree of a pass that is
        // about to be redone describes a layout that will never be drawn, so
        // drop it. This is the same test `Context::run` breaks its loop on —
        // `Context::will_discard` cannot be used here, because `end_pass` has
        // already taken the viewport's output by the time a plugin sees it.
        let max_passes = ctx.options(|options| options.max_passes.get());
        let redone = platform_output.requested_discard()
            && platform_output.num_completed_passes < max_passes;
        if redone {
            platform_output.accesskit_update = None;
            return;
        }

        let Some(update) = platform_output.accesskit_update.take() else {
            return;
        };
        // eframe pulls the canvas's focus back every frame, so this is "is the
        // app focused" rather than "is a mirror element focused".
        let is_focused = ctx.input(|input| input.focused);
        let pixels_per_point = f64::from(ctx.pixels_per_point());
        web::with_mirror(self.key, |mirror| {
            mirror.frame(update, is_focused, pixels_per_point);
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        // Everything a screen reader asked for since the last frame. egui
        // handles these where the widgets are: a `Click` on a button is the
        // same as a real one, `SetValue` moves a slider, `Focus` moves the
        // keyboard focus.
        web::take_actions(self.key, |request| {
            input
                .events
                .push(egui::Event::AccessKitActionRequest(request));
        });
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use accesskit_web::Adapter;
    use egui::accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
    use std::cell::RefCell;
    use std::rc::Rc;

    // Everything that is `!Send`: one entry per registered plugin.
    thread_local! {
        static MIRRORS: RefCell<Vec<Mirror>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn register(canvas_id: String) -> usize {
        MIRRORS.with_borrow_mut(|mirrors| {
            mirrors.push(Mirror {
                canvas_id,
                canvas: None,
                adapter: None,
                host_focused: None,
                actions: Rc::default(),
            });
            mirrors.len() - 1
        })
    }

    pub(super) fn with_mirror(key: usize, f: impl FnOnce(&mut Mirror)) {
        MIRRORS.with_borrow_mut(|mirrors| {
            if let Some(mirror) = mirrors.get_mut(key) {
                f(mirror);
            }
        });
    }

    /// Requests arrive from DOM listeners, which fire between frames and
    /// cannot reach into the registry the adapter itself is stored in. The
    /// queue is shared with them instead, so draining it never re-enters
    /// `MIRRORS`.
    type Actions = Rc<RefCell<Vec<ActionRequest>>>;

    pub(super) struct Mirror {
        canvas_id: String,
        canvas: Option<web_sys::Element>,
        adapter: Option<Adapter>,
        host_focused: Option<bool>,
        actions: Actions,
    }

    /// Hand egui everything assistive technology asked for since last frame.
    pub(super) fn take_actions(key: usize, mut push: impl FnMut(ActionRequest)) {
        let actions =
            MIRRORS.with_borrow(|mirrors| mirrors.get(key).map(|m| Rc::clone(&m.actions)));
        let Some(actions) = actions else {
            return;
        };
        for request in actions.borrow_mut().drain(..) {
            push(request);
        }
    }

    impl Mirror {
        /// Put the mirror next to the canvas, so that the two share a
        /// containing block and one offset lines them up.
        pub(super) fn attach(&mut self, ctx: &egui::Context) {
            if self.adapter.is_some() {
                return;
            }
            let canvas = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id(&self.canvas_id));
            let Some(canvas) = canvas else {
                log::error!(
                    "egui-react a11y: no <canvas id=\"{}\"> to mirror",
                    self.canvas_id
                );
                return;
            };
            let Some(parent) = canvas.parent_element() else {
                log::error!("egui-react a11y: the canvas has no parent element");
                return;
            };
            let queue = QueueActions {
                actions: Rc::clone(&self.actions),
                ctx: ctx.clone(),
            };
            // The canvas is both where the mirror is lined up and the one
            // element that holds the browser's focus: eframe pulls focus back
            // to it every frame, so the mirror says where the app's focus is
            // with `aria-activedescendant` instead of taking it.
            let Some(mut adapter) = Adapter::new(&parent, &canvas, NoActivation, queue) else {
                log::error!("egui-react a11y: no document to build the mirror in");
                return;
            };
            adapter.set_debug(debug_requested());
            self.canvas = Some(canvas);
            self.adapter = Some(adapter);
        }

        /// One frame: line the mirror up, then apply the tree diff.
        pub(super) fn frame(
            &mut self,
            update: TreeUpdate,
            is_focused: bool,
            pixels_per_point: f64,
        ) {
            let Some(adapter) = self.adapter.as_mut() else {
                return;
            };
            if self.host_focused != Some(is_focused) {
                self.host_focused = Some(is_focused);
                adapter.update_host_focus_state(is_focused);
            }
            if let Some(canvas) = &self.canvas {
                let rect = canvas.get_bounding_client_rect();
                adapter.set_viewport((rect.left(), rect.top()), pixels_per_point);
            }
            adapter.update_if_active(|| update);
        }
    }

    /// `?a11y-debug` in the URL shows the mirror: no transparency, and a green
    /// outline around every node.
    fn debug_requested() -> bool {
        web_sys::window()
            .and_then(|window| window.location().search().ok())
            .is_some_and(|search| search.contains("a11y-debug"))
    }

    /// The tree is pushed from `output_hook` every frame, so the adapter never
    /// has to ask for one.
    struct NoActivation;

    impl ActivationHandler for NoActivation {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            None
        }
    }

    /// The adapter's way out: park the request until egui next reads input.
    ///
    /// Holding a `Context` is what the `Plugin` docs warn against, but this is
    /// not the plugin — it is a DOM listener living in a thread-local, which
    /// the `Context` does not own, so there is no cycle. Without the repaint
    /// nothing would happen: egui draws on demand, and a screen reader
    /// activating an element is not an event it knows about.
    struct QueueActions {
        actions: Actions,
        ctx: egui::Context,
    }

    impl ActionHandler for QueueActions {
        fn do_action(&mut self, request: ActionRequest) {
            self.actions.borrow_mut().push(request);
            self.ctx.request_repaint();
        }
    }
}
