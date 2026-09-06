//! `provide_context` / `use_context`: two values handed down a tree without
//! being threaded through it.
//!
//! `Themed` owns a `Theme` and a `Locale` and provides both to its children.
//! `Page` and `Card` sit in between and take no props at all — they do not know
//! a theme or a locale exists. The leaves three levels down read them with
//! `use_context`, and `Toggles`, a child of the provider itself, writes them
//! back through the same `Handle`. Nothing in the middle carries a value it
//! does not use.
//!
//! `Orphan` at the bottom is drawn outside `Themed`, so its `use_context`
//! returns `None`: the binding lasts exactly as long as the children it was
//! provided to.
//!
//! Note the shape of `Themed`. `provide_context` takes its children as a
//! closure, and the `Handle` it publishes borrows the store, so the handle has
//! to be created in the same component body that provides it. A `<Provide
//! value={handle}>` element is not possible today — a component's props type
//! may not name the store's lifetime (ARCHITECTURE 6).

use example_meta::Meta;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

pub const META: Meta = Meta {
    name: "theme",
    summary: "Two values provided at the top and read three levels down, with nothing in between.",
    hooks: &[
        "use_handle",
        "use_state",
        "use_effect",
        "provide_context",
        "use_context",
    ],
    elements: &["View", "Text", "Button", "Frame", "Separator"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// One of the two provided values. A type of its own, because the context is
/// keyed by type: `use_context::<Theme>` finds this and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    pub fn name(self) -> &'static str {
        if self.dark { "dark" } else { "light" }
    }

    /// The swatch colour, so a leaf has something to show for the theme
    /// besides its name.
    pub fn swatch(self) -> egui::Color32 {
        if self.dark {
            egui::Color32::from_rgb(0x2f, 0x45, 0x6e)
        } else {
            egui::Color32::from_rgb(0xcf, 0xdd, 0xf5)
        }
    }
}

/// The other provided value.
///
/// English and French rather than the Japanese the plan asked for: egui's
/// bundled fonts are Hack, Ubuntu-Light and two emoji faces, none of which has
/// CJK glyphs, so Japanese would draw as empty boxes. Loading a font belongs in
/// an example about fonts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    Fr,
}

impl Locale {
    pub fn other(self) -> Self {
        match self {
            Self::En => Self::Fr,
            Self::Fr => Self::En,
        }
    }

    pub fn greeting(self) -> &'static str {
        match self {
            Self::En => "Hello",
            Self::Fr => "Bonjour",
        }
    }

    pub fn press(self) -> &'static str {
        match self {
            Self::En => "press me",
            Self::Fr => "appuyez ici",
        }
    }

    /// The label of the button that switches to the other language.
    pub fn switch(self) -> &'static str {
        match self {
            Self::En => "français",
            Self::Fr => "English",
        }
    }
}

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"theme"</Text>

            <Themed>
                <Toggles/>
                <Separator/>
                <Page/>
            </Themed>

            <Separator/>

            // Outside the provider.
            <Orphan/>
        </View>
    }
}

/// Owns both values and publishes them to its children.
///
/// `use_handle`, not `use_state`: a `Handle` is `Copy` and holds no borrow, so
/// it can be published to a subtree that then writes back through it. A guard
/// could do neither.
///
/// `shares_ui`, like `<Suspense>`: the provider draws nothing of its own, so
/// its children should become nodes of the parent's taffy tree rather than of
/// a tree of their own.
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme { dark: true });
    let locale = use_handle(cx, || Locale::En);

    // Follow the theme with egui's own visuals. Inside the gallery this
    // restyles the gallery too, because `set_visuals` is per `egui::Context`
    // and there is one of those, not one per example. That is what the egui
    // API does and the example does not hide it.
    let dark = theme.get().dark;
    let ctx = cx.ctx().clone();
    use_effect(cx, dark, move || {
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    });

    provide_context(cx, theme, |cx| {
        provide_context(cx, locale, |cx| children.show(cx));
    });
}

/// A child of the provider that writes back through it, the way a React
/// `useTheme()` hook hands out both the value and its setter.
#[component]
fn Toggles(cx: &mut Cx) {
    let Some(theme) = use_context::<Theme>(cx) else {
        return;
    };
    let Some(locale) = use_context::<Locale>(cx) else {
        return;
    };
    let dark = theme.get().dark;

    rsx! {
        <View direction="row" gap={8} align="center">
            <Button on_click={move || theme.set(Theme { dark: !dark })}>
                {if dark { "switch to light" } else { "switch to dark" }}
            </Button>
            <Button on_click={move || locale.set(locale.get().other())}>
                {locale.get().switch()}
            </Button>
        </View>
    }
}

/// Takes no props. It does not know a theme or a locale exists.
#[component]
fn Page(cx: &mut Cx) {
    rsx! {
        <View direction="column" gap={8}>
            <Text>"Page: passes nothing down."</Text>
            <Card/>
        </View>
    }
}

/// Also takes no props.
#[component]
fn Card(cx: &mut Cx) {
    rsx! {
        <Frame inner_margin={12.0}>
            <View direction="column" gap={8}>
                <Greeting/>
                <ThemedButton/>
                <Swatch/>
            </View>
        </Frame>
    }
}

/// Three levels below `Themed`, and the first thing here that reads a context.
#[component]
fn Greeting(cx: &mut Cx) {
    let locale = use_context::<Locale>(cx);
    let text = locale.map_or("(no locale)", |locale| locale.get().greeting());
    rsx! { <Text size={18.0}>{text}</Text> }
}

/// A localised button with a count of its own: a leaf can hold state as well as
/// read a context.
#[component]
fn ThemedButton(cx: &mut Cx) {
    let locale = use_context::<Locale>(cx);
    let mut clicks = use_state(cx, || 0u32);
    let label = locale.map_or("(no locale)", |locale| locale.get().press());

    rsx! {
        <View direction="row" gap={8} align="center">
            <Button on_click={|| *clicks += 1}>{label}</Button>
            <Text>{format!("pressed {} times", *clicks)}</Text>
        </View>
    }
}

/// Reads the other context. The mode is in a label so a test can read it back.
#[component]
fn Swatch(cx: &mut Cx) {
    let theme = use_context::<Theme>(cx).map_or(Theme { dark: true }, |theme| theme.get());

    rsx! {
        <View direction="row" gap={8} align="center">
            <Frame fill={theme.swatch()} inner_margin={8.0} corner_radius={4.0}>
                <Text>"swatch"</Text>
            </Frame>
            <Text>{format!("mode: {}", theme.name())}</Text>
        </View>
    }
}

/// Drawn outside the provider, so the lookup misses.
#[component]
fn Orphan(cx: &mut Cx) {
    let found = use_context::<Theme>(cx).is_some();
    rsx! {
        <Text>{if found { "outside: theme found" } else { "outside: no theme provided" }}</Text>
    }
}
