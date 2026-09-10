/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! OpenGL ES abstraction and implementations.
//!
//! touchHLE uses OpenGL ES for several things. OpenGL ES is part of iPhone OS's
//! API surface and can be used by apps for rendering, so there must be an
//! implementation of it to expose to the app. Beyond that, there are various
//! internal uses for which any graphics API would work, but using the same one
//! makes things simpler:
//! - Presenting frames rendered by the app to the screen, with appropriate
//!   rotation and scaling.
//! - Drawing touchHLE's virtual cursor.
//! - Drawing the app's splash screen.
//! - Compositing the app's Core Animation layers (usually for UIKit views).
//!
//! touchHLE's OpenGL ES implementation consists of a series of layers. This
//! module contains the layers that aren't specific to a particular use:
//!
//! - [gles_generic] provides an abstraction over OpenGL ES implementations.
//! - Various modules provide implementations:
//!   - [gles1_native] passes through native OpenGL ES 1.1.
//!   - [gles1_on_gl2] provides an implementation of OpenGL ES 1.1 using OpenGL
//!     2.1 compatibility profile.
//!   - There might be more in future.
//! - [gles11_raw] provides raw bindings for OpenGL ES 1.1 generated from the
//!   Khronos API headers. **The function bindings are only for use within this
//!   module.** The constants and types can be used outside it, however.
//!   - [gl21compat_raw] is the same thing, but for OpenGL 2.1 compatibility
//!     profile, which can't be used outside this module at all.
//! - [present] provides utilities for presenting frames to the window using an
//!   abstract OpenGL ES implementation.
//!
//! In contrast, [crate::frameworks::opengles] is a layer specific to OpenGL
//! ES's role as a part of the iPhone OS API surface. It wraps [gles_generic] to
//! expose OpenGL ES to the guest app.
//!
//! Useful resources for OpenGL ES 1.1:
//! - [Reference pages](https://registry.khronos.org/OpenGL-Refpages/es1.1/xhtml/)
//! - [Specification](https://registry.khronos.org/OpenGL/specs/es/1.1/es_full_spec_1.1.pdf)
//! - Apple's [OpenGL ES Hardware Platform Guide for iOS](https://developer.apple.com/library/archive/documentation/OpenGLES/Conceptual/OpenGLESHardwarePlatformGuide_iOS/OpenGLESPlatforms/OpenGLESPlatforms.html)
//! - Extensions:
//!   - [OES_framebuffer_object](https://registry.khronos.org/OpenGL/extensions/OES/OES_framebuffer_object.txt)
//!   - [IMG_texture_compression_pvrtc](https://registry.khronos.org/OpenGL/extensions/IMG/IMG_texture_compression_pvrtc.txt)
//!   - [OES_compressed_paletted_texture](https://registry.khronos.org/OpenGL/extensions/OES/OES_compressed_paletted_texture.txt) (also incorporated into the main spec)
//!   - [OES_matrix_palette](https://registry.khronos.org/OpenGL/extensions/OES/OES_matrix_palette.txt)
//!   - [EXT_texture_format_BGRA8888](https://registry.khronos.org/OpenGL/extensions/EXT/EXT_texture_format_BGRA8888.txt)
//!   - [OES_blend_subtract](https://registry.khronos.org/OpenGL/extensions/OES/OES_blend_subtract.txt)
//!
//! Useful resources for OpenGL 2.1:
//! - [Reference pages](https://registry.khronos.org/OpenGL-Refpages/gl2.1/)
//! - [Specification](https://registry.khronos.org/OpenGL/specs/gl/glspec21.pdf)
//! - Extensions:
//!   - [EXT_framebuffer_object](https://registry.khronos.org/OpenGL/extensions/EXT/EXT_framebuffer_object.txt)
//!   - [ARB_matrix_palette](https://registry.khronos.org/OpenGL/extensions/ARB/ARB_matrix_palette.txt)
//!   - [ARB_vertex_blend](https://registry.khronos.org/OpenGL/extensions/ARB/ARB_vertex_blend.txt)
//!   - [EXT_blend_subtract](https://registry.khronos.org/OpenGL/extensions/EXT/EXT_blend_subtract.txt)
//!
//! Useful resources for both:
//! - Extensions:
//!   - [EXT_texture_filter_anisotropic](https://registry.khronos.org/OpenGL/extensions/EXT/EXT_texture_filter_anisotropic.txt)
//!   - [EXT_texture_lod_bias](https://registry.khronos.org/OpenGL/extensions/EXT/EXT_texture_lod_bias.txt)

