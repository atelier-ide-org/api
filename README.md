# atelier-plugin-api

The interface between [Atelier IDE](https://atelier-ide-org.github.io/docs/) and its native plugins: the `Plugin` trait a plugin implements, the `Host` the IDE gives it while it draws, the `declare_plugin!` macro that exports it from a dynamic library, and `Locales` for its texts.

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
atelier-plugin-api = { git = "https://github.com/atelier-ide-org/api", tag = "v0.2.0" }
```

```rust
use atelier_plugin_api::{Host, Plugin, egui};
use std::path::Path;

#[derive(Default)]
struct Viewer;

impl Plugin for Viewer {
    fn opens_binary(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "xyz")
    }
    fn preview(&mut self, ui: &mut egui::Ui, path: &Path, _text: &mut String, host: &dyn Host) {
        ui.colored_label(host.theme().text, path.display().to_string());
    }
}

atelier_plugin_api::declare_plugin!(Viewer::default);
```

## A library is tied to one build of the IDE

Rust has no stable ABI, and the plugin shares egui with the IDE: the library must be built by the same compiler, for the same target and profile, from the same versions and features of the crates. The IDE checks it (`ATELIER_RUSTC` and `fingerprint()`) before calling anything in the library, and refuses one built otherwise.

So a plugin is built for one version of the IDE, with what that version shares in the **`kits` branch** of this repository: `vX.Y.Z/` holds its `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml` (no code), pushed by the release of the IDE. The CI of a plugin makes an empty package of it, puts the plugin in its `plugins/` folder (a member of its workspace) and runs `cargo build --release --workspace --target <target>`, as the IDE was built: the crates they share come out the same. It then publishes `<name>.atelier-plugin` as the release `plugins/<name>/vX.Y.Z` of [`atelier-ide-org/releases`](https://github.com/atelier-ide-org/releases/releases), which Atelier installs from Plugins, Install a plugin from a file. See [plugin-gltf-viewer](https://github.com/atelier-ide-org/plugin-gltf-viewer) and its `.github/workflows/release.yml`.

The IDE and its plugins must depend on **the same tag** of this repository, by the same URL.

## Dependencies

This crate depends on eframe alone, and must stay so. The identity of a crate includes that of its dependencies: if it depended on, say, `serde_json`, a plugin that enables a feature of `serde_json` (as `gltf` does) would change it, and with it the fingerprint, so that the plugin would no longer load in the IDE built without it.

## Versions

Change `ABI` whenever `Declaration` changes, and tag every version (`vX.Y.Z`): the IDE and the plugins pin a tag.
