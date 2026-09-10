# Dev commands

_default:
    @just --list

# Build the workspace (native).
build:
    cargo build --workspace

# Build the published site: VitePress pages + embed wasm + cargo doc.
build-site:
    bash site/build.sh

# Test the workspace.
test:
    cargo test --workspace

# fmt + clippy + test, the way CI runs them.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

# Site with both halves live: vitepress for the pages, trunk for the wasm.
dev:
    bash site/dev.sh

# One example in the browser, rebuilt on change: `just example counter`.
example name:
    trunk serve --config examples/{{name}}/Trunk.toml --open