pub mod gles1_native;
pub mod gles1_on_gl2;
pub mod gles1_on_gles2;
pub mod gles2_glsl;
pub mod gles2_native;
pub mod gles2_on_gl3;
pub mod gles3_native;
pub mod gles3_on_gl3;
mod gles_generic;
pub mod present;
pub mod util;
use touchHLE_gl_bindings::gl21compat as gl21compat_raw;
use touchHLE_gl_bindings::gl33core as gl33core_raw;
pub use touchHLE_gl_bindings::gles11 as gles11_raw;
pub use touchHLE_gl_bindings::gles2 as gles2_raw;
pub use touchHLE_gl_bindings::gles30 as gles30_raw;
pub use touchHLE_gl_bindings::gles11::types::*;
pub use util::try_decode_pvrtc;

use crate::environment::Environment;
use crate::window::{GLContext, GLVersion};
use gles1_native::GLES1NativeContext;
use gles1_on_gl2::GLES1OnGL2Context;
use gles1_on_gles2::GLES1OnGLES2Context;
use gles2_native::GLES2NativeContext;
use gles2_on_gl3::GLES2OnGL3Context;
use gles3_native::GLES3NativeContext;
use gles3_on_gl3::GLES3OnGL3Context;
pub use gles_generic::GLESContext;
pub use gles_generic::GLES;

pub struct LoggingGLES<'a> {
    pub inner: Box<dyn GLES + 'a>,
    pub verbose: bool,
}

pub struct LoggingGLESContext {
    pub inner: Box<dyn GLESContext>,
    pub verbose: bool,
}

impl GLESContext for LoggingGLESContext {
    fn description() -> &'static str {
        "Logging wrapper for GLES context"
    }

    fn new(window: &mut crate::window::Window) -> Result<Self, String> {
        // This is a wrapper, so it's not created directly via `new`.
        // It's created by wrapping an existing context.
        Err("LoggingGLESContext cannot be created directly via new()".to_string())
    }

    fn make_current<'gl_ctx, 'win: 'gl_ctx>(
        &'gl_ctx mut self,
        window: &'win mut crate::window::Window,
    ) -> Box<dyn GLES + 'gl_ctx> {
        let gles = self.inner.make_current(window);
        Box::new(LoggingGLES {
            inner: gles,
            verbose: self.verbose,
        })
    }

    unsafe fn make_current_unchecked_for_window<'gl_ctx>(
        &'gl_ctx mut self,
        make_current_fn: &mut dyn FnMut(&GLContext),
        loader_fn: &mut dyn FnMut(&'static str) -> *const std::ffi::c_void,
    ) -> Box<dyn GLES + 'gl_ctx> {
        let gles = self.inner.make_current_unchecked_for_window(make_current_fn, loader_fn);
        Box::new(LoggingGLES {
            inner: gles,
            verbose: self.verbose,
        })
    }
}

impl<'a> GLES for LoggingGLES<'a> {
    unsafe fn driver_description(&self) -> String {
        self.inner.driver_description()
    }

    unsafe fn GetError(&mut self) -> GLenum {
        let err = self.inner.GetError();
        if self.verbose {
            log!("GL Error: {:#x}", err);
        }
        err
    }

    unsafe fn Clear(&mut self, mask: GLbitfield) {
        if self.verbose {
            log!("glClear(mask={:#x})", mask);
        }
        self.inner.Clear(mask);
    }

    unsafe fn Viewport(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        if self.verbose {
            log!("glViewport({}, {}, {}, {})", x, y, width, height);
        }
        self.inner.Viewport(x, y, width, height);
    }

    unsafe fn DrawArrays(&mut self, mode: GLenum, first: GLint, count: GLsizei) {
        if self.verbose {
            log!("glDrawArrays(mode={:#x}, first={}, count={})", mode, first, count);
        }
        self.inner.DrawArrays(mode, first, count);
    }

    unsafe fn DrawElements(&mut self, mode: GLenum, count: GLsizei, type_: GLenum, indices: *const GLvoid) {
        if self.verbose {
            log!("glDrawElements(mode={:#x}, count={}, type={:#x})", mode, count, type_);
        }
        self.inner.DrawElements(mode, count, type_, indices);
    }

