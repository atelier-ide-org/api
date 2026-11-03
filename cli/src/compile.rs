//! `plugin compile` and `plugin merge`: the file Atelier installs, `<id>.atelier-plugin`, a zip of
//!
//! ```text
//! plugin.json                       the manifest, with the version of Cargo.toml
//! extensions/<ext>+<ext>.svg        the icons of the files of the plugin, by extension
//! <os>-<arch>/<library>             the library of each system (windows-x86_64/viewer.dll…)
//! ```

use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// What `compile` reads in `plugin.json`.
pub struct Manifest {
    pub json: Value,
    pub id: String,
    pub library: String,
}

/// Reads `plugin.json`, and checks what Atelier needs in it.
pub fn manifest(dir: &Path) -> Result<Manifest, String> {
    let file = dir.join("plugin.json");
    let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let json: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let field = |name: &str| -> Result<String, String> {
        json.get(name)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("{}: \"{name}\" is required", file.display()))
    };
    let id = field("id")?;
    let library = field("library")?;
    for name in [&id, &library] {
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!(
                "{}: {name}: letters, digits, _ and - only",
                file.display()
            ));
        }
    }
    field("name")?;
    if json.get("abi").and_then(Value::as_u64).is_none() {
        return Err(format!(
            "{}: \"abi\" is required (the version of the plugin interface, 2)",
            file.display()
        ));
    }
    match json.get("extensions") {
        Some(Value::Array(list)) if list.iter().all(Value::is_string) => {}
        _ => {
            return Err(format!(
                "{}: \"extensions\" is required: the extensions of the files the plugin handles \
                 ([\"glb\", \"gltf\"]), [] when none",
                file.display()
            ));
        }
    }
    Ok(Manifest { json, id, library })
}

/// The version of the package of `Cargo.toml`.
fn cargo_version(dir: &Path) -> Result<String, String> {
    let file = dir.join("Cargo.toml");
    let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some(value) = line.strip_prefix("version")
            && let Some(value) = value.trim_start().strip_prefix('=')
        {
            return Ok(value.trim().trim_matches('"').to_string());
        }
    }
    Err(format!("{}: no version in [package]", file.display()))
}

/// The folder of a system in a plugin (`windows-x86_64`), and the file of `library` there, for
/// a target triple, or for this system.
pub fn platform(target: Option<&str>, library: &str) -> Result<(String, String), String> {
    let (os, arch) = match target {
        None => (
            std::env::consts::OS.to_string(),
            std::env::consts::ARCH.to_string(),
        ),
        Some(triple) => {
            let arch = triple.split('-').next().unwrap_or_default().to_string();
            let os = if triple.contains("windows") {
                "windows"
            } else if triple.contains("apple-darwin") {
                "macos"
            } else if triple.contains("linux") {
                "linux"
            } else {
                return Err(format!("{triple}: an unknown system"));
            };
            (os.to_string(), arch)
        }
    };
    let file = match os.as_str() {
        "windows" => format!("{library}.dll"),
        "macos" => format!("lib{library}.dylib"),
        _ => format!("lib{library}.so"),
    };
    Ok((format!("{os}-{arch}"), file))
}

fn zip_error(e: impl std::fmt::Display) -> String {
    format!("zip: {e}")
}

/// Where Cargo builds the plugin of `dir` (`target/`, unless a workspace or `CARGO_TARGET_DIR`
/// says otherwise).
fn target_dir(dir: &Path) -> Result<PathBuf, String> {
    let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(dir)
        .output()
        .map_err(|e| format!("cargo: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let json: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    json.get("target_directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata: no target_directory".into())
}

/// Builds the plugin of `dir` and makes its `.atelier-plugin`. Returns its path.
pub fn compile(
    dir: &Path,
    debug: bool,
    target: Option<&str>,
    out: Option<PathBuf>,
) -> Result<PathBuf, String> {
    let manifest = manifest(dir)?;
    let version = cargo_version(dir)?;
    let mut cargo = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cargo.arg("build").current_dir(dir);
    if !debug {
        cargo.arg("--release");
    }
    if let Some(target) = target {
        cargo.args(["--target", target]);
    }
    let status = cargo.status().map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err("cargo build failed".into());
    }
    let profile = if debug { "debug" } else { "release" };
    let mut built = target_dir(dir)?;
    if let Some(target) = target {
        built.push(target);
    }
    built.push(profile);
    let (platform, file) = platform(target, &manifest.library)?;
    let library = built.join(&file);
    let library = fs::read(&library).map_err(|e| format!("{}: {e}", library.display()))?;
    let out = out.unwrap_or(built);
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let package = out.join(format!("{}.atelier-plugin", manifest.id));
    let mut json = manifest.json;
    json["version"] = Value::String(version);
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        (
            "plugin.json".into(),
            serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?,
        ),
        (format!("{platform}/{file}"), library),
    ];
    if let Ok(icons) = fs::read_dir(dir.join("extensions")) {
        for icon in icons.flatten() {
            let name = icon.file_name().to_string_lossy().into_owned();
            if name.ends_with(".svg") {
                let bytes = fs::read(icon.path()).map_err(|e| format!("{name}: {e}"))?;
                entries.push((format!("extensions/{name}"), bytes));
            }
        }
    }
    write_zip(&package, &entries)?;
    Ok(package)
}

fn write_zip(file: &Path, entries: &[(String, Vec<u8>)]) -> Result<(), String> {
    let out = fs::File::create(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut zip = ZipWriter::new(out);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        zip.start_file(name.as_str(), options).map_err(zip_error)?;
        zip.write_all(bytes).map_err(zip_error)?;
    }
    zip.finish().map_err(zip_error)?;
    Ok(())
}

/// One `.atelier-plugin` of several of the same plugin, each with the library of a system: the
/// libraries of all, the manifest and the icons of the first.
pub fn merge(files: &[PathBuf], out: &Path) -> Result<(), String> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut id: Option<String> = None;
    for file in files {
        let reader = fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut zip = ZipArchive::new(reader).map_err(|e| format!("{}: {e}", file.display()))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(zip_error)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(zip_error)?;
            if name == "plugin.json" {
                let json: Value = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("{}: plugin.json: {e}", file.display()))?;
                let this = json.get("id").and_then(Value::as_str).unwrap_or_default();
                match &id {
                    Some(first) if first != this => {
                        return Err(format!(
                            "{}: the plugin {this}, not {first}",
                            file.display()
                        ));
                    }
                    _ => id = Some(this.to_string()),
                }
            }
            if !entries.iter().any(|(n, _)| *n == name) {
                entries.push((name, bytes));
            }
        }
    }
    if id.is_none() {
        return Err("no plugin.json in the files".into());
    }
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    write_zip(out, &entries)
}
