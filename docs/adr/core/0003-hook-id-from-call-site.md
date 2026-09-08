# 0003: A hook's Id is its call-site stack, and collisions are reported

Date: 2026-08 or earlier · Status: accepted

## Context

Hook state lives in a store keyed by `Id`, so a hook needs an identity that is the same every frame. React uses call order, which is why it forbids hooks inside `if`. egui already keys its own memory by a hierarchical `Id`, so the same idea was available: identity from position, not from order.

## Decision

A hook's Id is `scope.with(Location::caller())`. `#[hook]` pushes a custom hook's own call site onto the stack, so nested hooks stay unique at any depth. Requesting the same Id twice in one pass is a collision: it panics if the first guard is still alive, and otherwise warns in the log and on screen in debug builds.

## Rejected

- **Mix the occurrence count at the same call site into the Id.** Collisions disappear, but when the structure changes state silently moves to another instance — the same damage a React rules-of-hooks violation does. It cannot even be detected, because a changing occurrence count inside a loop is legitimate use. That trades a loud error for a silent bug.

## Consequences

`use_state` inside `if` is allowed. A custom hook without `#[hook]`, or a `for` without `key`, is a collision with a message that names the fix. Because the Id carries `file:line:column`, editing code changes it, so persisted state cannot use it: `use_persisted` takes an explicit string key.

## Links

- [ARCHITECTURE section 3.4](../../ARCHITECTURE.md#34-id-derivation-and-collision-detection)
- [ARCHITECTURE section 2.2](../../ARCHITECTURE.md#22-state-lives-in-a-store-keyed-by-id)
