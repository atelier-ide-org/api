use super::*;
use std::fs;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atelier-cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn names_become_identifiers_types_and_libraries() {
    assert_eq!(create::identifier("My Viewer").unwrap(), "my-viewer");
    assert_eq!(create::identifier("  db__tools ").unwrap(), "db-tools");
    assert!(create::identifier("3d").is_err());
    assert!(create::identifier("--").is_err());
    let filled = create::fill("{{type}} {{library}} {{name}}", "my-viewer");
    assert_eq!(filled, "MyViewerPlugin my_viewer My Viewer");
}

#[test]
fn a_new_plugin_has_a_manifest_that_compile_accepts() {
    let dir = temp("create");
    let folder = create::create("Hello", &dir).unwrap();
    let manifest = compile::manifest(&folder).unwrap();
    assert_eq!(manifest.id, "hello");
    assert_eq!(manifest.library, "hello");
    assert!(folder.join("src/lib.rs").is_file());
    assert!(folder.join(".github/workflows/release.yml").is_file());
    let lib = fs::read_to_string(folder.join("src/lib.rs")).unwrap();
    assert!(!lib.contains("{{"), "every placeholder is filled");
    assert!(
        create::create("Hello", &dir).is_err(),
        "it never writes over a folder"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_manifest_must_list_its_extensions() {
    let dir = temp("manifest");
    fs::write(
        dir.join("plugin.json"),
        r#"{ "id": "x", "name": "X", "library": "x", "abi": 2 }"#,
    )
    .unwrap();
    assert!(
        compile::manifest(&dir)
            .err()
            .unwrap()
            .contains("extensions")
    );
    fs::write(
        dir.join("plugin.json"),
        r#"{ "id": "x", "name": "X", "library": "../x", "abi": 2, "extensions": [] }"#,
    )
    .unwrap();
    assert!(compile::manifest(&dir).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn systems_are_named_as_atelier_names_them() {
    let p = |t| compile::platform(Some(t), "viewer").unwrap();
    assert_eq!(
        p("x86_64-pc-windows-msvc"),
        ("windows-x86_64".into(), "viewer.dll".into())
    );
    assert_eq!(
        p("aarch64-apple-darwin"),
        ("macos-aarch64".into(), "libviewer.dylib".into())
    );
    assert_eq!(
        p("x86_64-unknown-linux-gnu"),
        ("linux-x86_64".into(), "libviewer.so".into())
    );
    assert!(compile::platform(Some("wasm32-unknown-unknown"), "v").is_err());
}

#[test]
fn files_of_several_systems_merge_into_one() {
    let dir = temp("merge");
    let write = |name: &str, entries: &[(&str, &[u8])]| {
        let file = dir.join(name);
        let mut zip = zip::ZipWriter::new(fs::File::create(&file).unwrap());
        for (path, bytes) in entries {
            zip.start_file(*path, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zip, bytes).unwrap();
        }
        zip.finish().unwrap();
        file
    };
    let manifest: &[u8] = br#"{ "id": "v" }"#;
    let a = write(
        "a.zip",
        &[("plugin.json", manifest), ("windows-x86_64/v.dll", b"a")],
    );
    let b = write(
        "b.zip",
        &[("plugin.json", manifest), ("linux-x86_64/libv.so", b"b")],
    );
    let out = dir.join("out/v.atelier-plugin");
    compile::merge(&[a.clone(), b], &out).unwrap();
    let zip = zip::ZipArchive::new(fs::File::open(&out).unwrap()).unwrap();
    let mut names: Vec<&str> = zip.file_names().collect();
    names.sort();
    assert_eq!(
        names,
        [
            "linux-x86_64/libv.so",
            "plugin.json",
            "windows-x86_64/v.dll"
        ]
    );
    let other = write("c.zip", &[("plugin.json", br#"{ "id": "w" }"#)]);
    assert!(compile::merge(&[a, other], &out).is_err(), "two plugins");
    let _ = fs::remove_dir_all(&dir);
}
