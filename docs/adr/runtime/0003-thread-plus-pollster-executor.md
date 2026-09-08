# 0003: One thread and `pollster` as the native executor

Date: 2026-08 or earlier · Status: accepted

## Context

`use_future` and `spawn` need somewhere to run a future. On wasm that is `wasm_bindgen_futures::spawn_local` and there is no choice. On native there is: pull in an async runtime, or run the future on a thread.

## Decision

Native spawns one thread per future and drives it with `pollster::block_on`. No pool, no runtime.

## Rejected

- **Require tokio.** A large dependency and a reactor to own, for futures that mostly just wait. It would also force every app onto tokio's shape.

## Consequences

Enough for `ehttp` and file IO; CPU-heavy work should split off its own thread inside the future. An app that needs tokio calls `Handle::current().spawn(..).await` inside the future, so the door is not shut. If thread creation fails the future is dropped and `log::error!` is called — no panic, and the hook stays `Pending`. The platform difference is confined to the `SpawnFuture<T>` trait and `task::spawn` in core, so user types never vary by target.

## Links

- [ARCHITECTURE section 4, `use_future` details](../../ARCHITECTURE.md#use_future-details)
