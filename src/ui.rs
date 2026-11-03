//! The look of the IDE for what a plugin draws: its colors ([`theme`]), its fonts
//! ([`semibold`]), its icons ([`icon`], drawn by the IDE) and the widgets it draws differently
//! from egui (the same code as the IDE's): 13 px text, 16 px icons, 24 px rows, 4 to 8 px radii.

use crate::{protocol, runtime::Host};
use egui::{
    Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2,
};
use std::collections::HashMap;

/// Height of tree, list and table rows.
pub const ROW: f32 = 24.0;
/// Size of toolbar icon buttons.
pub const BUTTON: f32 = 26.0;

/// The colors of the IDE ([`protocol::Theme`]) as egui's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub frame: Color32,
    pub island: Color32,
    pub popup: Color32,
    pub border: Color32,
    pub separator: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub selection: Color32,
    pub selection_inactive: Color32,
    pub tool_open: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub link: Color32,
    pub text_selection: Color32,
    pub field: Color32,
    pub added: Color32,
    pub modified: Color32,
    pub deleted: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
}

impl From<protocol::Theme> for Theme {
    fn from(t: protocol::Theme) -> Self {
        let c = |[r, g, b, a]: protocol::Color| Color32::from_rgba_unmultiplied(r, g, b, a);
        Self {
            dark: t.dark,
            frame: c(t.frame),
            island: c(t.island),
            popup: c(t.popup),
            border: c(t.border),
            separator: c(t.separator),
            hover: c(t.hover),
            pressed: c(t.pressed),
            selection: c(t.selection),
            selection_inactive: c(t.selection_inactive),
            tool_open: c(t.tool_open),
            accent: c(t.accent),
            accent_hover: c(t.accent_hover),
            text: c(t.text),
            text_secondary: c(t.text_secondary),
            text_disabled: c(t.text_disabled),
            link: c(t.link),
            text_selection: c(t.text_selection),
            field: c(t.field),
            added: c(t.added),
            modified: c(t.modified),
            deleted: c(t.deleted),
            success: c(t.success),
            warning: c(t.warning),
            error: c(t.error),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        protocol::Theme::default().into()
    }
}

fn theme_id() -> egui::Id {
    egui::Id::new("atelier_theme")
}

/// Gives `ctx` the look of the IDE without an IDE (tests): its default theme, and egui's fonts
/// with the families of the IDE ([`semibold`]).
pub fn install(ctx: &egui::Context) {
    let theme = Theme::default();
    ctx.set_visuals(if theme.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
    ctx.set_fonts(crate::runtime::font_definitions(None));
    set_theme(ctx, theme);
}

pub(crate) fn set_theme(ctx: &egui::Context, theme: Theme) {
    ctx.data_mut(|d| d.insert_temp(theme_id(), theme));
}

/// The colors of the IDE.
pub fn theme(ctx: &egui::Context) -> Theme {
    ctx.data(|d| d.get_temp(theme_id())).unwrap_or_default()
}

/// The semibold interface font of the IDE, for titles.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(crate::runtime::SEMIBOLD.into()))
}

/// The language of the interface (`en_US`, `fr_FR`).
pub fn language(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::new("atelier_language")))
        .unwrap_or_else(|| "en_US".into())
}

type PictureMap = HashMap<(String, bool, u32), Option<TextureHandle>>;

#[derive(Clone, Default)]
struct Pictures(std::sync::Arc<std::sync::Mutex<PictureMap>>);

/// A picture drawn by the IDE, as a texture of this surface: an icon of the IDE, or an SVG of
/// the plugin (`svg`, with a `key` of its own).
fn picture(ctx: &egui::Context, key: &str, svg: Option<&str>, size: Vec2) -> Option<TextureHandle> {
    let dark = theme(ctx).dark;
    let scale = ctx.pixels_per_point();
    let px = ((size.x.max(size.y)) * scale).round().max(1.0) as u32;
    let pictures = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Pictures>(egui::Id::new("atelier_pictures"))
            .clone()
    });
    let mut pictures = pictures.0.lock().unwrap();
    pictures
        .entry((key.to_string(), dark, px))
        .or_insert_with(|| {
            let host = ctx.data(|d| d.get_temp::<Host>(egui::Id::new("atelier_host")))?;
            let bytes = match svg {
                Some(svg) => host.call_json_bytes(
                    protocol::SVG,
                    &protocol::SvgRequest {
                        svg: svg.to_string(),
                        width: (size.x * scale).round().max(1.0) as u32,
                        height: (size.y * scale).round().max(1.0) as u32,
                    },
                ),
                None => host.call_json_bytes(
                    protocol::ICON,
                    &protocol::IconRequest {
                        name: key.to_string(),
                        dark,
                        size: px,
                    },
                ),
            };
            let (width, height, pixels) = protocol::decode_image(&bytes)?;
            let pixels = pixels
                .chunks_exact(4)
                .map(|p| Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
                .collect();
            let image = egui::ColorImage::new([width as usize, height as usize], pixels);
            Some(ctx.load_texture(format!("atelier://{key}"), image, TextureOptions::LINEAR))
        })
        .clone()
}

