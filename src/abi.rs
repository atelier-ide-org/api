//! The C interface between the IDE and a plugin: `#[repr(C)]` data and `extern "C"` functions,
//! with no Rust type crossing it. It changes only by additions at the end of its structures
//! (each one starts with its `size`, which tells what the other side knows), or with a new
//! [`VERSION`].
//!
//! A plugin library exports a [`PluginApi`] under [`SYMBOL`]. The IDE reads its `abi` first,
//! and refuses a library of another version. Then:
//! - [`PluginApi::create`] makes the plugin, with the [`HostApi`] it may call (from any thread)
//!   until [`PluginApi::destroy`];
//! - [`PluginApi::call`] asks it something, by name, with JSON in and out (see [`crate::protocol`]);
//! - [`PluginApi::frame`] gives it the input of a frame of one of its surfaces (a preview, a
//!   tool window) and takes back what to paint: meshes in the coordinates of the window, the
//!   textures they use, and OpenGL callbacks, which the IDE calls back with [`PluginApi::paint`].
//!
//! Strings are UTF-8, given as pointer and length, valid for the time of the call. What one side
//! allocates, it frees: a [`Buffer`] goes back to the `free` of the side that made it. A
//! [`FrameOutput`] stays valid until the next call into the plugin. A panic never crosses the
//! interface.

use std::ffi::{c_char, c_void};

/// Changes when the interface changes in a way older plugins or IDEs cannot follow.
pub const VERSION: u32 = 2;

/// The name of the [`PluginApi`] static in a library.
pub const SYMBOL: &[u8] = b"ATELIER_PLUGIN_V2\0";

/// Borrowed bytes (UTF-8 for a string), valid for the time of the call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Str {
    pub ptr: *const u8,
    pub len: usize,
}

impl Str {
    pub const EMPTY: Self = Self {
        ptr: std::ptr::null(),
        len: 0,
    };

    pub fn new(text: &str) -> Self {
        Self::bytes(text.as_bytes())
    }

    pub fn bytes(bytes: &[u8]) -> Self {
        Self {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    /// # Safety
    /// `ptr` points to `len` bytes that live for `'a`, or is null.
    pub unsafe fn as_bytes<'a>(self) -> &'a [u8] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            // SAFETY: the caller's.
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    /// The text, empty when it is not UTF-8.
    ///
    /// # Safety
    /// As for [`Str::as_bytes`].
    pub unsafe fn as_str<'a>(self) -> &'a str {
        // SAFETY: the caller's.
        std::str::from_utf8(unsafe { self.as_bytes() }).unwrap_or("")
    }
}

/// Bytes that the callee allocated and gives to the caller, who hands them back to the
/// callee's `free` once read. Empty (null) when there is nothing to answer.
#[repr(C)]
#[derive(Debug)]
pub struct Buffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl Buffer {
    pub const EMPTY: Self = Self {
        ptr: std::ptr::null_mut(),
        len: 0,
        cap: 0,
    };

    /// Gives a vector away: [`Buffer::into_vec`] in the same library takes it back.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        let mut bytes = std::mem::ManuallyDrop::new(bytes);
        Self {
            ptr: bytes.as_mut_ptr(),
            len: bytes.len(),
            cap: bytes.capacity(),
        }
    }

    /// # Safety
    /// The buffer comes from [`Buffer::from_vec`] of the same library, and is taken back once.
    pub unsafe fn into_vec(self) -> Vec<u8> {
        if self.ptr.is_null() {
            Vec::new()
        } else {
            // SAFETY: the caller's.
            unsafe { Vec::from_raw_parts(self.ptr, self.len, self.cap) }
        }
    }

    /// # Safety
    /// The buffer is still owned by its maker.
    pub unsafe fn as_bytes(&self) -> &[u8] {
        if self.ptr.is_null() {
            &[]
        } else {
            // SAFETY: the caller's.
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }
}

/// A rectangle, in points of the window (egui's coordinates).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.min_x <= x && x <= self.max_x && self.min_y <= y && y <= self.max_y
    }
}

