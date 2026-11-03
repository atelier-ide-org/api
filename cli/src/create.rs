//! `plugin create`: a new plugin, from the templates of `templates/`, ready to compile.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The tag of atelier-ide-org/api a new plugin depends on.
pub const API_TAG: &str = "v0.4.0";

/// The files of a new plugin, by path, with `{{id}}`, `{{name}}`, `{{library}}`, `{{type}}` and
/// `{{api_tag}}` to replace.
const FILES: [(&str, &str); 13] = [
    ("Cargo.toml", include_str!("../templates/Cargo.toml.txt")),
    ("src/lib.rs", include_str!("../templates/lib.rs.txt")),
    ("src/i18n.rs", include_str!("../templates/i18n.rs.txt")),
    (
        "locales/en_US.json",
        include_str!("../templates/en_US.json.txt"),
    ),
    (
        "locales/fr_FR.json",
        include_str!("../templates/fr_FR.json.txt"),
    ),
    ("doc/en-US.md", include_str!("../templates/doc-en.md.txt")),
    ("doc/fr-FR.md", include_str!("../templates/doc-fr.md.txt")),
    (
        "extensions/README.md",
        include_str!("../templates/extensions-README.md.txt"),
    ),
    ("README.md", include_str!("../templates/README.md.txt")),
    (".gitignore", "/target\n"),
    (
        ".github/workflows/ci.yml",
        include_str!("../templates/workflows/ci.yml"),
    ),
    (
        ".github/workflows/release.yml",
        include_str!("../templates/workflows/release.yml"),
    ),
    ("plugin.json", PLUGIN_JSON),
];

const PLUGIN_JSON: &str = r#"{
  "id": "{{id}}",
  "name": "{{name}}",
  "version": "0.1.0",
  "description": "What {{name}} does, in one sentence.",
  "library": "{{library}}",
  "abi": 2,
  "extensions": []
}
"#;

/// The identifier of a plugin from the name given (`My viewer` is `my-viewer`).
pub fn identifier(name: &str) -> Result<String, String> {
    let mut id = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if (c == '-' || c == '_' || c.is_whitespace()) && !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_matches('-').to_string();
    match id.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => Ok(id),
        _ => Err(format!(
            "{name}: the name of a plugin starts with a letter (letters, digits and -)"
        )),
    }
}

/// `MyViewer` for `my-viewer`.
fn type_name(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<String>()
        + "Plugin"
}

/// `My Viewer` for `my-viewer`.
fn display_name(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn fill(template: &str, id: &str) -> String {
    template
        .replace("{{id}}", id)
        .replace("{{name}}", &display_name(id))
        .replace("{{library}}", &id.replace('-', "_"))
        .replace("{{type}}", &type_name(id))
        .replace("{{api_tag}}", API_TAG)
}

/// Writes a new plugin in `dir/<id>`. Returns its folder.
pub fn create(name: &str, dir: &Path) -> Result<PathBuf, String> {
    let id = identifier(name)?;
    let folder = dir.join(&id);
    if folder.exists() {
        return Err(format!("{} already exists", folder.display()));
    }
    for (path, template) in FILES {
        let file = folder.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&file, fill(template, &id)).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Ok(folder)
}
