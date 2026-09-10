# 0001: Layout runs on taffy, not egui_flex

Date: 2026-08 or earlier · Status: accepted

## Context

Flexbox and Grid are a stated goal, not an add-on: `<View>` is meant to be a layout node the way it is in React Native. egui's own layout is a cursor, so the choice was which flexbox implementation to put on top of it. There were two: `egui_flex`, written for egui, and `taffy`, the engine Dioxus and Bevy use.

## Decision

Layout runs on taffy, reached through a layout engine of our own. `<View>` is a taffy node and egui widgets are leaves.

## Rejected

- **egui_flex.** It stops at egui 0.35, has no `justify-content`, and its own README says `flex-shrink` is structurally impossible. Not enough for a first-class citizen, and slow to track egui releases.

## Consequences

`egui-reactor` depends on taffy 0.9 directly, with the feature set egui_taffy 0.14 pulled in kept as it was so layouts come out the same. taffy is pure Rust and builds on wasm as-is. The layout attributes (`ItemStyle`, `ContainerStyle`) are a thin CSS-spelled surface over `taffy::Style`. The engine itself came later; see [0002](0002-own-engine-over-taffy.md).

## Links

- [ARCHITECTURE section 6](../../ARCHITECTURE.md#6-layout)
- [ARCHITECTURE section 6, layout attributes](../../ARCHITECTURE.md#layout-attributes)
