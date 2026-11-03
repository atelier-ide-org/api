# atelier-plugin-api

The interface between [Atelier IDE](https://atelier-ide-org.github.io/docs/) and its native plugins.

A native plugin is a dynamic library (`.dll`, `.so`, `.dylib`) the IDE loads once the user installed it (Plugins, Available, or the + button for a file). The IDE and the library share **only C types and functions** (`abi`): no Rust type crosses the boundary. So a plugin builds alone, with any compiler and any crates, and loads in every version of Atelier that reads the same version of the interface (`abi::VERSION`, today **2**).

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
atelier-plugin-api = { git = "https://github.com/atelier-ide-org/api", tag = "v0.4.0" }
```

```rust
use atelier_plugin_api::{Host, Plugin, Surface, egui, protocol, ui};

#[derive(Default)]
struct Viewer;

impl Plugin for Viewer {
    fn info(&mut self, _language: &str) -> protocol::Info {
        protocol::Info {
            file_types: vec![protocol::FileType {
                extensions: vec!["xyz".into()],
                binary: true,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, surface: Surface<'_>, _host: &Host) {
        if let Surface::Preview(path) = surface {
            let t = ui::theme(ui.ctx());
            ui.colored_label(t.text, path.display().to_string());
        }
    }
}

atelier_plugin_api::export_plugin!(Viewer::default);
```

The plugin's `plugin.json` names the library, the version of the interface and the extensions of the files it handles, whose icons are in its `extensions/` folder (`xyz.svg`, or `a+b.svg` for several):

```json
{ "id": "viewer", "name": "Viewer", "version": "1.0.0", "description": "…", "library": "viewer", "abi": 2, "extensions": ["xyz"] }
```

## atelier-cli

The command line of the plugins, in `cli/`:

```sh
cargo install --git https://github.com/atelier-ide-org/api atelier-cli
atelier-cli plugin create "My viewer"        # ./my-viewer: a plugin ready to build, with its CI and release
atelier-cli plugin compile [<path>] [--debug] [--target <triple>] [--out <folder>]
atelier-cli plugin merge a.atelier-plugin b.atelier-plugin --out all.atelier-plugin
```

`compile` builds the plugin and makes the file Atelier installs (Plugins, the + button), `<id>.atelier-plugin`: its `plugin.json` (with the version of `Cargo.toml`), its `extensions/` icons and its library for the system, in `<os>-<arch>/`. The release workflow of a plugin builds it for each system and joins the files.

## How a plugin draws

A plugin draws with **its own egui** (feature `egui`, on by default), of any version. Each of its surfaces (the preview of a file in an editor tab, a tool window) has an `egui::Context` of its own:

1. each frame, the IDE sends the input of the surface (pointer, wheel, keys and text when the surface has the keyboard, the size of the window and of the surface, the time);
2. the plugin runs its egui on it (`Plugin::ui`) and gives back what to paint: meshes in the coordinates of the window, the textures they use (its font atlas, its pictures), and where its windows and menus float;
3. the IDE paints the meshes in the surface, and the floating ones above itself, where they take the pointer.

The look follows the IDE: it sends its egui style (as JSON), its colors (`ui::theme`) and its fonts, and `ui` has its widgets (icon buttons, menus, rows, dialogs) with the icons of the IDE, which the IDE draws for the plugin (`ui::icon`); a plugin adds its own with `ui::register_icon`. With the feature `glow`, a surface may paint with OpenGL (`gl::paint_callback`): the IDE calls the plugin back with its context current, and the plugin loads its own `glow` from it.

Anything else goes through `call`, by name, with JSON (`protocol`): what the plugin brings (`info`: tool windows, commands, file types, icons), the commands the user runs, the files closed; and from the plugin to the IDE (`Host`): notifications, opening a file or a tool window, the project, the folder of the settings. A side that does not know a method answers nothing, so both add methods without a new version.

## Rules of the interface

- Strings are UTF-8, as pointer and length, valid for the time of the call. What one side allocates, it frees (`Buffer`, given back to the `free` of its maker).
- A panic never crosses the interface: the plugin catches it, and is then marked failed.
- The structures start with their `size`; they only grow at their end. A change that older plugins or IDEs cannot follow takes a new `abi::VERSION`.
- Without features, the crate depends on nothing: the IDE enables `serde` alone, for the messages of `protocol`.

## Versions

Tag every version (`vX.Y.Z`): plugins depend on a tag. Plugins: [plugin-gltf-viewer](https://github.com/atelier-ide-org/plugin-gltf-viewer), [plugin-database](https://github.com/atelier-ide-org/plugin-database), [plugin-msoffice-viewer](https://github.com/atelier-ide-org/plugin-msoffice-viewer).

Version 1 of the interface (0.2.x of this crate, Atelier 0.2) shared Rust types with the IDE: a plugin had to be built like the IDE, with its build kit (the `kits` branch). It is no longer read.