/// The icons of the plugin ([`register_icon`]), by name: an SVG for every theme, or one for
/// the light themes and one for the dark ones.
type Icons = HashMap<String, (&'static str, Option<&'static str>)>;

static ICONS: std::sync::RwLock<Option<Icons>> = std::sync::RwLock::new(None);

/// Adds an icon of the plugin, which [`icon`] and the other functions find by `name` as an icon
/// of the IDE. `svg_dark` is the one of the dark themes, when it differs.
pub fn register_icon(name: &str, svg: &'static str, svg_dark: Option<&'static str>) {
    ICONS
        .write()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(name.to_string(), (svg, svg_dark));
}

/// A 16 px icon of the plugin ([`register_icon`]) or of the IDE (`run`, `refresh`, `add`…),
/// for the current theme.
pub fn icon(ui: &Ui, name: &str) -> egui::Image<'static> {
    sized_icon(ui, name, Vec2::splat(16.0))
}

/// An icon as [`icon`], at another size.
pub fn sized_icon(ui: &Ui, name: &str, size: Vec2) -> egui::Image<'static> {
    let own = ICONS
        .read()
        .unwrap()
        .as_ref()
        .and_then(|icons| icons.get(name).copied());
    let texture = match own {
        Some((light, dark)) => {
            let svg = match (theme(ui.ctx()).dark, dark) {
                (true, Some(dark)) => dark,
                _ => light,
            };
            picture(ui.ctx(), &format!("own:{name}"), Some(svg), size)
        }
        None => picture(ui.ctx(), name, None, size),
    };
    match texture {
        Some(texture) => egui::Image::from_texture((texture.id(), size)),
        None => egui::Image::from_texture((egui::TextureId::default(), Vec2::ZERO)),
    }
}

/// An SVG of the plugin, drawn by the IDE at `size` points. `key` names it (one per picture).
pub fn svg(ui: &Ui, key: &str, svg: &str, size: Vec2) -> egui::Image<'static> {
    match picture(ui.ctx(), key, Some(svg), size) {
        Some(texture) => egui::Image::from_texture((texture.id(), size)),
        None => egui::Image::from_texture((egui::TextureId::default(), Vec2::ZERO)),
    }
}

pub fn paint_icon(ui: &Ui, name: &str, rect: Rect) {
    icon(ui, name).paint_at(ui, rect);
}

/// The text of "Cancel" in the language of the interface.
fn cancel_text(ui: &Ui) -> &'static str {
    match language(ui.ctx()).as_str() {
        "fr_FR" => "Annuler",
        _ => "Cancel",
    }
}

/// Background of a hovered or pressed flat control.
pub fn paint_hover(ui: &Ui, rect: Rect, response: &Response, selected: bool) {
    let t = theme(ui.ctx());
    let fill = if response.is_pointer_button_down_on() {
        t.pressed
    } else if selected {
        t.tool_open
    } else if response.hovered() {
        t.hover
    } else {
        return;
    };
    ui.painter().rect_filled(rect, 5.0, fill);
}

/// Toolbar button: a 16 px icon, a rounded background on hover, a tooltip.
pub fn icon_button(ui: &mut Ui, name: &str, tooltip: &str) -> Response {
    icon_toggle(ui, name, false, tooltip)
}

pub fn icon_toggle(ui: &mut Ui, name: &str, selected: bool, tooltip: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(BUTTON), Sense::click());
    paint_hover(ui, rect, &response, selected);
    paint_icon(
        ui,
        name,
        Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
    );
    if tooltip.is_empty() {
        response
    } else {
        response.on_hover_text(tooltip)
    }
}

