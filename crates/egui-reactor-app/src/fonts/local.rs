//! Local Font Access: the fonts installed on the user's machine, read from a
//! browser.
//!
//! wasm cannot read the OS font directory, so a `System("Hiragino Sans")`
//! entry resolves to nothing on the web unless something puts that font's
//! bytes into the database. The Local Font Access API is the one route to
//! them: `window.queryLocalFonts()` lists every installed face, and each
//! face's `blob()` is the whole font file. Facts that shape the code:
//!
//! - Chromium only (Chrome / Edge 103 and later); Firefox and Safari have
//!   not shipped it. [`Fonts::local_fonts_available`] is the feature test.
//! - Secure context, a permission prompt on the first `query()`, and the
//!   call needs transient user activation: it has to be made from a click
//!   handler, so [`Fonts::request_local_fonts`] is `async` and meant for
//!   `wasm_bindgen_futures::spawn_local` inside an `on_click`.
//! - `blob()` is the whole file, and a `.ttc` such as Hiragino Sans is tens
//!   of megabytes. So a blob is read only for a family some stack asked for,
//!   one per family (the file has the other faces), never for everything the
//!   query returned.
//! - web-sys 0.3.104 has a `FontData` binding, but the whole type sits
//!   behind `--cfg=web_sys_unstable_apis`, a flag every user of this crate
//!   would then need, and `Window` has no `queryLocalFonts` binding at all.
//!   So the API is reached with `js_sys::Reflect`: four property reads and
//!   two calls, and nothing unstable to opt into.
//!
//! After a grant, the same `System(name)` entry that native resolves through
//! `load_system_fonts` resolves here through the browser: same model, same
//! report. On every target but wasm32 the three methods exist and answer
//! `false` / `None` / `Ok(0)`, so an example is one source file.

use super::{FontSource, Fonts};

/// The state of the `local-fonts` permission, without prompting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalFontsPermission {
    Granted,
    Denied,
    /// Not decided yet: the next `query()` will show the prompt.
    Prompt,
}

/// Why [`Fonts::request_local_fonts`] failed: the API is missing, the user
/// declined, or the browser threw. The message is the browser's, as text, so
/// the type is the same on every target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalFontsError(pub String);

impl std::fmt::Display for LocalFontsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LocalFontsError {}

#[cfg(target_arch = "wasm32")]
impl From<wasm_bindgen::JsValue> for LocalFontsError {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        use wasm_bindgen::JsCast as _;
        let message = match value.dyn_ref::<js_sys::Error>() {
            Some(err) => String::from(err.message()),
            None => value.as_string().unwrap_or_else(|| format!("{value:?}")),
        };
        Self(message)
    }
}