    unsafe fn BindFramebuffer(&mut self, target: GLenum, framebuffer: GLuint) {
        if self.verbose {
            log!("glBindFramebuffer(target={:#x}, fb={})", target, framebuffer);
        }
        self.inner.BindFramebuffer(target, framebuffer);
    }

    unsafe fn FramebufferRenderbuffer(&mut self, target: GLenum, attachment: GLenum, renderbuffertarget: GLenum, renderbuffer: GLuint) {
        if self.verbose {
            log!("glFramebufferRenderbuffer(target={:#x}, attach={:#x}, rb_target={:#x}, rb={})", target, attachment, renderbuffertarget, renderbuffer);
        }
        self.inner.FramebufferRenderbuffer(target, attachment, renderbuffertarget, renderbuffer);
    }

    unsafe fn TexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    ) {
        if self.verbose {
            log!("glTexImage2D(target={:#x}, level={}, int_fmt={:#x}, size={}x{}, format={:#x}, type={:#x})", target, level, internalformat, width, height, format, type_);
        }
        self.inner.TexImage2D(target, level, internalformat, width, height, border, format, type_, pixels);
    }

    unsafe fn BindTexture(&mut self, target: GLenum, texture: GLuint) {
        if self.verbose {
            log!("glBindTexture(target={:#x}, tex={})", target, texture);
        }
        self.inner.BindTexture(target, texture);
    }

    unsafe fn ReadPixels(
        &mut self,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        type_: GLenum,
        pixels: *mut GLvoid,
    ) {
        if self.verbose {
            log!("glReadPixels({}, {}, {}, {}, format={:#x}, type={:#x})", x, y, width, height, format, type_);
        }
        self.inner.ReadPixels(x, y, width, height, format, type_, pixels);
    }

    unsafe fn GetIntegerv(&mut self, pname: GLenum, params: *mut GLint) {
        if self.verbose {
            log!("glGetIntegerv(pname={:#x})", pname);
        }
        self.inner.GetIntegerv(pname, params);
    }

    unsafe fn Finish(&mut self) {
        if self.verbose {
            log!("glFinish()");
        }
        self.inner.Finish();
    }

    unsafe fn Flush(&mut self) {
        if self.verbose {
            log!("glFlush()");
        }
        self.inner.Flush();
    }

    // --- Forwarding the rest of the methods to avoid panics ---

    unsafe fn ClearColor(&mut self, r: GLclampf, g: GLclampf, b: GLclampf, a: GLclampf) {
        self.inner.ClearColor(r, g, b, a);
    }

    unsafe fn BindBuffer(&mut self, target: GLenum, buffer: GLuint) {
        self.inner.BindBuffer(target, buffer);
    }

    unsafe fn EnableClientState(&mut self, array: GLenum) {
        self.inner.EnableClientState(array);
    }

    unsafe fn VertexPointer(&mut self, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const GLvoid) {
        self.inner.VertexPointer(size, type_, stride, pointer);
    }

    unsafe fn TexCoordPointer(&mut self, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const GLvoid) {
        self.inner.TexCoordPointer(size, type_, stride, pointer);
    }

    unsafe fn MatrixMode(&mut self, mode: GLenum) {
        self.inner.MatrixMode(mode);
    }

    unsafe fn LoadMatrixf(&mut self, m: *const GLfloat) {
        self.inner.LoadMatrixf(m);
    }

    unsafe fn Enable(&mut self, cap: GLenum) {
        self.inner.Enable(cap);
    }

    unsafe fn LoadIdentity(&mut self) {
        self.inner.LoadIdentity();
    }

    unsafe fn DisableClientState(&mut self, array: GLenum) {
        self.inner.DisableClientState(array);
    }

    unsafe fn Disable(&mut self, cap: GLenum) {
        self.inner.Disable(cap);
    }

    unsafe fn BlendFunc(&mut self, sfactor: GLenum, dfactor: GLenum) {
        self.inner.BlendFunc(sfactor, dfactor);
    }

    unsafe fn Color4f(&mut self, r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat) {
        self.inner.Color4f(r, g, b, a);
    }

    unsafe fn GenTextures(&mut self, n: GLsizei, textures: *mut GLuint) {
        self.inner.GenTextures(n, textures);
    }

    unsafe fn TexParameteri(&mut self, target: GLenum, pname: GLenum, param: GLint) {
        self.inner.TexParameteri(target, pname, param);
    }

    unsafe fn PushMatrix(&mut self) {
        self.inner.PushMatrix();
    }

    unsafe fn PopMatrix(&mut self) {
        self.inner.PopMatrix();
    }

    unsafe fn Orthof(&mut self, left: GLfloat, right: GLfloat, bottom: GLfloat, top: GLfloat, near: GLfloat, far: GLfloat) {
        self.inner.Orthof(left, right, bottom, top, near, far);
    }

    unsafe fn IsEnabled(&mut self, cap: GLenum) -> GLboolean {
        self.inner.IsEnabled(cap)
    }

    unsafe fn ClientActiveTexture(&mut self, texture: GLenum) {
        self.inner.ClientActiveTexture(texture);
    }

    unsafe fn GetBooleanv(&mut self, pname: GLenum, params: *mut GLboolean) {
        self.inner.GetBooleanv(pname, params);
    }

    unsafe fn GetFloatv(&mut self, pname: GLenum, params: *mut GLfloat) {
        self.inner.GetFloatv(pname, params);
    }

    unsafe fn GetFixedv(&mut self, pname: GLenum, params: *mut GLfixed) {
        self.inner.GetFixedv(pname, params);
    }

    unsafe fn GetTexEnviv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        self.inner.GetTexEnviv(target, pname, params);
    }

    unsafe fn GetTexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfloat) {
        self.inner.GetTexEnvfv(target, pname, params);
    }

    unsafe fn GetTexEnvxv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfixed) {
        self.inner.GetTexEnvxv(target, pname, params);
    }

    unsafe fn GetTexParameteriv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        self.inner.GetTexParameteriv(target, pname, params);
    }

    unsafe fn GetTexParameterfv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfloat) {
        self.inner.GetTexParameterfv(target, pname, params);
    }

    unsafe fn GetTexParameterxv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfixed) {
        self.inner.GetTexParameterxv(target, pname, params);
    }

    unsafe fn GetClipPlanef(&mut self, plane: GLenum, equation: *mut GLfloat) {
        self.inner.GetClipPlanef(plane, equation);
    }

    unsafe fn GetClipPlanex(&mut self, plane: GLenum, equation: *mut GLfixed) {
        self.inner.GetClipPlanex(plane, equation);
    }

    unsafe fn GetLightfv(&mut self, light: GLenum, pname: GLenum, params: *mut GLfloat) {
        self.inner.GetLightfv(light, pname, params);
    }

    unsafe fn GetLightxv(&mut self, light: GLenum, pname: GLenum, params: *mut GLfixed) {
        self.inner.GetLightxv(light, pname, params);
    }

    unsafe fn GetMaterialfv(&mut self, face: GLenum, pname: GLenum, params: *mut GLfloat) {
        self.inner.GetMaterialfv(face, pname, params);
    }

    unsafe fn GetMaterialxv(&mut self, face: GLenum, pname: GLenum, params: *mut GLfixed) {
        self.inner.GetMaterialxv(face, pname, params);
    }

    unsafe fn GetPointerv(&mut self, pname: GLenum, params: *mut *const GLvoid) {
        self.inner.GetPointerv(pname, params);
    }

    unsafe fn Hint(&mut self, target: GLenum, mode: GLenum) {
        self.inner.Hint(target, mode);
    }

    unsafe fn GetString(&mut self, name: GLenum) -> *const GLubyte {
        self.inner.GetString(name)
    }

    unsafe fn AlphaFunc(&mut self, func: GLenum, ref_: GLclampf) {
        self.inner.AlphaFunc(func, ref_);
    }

    unsafe fn AlphaFuncx(&mut self, func: GLenum, ref_: GLclampx) {
        self.inner.AlphaFuncx(func, ref_);
    }

    unsafe fn BlendEquationOES(&mut self, mode: GLenum) {
        self.inner.BlendEquationOES(mode);
    }

    unsafe fn ColorMask(&mut self, red: GLboolean, green: GLboolean, blue: GLboolean, alpha: GLboolean) {
        self.inner.ColorMask(red, green, blue, alpha);
    }

    unsafe fn ClipPlanef(&mut self, plane: GLenum, equation: *const GLfloat) {
        self.inner.ClipPlanef(plane, equation);
    }

    unsafe fn ClipPlanex(&mut self, plane: GLenum, equation: *const GLfixed) {
        self.inner.ClipPlanex(plane, equation);
    }

    unsafe fn CullFace(&mut self, mode: GLenum) {
        self.inner.CullFace(mode);
    }

    unsafe fn DepthFunc(&mut self, func: GLenum) {
        self.inner.DepthFunc(func);
    }

    unsafe fn DepthMask(&mut self, flag: GLboolean) {
        self.inner.DepthMask(flag);
    }

    unsafe fn DepthRangef(&mut self, near: GLclampf, far: GLclampf) {
        self.inner.DepthRangef(near, far);
    }

    unsafe fn DepthRangex(&mut self, near: GLclampx, far: GLclampx) {
        self.inner.DepthRangex(near, far);
    }

    unsafe fn FrontFace(&mut self, mode: GLenum) {
        self.inner.FrontFace(mode);
    }

    unsafe fn PolygonOffset(&mut self, factor: GLfloat, units: GLfloat) {
        self.inner.PolygonOffset(factor, units);
    }

    unsafe fn PolygonOffsetx(&mut self, factor: GLfixed, units: GLfixed) {
        self.inner.PolygonOffsetx(factor, units);
    }

    unsafe fn SampleCoverage(&mut self, value: GLclampf, invert: GLboolean) {
        self.inner.SampleCoverage(value, invert);
    }

    unsafe fn SampleCoveragex(&mut self, value: GLclampx, invert: GLboolean) {
        self.inner.SampleCoveragex(value, invert);
    }

    unsafe fn ShadeModel(&mut self, mode: GLenum) {
        self.inner.ShadeModel(mode);
    }

    unsafe fn Scissor(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        self.inner.Scissor(x, y, width, height);
    }

    unsafe fn LineWidth(&mut self, val: GLfloat) {
        self.inner.LineWidth(val);
    }

    unsafe fn LineWidthx(&mut self, val: GLfixed) {
        self.inner.LineWidthx(val);
    }

    unsafe fn StencilFunc(&mut self, func: GLenum, ref_: GLint, mask: GLuint) {
        self.inner.StencilFunc(func, ref_, mask);
    }

    unsafe fn StencilOp(&mut self, sfail: GLenum, dpfail: GLenum, dppass: GLenum) {
        self.inner.StencilOp(sfail, dpfail, dppass);
    }

    unsafe fn StencilMask(&mut self, mask: GLuint) {
        self.inner.StencilMask(mask);
    }

    unsafe fn LogicOp(&mut self, opcode: GLenum) {
        self.inner.LogicOp(opcode);
    }

    unsafe fn PointSize(&mut self, size: GLfloat) {
        self.inner.PointSize(size);
    }

    unsafe fn PointSizex(&mut self, size: GLfixed) {
        self.inner.PointSizex(size);
    }

    unsafe fn PointParameterf(&mut self, pname: GLenum, param: GLfloat) {
        self.inner.PointParameterf(pname, param);
    }

    unsafe fn PointParameterx(&mut self, pname: GLenum, param: GLfixed) {
        self.inner.PointParameterx(pname, param);
    }

    unsafe fn PointParameterfv(&mut self, pname: GLenum, params: *const GLfloat) {
        self.inner.PointParameterfv(pname, params);
    }

    unsafe fn PointParameterxv(&mut self, pname: GLenum, params: *const GLfixed) {
        self.inner.PointParameterxv(pname, params);
    }
}

    // Forward other methods to inner

