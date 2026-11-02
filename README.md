# atelier-plugin-api

The interface between [Atelier IDE](https://atelier-ide-org.github.io/docs/) and its native plugins: the `Plugin` trait a plugin implements, the `Host` the IDE gives it while it draws, the `declare_plugin!` macro that exports it from a dynamic library, and `Locales` for its texts.

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
atelier-plugin-api = { git = "https://github.com/atelier-ide-org/plugin-api", tag = "v0.1.0" }
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

So the plugins shipped with Atelier are built by its own CI, in one `cargo build` with the IDE: they are git submodules of `atelier-ide-org/app` (`plugins/<name>`), and each release of [`atelier-ide-org/releases`](https://github.com/atelier-ide-org/releases/releases) carries the plugins of that version. The IDE and its plugins must depend on **the same tag** of this repository.

## Versions

Change `ABI` whenever `Declaration` changes, and tag every version (`vX.Y.Z`): the IDE and the plugins pin a tag.