impl Fonts {
    /// Whether this browser has `window.queryLocalFonts`. Always `false` off
    /// wasm.
    pub fn local_fonts_available() -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            web::available()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }
    }

    /// The `local-fonts` permission state, without prompting; `None` when
    /// the browser cannot say (or off wasm). For a button's label.
    pub async fn local_fonts_permission() -> Option<LocalFontsPermission> {
        #[cfg(target_arch = "wasm32")]
        {
            web::permission().await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            None
        }
    }

    /// Ask the browser for the installed fonts the stacks name, load them,
    /// and apply again. Returns how many of those families were found.
    ///
    /// Call it from a click handler:
    ///
    /// ```ignore
    /// let fonts = fonts.clone();
    /// wasm_bindgen_futures::spawn_local(async move {
    ///     match fonts.request_local_fonts().await {
    ///         Ok(n) => log::info!("{n} local families loaded"),
    ///         Err(err) => log::warn!("local fonts: {err}"),
    ///     }
    /// });
    /// ```
    ///
    /// The first call shows the permission prompt; a refusal is an `Err`.
    /// Off wasm it is `Ok(0)`: native reads the same fonts through
    /// `load_system_fonts` on the first `apply`.
    pub async fn request_local_fonts(&self) -> Result<usize, LocalFontsError> {
        let wanted = self.wanted_system_families();
        if wanted.is_empty() {
            return Ok(0);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let blobs = web::query(&wanted).await?;
            let mut inner = self.lock();
            let mut families = 0;
            for (family, bytes) in blobs {
                let ids = inner
                    .db
                    .load_font_source(fontdb::Source::Binary(std::sync::Arc::new(bytes)));
                if ids.is_empty() {
                    log::warn!(
                        "egui-reactor fonts: local font {family:?}: fontdb could not parse it"
                    );
                } else {
                    families += 1;
                }
            }
            if families > 0 {
                inner.reapply();
                inner.repaint();
            }
            Ok(families)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(0)
        }
    }

    /// The family names the stacks ask the device for, deduplicated: what
    /// blobs are read for, and nothing else.
    fn wanted_system_families(&self) -> Vec<String> {
        let inner = self.lock();
        let mut names: Vec<String> = Vec::new();
        for stack in &inner.stacks {
            for source in &stack.chain {
                if let FontSource::System(name) = source
                    && !names.contains(name)
                {
                    names.push(name.clone());
                }
            }
        }
        names
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::LocalFontsPermission;
    use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;

    /// `window.queryLocalFonts`, when the browser has it.
    ///
    /// That is the shipped shape of the API (Chrome 103 and later): a method
    /// on `Window`. The `navigator.fonts.query()` of the early drafts never
    /// shipped, and a check for it is `false` in every browser.
    fn query_local_fonts() -> Option<Function> {
        let window = web_sys::window()?;
        Reflect::get(&window, &JsValue::from_str("queryLocalFonts"))
            .ok()?
            .dyn_into()
            .ok()
    }

    pub(super) fn available() -> bool {
        query_local_fonts().is_some()
    }

    pub(super) async fn permission() -> Option<LocalFontsPermission> {
        let permissions = web_sys::window()?.navigator().permissions().ok()?;
        let descriptor = Object::new();
        Reflect::set(
            &descriptor,
            &JsValue::from_str("name"),
            &JsValue::from_str("local-fonts"),
        )
        .ok()?;
        let status = JsFuture::from(permissions.query(&descriptor).ok()?)
            .await
            .ok()?;
        let status: web_sys::PermissionStatus = status.dyn_into().ok()?;
        match status.state() {
            web_sys::PermissionState::Granted => Some(LocalFontsPermission::Granted),
            web_sys::PermissionState::Denied => Some(LocalFontsPermission::Denied),
            web_sys::PermissionState::Prompt => Some(LocalFontsPermission::Prompt),
            _ => None,
        }
    }

    /// A string property of a JS object, or empty.
    fn string_of(object: &JsValue, name: &str) -> String {
        Reflect::get(object, &JsValue::from_str(name))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default()
    }

    /// Call a zero-argument method that returns a promise, and await it.
    async fn call_async(object: &JsValue, name: &str) -> Result<JsValue, JsValue> {
        let method: Function = Reflect::get(object, &JsValue::from_str(name))?.dyn_into()?;
        let promise: Promise = method.call0(object)?.dyn_into()?;
        JsFuture::from(promise).await
    }

    /// `window.queryLocalFonts()`, then one blob per wanted family.
    pub(super) async fn query(wanted: &[String]) -> Result<Vec<(String, Vec<u8>)>, JsValue> {
        let query = query_local_fonts().ok_or_else(|| {
            JsValue::from_str("window.queryLocalFonts is not available in this browser")
        })?;
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let promise: Promise = query.call0(&window)?.dyn_into()?;
        let faces = JsFuture::from(promise).await?;
        let mut out: Vec<(String, Vec<u8>)> = Vec::new();
        for face in Array::from(&faces).iter() {
            let family = string_of(&face, "family");
            if !wanted.contains(&family) || out.iter().any(|(f, _)| *f == family) {
                continue;
            }
            let blob: web_sys::Blob = call_async(&face, "blob").await?.dyn_into()?;
            let buffer = JsFuture::from(blob.array_buffer()).await?;
            let bytes = Uint8Array::new(&buffer).to_vec();
            log::info!(
                "egui-reactor fonts: local font {family:?} ({}), {} KB",
                string_of(&face, "postscriptName"),
                bytes.len() / 1024
            );
            out.push((family, bytes));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{FontSource, Fonts};

    #[test]
    fn wanted_system_families_are_deduplicated() {
        let fonts = Fonts::new()
            .stack("a", [FontSource::System("X".into())])
            .stack(
                "b",
                [
                    FontSource::System("X".into()),
                    FontSource::System("Y".into()),
                ],
            );
        assert_eq!(fonts.wanted_system_families(), ["X", "Y"]);
    }

    /// Off wasm the three entry points answer without touching anything.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_stubs_answer_false_none_and_zero() {
        assert!(!Fonts::local_fonts_available());
        assert_eq!(pollster::block_on(Fonts::local_fonts_permission()), None);
        let fonts = Fonts::new().stack("a", [FontSource::System("X".into())]);
        assert_eq!(pollster::block_on(fonts.request_local_fonts()), Ok(0));
    }
}