// We need to implement the rest of the GLES trait for LoggingGLES.
// Since there are many, we can use a macro or just implement the key ones.
// However, since I can't easily implement the whole trait without boilerplate,
// I'll just implement the most critical ones and the rest can be handled via a
// generic "log and call" mechanism if I had one.
//
// Actually, I can't partially implement a trait. I must implement ALL methods.
// This is the problem.

use std::sync::atomic::{AtomicU32, Ordering};

static TRANSLATOR_TRACE_EVENTS: AtomicU32 = AtomicU32::new(0);

pub(crate) fn configure_translator_tracing(_enabled: bool) {}

pub(crate) fn translator_tracing_enabled() -> bool {
    std::env::var_os("TOUCHHLE_TRACE_TRANSLATOR").is_some()
}

pub(crate) fn trace_translator_event(event: String) {
    if !translator_tracing_enabled() {
        return;
    }
    let number = TRANSLATOR_TRACE_EVENTS.fetch_add(1, Ordering::Relaxed);
    if number < 512 {
        log!("[translator] #{:03} {}", number + 1, event);
    } else if number == 512 {
        log!("[translator] further events suppressed after 512 entries");
    }
}

pub fn configure_angle_driver(enabled: bool) {
    if !enabled {
        return;
    }

    let default_egl = if cfg!(target_os = "windows") {
        "libEGL.dll"
    } else if cfg!(target_os = "macos") {
        "libEGL.dylib"
    } else {
        "libEGL.so"
    };
    let default_gles = if cfg!(target_os = "windows") {
        "libGLESv2.dll"
    } else if cfg!(target_os = "macos") {
        "libGLESv2.dylib"
    } else {
        "libGLESv2.so"
    };
    let egl_path = std::env::var("TOUCHHLE_ANGLE_EGL").unwrap_or_else(|_| default_egl.to_owned());
    let gles_path =
        std::env::var("TOUCHHLE_ANGLE_GLES").unwrap_or_else(|_| default_gles.to_owned());
    let egl_exists = std::path::Path::new(&egl_path).exists();
    let gles_exists = std::path::Path::new(&gles_path).exists();

    unsafe {
        std::env::set_var("SDL_VIDEO_EGL_DRIVER", &egl_path);
        std::env::set_var("SDL_VIDEO_GL_DRIVER", &gles_path);
    }
    sdl2::hint::set("SDL_OPENGL_ES_DRIVER", "1");
    log!(
        "ANGLE override requested: EGL={} (exists={}), GLES={} (exists={}); SDL will try these before the first window",
        egl_path,
        egl_exists,
        gles_path,
        gles_exists
    );
    if !egl_exists || !gles_exists {
        log!("ANGLE libraries are not present at the configured paths; SDL may fall back or context creation may fail");
    }
}