/// What happened since the last frame of a surface ([`Event::kind`]).
pub mod event {
    /// The pointer moved to `x`, `y`.
    pub const POINTER_MOVED: u32 = 1;
    /// `button` pressed or released at `x`, `y`.
    pub const POINTER_BUTTON: u32 = 2;
    /// The pointer left the surface.
    pub const POINTER_GONE: u32 = 3;
    /// The wheel turned by `x`, `y`, in `unit`s.
    pub const WHEEL: u32 = 4;
    /// The key named `text` (egui's names: `A`, `Enter`, `ArrowDown`…), `pressed` or released.
    pub const KEY: u32 = 5;
    /// Text typed.
    pub const TEXT: u32 = 6;
    pub const COPY: u32 = 7;
    pub const CUT: u32 = 8;
    /// Text pasted.
    pub const PASTE: u32 = 9;
    /// Zoom by the factor `x` (pinch, or Ctrl and the wheel).
    pub const ZOOM: u32 = 10;
    /// The window of the IDE got (`pressed`) or lost the focus.
    pub const FOCUS: u32 = 11;
}

/// The bits of [`Event::modifiers`] and [`FrameInput::modifiers`].
pub mod modifier {
    pub const ALT: u32 = 1;
    pub const CTRL: u32 = 2;
    pub const SHIFT: u32 = 4;
    pub const MAC_CMD: u32 = 8;
    /// Ctrl, or Cmd on macOS.
    pub const COMMAND: u32 = 16;
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub kind: u32,
    pub x: f32,
    pub y: f32,
    /// 0 primary, 1 secondary, 2 middle, 3 and 4 the extra ones.
    pub button: u32,
    pub pressed: u8,
    pub repeat: u8,
    /// Of the wheel: 0 points, 1 lines, 2 pages.
    pub unit: u8,
    pub modifiers: u32,
    /// The text of [`event::TEXT`] and [`event::PASTE`], the name of the key of [`event::KEY`].
    pub text: Str,
    /// The name of the physical key of [`event::KEY`], empty when unknown.
    pub physical: Str,
}

impl Default for Event {
    fn default() -> Self {
        Self {
            kind: 0,
            x: 0.0,
            y: 0.0,
            button: 0,
            pressed: 0,
            repeat: 0,
            unit: 0,
            modifiers: 0,
            text: Str::EMPTY,
            physical: Str::EMPTY,
        }
    }
}

/// The kinds of surface ([`FrameInput::surface`]).
pub mod surface {
    /// The view of a file in an editor tab: `key` is its path.
    pub const PREVIEW: u32 = 1;
    /// A tool window: `key` is its identifier.
    pub const TOOL_WINDOW: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameInput {
    /// `size_of::<FrameInput>()` for the IDE.
    pub size: usize,
    pub surface: u32,
    pub key: Str,
    /// The whole window: windows and menus of the plugin may go anywhere in it.
    pub screen: Rect,
    /// Where the surface is: what the plugin draws there is clipped to it.
    pub content: Rect,
    pub pixels_per_point: f32,
    /// Seconds since the IDE started, and since the last frame.
    pub time: f64,
    pub dt: f32,
    /// The keyboard goes to this surface.
    pub focused: u8,
    pub modifiers: u32,
    pub events: *const Event,
    pub events_len: usize,
    /// The language of the interface (`en_US`, `fr_FR`).
    pub language: Str,
    /// Changes when the look of the IDE does: the plugin asks for the `style` again.
    pub style_generation: u64,
    /// Changes when the fonts of the IDE do: the plugin asks for the `fonts` again.
    pub fonts_generation: u64,
}

/// The kinds of [`Primitive`].
pub mod primitive {
    /// Triangles: `vertices`, `indices`, painted with `texture`.
    pub const MESH: u32 = 0;
    /// An OpenGL callback in `rect`: the IDE calls [`super::PluginApi::paint`] with `texture`
    /// as its identifier when it paints it.
    pub const CALLBACK: u32 = 1;
}

/// A vertex, as egui's: position in points, texture coordinates from 0 to 1, color in sRGBA
/// with premultiplied alpha.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
    pub color: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Primitive {
    pub kind: u32,
    pub clip: Rect,
    /// The texture of a mesh (an identifier of the plugin, see [`TextureUpdate`]), or the
    /// identifier of a callback.
    pub texture: u64,
    pub vertices: *const Vertex,
    pub vertices_len: usize,
    pub indices: *const u32,
    pub indices_len: usize,
    /// Where a callback paints.
    pub rect: Rect,
}

/// A texture to make, or a part of one to replace. Identifiers are the plugin's, one set per
/// surface.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TextureUpdate {
    pub id: u64,
    /// 1 when this is the whole texture (`x`, `y` ignored), 0 for a part of it.
    pub whole: u8,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// `width * height` pixels of 4 bytes, sRGBA with premultiplied alpha.
    pub pixels: *const u8,
    /// 0 linear, 1 nearest.
    pub magnification: u8,
    pub minification: u8,
    /// 0 clamp to the edge, 1 repeat, 2 mirrored repeat.
    pub wrap: u8,
    /// 0 no mipmaps, 1 linear, 2 nearest.
    pub mipmap: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameOutput {
    /// `size_of::<FrameOutput>()` for the plugin.
    pub size: usize,
    /// What the plugin drew in its surface, painted there, under the menus of the IDE.
    pub base: *const Primitive,
    pub base_len: usize,
    /// Its windows, menus and tooltips, painted above the IDE.
    pub floating: *const Primitive,
    pub floating_len: usize,
    /// Where the floating ones are: there, the pointer goes to the plugin.
    pub floating_rects: *const Rect,
    pub floating_rects_len: usize,
    pub textures: *const TextureUpdate,
    pub textures_len: usize,
    pub freed: *const u64,
    pub freed_len: usize,
    /// The cursor over the plugin: the name of an egui `CursorIcon` (`PointingHand`, `Text`…).
    pub cursor: Str,
    /// A text field of the plugin has the keyboard.
    pub wants_keyboard: u8,
    /// The pointer is over something of the plugin, or drags it.
    pub wants_pointer: u8,
    /// When to paint again: 0 now, `u64::MAX` when nothing changes by itself.
    pub repaint_after_ms: u64,
    /// Text to put in the clipboard, when not empty.
    pub copied_text: Str,
    /// A link to open, when not empty.
    pub open_url: Str,
}

/// Where and how an OpenGL callback paints ([`PluginApi::paint`]). The OpenGL context of the IDE
/// is current on the calling thread; the plugin loads its functions with `get_proc_address`,
/// and leaves the state as it found it, except for the bindings that egui sets before each mesh.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PaintInfo {
    /// The rectangle of the callback, and the part of it to paint, in points.
    pub viewport: Rect,
    pub clip: Rect,
    pub pixels_per_point: f32,
    /// The size of the window, in pixels.
    pub screen_width: u32,
    pub screen_height: u32,
    /// The address of an OpenGL function by its name (a C string), null when unknown.
    pub get_proc_address: unsafe extern "C" fn(name: *const c_char) -> *const c_void,
}