/// Disabled-looking icon button that cannot be clicked.
pub fn icon_button_enabled(ui: &mut Ui, name: &str, enabled: bool, tooltip: &str) -> Response {
    ui.add_enabled_ui(enabled, |ui| icon_button(ui, name, tooltip))
        .inner
}

/// Flat button with an icon and a label, as in tool window toolbars and title bar widgets.
pub fn icon_text_button(ui: &mut Ui, name: Option<&str>, text: &str, chevron: bool) -> Response {
    let t = theme(ui.ctx());
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), FontId::proportional(13.0), t.text);
    let icon_w = if name.is_some() { 22.0 } else { 0.0 };
    let chevron_w = if chevron { 18.0 } else { 0.0 };
    let size = vec2(16.0 + icon_w + galley.size().x + chevron_w, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    paint_hover(ui, rect, &response, false);
    let mut x = rect.left() + 8.0;
    if let Some(name) = name {
        paint_icon(
            ui,
            name,
            Rect::from_min_size(pos2(x, rect.center().y - 8.0), Vec2::splat(16.0)),
        );
        x += icon_w;
    }
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley.clone(),
        t.text,
    );
    if chevron {
        let cx = x + galley.size().x + 4.0;
        paint_icon(
            ui,
            "chevronDown",
            Rect::from_min_size(pos2(cx, rect.center().y - 8.0), Vec2::splat(16.0)),
        );
    }
    response
}

fn text_button(ui: &mut Ui, text: &str, primary: bool) -> Response {
    let t = theme(ui.ctx());
    let color = if primary { Color32::WHITE } else { t.text };
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), FontId::proportional(13.0), color);
    let size = vec2((galley.size().x + 28.0).max(72.0), 28.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    let painter = ui.painter();
    if primary {
        let fill = if response.hovered() && enabled {
            t.accent_hover
        } else {
            t.accent
        };
        painter.rect_filled(
            rect,
            5.0,
            if enabled {
                fill
            } else {
                fill.gamma_multiply(0.5)
            },
        );
    } else {
        let fill = if response.hovered() && enabled {
            t.hover
        } else {
            Color32::TRANSPARENT
        };
        painter.rect(
            rect,
            5.0,
            fill,
            Stroke::new(1.0_f32, t.border),
            StrokeKind::Inside,
        );
    }
    let color = if enabled { color } else { t.text_disabled };
    painter.galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// Blue button of the main action of a dialog or a tool window.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    text_button(ui, text, true)
}

/// Outlined button.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    text_button(ui, text, false)
}

/// IntelliJ checkbox: blue when checked. `None` draws the indeterminate state.
pub fn check_state(ui: &mut Ui, state: Option<bool>, text: &str) -> Response {
    let t = theme(ui.ctx());
    let galley = (!text.is_empty()).then(|| {
        ui.painter()
            .layout_no_wrap(text.into(), FontId::proportional(13.0), t.text)
    });
    let width = 16.0 + galley.as_ref().map_or(0.0, |g| g.size().x + 8.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click());
    let boxed = Rect::from_min_size(pos2(rect.left(), rect.center().y - 8.0), Vec2::splat(16.0));
    let painter = ui.painter();
    let on = state != Some(false);
    if on {
        painter.rect_filled(
            boxed,
            4.0,
            if response.hovered() {
                t.accent_hover
            } else {
                t.accent
            },
        );
        let stroke = Stroke::new(1.6_f32, Color32::WHITE);
        if state.is_none() {
            painter.line_segment(
                [
                    boxed.center() - vec2(4.0, 0.0),
                    boxed.center() + vec2(4.0, 0.0),
                ],
                stroke,
            );
        } else {
            let c = boxed.center();
            painter.line_segment([c + vec2(-4.0, 0.0), c + vec2(-1.2, 3.0)], stroke);
            painter.line_segment([c + vec2(-1.2, 3.0), c + vec2(4.2, -3.2)], stroke);
        }
    } else {
        let border = if response.hovered() {
            t.accent
        } else {
            t.border
        };
        painter.rect(
            boxed,
            4.0,
            t.field,
            Stroke::new(1.0_f32, border),
            StrokeKind::Inside,
        );
    }
    if let Some(galley) = galley {
        painter.galley(
            pos2(boxed.right() + 8.0, rect.center().y - galley.size().y / 2.0),
            galley,
            t.text,
        );
    }
    response
}