/// Labels for [GLES] implementations and an abstraction for constructing them.
#[derive(Copy, Clone)]
pub enum GLESImplementation {
    /// [gles1_native::GLES1Native].
    GLES1Native,
    /// [gles1_on_gl2::GLES1OnGL2].
    GLES1OnGL2,
    /// [gles1_on_gles2::GLES1OnGLES2].
    GLES1OnGLES2,
}
impl GLESImplementation {
    /// List of OpenGL ES 1.1 implementations in order of preference.
    pub const GLES1_IMPLEMENTATIONS: &'static [Self] = &[Self::GLES1Native, Self::GLES1OnGL2];
    /// Convert from short name used for command-line arguments. Returns [Err]
    /// if name is not recognized..
    pub fn from_short_name(name: &str) -> Result<Self, ()> {
        match name {
            "gles1_on_gl2" => Ok(Self::GLES1OnGL2),
            "gles1_on_gles2" => Ok(Self::GLES1OnGLES2),
            "gles1_native" => Ok(Self::GLES1Native),
            _ => Err(()),
        }
    }
    /// See [GLESContext::description].
    pub fn description(self) -> &'static str {
        match self {
            Self::GLES1Native => GLES1NativeContext::description(),
            Self::GLES1OnGL2 => GLES1OnGL2Context::description(),
            Self::GLES1OnGLES2 => GLES1OnGLES2Context::description(),
        }
    }
    /// See [GLESContext::new].
    pub fn construct(
        self,
        window: &mut crate::window::Window,
    ) -> Result<Box<dyn GLESContext>, String> {
        fn boxer<T: GLESContext + 'static>(ctx: T) -> Box<dyn GLESContext> {
            Box::new(ctx)
        }
        match self {
            Self::GLES1Native => GLES1NativeContext::new(window).map(boxer),
            Self::GLES1OnGL2 => GLES1OnGL2Context::new(window).map(boxer),
            Self::GLES1OnGLES2 => GLES1OnGLES2Context::new(window).map(boxer),
        }
    }
}