/// What the IDE gives a plugin. Its functions may be called from any thread.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HostApi {
    /// `size_of::<HostApi>()` for the IDE.
    pub size: usize,
    /// The IDE's, given back to each function.
    pub host: *mut c_void,
    /// Asks the IDE something by name, with JSON (or bytes) in and out (see
    /// [`crate::protocol`]). The answer goes back to `free`.
    pub call: unsafe extern "C" fn(host: *mut c_void, method: Str, input: Str) -> Buffer,
    pub free: unsafe extern "C" fn(host: *mut c_void, buffer: Buffer),
    /// Asks for a new frame of every surface, e.g. when a background task is done.
    pub request_repaint: unsafe extern "C" fn(host: *mut c_void),
}

/// What a library exports under [`SYMBOL`]: `abi` first, read before anything else.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PluginApi {
    pub abi: u32,
    /// `size_of::<PluginApi>()` for the plugin.
    pub size: usize,
    /// A new plugin, which keeps `host` until `destroy`.
    pub create: unsafe extern "C" fn(host: *const HostApi) -> *mut c_void,
    pub destroy: unsafe extern "C" fn(plugin: *mut c_void),
    /// Asks the plugin something by name (see [`crate::protocol`]). The answer goes back to
    /// `free`.
    pub call: unsafe extern "C" fn(plugin: *mut c_void, method: Str, input: Str) -> Buffer,
    pub free: unsafe extern "C" fn(buffer: Buffer),
    /// A frame of a surface. Null when the plugin failed.
    pub frame:
        unsafe extern "C" fn(plugin: *mut c_void, input: *const FrameInput) -> *const FrameOutput,
    /// Paints the callback `callback` of the last frame.
    pub paint: unsafe extern "C" fn(plugin: *mut c_void, callback: u64, info: *const PaintInfo),
}

// SAFETY: the pointers are to functions, and to an IDE that is thread-safe (see `HostApi`).
unsafe impl Send for HostApi {}
unsafe impl Sync for HostApi {}
// SAFETY: functions only.
unsafe impl Sync for PluginApi {}
