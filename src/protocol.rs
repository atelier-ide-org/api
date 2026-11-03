//! What the IDE and a plugin ask each other through the `call` functions of the interface: a
//! method name, and JSON in and out (bytes for pictures and fonts). A method one side does not
//! know answers nothing (an empty buffer), so both may add methods without a new version.
//!
//! Methods of the plugin ([`crate::abi::PluginApi::call`]):
//! - [`INFO`]: input [`InfoRequest`], output [`Info`];
//! - [`RUN_COMMAND`]: input [`RunCommand`];
//! - [`FILE_CLOSED`]: input [`PathMessage`]: the last tab of a file closed.
//!
//! Methods of the IDE ([`crate::abi::HostApi::call`]):
//! - [`NOTIFY`]: input [`Notify`];
//! - [`OPEN_FILE`]: input [`PathMessage`];
//! - [`SHOW_TOOL_WINDOW`]: input [`IdMessage`];
//! - [`PROJECT`]: output [`Project`];
//! - [`STYLE`]: output [`Style`];
//! - [`FONTS`]: output the fonts of the IDE ([`encode_fonts`]);
//! - [`ICON`]: input [`IconRequest`], output a picture ([`encode_image`]);
//! - [`SVG`]: input [`SvgRequest`], output a picture ([`encode_image`]).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const INFO: &str = "info";
pub const RUN_COMMAND: &str = "run_command";
pub const FILE_CLOSED: &str = "file_closed";

pub const NOTIFY: &str = "notify";
pub const OPEN_FILE: &str = "open_file";
pub const SHOW_TOOL_WINDOW: &str = "show_tool_window";
pub const PROJECT: &str = "project";
pub const STYLE: &str = "style";
pub const FONTS: &str = "fonts";
pub const ICON: &str = "icon";
pub const SVG: &str = "svg";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct InfoRequest {
    pub language: String,
}

