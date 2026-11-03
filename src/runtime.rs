//! The side of the interface a plugin written with egui uses: it implements [`Plugin`], exports
//! it with [`crate::export_plugin!`], and draws with its own egui. Each surface (a preview, a
//! tool window) has an `egui::Context` of its own, fed with the input the IDE sends; what it
//! paints goes back to the IDE as meshes and textures. egui is the plugin's: it does not have to
//! be the IDE's version.

use crate::{abi, protocol};
use egui::{
    Context, FontDefinitions, FontFamily, Id, LayerId, Order, Shape, TextureId, UiBuilder,
    epaint::{ClippedShape, Primitive},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

/// The family of the semibold interface font ([`crate::ui::semibold`]).
pub const SEMIBOLD: &str = "semibold";

/// Where the plugin draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface<'a> {
    /// The view of a file in an editor tab.
    Preview(&'a Path),
    /// One of the tool windows of [`protocol::Info::tool_windows`].
    ToolWindow(&'a str),
}

/// A native plugin. Every method has a default: a plugin implements what it uses.
#[allow(unused_variables)]
pub trait Plugin: 'static {
    /// What the plugin brings: tool windows, commands, the files it shows, with their texts
    /// in `language` (`en_US`, `fr_FR`).
    fn info(&mut self, language: &str) -> protocol::Info {
        protocol::Info::default()
    }
    /// Draws a surface, in `ui`. Windows and menus may go anywhere in the window of the IDE.
    fn ui(&mut self, ui: &mut egui::Ui, surface: Surface<'_>, host: &Host) {}
    /// A command of [`protocol::Info::commands`] was chosen.
    fn run_command(&mut self, id: &str, host: &Host) {}
    /// The last tab of `path` closed: what the plugin kept for it goes.
    fn file_closed(&mut self, path: &Path) {}
    /// Every frame of the IDE, before it draws, even when no surface of the plugin is shown:
    /// background work, and what to stop when its files are no longer shown (a surface not
    /// drawn between two updates is hidden).
    fn update(&mut self, update: &protocol::Update, host: &Host) {}
}

/// What a plugin may ask the IDE, from any thread.
#[derive(Clone)]
pub struct Host {
    api: Arc<abi::HostApi>,
}

impl Host {
    /// # Safety
    /// `api` is valid until the plugin is destroyed, and thread-safe.
    unsafe fn new(api: *const abi::HostApi) -> Self {
        // SAFETY: the caller's.
        Self {
            api: Arc::new(unsafe { *api }),
        }
    }

    /// Asks the IDE something by name ([`protocol`]); empty when it does not know it.
    pub fn call(&self, method: &str, input: &[u8]) -> Vec<u8> {
        let api = &*self.api;
        // SAFETY: the functions of the IDE, which frees what it gave.
        unsafe {
            let buffer = (api.call)(api.host, abi::Str::new(method), abi::Str::bytes(input));
            let bytes = buffer.as_bytes().to_vec();
            (api.free)(api.host, buffer);
            bytes
        }
    }

    pub fn call_json_bytes(&self, method: &str, input: &impl Serialize) -> Vec<u8> {
        self.call(method, &serde_json::to_vec(input).unwrap_or_default())
    }

    /// The answer of a method, read as JSON; the default when there is none.
    pub fn call_json<O: DeserializeOwned + Default>(
        &self,
        method: &str,
        input: &impl Serialize,
    ) -> O {
        serde_json::from_slice(&self.call_json_bytes(method, input)).unwrap_or_default()
    }

    /// A message kept in the notifications of the IDE.
    pub fn notify(&self, message: impl Into<String>) {
        self.call_json_bytes(
            protocol::NOTIFY,
            &protocol::Notify {
                message: message.into(),
                error: false,
            },
        );
    }

    /// An error: kept in the notifications, and shown in a balloon.
    pub fn error(&self, message: impl Into<String>) {
        self.call_json_bytes(
            protocol::NOTIFY,
            &protocol::Notify {
                message: message.into(),
                error: true,
            },
        );
    }

    pub fn open_file(&self, path: impl Into<PathBuf>) {
        self.call_json_bytes(
            protocol::OPEN_FILE,
            &protocol::PathMessage { path: path.into() },
        );
    }

    pub fn show_tool_window(&self, id: &str) {
        self.call_json_bytes(
            protocol::SHOW_TOOL_WINDOW,
            &protocol::IdMessage { id: id.into() },
        );
    }

    /// The open project and the active file.
    pub fn project(&self) -> protocol::Project {
        self.call_json(protocol::PROJECT, &())
    }

    /// The folder where the IDE keeps its settings.
    pub fn data_dir(&self) -> PathBuf {
        self.project().data_dir
    }

    /// Asks for a new frame, e.g. when a background task is done.
    pub fn request_repaint(&self) {
        let api = &*self.api;
        // SAFETY: a function of the IDE, callable from any thread.
        unsafe { (api.request_repaint)(api.host) }
    }
}

/// Splits what egui painted: what comes before it in the output is in the surface, what comes
/// after floats above the IDE. It is painted in an area of the middle order, made before any
/// window, so it stays under them.
struct Split;

fn split_id() -> Id {
    Id::new("atelier_split")
}

struct SurfaceState {
    ctx: Context,
    style: u64,
    fonts: u64,
}

/// The vectors a [`abi::FrameOutput`] points to, kept until the next call.
#[derive(Default)]
struct Output {
    base: Vec<abi::Primitive>,
    floating: Vec<abi::Primitive>,
    rects: Vec<abi::Rect>,
    vertices: Vec<Vec<abi::Vertex>>,
    indices: Vec<Vec<u32>>,
    textures: Vec<abi::TextureUpdate>,
    pixels: Vec<Vec<u8>>,
    freed: Vec<u64>,
    cursor: String,
    copied: String,
    url: String,
    frame: Option<abi::FrameOutput>,
}

#[cfg(feature = "glow")]
type Callback = Arc<crate::gl::Callback>;

pub struct Instance {
    plugin: Box<dyn Plugin>,
    host: Host,
    surfaces: HashMap<(u32, String), SurfaceState>,
    style: Option<(u64, egui::Style, crate::ui::Theme)>,
    fonts: Option<(u64, FontDefinitions)>,
    output: Output,
    #[cfg(feature = "glow")]
    callbacks: HashMap<u64, ((u32, String), Callback)>,
    #[cfg(feature = "glow")]
    gl: Option<Arc<glow::Context>>,
    #[cfg(feature = "glow")]
    next_callback: u64,
}

fn rect(r: abi::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(r.min_x, r.min_y), egui::pos2(r.max_x, r.max_y))
}

fn abi_rect(r: egui::Rect) -> abi::Rect {
    abi::Rect {
        min_x: r.min.x,
        min_y: r.min.y,
        max_x: r.max.x,
        max_y: r.max.y,
    }
}

fn modifiers(bits: u32) -> egui::Modifiers {
    use abi::modifier::*;
    egui::Modifiers {
        alt: bits & ALT != 0,
        ctrl: bits & CTRL != 0,
        shift: bits & SHIFT != 0,
        mac_cmd: bits & MAC_CMD != 0,
        command: bits & COMMAND != 0,
    }
}

/// The input of egui from the one of the IDE.
///
/// # Safety
/// The pointers of `input` are valid for the call.
unsafe fn raw_input(input: &abi::FrameInput) -> egui::RawInput {
    use abi::event::*;
    let events = if input.events.is_null() {
        &[][..]
    } else {
        // SAFETY: the caller's.
        unsafe { std::slice::from_raw_parts(input.events, input.events_len) }
    };
    let pos = |e: &abi::Event| egui::pos2(e.x, e.y);
    let events = events
        .iter()
        .filter_map(|e| {
            // SAFETY: the caller's.
            let text = unsafe { e.text.as_str() };
            Some(match e.kind {
                POINTER_MOVED => egui::Event::PointerMoved(pos(e)),
                POINTER_BUTTON => egui::Event::PointerButton {
                    pos: pos(e),
                    button: match e.button {
                        1 => egui::PointerButton::Secondary,
                        2 => egui::PointerButton::Middle,
                        3 => egui::PointerButton::Extra1,
                        4 => egui::PointerButton::Extra2,
                        _ => egui::PointerButton::Primary,
                    },
                    pressed: e.pressed != 0,
                    modifiers: modifiers(e.modifiers),
                },
                POINTER_GONE => egui::Event::PointerGone,
                WHEEL => egui::Event::MouseWheel {
                    unit: match e.unit {
                        1 => egui::MouseWheelUnit::Line,
                        2 => egui::MouseWheelUnit::Page,
                        _ => egui::MouseWheelUnit::Point,
                    },
                    delta: egui::vec2(e.x, e.y),
                    modifiers: modifiers(e.modifiers),
                },
                KEY => egui::Event::Key {
                    key: egui::Key::from_name(text)?,
                    // SAFETY: the caller's.
                    physical_key: egui::Key::from_name(unsafe { e.physical.as_str() }),
                    pressed: e.pressed != 0,
                    repeat: e.repeat != 0,
                    modifiers: modifiers(e.modifiers),
                },
                TEXT => egui::Event::Text(text.to_string()),
                COPY => egui::Event::Copy,
                CUT => egui::Event::Cut,
                PASTE => egui::Event::Paste(text.to_string()),
                ZOOM => egui::Event::Zoom(e.x),
                FOCUS => egui::Event::WindowFocused(e.pressed != 0),
                _ => return None,
            })
        })
        .collect();
    let mut raw = egui::RawInput {
        screen_rect: Some(rect(input.screen)),
        time: Some(input.time),
        predicted_dt: input.dt,
        modifiers: modifiers(input.modifiers),
        events,
        focused: input.focused != 0,
        ..Default::default()
    };
    if let Some(viewport) = raw.viewports.get_mut(&raw.viewport_id) {
        viewport.native_pixels_per_point = Some(input.pixels_per_point);
    }
    raw
}

/// The fonts of the IDE as egui's, with the families a plugin may name even when the IDE gave
/// none.
pub(crate) fn font_definitions(fonts: Option<protocol::Fonts>) -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    if let Some(fonts) = fonts.filter(|f| !f.data.is_empty()) {
        definitions.font_data.clear();
        definitions.families.clear();
        for font in fonts.data {
            let mut data = egui::FontData::from_owned(font.bytes);
            data.index = font.index;
            definitions.font_data.insert(font.name, Arc::new(data));
        }
        for (family, names) in fonts.families {
            let names = names
                .into_iter()
                .filter(|n| definitions.font_data.contains_key(n))
                .collect();
            let family = match family.as_str() {
                "Proportional" => FontFamily::Proportional,
                "Monospace" => FontFamily::Monospace,
                name => FontFamily::Name(name.into()),
            };
            definitions.families.insert(family, names);
        }
    }
    let proportional = definitions
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_insert_with(|| proportional.clone());
    }
    definitions
        .families
        .entry(FontFamily::Name(SEMIBOLD.into()))
        .or_insert(proportional);
    definitions
}

