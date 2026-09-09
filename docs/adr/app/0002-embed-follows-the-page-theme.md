# 0002: The embed follows the page's theme through the hash

Date: 2026-09-09 · Status: accepted

## Context

The site has an appearance switch in its nav bar, and the example in the iframe did not follow it: egui followed the system, and two examples (`board`, `notes`) kept a theme of their own with a button to flip it. A light page could hold a dark example, and the example's own button changed the example and nothing around it. The page and the embed are two documents, so the page's theme has to be handed over on purpose.

## Decision

The page writes its appearance into the hash it already uses to pick the example (`/embed/#board?theme=dark`), and the embed calls `Context::set_theme` when that differs from what egui is in. Flipping the switch changes only the hash, which is a fragment navigation: the example keeps running and the `hashchange` listener repaints it. Examples with colours of their own read egui's theme instead of keeping one, and none has a switch.

## Rejected

- **`postMessage` from the page to the iframe.** Needs a listener in the wasm and a handshake for the message that arrives before the app is running, and the page's `load` event fires before the wasm does. The hash is read on every pass already.
- **Restarting the iframe on a theme switch** (the theme in the iframe's `key`). Loses the example's state for a colour change.
- **Leaving egui on the system theme.** The page's switch would change everything but the example.

## Consequences

- The iframe's `src` carries the theme and the theme is only known in the browser, so the iframe is rendered inside `<ClientOnly>`: a server-rendered `src` is not patched on hydration.
- An example may not call `set_visuals` or `set_theme` itself; that would fight the page. `board` and `notes` read `Context::theme` at the top of their tree and provide it from there.
- Natively and on an embed opened without the query, egui follows the system as before.

## Links

- [ARCHITECTURE section 12, Website](../../ARCHITECTURE.md#12-website)
