# 0003: Persisted-state keys keep the pre-rename name

Date: 2026-09-09 · Status: accepted

## Context

The crates were renamed `egui-react` to `egui-reactor`. The rename was mechanical, so it also rewrote two constants in `egui-reactor-app`: `STORAGE_KEY`, the single eframe storage key everything `use_persisted` is written under, and `ROOT_ID`, the root egui id that scopes the egui memory saved with it (scroll offsets and the like). Both are keys into data users have already saved. Changing them reads back as an empty store: the site keeps working, everything anyone had saved is gone, and nothing reports it.

## Decision

An identifier that keys persisted state keeps its name across a crate rename. `STORAGE_KEY` stays `"egui_react"` and `ROOT_ID` stays `"egui_react_root"`, each with a comment saying why.

## Rejected

- **Rename, and read the old key as a fallback when the new one is empty.** A migration path for data that was never written under the new name — the new key has never shipped. It buys nothing and leaves a branch to carry forever.
- **Rename and accept the loss.** The site has real users with saved state. A rename of our crates is not a reason to drop it.

## Consequences

- The constant names no longer match the crate name. A reader who greps for `egui_reactor` will not find them; the comment on each is what explains the gap.
- The native storage directory is not pinned. eframe derives it from the window title, which the rename did change, so examples on a developer's machine reset once. That was accepted: it is dev-machine state, not the published site.
- `persisted_id`'s `"egui_reactor_persisted"` was renamed and stays renamed. That id keys the store's in-memory slot map; the saved JSON is keyed by the user's own `use_persisted` key, so it never reaches storage.

## Links

- [ARCHITECTURE section 4, `use_persisted` details](../../ARCHITECTURE.md#use_persisted-details)
- `crates/egui-reactor-app/src/lib.rs`, `STORAGE_KEY` and `ROOT_ID`