fn texture_id(id: TextureId) -> u64 {
    match id {
        TextureId::Managed(id) => id & !(1 << 63),
        TextureId::User(id) => id | (1 << 63),
    }
}

impl Instance {
    fn new(plugin: Box<dyn Plugin>, host: Host) -> Self {
        Self {
            plugin,
            host,
            surfaces: HashMap::new(),
            style: None,
            fonts: None,
            output: Output::default(),
            #[cfg(feature = "glow")]
            callbacks: HashMap::new(),
            #[cfg(feature = "glow")]
            gl: None,
            #[cfg(feature = "glow")]
            next_callback: 1,
        }
    }

    fn refresh_style(&mut self, generation: u64) {
        if self.style.as_ref().is_some_and(|(g, ..)| *g == generation) {
            return;
        }
        let style: protocol::Style = self.host.call_json(protocol::STYLE, &());
        let theme = crate::ui::Theme::from(style.theme);
        let mut egui_style = if theme.dark {
            egui::Style {
                visuals: egui::Visuals::dark(),
                ..Default::default()
            }
        } else {
            egui::Style {
                visuals: egui::Visuals::light(),
                ..Default::default()
            }
        };
        if let Ok(read) = serde_json::from_value::<egui::Style>(style.egui) {
            egui_style = read;
        }
        // The IDE's own text styles may name fonts this plugin does not have.
        egui_style.text_styles.retain(|_, font| match &font.family {
            FontFamily::Name(name) => name.as_ref() == SEMIBOLD,
            _ => true,
        });
        self.style = Some((generation, egui_style, theme));
    }

