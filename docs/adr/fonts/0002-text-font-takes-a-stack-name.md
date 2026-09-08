# 0002: `<Text font>` takes the name of a stack, as a string

Date: 2026-09-07 · Status: accepted

## Context

Once an app can register named font stacks ([0001](0001-css-font-chains-through-fontdb.md)), a `<Text>` has to say which one to draw in. The registry lives in `egui-react-app`; `<Text>` lives in `egui-react-elements`, which knows nothing about it.

## Decision

`font: Option<&str>`. The value is the name of a registered stack, which becomes `FontFamily::Name`, or `"proportional"` / `"monospace"` for egui's own two. The name is checked against `Context::fonts(|f| f.definitions())` each pass; an unknown name falls back to the style's family and warns once per name.

## Rejected

- **A typed handle returned by `Fonts::stack`.** The elements crate would have to depend on the app crate, or every component would have to carry the handle down. It also does not match what it wraps: egui's own font families are string keys, and CSS `font-family` is a string.

## Consequences

A typo in a stack name is a runtime warning, not a compile error. The check is needed, not optional: a `FontFamily::Name` that is not in the definitions panics inside epaint at layout, so the lookup is what keeps a typo from killing the app. It costs one `BTreeMap::contains_key` under the fonts lock per `<Text font>` per pass; a per-pass cache is the fallback if a profile ever shows it, and would have to be invalidated whenever a source arrives and swaps the definitions. Other widgets take `impl Into<WidgetText>`, so their route is `default_proportional` or a `RichText::new(..).family(..)` child.

## Links

- [ARCHITECTURE section 6, elements list](../../ARCHITECTURE.md#elements-list-egui-react-elements)
- [ARCHITECTURE section 8, fonts](../../ARCHITECTURE.md#8-platforms)
- `docs/tasks/font/plan.md` (2.3, section 5)
- Commit `3bbc057`