pub fn checkbox(ui: &mut Ui, value: &mut bool, text: &str) -> Response {
    let mut response = check_state(ui, Some(*value), text);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response
}

/// Radio button with the IntelliJ look.
pub fn radio(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let t = theme(ui.ctx());
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), FontId::proportional(13.0), t.text);
    let (rect, response) =
        ui.allocate_exact_size(vec2(24.0 + galley.size().x, 22.0), Sense::click());
    let center = pos2(rect.left() + 8.0, rect.center().y);
    let painter = ui.painter();
    if selected {
        painter.circle_filled(center, 8.0, t.accent);
        painter.circle_filled(center, 3.0, Color32::WHITE);
    } else {
        let border = if response.hovered() {
            t.accent
        } else {
            t.border
        };
        painter.circle(center, 7.5, t.field, Stroke::new(1.0_f32, border));
    }
    painter.galley(
        pos2(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0),
        galley,
        t.text,
    );
    response
}

pub fn radio_value<T: PartialEq>(ui: &mut Ui, current: &mut T, value: T, text: &str) -> Response {
    let mut response = radio(ui, *current == value, text);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

/// Rounded search field with a magnifier, as in IntelliJ popups and tool windows.
pub fn search_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let t = theme(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    let edit = egui::TextEdit::singleline(text)
        .hint_text(hint)
        .frame(false)
        .vertical_align(egui::Align::Center)
        .margin(egui::Margin {
            left: 28,
            right: 8,
            top: 0,
            bottom: 0,
        });
    let response = ui.put(rect, edit);
    let border = if response.has_focus() {
        t.accent
    } else {
        t.border
    };
    ui.painter()
        .rect_stroke(rect, 5.0, Stroke::new(1.0_f32, border), StrokeKind::Inside);
    paint_icon(
        ui,
        "search",
        Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.center().y - 8.0),
            Vec2::splat(16.0),
        ),
    );
    response
}

/// Chevron of a tree node, 16 px, pointing right or down.
pub fn paint_chevron(ui: &Ui, center: Pos2, open: bool) {
    let name = if open { "chevronDown" } else { "chevronRight" };
    paint_icon(ui, name, Rect::from_center_size(center, Vec2::splat(16.0)));
}

/// A full-width row of a tree or list. Paints the hover and selection background.
pub fn row(ui: &mut Ui, selected: bool) -> (Rect, Response) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    paint_row(ui, rect, selected, response.hovered());
    (rect, response)
}

pub fn paint_row(ui: &Ui, rect: Rect, selected: bool, hovered: bool) {
    let t = theme(ui.ctx());
    let fill = if selected {
        t.selection
    } else if hovered {
        t.hover
    } else {
        return;
    };
    ui.painter()
        .rect_filled(rect.shrink2(vec2(4.0, 0.0)), 4.0, fill);
}

/// Text drawn left-aligned and vertically centered in a row; returns its right edge.
pub fn paint_text(ui: &Ui, rect: Rect, x: f32, text: &str, color: Color32, font: FontId) -> f32 {
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    let width = galley.size().x;
    ui.painter().with_clip_rect(rect).galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    x + width
}

/// Rounded label, e.g. a branch or tag on a commit.
pub fn paint_badge(
    ui: &Ui,
    painter: &egui::Painter,
    x: f32,
    center_y: f32,
    text: &str,
    color: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.into(), FontId::proportional(11.5), color);
    let rect = Rect::from_min_size(
        pos2(x, center_y - galley.size().y / 2.0 - 2.0),
        galley.size() + vec2(10.0, 4.0),
    );
    let _ = ui;
    painter.rect(
        rect,
        4.0,
        color.gamma_multiply(0.14),
        Stroke::new(1.0_f32, color.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + vec2(5.0, 2.0), galley, color);
    rect.right()
}

/// Small secondary title inside a tool window or dialog.
pub fn section(ui: &mut Ui, text: &str) {
    let t = theme(ui.ctx());
    ui.label(
        RichText::new(text)
            .font(semibold(12.0))
            .color(t.text_secondary),
    );
}