    fn refresh_fonts(&mut self, generation: u64) {
        if self.fonts.as_ref().is_some_and(|(g, _)| *g == generation) {
            return;
        }
        let bytes = self.host.call(protocol::FONTS, &[]);
        self.fonts = Some((generation, font_definitions(protocol::decode_fonts(&bytes))));
    }

    /// # Safety
    /// The pointers of `input` are valid for the call.
    unsafe fn frame(&mut self, input: &abi::FrameInput) -> &abi::FrameOutput {
        // SAFETY: the caller's.
        let (key, language) = unsafe { (input.key.as_str().to_string(), input.language.as_str()) };
        self.refresh_style(input.style_generation);
        self.refresh_fonts(input.fonts_generation);
        let surface_key = (input.surface, key.clone());
        let host = self.host.clone();
        let state = self.surfaces.entry(surface_key.clone()).or_insert_with(|| {
            let ctx = Context::default();
            ctx.options_mut(|o| {
                o.zoom_with_keyboard = false;
            });
            // A repaint asked now (from a thread of the plugin, or for an animation) is one of
            // the IDE; a later one comes back with the frame (`repaint_after_ms`).
            ctx.set_request_repaint_callback(move |info| {
                if info.delay.is_zero() {
                    host.request_repaint();
                }
            });
            SurfaceState {
                ctx,
                style: u64::MAX,
                fonts: u64::MAX,
            }
        });
        let ctx = state.ctx.clone();
        if let Some((generation, style, theme)) = &self.style
            && state.style != *generation
        {
            ctx.set_style(style.clone());
            crate::ui::set_theme(&ctx, *theme);
            state.style = *generation;
        }
        if let Some((generation, fonts)) = &self.fonts
            && state.fonts != *generation
        {
            ctx.set_fonts(fonts.clone());
            state.fonts = *generation;
        }
        ctx.data_mut(|d| {
            d.insert_temp(Id::new("atelier_language"), language.to_string());
            d.insert_temp(Id::new("atelier_host"), self.host.clone());
        });
        // SAFETY: the caller's.
        let raw = unsafe { raw_input(input) };
        let screen = rect(input.screen);
        let content = rect(input.content);
        let path = PathBuf::from(&key);
        let surface = match input.surface {
            abi::surface::PREVIEW => Surface::Preview(&path),
            _ => Surface::ToolWindow(&key),
        };
        let (plugin, host) = (&mut self.plugin, &self.host);
        let output = ctx.run(raw, |ctx| {
            egui::Area::new(split_id())
                .order(Order::Middle)
                .fixed_pos(screen.min)
                .interactable(false)
                .show(ctx, |ui| {
                    ui.painter().add(Shape::Callback(egui::PaintCallback {
                        rect: egui::Rect::from_min_size(screen.min, egui::vec2(1.0, 1.0)),
                        callback: Arc::new(Split),
                    }));
                });
            let mut ui = egui::Ui::new(
                ctx.clone(),
                Id::new("atelier_surface"),
                UiBuilder::new()
                    .layer_id(LayerId::background())
                    .max_rect(content),
            );
            ui.set_clip_rect(content);
            plugin.ui(&mut ui, surface, host);
        });
        self.fill(&ctx, surface_key, output)
    }

