//! One example, filling the canvas, picked by `location.hash`.
//!
//! This is what the documentation site puts in an iframe: the gallery's centre
//! column and nothing else — no list, no code pane, no header, because the page
//! around the iframe already carries all three. One binary serves every example
//! page, so a reader who walks the site downloads the wasm once.
//!
//! The hash is `#<name>` or `#<name>/plain` (`#todo`, `#counter/plain`). An
//! unknown or missing name falls back to the first example, and a `plain`
//! request for an example that has no plain version falls back to the
//! egui-reactor one — the same rule the gallery follows for its own toggle.
//!
//! `?theme=dark` or `?theme=light` after the name is the page's appearance
//! (`#board?theme=dark`): the page around the iframe has a switch of its own
//! and the example is drawn in the same colours. Without it, egui follows the
//! system, as it does anywhere else. The page changes only the hash when the
//! switch is flipped, so the example keeps running and picks it up here.

use egui_reactor::prelude::*;
use egui_reactor_app::a11y::WebA11y;
use egui_reactor_app::{Options, run};
use egui_reactor_elements::prelude::*;
use gallery::{EXAMPLES, Running, find};

/// How much larger the embed draws than egui's default: the page's 16px body
/// over egui's 12.5-point body text.
const ZOOM: f32 = 16.0 / 12.5;

fn main() -> eframe::Result {
    let canvas_id = Options::default().canvas_id;
    run(
        Options {
            title: String::from("egui-reactor: example"),
            setup: Some(Box::new(move |cc| {
                // What the two wgpu examples need before anything is drawn: the
                // shader example's pipeline, and the buffer and layouts the
                // patch example builds its pipelines from. The embed can be
                // asked for either of them by the hash, so both are registered
                // here exactly as the gallery binary does.
                shader::gpu::setup(cc);
                patch::gpu::setup(cc);
                // The accessibility tree, mirrored into hidden DOM elements
                // over the canvas. Does nothing off the web; there, egui's tree
                // would otherwise be thrown away by eframe.
                cc.egui_ctx.add_plugin(WebA11y::new(canvas_id));
                // egui's body text is 12.5 points and the page around the
                // iframe sets its text at 16px, so an example at egui's own
                // scale reads as small print next to the notes. Zoom rather
                // than a bigger text style: spacing and widgets grow with the
                // text, which is what a reader on a page at 16px expects.
                cc.egui_ctx.set_zoom_factor(ZOOM);
                // The `Context` is only reachable here, and this is the one
                // place that runs before the first pass.
                watch_hash(&cc.egui_ctx);
            })),
            ..Default::default()
        },
        // The hash is read one level in, inside the view rather than beside it:
        // `rsx!` expands to a closure that borrows what it reads, and the view
        // the root hands back may not borrow the frame it was built in.
        move |_cx| {
            view(|cx| {
                // Read every pass, not once: the page swaps examples and
                // themes by assigning a new hash, and this is where that is
                // noticed.
                let (name, plain, theme) = requested();
                if let Some(theme) = theme
                    && cx.ctx().theme() != theme
                {
                    cx.ctx().set_theme(theme);
                }
                rsx! {
                    // The `key` is what makes a swap clean: change it and the
                    // previous example's hooks are unreachable, so the pass-end
                    // sweep drops them and the new example starts from nothing.
                    <View key={name} direction="column" grow={1.0}>
                        <Running name={name} plain={plain}/>
                    </View>
                }
                .show(cx);
            })
        },
    )
}

/// The example the hash (web) or the first argument (native) asks for,
/// whether it is the plain egui version, and the theme the page is in, if it
/// said.
fn requested() -> (&'static str, bool, Option<egui::Theme>) {
    let asked = location().unwrap_or_default();
    let (which, query) = asked.split_once('?').unwrap_or((asked.as_str(), ""));
    let (name, version) = which.split_once('/').unwrap_or((which, ""));
    let meta = find(name).unwrap_or(&EXAMPLES[0]);
    let theme = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("theme="))
        .and_then(|theme| match theme {
            "dark" => Some(egui::Theme::Dark),
            "light" => Some(egui::Theme::Light),
            _ => None,
        });
    (meta.name, version == "plain" && meta.plain.is_some(), theme)
}

#[cfg(target_arch = "wasm32")]
fn location() -> Option<String> {
    let hash = web_sys::window()?.location().hash().ok()?;
    Some(hash.trim_start_matches('#').to_owned())
}

/// Natively there is no address bar, so the first argument names the example:
/// `cargo run -p gallery --bin embed counter/plain` or
/// `cargo run -p gallery --bin embed 'board?theme=light'`.
#[cfg(not(target_arch = "wasm32"))]
fn location() -> Option<String> {
    std::env::args().nth(1)
}

/// Repaint when the hash changes.
///
/// egui repaints on input and on a timer it sets itself, and a hash change is
/// neither: without this the canvas would keep drawing the old example until
/// the pointer moved over it. The listener lives as long as the page does, so
/// the closure is leaked rather than kept somewhere to be dropped.
#[cfg(target_arch = "wasm32")]
fn watch_hash(ctx: &egui::Context) {
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::prelude::Closure;

    let Some(window) = web_sys::window() else {
        return;
    };
    let ctx = ctx.clone();
    let listener = Closure::<dyn FnMut()>::new(move || ctx.request_repaint());
    if window
        .add_event_listener_with_callback("hashchange", listener.as_ref().unchecked_ref())
        .is_ok()
    {
        listener.forget();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn watch_hash(_ctx: &egui::Context) {}