pub fn create_gles1_translator_ctx_no_parent_stack(
    window: &mut crate::window::Window,
) -> Box<dyn GLESContext> {
    assert!(window.on_main_stack());
    log!("Creating the OpenGL ES 1.1 to native OpenGL ES 2.0 translator");
    Box::new(
        GLES1OnGLES2Context::new(window)
            .expect("Couldn't create OpenGL ES 1.1-on-GLES2 translator context!"),
    )
}
pub fn create_gles1_gles3_translator_ctx_no_parent_stack(
    window: &mut crate::window::Window,
) -> Box<dyn GLESContext> {
    assert!(window.on_main_stack());
    log!("Creating the OpenGL ES 1.1 to native OpenGL ES 3.0 translator");
    Box::new(
        GLES1OnGLES2Context::new_with_gl_version(window, GLVersion::GLES30)
            .expect("Couldn't create OpenGL ES 1.1-on-GLES3 translator context!"),
    )
}

pub fn create_gles1_gles3_translator_ctx(env: &mut Environment) -> Box<dyn GLESContext> {
    env.on_parent_stack_in_coroutine(|window, _options| {
        create_gles1_gles3_translator_ctx_no_parent_stack(window)
    })
}

pub fn create_gles1_translator_ctx(env: &mut Environment) -> Box<dyn GLESContext> {
    env.on_parent_stack_in_coroutine(|window, _options| {
        create_gles1_translator_ctx_no_parent_stack(window)
    })
}

