# {{name}} (`{{id}}`)

A native plugin of [Atelier IDE](https://atelier-ide-org.github.io/docs/), made with `atelier-cli plugin create`.

```sh
cargo test
atelier-cli plugin compile           # target/release/{{id}}.atelier-plugin, for this system
atelier-cli plugin compile --debug   # target/debug/{{id}}.atelier-plugin
```

Install the file in Atelier: **Plugins**, **Install a plugin from a file** (the + button).

- `plugin.json`: its identifier, name, description, library, the version of the plugin interface (`abi`) and the extensions of the files it handles.
- `extensions/`: the icons of those files (`a+b.svg`).
- `src/lib.rs`: the plugin (`atelier_plugin_api::Plugin`), drawn with its own egui.
- `locales/`, `doc/`: its texts and its documentation page.

Publishing: in a repository of atelier-ide-org (`plugin-{{id}}`), raising the version of `Cargo.toml` on `main` publishes it as the release `plugins/{{id}}/vX.Y.Z` of atelier-ide-org/releases (`.github/workflows/release.yml`), and Atelier lists it in **Plugins › Marketplace**.