/// Title row of a dialog.
pub fn dialog_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(semibold(15.0)));
    ui.add_space(6.0);
}

/// Buttons of a dialog, right-aligned, the primary one last as in IntelliJ on Windows.
/// Enter confirms and Escape cancels. Returns (confirmed, cancelled).
pub fn dialog_buttons(ui: &mut Ui, primary: &str) -> (bool, bool) {
    ui.add_space(10.0);
    let mut ok = ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
    let mut cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        cancel |= button(ui, cancel_text(ui)).clicked();
        ok |= primary_button(ui, primary).clicked();
    });
    (ok, cancel)
}

// ---- Menus --------------------------------------------------------------------------------

/// Width menus open at, and height of their entries.
pub const MENU_WIDTH: f32 = 230.0;
const MENU_ROW: f32 = 28.0;

/// Call first in a context menu or popup: sets the width and tightens the spacing.
pub fn menu_begin(ui: &mut Ui) {
    ui.set_min_width(MENU_WIDTH);
    ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
}

/// A menu entry: optional icon, label, optional shortcut on the right. `danger` colors it as
/// an error (delete…). Closes the menu on click, as the caller reads `clicked()`.
pub fn menu_item(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    shortcut: Option<&str>,
    danger: bool,
) -> Response {
    menu_entry(ui, icon, label, shortcut, danger, true)
}

pub fn menu_entry(
    ui: &mut Ui,
    icon: Option<&str>,
    label: &str,
    shortcut: Option<&str>,
    danger: bool,
    enabled: bool,
) -> Response {
    let t = theme(ui.ctx());
    let width = ui.available_width().max(MENU_WIDTH - 12.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, MENU_ROW), sense);
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 6.0, t.selection);
    }
    let color = match (enabled, danger) {
        (false, _) => t.text_secondary.gamma_multiply(0.6),
        (true, true) => t.error,
        (true, false) => t.text,
    };
    if let Some(icon) = icon {
        let slot =
            Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), vec2(16.0, 16.0));
        let image = self::icon(ui, icon);
        if enabled {
            image.paint_at(ui, slot);
        } else {
            image
                .tint(Color32::from_white_alpha(100))
                .paint_at(ui, slot);
        }
    }
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        color,
    );
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(12.0),
            t.text_secondary,
        );
    }
    response
}

/// Thin line between groups of entries.
pub fn menu_separator(ui: &mut Ui) {
    let t = theme(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(
        rect.left() + 8.0..=rect.right() - 8.0,
        rect.center().y,
        Stroke::new(1.0_f32, t.separator),
    );
}

/// Small grey title of a group of entries.
pub fn menu_title(ui: &mut Ui, text: &str) {
    let t = theme(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().text(
        pos2(rect.left() + 10.0, rect.center().y + 2.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(11.5),
        t.text_secondary,
    );
}

/// Text with a vertical separator before, as between status bar widgets.
pub fn status_text(ui: &mut Ui, text: &str) -> Response {
    let t = theme(ui.ctx());
    let galley =
        ui.painter()
            .layout_no_wrap(text.into(), FontId::proportional(12.5), t.text_secondary);
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 14.0, 24.0), Sense::click());
    paint_hover(ui, rect, &response, false);
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        t.text_secondary,
    );
    response
}

/// Status bar text of a fixed width, the one `widest` needs, aligned right: a value that changes
/// every second (memory, processor) must not push its neighbours around.
pub fn status_text_fixed(ui: &mut Ui, text: &str, widest: &str) -> Response {
    let t = theme(ui.ctx());
    let font = FontId::proportional(12.5);
    let width = ui
        .painter()
        .layout_no_wrap(widest.into(), font.clone(), t.text_secondary)
        .size()
        .x;
    let galley = ui
        .painter()
        .layout_no_wrap(text.into(), font, t.text_secondary);
    let (rect, response) = ui.allocate_exact_size(
        vec2(width.max(galley.size().x) + 14.0, 24.0),
        Sense::hover(),
    );
    ui.painter().galley(
        egui::pos2(
            rect.right() - 7.0 - galley.size().x,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        t.text_secondary,
    );
    response
}

/// Paints `text` centered in `rect`, in the secondary color.
pub fn paint_centered(ui: &Ui, rect: Rect, text: &str) {
    let t = theme(ui.ctx());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(13.0),
        t.text_secondary,
    );
}
