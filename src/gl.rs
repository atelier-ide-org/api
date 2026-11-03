//! OpenGL in a surface: a [`Callback`] painted in a rectangle, called with the OpenGL context of
//! the IDE (through a `glow::Context` of the plugin, loaded from the IDE's functions).
//!
//! The callback leaves the OpenGL state as egui expects it: egui sets its own bindings again
//! before its next mesh, but not the rest (enabled tests, the framebuffer, the viewport).

use std::sync::Arc;

pub use glow;

/// What [`paint_callback`] paints with.
#[allow(clippy::type_complexity)]
pub struct Callback(pub(crate) Box<dyn Fn(&egui::PaintCallbackInfo, &glow::Context) + Send + Sync>);

/// A shape that calls `paint` with the OpenGL context of the IDE when it paints `rect`.
pub fn paint_callback(
    rect: egui::Rect,
    paint: impl Fn(&egui::PaintCallbackInfo, &glow::Context) + Send + Sync + 'static,
) -> egui::PaintCallback {
    egui::PaintCallback {
        rect,
        callback: Arc::new(Callback(Box::new(paint))),
    }
}

/// The version of GLSL the context reads, as egui_glow finds it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ShaderVersion {
    Gl120,
    Gl140,
    Es100,
    Es300,
}

impl ShaderVersion {
    pub fn get(gl: &glow::Context) -> Self {
        use glow::HasContext as _;
        // SAFETY: a query of the current context.
        Self::parse(&unsafe { gl.get_parameter_string(glow::SHADING_LANGUAGE_VERSION) })
    }

    pub fn parse(glsl: &str) -> Self {
        let Some(start) = glsl.find(|c: char| c.is_ascii_digit()) else {
            return Self::Gl120;
        };
        let es = glsl[..start].contains(" ES ");
        let version = glsl[start..]
            .split_once(' ')
            .map_or(&glsl[start..], |x| x.0);
        let mut parts = version
            .splitn(3, '.')
            .map(|x| x.parse::<u8>().unwrap_or_default());
        let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        if es {
            if major >= 3 { Self::Es300 } else { Self::Es100 }
        } else if major > 1 || (major == 1 && minor >= 40) {
            Self::Gl140
        } else {
            Self::Gl120
        }
    }

    pub fn version_declaration(&self) -> &'static str {
        match self {
            Self::Gl120 => "#version 120\n",
            Self::Gl140 => "#version 140\n",
            Self::Es100 => "#version 100\n",
            Self::Es300 => "#version 300 es\n",
        }
    }

    pub fn is_new_shader_interface(&self) -> bool {
        matches!(self, Self::Es300 | Self::Gl140)
    }
}

#[cfg(test)]
mod tests {
    use super::ShaderVersion::{self, *};

    #[test]
    fn shader_versions_are_read() {
        for (s, v) in [
            ("1.2 OpenGL foo bar", Gl120),
            ("3.0", Gl140),
            ("4.60 NVIDIA", Gl140),
            ("OpenGL ES GLSL ES 1.00 foo bar", Es100),
            ("WebGL GLSL ES 3.00", Es300),
            ("", Gl120),
        ] {
            assert_eq!(ShaderVersion::parse(s), v, "{s}");
        }
    }
}
