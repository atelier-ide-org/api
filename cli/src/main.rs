//! `atelier-cli`: the command line of the native plugins of Atelier.
//!
//! ```text
//! atelier-cli plugin create <name> [--dir <folder>]
//! atelier-cli plugin compile [<path>] [--debug] [--target <triple>] [--out <folder>]
//! atelier-cli plugin merge <file>... --out <file>
//! ```
//!
//! Install it with `cargo install --git https://github.com/atelier-ide-org/api atelier-cli`.

mod compile;
mod create;

use std::{path::PathBuf, process::ExitCode};

const USAGE: &str = "\
atelier-cli: the native plugins of Atelier

Usage:
  atelier-cli plugin create <name> [--dir <folder>]
      A new plugin in <folder>/<name> (by default ./<name>): its Cargo.toml, plugin.json,
      code, texts, documentation, icons of its files and workflows.

  atelier-cli plugin compile [<path>] [--debug] [--target <triple>] [--out <folder>]
      Builds the plugin of <path> (by default .) and makes the file Atelier installs,
      <id>.atelier-plugin: its plugin.json, the icons of its files (extensions/) and its library
      for this system (or for <triple>), in target/<profile> (or <folder>).

  atelier-cli plugin merge <file>... --out <file>
      One .atelier-plugin of several, each with the library of another system.
";

/// The arguments left once the options are read.
struct Args {
    words: Vec<String>,
    options: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(raw: impl Iterator<Item = String>) -> Result<Self, String> {
        const WITH_VALUE: [&str; 3] = ["--dir", "--target", "--out"];
        let mut words = Vec::new();
        let mut options = Vec::new();
        let mut raw = raw.peekable();
        while let Some(arg) = raw.next() {
            if WITH_VALUE.contains(&arg.as_str()) {
                let value = raw.next().ok_or_else(|| format!("{arg} needs a value"))?;
                options.push((arg, Some(value)));
            } else if arg.starts_with("--") {
                options.push((arg, None));
            } else {
                words.push(arg);
            }
        }
        Ok(Self { words, options })
    }

    fn value(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, v)| v.as_deref())
    }

    fn flag(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(std::env::args().skip(1))?;
    if args.flag("--help") || args.words.is_empty() {
        print!("{USAGE}");
        return Ok(());
    }
    let words: Vec<&str> = args.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plugin", "create", name] => {
            let dir = PathBuf::from(args.value("--dir").unwrap_or("."));
            let folder = create::create(name, &dir)?;
            println!("{}", folder.display());
            println!(
                "Next: cd {}, then atelier-cli plugin compile",
                folder.display()
            );
            Ok(())
        }
        ["plugin", "compile", rest @ ..] if rest.len() <= 1 => {
            let path = PathBuf::from(rest.first().copied().unwrap_or("."));
            let file = compile::compile(
                &path,
                args.flag("--debug"),
                args.value("--target"),
                args.value("--out").map(PathBuf::from),
            )?;
            println!("{}", file.display());
            println!("In Atelier: Plugins, Install a plugin from a file (the + button).");
            Ok(())
        }
        ["plugin", "merge", files @ ..] if !files.is_empty() => {
            let out = args.value("--out").ok_or("merge needs --out <file>")?;
            let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
            compile::merge(&files, &PathBuf::from(out))?;
            println!("{out}");
            Ok(())
        }
        _ => Err(format!("unknown command: {}\n\n{USAGE}", words.join(" "))),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