    fn fill(
        &mut self,
        ctx: &Context,
        surface: (u32, String),
        output: egui::FullOutput,
    ) -> &abi::FrameOutput {
        let o = &mut self.output;
        *o = Output::default();
        let mut shapes = output.shapes;
        let is_split = |s: &ClippedShape| matches!(&s.shape, Shape::Callback(c) if c.callback.downcast_ref::<Split>().is_some());
        let floating = match shapes.iter().position(is_split) {
            Some(i) => {
                let mut rest = shapes.split_off(i);
                rest.remove(0);
                rest
            }
            None => Vec::new(),
        };
        let ppp = output.pixels_per_point;
        #[cfg(feature = "glow")]
        self.callbacks.retain(|_, (s, _)| *s != surface);
        #[cfg(not(feature = "glow"))]
        let _ = &surface;
        for (shapes, floating) in [(shapes, false), (floating, true)] {
            for clipped in ctx.tessellate(shapes, ppp) {
                let clip = abi_rect(clipped.clip_rect);
                let primitive = match clipped.primitive {
                    Primitive::Mesh(mesh) => {
                        let vertices: Vec<abi::Vertex> = mesh
                            .vertices
                            .iter()
                            .map(|v| abi::Vertex {
                                x: v.pos.x,
                                y: v.pos.y,
                                u: v.uv.x,
                                v: v.uv.y,
                                color: v.color.to_array(),
                            })
                            .collect();
                        let primitive = abi::Primitive {
                            kind: abi::primitive::MESH,
                            clip,
                            texture: texture_id(mesh.texture_id),
                            vertices: vertices.as_ptr(),
                            vertices_len: vertices.len(),
                            indices: mesh.indices.as_ptr(),
                            indices_len: mesh.indices.len(),
                            rect: abi::Rect::default(),
                        };
                        o.vertices.push(vertices);
                        o.indices.push(mesh.indices);
                        primitive
                    }
                    Primitive::Callback(callback) => {
                        #[cfg(feature = "glow")]
                        {
                            let Ok(gl) =
                                callback.callback.clone().downcast::<crate::gl::Callback>()
                            else {
                                continue;
                            };
                            let id = self.next_callback;
                            self.next_callback += 1;
                            self.callbacks.insert(id, (surface.clone(), gl));
                            abi::Primitive {
                                kind: abi::primitive::CALLBACK,
                                clip,
                                texture: id,
                                vertices: std::ptr::null(),
                                vertices_len: 0,
                                indices: std::ptr::null(),
                                indices_len: 0,
                                rect: abi_rect(callback.rect),
                            }
                        }
                        #[cfg(not(feature = "glow"))]
                        {
                            let _ = callback;
                            continue;
                        }
                    }
                };
                if floating {
                    o.floating.push(primitive);
                } else {
                    o.base.push(primitive);
                }
            }
        }
        for (id, delta) in output.textures_delta.set {
            let egui::ImageData::Color(image) = &delta.image;
            let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            let [width, height] = image.size;
            let filter = |f: egui::TextureFilter| match f {
                egui::TextureFilter::Nearest => 1,
                egui::TextureFilter::Linear => 0,
            };
            let options = delta.options;
            o.textures.push(abi::TextureUpdate {
                id: texture_id(id),
                whole: u8::from(delta.pos.is_none()),
                x: delta.pos.map_or(0, |p| p[0] as u32),
                y: delta.pos.map_or(0, |p| p[1] as u32),
                width: width as u32,
                height: height as u32,
                pixels: pixels.as_ptr(),
                magnification: filter(options.magnification),
                minification: filter(options.minification),
                wrap: match options.wrap_mode {
                    egui::TextureWrapMode::ClampToEdge => 0,
                    egui::TextureWrapMode::Repeat => 1,
                    egui::TextureWrapMode::MirroredRepeat => 2,
                },
                mipmap: match options.mipmap_mode {
                    None => 0,
                    Some(egui::TextureFilter::Linear) => 1,
                    Some(egui::TextureFilter::Nearest) => 2,
                },
            });
            o.pixels.push(pixels);
        }
        o.freed = output
            .textures_delta
            .free
            .into_iter()
            .map(texture_id)
            .collect();
        o.rects = ctx.memory(|m| {
            m.areas()
                .visible_layer_ids()
                .into_iter()
                .filter(|l| matches!(l.order, Order::Middle | Order::Foreground))
                .filter(|l| l.id != split_id())
                .filter_map(|l| m.area_rect(l.id))
                .map(abi_rect)
                .collect()
        });
        o.cursor = format!("{:?}", output.platform_output.cursor_icon);
        for command in output.platform_output.commands {
            match command {
                egui::OutputCommand::CopyText(text) => o.copied = text,
                egui::OutputCommand::OpenUrl(url) => o.url = url.url,
                _ => {}
            }
        }
        let repaint = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |v| v.repaint_delay);
        let frame = abi::FrameOutput {
            size: std::mem::size_of::<abi::FrameOutput>(),
            base: o.base.as_ptr(),
            base_len: o.base.len(),
            floating: o.floating.as_ptr(),
            floating_len: o.floating.len(),
            floating_rects: o.rects.as_ptr(),
            floating_rects_len: o.rects.len(),
            textures: o.textures.as_ptr(),
            textures_len: o.textures.len(),
            freed: o.freed.as_ptr(),
            freed_len: o.freed.len(),
            cursor: abi::Str::new(&o.cursor),
            wants_keyboard: u8::from(ctx.wants_keyboard_input()),
            wants_pointer: u8::from(ctx.wants_pointer_input() || ctx.is_using_pointer()),
            repaint_after_ms: if repaint == Duration::MAX {
                u64::MAX
            } else {
                repaint.as_millis().min(u128::from(u64::MAX - 1)) as u64
            },
            copied_text: abi::Str::new(&o.copied),
            open_url: abi::Str::new(&o.url),
        };
        o.frame.insert(frame)
    }

    fn call(&mut self, method: &str, input: &[u8]) -> Vec<u8> {
        match method {
            protocol::INFO => {
                let request: protocol::InfoRequest =
                    serde_json::from_slice(input).unwrap_or_default();
                serde_json::to_vec(&self.plugin.info(&request.language)).unwrap_or_default()
            }
            protocol::RUN_COMMAND => {
                let request: protocol::RunCommand =
                    serde_json::from_slice(input).unwrap_or_default();
                self.plugin.run_command(&request.id, &self.host);
                Vec::new()
            }
            protocol::UPDATE => {
                let update: protocol::Update = serde_json::from_slice(input).unwrap_or_default();
                self.plugin.update(&update, &self.host);
                Vec::new()
            }
            protocol::FILE_CLOSED => {
                let request: protocol::PathMessage =
                    serde_json::from_slice(input).unwrap_or_default();
                let key = (
                    abi::surface::PREVIEW,
                    request.path.to_string_lossy().into_owned(),
                );
                self.surfaces.remove(&key);
                #[cfg(feature = "glow")]
                self.callbacks.retain(|_, (s, _)| *s != key);
                self.plugin.file_closed(&request.path);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    #[cfg(feature = "glow")]
    fn paint(&mut self, id: u64, info: &abi::PaintInfo) {
        let Some((_, callback)) = self.callbacks.get(&id) else {
            return;
        };
        let get = info.get_proc_address;
        let gl = self.gl.get_or_insert_with(|| {
            // SAFETY: the IDE's OpenGL context is current, and `get` finds its functions.
            Arc::new(unsafe { glow::Context::from_loader_function_cstr(|name| get(name.as_ptr())) })
        });
        let egui_info = egui::PaintCallbackInfo {
            viewport: rect(info.viewport),
            clip_rect: rect(info.clip),
            pixels_per_point: info.pixels_per_point,
            screen_size_px: [info.screen_width, info.screen_height],
        };
        (callback.0)(&egui_info, gl);
    }
}

/// A panic of the plugin stops at the interface: the plugin is then failed, and answers nothing.
fn guard<T>(failed: &Mutex<bool>, f: impl FnOnce() -> T) -> Option<T> {
    if *failed.lock().unwrap() {
        return None;
    }
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => Some(value),
        Err(_) => {
            *failed.lock().unwrap() = true;
            None
        }
    }
}

#[doc(hidden)]
pub struct Handle {
    instance: Instance,
    failed: Mutex<bool>,
}

/// Makes a plugin for the IDE: what [`crate::export_plugin!`] calls.
///
/// # Safety
/// `host` is valid until [`destroy`].
#[doc(hidden)]
pub unsafe fn create(
    host: *const abi::HostApi,
    make: impl FnOnce() -> Box<dyn Plugin>,
) -> *mut c_void {
    let Ok(plugin) = catch_unwind(AssertUnwindSafe(make)) else {
        return std::ptr::null_mut();
    };
    // SAFETY: the caller's.
    let host = unsafe { Host::new(host) };
    Box::into_raw(Box::new(Handle {
        instance: Instance::new(plugin, host),
        failed: Mutex::new(false),
    }))
    .cast()
}

/// # Safety
/// `plugin` comes from [`create`], and is not used after.
#[doc(hidden)]
pub unsafe extern "C" fn destroy(plugin: *mut c_void) {
    if !plugin.is_null() {
        // SAFETY: the caller's.
        let handle = unsafe { Box::from_raw(plugin.cast::<Handle>()) };
        let _ = catch_unwind(AssertUnwindSafe(move || drop(handle)));
    }
}

/// # Safety
/// `plugin` comes from [`create`]; `method` and `input` are valid for the call.
#[doc(hidden)]
pub unsafe extern "C" fn call(
    plugin: *mut c_void,
    method: abi::Str,
    input: abi::Str,
) -> abi::Buffer {
    // SAFETY: the caller's.
    let handle = unsafe { &mut *plugin.cast::<Handle>() };
    // SAFETY: the caller's.
    let (method, input) = unsafe { (method.as_str(), input.as_bytes()) };
    match guard(&handle.failed, || handle.instance.call(method, input)) {
        Some(bytes) if !bytes.is_empty() => abi::Buffer::from_vec(bytes),
        _ => abi::Buffer::EMPTY,
    }
}

/// # Safety
/// `buffer` comes from [`call`].
#[doc(hidden)]
pub unsafe extern "C" fn free(buffer: abi::Buffer) {
    // SAFETY: the caller's.
    drop(unsafe { buffer.into_vec() });
}

/// # Safety
/// `plugin` comes from [`create`]; `input` and its pointers are valid for the call.
#[doc(hidden)]
pub unsafe extern "C" fn frame(
    plugin: *mut c_void,
    input: *const abi::FrameInput,
) -> *const abi::FrameOutput {
    // SAFETY: the caller's.
    let handle = unsafe { &mut *plugin.cast::<Handle>() };
    // SAFETY: the caller's.
    let input = unsafe { &*input };
    // SAFETY: the caller's.
    guard(&handle.failed, || unsafe {
        handle.instance.frame(input) as *const abi::FrameOutput
    })
    .unwrap_or(std::ptr::null())
}

/// # Safety
/// `plugin` comes from [`create`]; `info` is valid for the call, with the OpenGL context of the
/// IDE current.
#[doc(hidden)]
pub unsafe extern "C" fn paint(plugin: *mut c_void, callback: u64, info: *const abi::PaintInfo) {
    // SAFETY: the caller's.
    let handle = unsafe { &mut *plugin.cast::<Handle>() };
    // SAFETY: the caller's.
    let info = unsafe { &*info };
    #[cfg(feature = "glow")]
    guard(&handle.failed, || handle.instance.paint(callback, info));
    #[cfg(not(feature = "glow"))]
    let _ = (handle, callback, info);
}

/// Exports a plugin from a library: `export_plugin!(MyPlugin::default);`, with a function or
/// closure that makes it.
#[macro_export]
macro_rules! export_plugin {
    ($make:expr) => {
        unsafe extern "C" fn __atelier_create(
            host: *const $crate::abi::HostApi,
        ) -> *mut ::std::ffi::c_void {
            // SAFETY: the IDE keeps `host` until it destroys the plugin.
            unsafe {
                $crate::runtime::create(host, || {
                    ::std::boxed::Box::new(($make)()) as ::std::boxed::Box<dyn $crate::Plugin>
                })
            }
        }

        #[unsafe(no_mangle)]
        pub static ATELIER_PLUGIN_V2: $crate::abi::PluginApi = $crate::abi::PluginApi {
            abi: $crate::abi::VERSION,
            size: ::std::mem::size_of::<$crate::abi::PluginApi>(),
            create: __atelier_create,
            destroy: $crate::runtime::destroy,
            call: $crate::runtime::call,
            free: $crate::runtime::free,
            frame: $crate::runtime::frame,
            paint: $crate::runtime::paint,
        };
    };
}
