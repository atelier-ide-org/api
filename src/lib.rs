//! The interface between Atelier and its native plugins.
//!
//! A native plugin is a dynamic library (`.dll`, `.so`, `.dylib`) that the IDE loads once the
//! user installed it (Plugins, **+**). The two sides share only C types and functions
//! ([`abi`]): a plugin built once works with every version of the IDE that keeps
//! [`abi::VERSION`], whatever compilers and crates each was built with.
//!
//! A plugin written in Rust draws with its own egui (feature `egui`, on by default): it
//! implements [`Plugin`] and exports it with [`export_plugin!`]. Each of its surfaces (the
//! preview of a file, a tool window) gets the input of the IDE and gives back meshes and
//! textures, which the IDE paints ([`runtime`]). [`ui`] has the look of the IDE, [`gl`]
//! (feature `glow`) OpenGL callbacks. The IDE itself enables `serde` alone, for the messages of
//! [`protocol`].

pub mod abi;
#[cfg(feature = "glow")]
pub mod gl;
#[cfg(feature = "serde")]
pub mod protocol;
#[cfg(feature = "egui")]
pub mod runtime;
#[cfg(feature = "egui")]
pub mod ui;

#[cfg(feature = "egui")]
pub use egui;
#[cfg(feature = "glow")]
pub use glow;
#[cfg(feature = "egui")]
pub use runtime::{Host, Plugin, Surface};
#[cfg(feature = "serde")]
pub use serde_json;

use std::collections::HashMap;

/// The translations of a plugin: for each language, its keys and texts (the plugin reads its
/// `locales/<identifier>.json` files itself). A text missing
/// in a language is the English one, else the key.
pub struct Locales {
    catalogs: HashMap<&'static str, HashMap<String, String>>,
}

impl Locales {
    /// Each language, by its identifier (`en_US`), with its texts by key.
    pub fn new(
        catalogs: impl IntoIterator<Item = (&'static str, HashMap<String, String>)>,
    ) -> Self {
        Self {
            catalogs: catalogs.into_iter().collect(),
        }
    }

    pub fn get<'a>(&'a self, language: &str, key: &'a str) -> &'a str {
        let texts = |l: &str| self.catalogs.get(l).and_then(|c| c.get(key));
        texts(language)
            .or_else(|| texts("en_US"))
            .map_or(key, String::as_str)
    }

    /// The text of `key` with each `{name}` replaced by its value.
    pub fn format(
        &self,
        language: &str,
        key: &str,
        values: &[(&str, &dyn std::fmt::Display)],
    ) -> String {
        let mut text = self.get(language, key).to_string();
        for (name, value) in values {
            text = text.replace(&format!("{{{name}}}"), &value.to_string());
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_fall_back_to_english_then_to_the_key() {
        let texts = |pairs: &[(&str, &str)]| {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>()
        };
        let locales = Locales::new([
            (
                "en_US",
                texts(&[("a", "One"), ("b", "Two {n}"), ("q", "Say \"hi\"\n")]),
            ),
            ("fr_FR", texts(&[("a", "Un")])),
        ]);
        assert_eq!(locales.get("fr_FR", "a"), "Un");
        assert_eq!(locales.get("fr_FR", "b"), "Two {n}");
        assert_eq!(locales.get("fr_FR", "c"), "c");
        assert_eq!(locales.get("en_US", "q"), "Say \"hi\"\n");
        assert_eq!(locales.format("en_US", "b", &[("n", &3)]), "Two 3");
    }
}