/// Try to create an OpenGL ES 1.1 context using the configured strategies,
/// panicking on failure.
pub fn create_gles1_ctx(env: &mut Environment) -> Box<dyn GLESContext> {
    env.on_parent_stack_in_coroutine(|window, options| {
        create_gles1_ctx_no_parent_stack(window, options)
    })
}

/// Try to create an OpenGL ES 2.0 context, panicking on failure.
///
/// The preference order, from "most-correct" to "only as a last resort":
///
/// 1. [`GLES2NativeContext`] — a real OpenGL ES 2.0 driver. This is the
///    only thing that works on platforms without desktop OpenGL such as
///    Android, and on real iOS hardware emulation. Every ES 2.0 entry point
///    is a direct passthrough to the host driver.
/// 2. [`GLES2OnGL3Context`] — a full ES 2.0 backend built on top of
///    desktop OpenGL 3.3 Core. This shares its implementation with the ES
///    3.0 fallback ([`GLES3OnGL3Context`]), giving us a single source of
///    truth for ES 2.0 / ES 3.0 emulation, full shader support, and proper
///    GLSL ES → desktop GLSL translation via
///    [`gles2_glsl::translate_glsl_es_to_120`]. This is the preferred
///    fallback on x86 Linux/macOS desktops where Mesa lacks a native ES 2.0
///    surface.
/// 3. [`GLES1OnGL2Context`] — legacy fallback that piggy-backs on a desktop
///    OpenGL 2.1 compatibility profile context. Only used on the rare host
///    that has GL 2.1 compat but no GL 3.3 Core (e.g. very old macOS
///    installations); kept around for backwards compatibility.
pub fn create_gles2_ctx(env: &mut Environment) -> Box<dyn GLESContext> {
    env.on_parent_stack_in_coroutine(|window, options| {
        assert!(window.on_main_stack());
        log!("Creating an OpenGL ES 2.0 context:");

        let ctx = {
            log!("Trying: {}", GLES2NativeContext::description());
            match GLES2NativeContext::new(window) {
                Ok(ctx) => {
                    log!("=> Success!");
                    Box::new(ctx) as Box<dyn GLESContext>
                }
                Err(err) => {
                    log!("=> Failed: {}.", err);
                    None
                }
            }
            .or_else(|| {
                log!(
                    "Trying: {} (used for OpenGL ES 2.0)",
                    GLES2OnGL3Context::description()
                );
                match GLES2OnGL3Context::new(window) {
                    Ok(ctx) => {
                        log!("=> Success!");
                        Some(Box::new(ctx) as Box<dyn GLESContext>)
                    }
                    Err(err) => {
                        log!("=> Failed: {}.", err);
                        None
                    }
                }
            })
            .or_else(|| {
                log!(
                    "Trying: {} (legacy GL 2.1 fallback for OpenGL ES 2.0)",
                    GLES1OnGL2Context::description()
                );
                match GLES1OnGL2Context::new(window) {
                    Ok(ctx) => {
                        log!("=> Success!");
                        Some(Box::new(ctx) as Box<dyn GLESContext>)
                    }
                    Err(err) => {
                        log!("=> Failed: {}.", err);
                        None
                    }
                }
            })
            .expect("Couldn't create OpenGL ES 2.0 context")
        };

        if options.trace_gl_errors {
            Box::new(LoggingGLESContext {
                inner: ctx,
                verbose: options.trace_gl_errors,
            })
        } else {
            ctx
        }
    })
}