/// What the plugin brings to the IDE. Its texts are in the language of the request: the IDE
/// asks again when the language changes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Info {
    pub tool_windows: Vec<ToolWindow>,
    /// The commands of the palette and of the Plugins menu.
    pub commands: Vec<Command>,
    /// The files the plugin shows.
    pub file_types: Vec<FileType>,
    /// The icons the other fields name, by `name`.
    pub icons: Vec<Icon>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    Left,
    Bottom,
    #[default]
    Right,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ToolWindow {
    /// Unique among the tool windows of the IDE: start it with the identifier of the plugin.
    pub id: String,
    pub title: String,
    /// The name of an icon of [`Info::icons`], or of the IDE (`database`, `run`…).
    pub icon: String,
    pub dock: Dock,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Command {
    pub id: String,
    pub label: String,
}

/// How the editor shows a document: source, source and preview side by side, or preview only.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    Text,
    Split,
    #[default]
    Preview,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FileType {
    /// Lowercase, without the dot (`glb`, `docx`).
    pub extensions: Vec<String>,
    pub layout: Layout,
    /// The file is not text: it opens without being read.
    pub binary: bool,
    /// Whether the editor offers to switch between the source and the preview.
    pub layout_switch: bool,
    /// The name of an icon of [`Info::icons`], for the project tree and the tabs.
    pub icon: Option<String>,
}

/// An SVG icon of the plugin, for each theme.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Icon {
    pub name: String,
    pub svg: String,
    /// For dark themes, when it differs.
    pub svg_dark: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RunCommand {
    pub id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PathMessage {
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct IdMessage {
    pub id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Notify {
    pub message: String,
    /// Also shows an error balloon.
    pub error: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Project {
    /// The folder of the open project, None on the start screen.
    pub root: Option<PathBuf>,
    /// The file of the active editor tab.
    pub active_file: Option<PathBuf>,
    /// The folder where the IDE keeps its settings: a plugin keeps its own under
    /// `plugins/<id>/`.
    pub data_dir: PathBuf,
}

/// The look of the IDE.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Style {
    /// egui's `Style`, as serde writes it: a plugin built with another egui reads what it can.
    pub egui: serde_json::Value,
    pub theme: Theme,
}

/// An sRGBA color, without premultiplied alpha.
pub type Color = [u8; 4];

/// The colors of the IDE, for what a plugin draws itself.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Theme {
    pub dark: bool,
    /// Title bar, tool window stripes, status bar and the gaps between islands.
    pub frame: Color,
    /// Tool windows and editor.
    pub island: Color,
    pub popup: Color,
    pub border: Color,
    pub separator: Color,
    pub hover: Color,
    pub pressed: Color,
    /// Selected row in a focused list, and when it is not focused.
    pub selection: Color,
    pub selection_inactive: Color,
    pub tool_open: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub link: Color,
    pub text_selection: Color,
    /// Text fields.
    pub field: Color,
    pub added: Color,
    pub modified: Color,
    pub deleted: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Theme {
    /// The Islands Dark theme of the IDE.
    fn default() -> Self {
        let hex = |rgb: u32| [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255];
        Self {
            dark: true,
            frame: hex(0x26282C),
            island: hex(0x191A1C),
            popup: hex(0x2B2D30),
            border: hex(0x40434A),
            separator: hex(0x2B2D30),
            hover: hex(0x2F3135),
            pressed: hex(0x393B40),
            selection: hex(0x2E436E),
            selection_inactive: hex(0x2F3135),
            tool_open: hex(0x4A4D50),
            accent: hex(0x3871E1),
            accent_hover: hex(0x4682FA),
            text: hex(0xDFE1E5),
            text_secondary: hex(0x868A91),
            text_disabled: hex(0x5A5D63),
            link: hex(0x548AF7),
            text_selection: hex(0x214283),
            field: hex(0x191A1C),
            added: hex(0x73BD79),
            modified: hex(0x70AEFF),
            deleted: hex(0x868A91),
            success: hex(0x73BD79),
            warning: hex(0xF2C55C),
            error: hex(0xDB5C5C),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct IconRequest {
    /// An icon of the IDE (`run`, `refresh`, `database`…).
    pub name: String,
    pub dark: bool,
    /// The side of the picture, in pixels.
    pub size: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SvgRequest {
    pub svg: String,
    /// The size of the picture, in pixels: the SVG is scaled to fit in it.
    pub width: u32,
    pub height: u32,
}

/// A picture: width and height (little-endian `u32`), then the pixels, 4 bytes each, sRGBA
/// with premultiplied alpha.
pub fn encode_image(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + pixels.len());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(pixels);
    out
}

/// The width, height and pixels of [`encode_image`].
pub fn decode_image(bytes: &[u8]) -> Option<(u32, u32, &[u8])> {
    let width = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?);
    let height = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?);
    let pixels = bytes.get(8..)?;
    (pixels.len() as u64 == u64::from(width) * u64::from(height) * 4)
        .then_some((width, height, pixels))
}

/// A font file and the face of it to use.
#[derive(Clone, Debug, PartialEq)]
pub struct FontData {
    pub name: String,
    pub bytes: Vec<u8>,
    pub index: u32,
}

/// The fonts of the IDE: the files, and for each family (`Proportional`, `Monospace`, or a name
/// of the IDE such as `semibold`) the names of its files, by priority.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fonts {
    pub data: Vec<FontData>,
    pub families: Vec<(String, Vec<String>)>,
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// [`Fonts`] as bytes: a count then each item, every string and file as a length (`u32`) and
/// its bytes.
pub fn encode_fonts(fonts: &Fonts) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(fonts.data.len() as u32).to_le_bytes());
    for font in &fonts.data {
        put(&mut out, font.name.as_bytes());
        put(&mut out, &font.bytes);
        out.extend_from_slice(&font.index.to_le_bytes());
    }
    out.extend_from_slice(&(fonts.families.len() as u32).to_le_bytes());
    for (family, names) in &fonts.families {
        put(&mut out, family.as_bytes());
        out.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for name in names {
            put(&mut out, name.as_bytes());
        }
    }
    out
}

pub fn decode_fonts(bytes: &[u8]) -> Option<Fonts> {
    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
    }
    impl Reader<'_> {
        fn u32(&mut self) -> Option<u32> {
            let v = u32::from_le_bytes(self.bytes.get(self.at..self.at + 4)?.try_into().ok()?);
            self.at += 4;
            Some(v)
        }
        fn bytes(&mut self) -> Option<Vec<u8>> {
            let len = self.u32()? as usize;
            let v = self.bytes.get(self.at..self.at.checked_add(len)?)?.to_vec();
            self.at += len;
            Some(v)
        }
        fn string(&mut self) -> Option<String> {
            String::from_utf8(self.bytes()?).ok()
        }
    }
    let mut r = Reader { bytes, at: 0 };
    let mut fonts = Fonts::default();
    for _ in 0..r.u32()? {
        let name = r.string()?;
        let bytes = r.bytes()?;
        let index = r.u32()?;
        fonts.data.push(FontData { name, bytes, index });
    }
    for _ in 0..r.u32()? {
        let family = r.string()?;
        let mut names = Vec::new();
        for _ in 0..r.u32()? {
            names.push(r.string()?);
        }
        fonts.families.push((family, names));
    }
    Some(fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_and_images_go_through() {
        let fonts = Fonts {
            data: vec![FontData {
                name: "Inter".into(),
                bytes: vec![1, 2, 3],
                index: 0,
            }],
            families: vec![("Proportional".into(), vec!["Inter".into()])],
        };
        assert_eq!(decode_fonts(&encode_fonts(&fonts)), Some(fonts));
        assert_eq!(decode_fonts(&[1, 0]), None);
        let image = encode_image(1, 2, &[9; 8]);
        assert_eq!(decode_image(&image), Some((1, 2, &[9u8; 8][..])));
        assert_eq!(decode_image(&image[..10]), None);
    }

    #[test]
    fn unknown_fields_are_ignored_and_missing_ones_default() {
        let info: Info = serde_json::from_str(
            r#"{"tool_windows":[{"id":"a","dock":"left","later":1}],"other":true}"#,
        )
        .unwrap();
        assert_eq!(info.tool_windows[0].dock, Dock::Left);
        assert!(info.commands.is_empty());
    }
}