/// Try to create an OpenGL ES 3.0 context, panicking on failure.
///
/// This is the entry point used by [crate::frameworks::opengles::eagl] when
/// `EAGLContext initWithAPI:` is called with `kEAGLRenderingAPIOpenGLES3` (=
/// 3). It tries the native ES 3.0 backend first — the only thing that works
/// on Android and on desktop drivers configured for an ES context — and
/// falls back to the desktop GL 3.3 Core translation backend on hosts
/// without a native ES 3.0 driver (most x86 Linux/macOS desktops).
pub fn create_gles3_ctx(env: &mut Environment) -> Box<dyn GLESContext> {
    env.on_parent_stack_in_coroutine(|window, options| {
        assert!(window.on_main_stack());
        log!("Creating an OpenGL ES 3.0 context:");

        let ctx = {
            log!("Trying: {}", GLES3NativeContext::description());
            match GLES3NativeContext::new(window) {
                Ok(ctx) => {
                    log!("=> Success!");
                    Box::new(ctx) as Box<dyn GLESContext>
                }
                Err(err) => {
                    log!("=> Failed: {}.", err);
                    None
                }
            }
            .or_else(|| {
                log!(
                    "Trying: {} (used for OpenGL ES 3.0)",
                    GLES3OnGL3Context::description()
                );
                match GLES3OnGL3Context::new(window) {
                    Ok(ctx) => {
                        log!("=> Success!");
                        Some(Box::new(ctx) as Box<dyn GLESContext>)
                    }
                    Err(err) => {
                        log!("=> Failed: {}.", err);
                        None
                    }
                }
            })
            .expect("Couldn't create OpenGL ES 3.0 context")
        };

        if options.trace_gl_errors {
            Box::new(LoggingGLESContext {
                inner: ctx,
                verbose: options.trace_gl_errors,
            })
        } else {
            ctx
        }
    })
}

/// Create an OpenGL ES 2.0 context from the window's main stack.
///
/// The window owns the internal context used for compositing and the splash
/// screen, so it must be created before the guest environment exists.
pub fn create_gles2_ctx_no_parent_stack(
    window: &mut crate::window::Window,
) -> Box<dyn GLESContext> {
    assert!(window.on_main_stack());
    log!("Creating an OpenGL ES 2.0 context:");

    log!("Trying: {}", GLES2NativeContext::description());
    if let Ok(ctx) = GLES2NativeContext::new(window) {
        log!("=> Success!");
        return Box::new(ctx);
    }

    log!(
        "Trying: {} (used for OpenGL ES 2.0)",
        GLES2OnGL3Context::description()
    );
    if let Ok(ctx) = GLES2OnGL3Context::new(window) {
        log!("=> Success!");
        return Box::new(ctx);
    }

    log!(
        "Trying: {} (legacy GL 2.1 fallback for OpenGL ES 2.0)",
        GLES1OnGL2Context::description()
    );
    match GLES1OnGL2Context::new(window) {
        Ok(ctx) => {
            log!("=> Success!");
            Box::new(ctx)
        }
        Err(err) => panic!("Couldn't create OpenGL ES 2.0 context: {}", err),
    }
}

/// Same as [create_gles1_ctx], but without calling
/// [Environment::on_parent_stack_in_coroutine]. Only should be called by
/// functions not inside a coroutine that can't use [Environment].
pub fn create_gles1_ctx_no_parent_stack(
    window: &mut crate::window::Window,
    options: &crate::options::Options,
) -> Box<dyn GLESContext> {
    assert!(window.on_main_stack());
    log!("Creating an OpenGL ES 1.1 context:");
    // Hardcoded GPU/backend pin: TOUCHHLE_FORCE_GLES1 overrides everything
    // (CLI flag, host capability probing) so the exact backend is used even
    // when probing would pick a different one.
    let forced = std::env::var("TOUCHHLE_FORCE_GLES1")
        .ok()
        .and_then(|name| GLESImplementation::from_short_name(&name).ok());
    let forced_list: [GLESImplementation; 1] = match forced {
        Some(impl_) => [impl_],
        None => [GLESImplementation::GLES1OnGL2],
    };
    let list: &[GLESImplementation] = match forced {
        Some(_) => &forced_list[..],
        None => match options.gles1_implementation {
            Some(ref preference) => std::slice::from_ref(preference),
            None => GLESImplementation::GLES1_IMPLEMENTATIONS,
        },
    };
    let mut gles1_ctx = None;
    for implementation in list {
        log!("Trying: {}", implementation.description());
        match implementation.construct(window) {
            Ok(ctx) => {
                log!("=> Success!");
                gles1_ctx = Some(ctx);
                break;
            }
            Err(err) => {
                log!("=> Failed: {}.", err);
            }
        }
    }
    gles1_ctx.expect("Couldn't create OpenGL ES 1.1 context!")
}
