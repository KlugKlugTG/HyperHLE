/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Passthrough for a native OpenGL ES 1.1 driver.
//!
//! Unlike for the GLES1-on-GL2 driver, there's almost no validation of
//! arguments here, because we assume the driver is complete and the app uses it
//! correctly. The exception is where we expect an extension could be used that
//! the driver might not support (e.g. vendor-specific texture compression).
//! In such cases, we should reject vendor-specific things unless we've made
//! sure we can emulate them on all host platforms for touchHLE.

use super::gles11_raw as gles11;
use super::gles11_raw::types::*;
use super::gles_generic::GLES;
use super::util::{try_decode_pvrtc, PalettedTextureFormat};
use super::GLESContext;
use crate::window::{GLContext, GLVersion, Window};
use std::ffi::CStr;
use std::marker::PhantomData;

// FboStructFix
pub struct GLES1NativeContext {
    gl_ctx: GLContext,
    is_loaded: bool,
    is_gles2: bool,
}
impl GLESContext for GLES1NativeContext {
    fn description() -> &'static str {
        "Native OpenGL ES 1.1"
    }

    // FboNewFix
    fn new(window: &mut Window, options: &crate::options::Options) -> Result<Self, String> {
        // PassOptions
        let is_gles2 = options.gles_version == 2;
        let version = if is_gles2 {
            GLVersion::GLES20 // DynamicEsTwo
        } else {
            GLVersion::GLES11 // DynamicEsOne
        };
        Ok(Self {
            gl_ctx: window.create_gl_context(version)?,
            is_loaded: false,
            is_gles2,
        })
    }

    // MakeCurrentFix
    fn make_current<'gl_ctx, 'win: 'gl_ctx>(
        &'gl_ctx mut self,
        window: &'win mut Window,
    ) -> Box<dyn GLES + 'gl_ctx> {
        if self.gl_ctx.is_current() && self.is_loaded {
            return Box::new(GLES1Native {
                _gl_lifetime: PhantomData,
                is_gles2: self.is_gles2,
            });
        }

        unsafe {
            window.make_gl_context_current(&self.gl_ctx);
        }
        gles11::load_with(|s| window.gl_get_proc_address(s));
        // LoadEsTwo
        touchHLE_gl_bindings::gles2::load_with(|s| window.gl_get_proc_address(s));
        self.is_loaded = true;
        Box::new(GLES1Native {
            _gl_lifetime: PhantomData,
            is_gles2: self.is_gles2,
        })
    }

    // MakeCurrentUncheckedFix
    unsafe fn make_current_unchecked_for_window<'gl_ctx>(
        &'gl_ctx mut self,
        make_current_fn: &mut dyn FnMut(&GLContext),
        loader_fn: &mut dyn FnMut(&'static str) -> *const std::ffi::c_void,
    ) -> Box<dyn GLES + 'gl_ctx> {
        if self.gl_ctx.is_current() && self.is_loaded {
            return Box::new(GLES1Native {
                _gl_lifetime: PhantomData,
                is_gles2: self.is_gles2,
            });
        }

        make_current_fn(&self.gl_ctx);
        // LoadEsTwo
        gles11::load_with(&mut *loader_fn);
        touchHLE_gl_bindings::gles2::load_with(loader_fn);
        self.is_loaded = true;
        Box::new(GLES1Native {
            _gl_lifetime: PhantomData,
            is_gles2: self.is_gles2,
        })
    }
}

// GlesNativeStructFix
pub struct GLES1Native<'gl_ctx> {
    _gl_lifetime: PhantomData<&'gl_ctx ()>,
    is_gles2: bool,
}

// EsTwoCheckImpl
impl GLES for GLES1Native<'_> {
    fn is_es2(&self) -> bool {
        self.is_gles2
    }
    unsafe fn driver_description(&self) -> String {
        let version = CStr::from_ptr(gles11::GetString(gles11::VERSION) as *const _);
        let vendor = CStr::from_ptr(gles11::GetString(gles11::VENDOR) as *const _);
        let renderer = CStr::from_ptr(gles11::GetString(gles11::RENDERER) as *const _);
        // OpenGL ES requires the version to be prefixed "OpenGL ES", so we
        // don't need to contextualize it.
        format!(
            "{} / {} / {}",
            version.to_string_lossy(),
            vendor.to_string_lossy(),
            renderer.to_string_lossy()
        )
    }

    // Generic state manipulation
    unsafe fn GetError(&mut self) -> GLenum {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetError()
        } else {
            gles11::GetError()
        }
    }
    unsafe fn Enable(&mut self, cap: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Enable(cap)
        } else {
            gles11::Enable(cap)
        }
    }
    unsafe fn IsEnabled(&mut self, cap: GLenum) -> GLboolean {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::IsEnabled(cap)
        } else {
            gles11::IsEnabled(cap)
        }
    }
    unsafe fn Disable(&mut self, cap: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Disable(cap)
        } else {
            gles11::Disable(cap)
        }
    }
    unsafe fn ClientActiveTexture(&mut self, texture: GLenum) {
        gles11::ClientActiveTexture(texture);
    }
    // AliasClientState
    unsafe fn EnableClientState(&mut self, array: GLenum) {
        if self.is_gles2 {
            match array {
                gles11::VERTEX_ARRAY => touchHLE_gl_bindings::gles2::EnableVertexAttribArray(0),
                gles11::NORMAL_ARRAY => touchHLE_gl_bindings::gles2::EnableVertexAttribArray(1),
                gles11::COLOR_ARRAY => touchHLE_gl_bindings::gles2::EnableVertexAttribArray(2),
                gles11::TEXTURE_COORD_ARRAY => {
                    let mut ct = 0;
                    gles11::GetIntegerv(gles11::CLIENT_ACTIVE_TEXTURE, &mut ct);
                    let attr = if ct == gles11::TEXTURE1 as GLint {
                        4
                    } else {
                        3
                    };
                    touchHLE_gl_bindings::gles2::EnableVertexAttribArray(attr);
                }
                _ => {}
            }
        } else {
            gles11::EnableClientState(array)
        }
    }
    unsafe fn DisableClientState(&mut self, array: GLenum) {
        if self.is_gles2 {
            match array {
                gles11::VERTEX_ARRAY => touchHLE_gl_bindings::gles2::DisableVertexAttribArray(0),
                gles11::NORMAL_ARRAY => touchHLE_gl_bindings::gles2::DisableVertexAttribArray(1),
                gles11::COLOR_ARRAY => touchHLE_gl_bindings::gles2::DisableVertexAttribArray(2),
                gles11::TEXTURE_COORD_ARRAY => {
                    let mut ct = 0;
                    gles11::GetIntegerv(gles11::CLIENT_ACTIVE_TEXTURE, &mut ct);
                    let attr = if ct == gles11::TEXTURE1 as GLint {
                        4
                    } else {
                        3
                    };
                    touchHLE_gl_bindings::gles2::DisableVertexAttribArray(attr);
                }
                _ => {}
            }
        } else {
            gles11::DisableClientState(array)
        }
    }
    // RouteGettersState
    unsafe fn GetBooleanv(&mut self, pname: GLenum, params: *mut GLboolean) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetBooleanv(pname, params)
        } else {
            gles11::GetBooleanv(pname, params)
        }
    }
    unsafe fn GetFloatv(&mut self, pname: GLenum, params: *mut GLfloat) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetFloatv(pname, params)
        } else {
            gles11::GetFloatv(pname, params)
        }
    }
    unsafe fn GetIntegerv(&mut self, pname: GLenum, params: *mut GLint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetIntegerv(pname, params)
        } else {
            gles11::GetIntegerv(pname, params)
        }
    }
    unsafe fn GetTexEnviv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        gles11::GetTexEnviv(target, pname, params)
    }
    unsafe fn GetTexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *mut GLfloat) {
        gles11::GetTexEnvfv(target, pname, params)
    }
    unsafe fn GetPointerv(&mut self, pname: GLenum, params: *mut *const GLvoid) {
        gles11::GetPointerv(pname, params as *mut _ as *const _)
    }
    unsafe fn Hint(&mut self, target: GLenum, mode: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Hint(target, mode)
        } else {
            gles11::Hint(target, mode)
        }
    }
    unsafe fn Finish(&mut self) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Finish()
        } else {
            gles11::Finish()
        }
    }
    unsafe fn Flush(&mut self) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Flush()
        } else {
            gles11::Flush()
        }
    }
    unsafe fn GetString(&mut self, name: GLenum) -> *const GLubyte {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetString(name)
        } else {
            gles11::GetString(name)
        }
    }

    // Other state manipulation
    unsafe fn AlphaFunc(&mut self, func: GLenum, ref_: GLclampf) {
        gles11::AlphaFunc(func, ref_)
    }
    unsafe fn AlphaFuncx(&mut self, func: GLenum, ref_: GLclampx) {
        gles11::AlphaFuncx(func, ref_)
    }
    // RouteOtherState
    unsafe fn BlendFunc(&mut self, sfactor: GLenum, dfactor: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BlendFunc(sfactor, dfactor)
        } else {
            gles11::BlendFunc(sfactor, dfactor)
        }
    }
    unsafe fn BlendEquationOES(&mut self, mode: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BlendEquation(mode)
        } else {
            gles11::BlendEquationOES(mode)
        }
    }
    unsafe fn ColorMask(
        &mut self,
        red: GLboolean,
        green: GLboolean,
        blue: GLboolean,
        alpha: GLboolean,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::ColorMask(red, green, blue, alpha)
        } else {
            gles11::ColorMask(red, green, blue, alpha)
        }
    }
    unsafe fn ClipPlanef(&mut self, plane: GLenum, equation: *const GLfloat) {
        gles11::ClipPlanef(plane, equation)
    }
    unsafe fn ClipPlanex(&mut self, plane: GLenum, equation: *const GLfixed) {
        gles11::ClipPlanex(plane, equation)
    }
    unsafe fn CullFace(&mut self, mode: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::CullFace(mode)
        } else {
            gles11::CullFace(mode)
        }
    }
    unsafe fn DepthFunc(&mut self, func: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DepthFunc(func)
        } else {
            gles11::DepthFunc(func)
        }
    }
    unsafe fn DepthMask(&mut self, flag: GLboolean) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DepthMask(flag)
        } else {
            gles11::DepthMask(flag)
        }
    }
    unsafe fn FrontFace(&mut self, mode: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::FrontFace(mode)
        } else {
            gles11::FrontFace(mode)
        }
    }
    unsafe fn DepthRangef(&mut self, near: GLclampf, far: GLclampf) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DepthRangef(near, far)
        } else {
            gles11::DepthRangef(near, far)
        }
    }
    unsafe fn DepthRangex(&mut self, near: GLclampx, far: GLclampx) {
        gles11::DepthRangex(near, far)
    }
    unsafe fn PolygonOffset(&mut self, factor: GLfloat, units: GLfloat) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::PolygonOffset(factor, units)
        } else {
            gles11::PolygonOffset(factor, units)
        }
    }
    unsafe fn PolygonOffsetx(&mut self, factor: GLfixed, units: GLfixed) {
        gles11::PolygonOffsetx(factor, units)
    }
    unsafe fn SampleCoverage(&mut self, value: GLclampf, invert: GLboolean) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::SampleCoverage(value, invert)
        } else {
            gles11::SampleCoverage(value, invert)
        }
    }
    unsafe fn SampleCoveragex(&mut self, value: GLclampx, invert: GLboolean) {
        gles11::SampleCoveragex(value, invert)
    }
    unsafe fn ShadeModel(&mut self, mode: GLenum) {
        gles11::ShadeModel(mode)
    }
    unsafe fn Scissor(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Scissor(x, y, width, height)
        } else {
            gles11::Scissor(x, y, width, height)
        }
    }
    unsafe fn Viewport(&mut self, x: GLint, y: GLint, width: GLsizei, height: GLsizei) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Viewport(x, y, width, height)
        } else {
            gles11::Viewport(x, y, width, height)
        }
    }
    unsafe fn LineWidth(&mut self, val: GLfloat) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::LineWidth(val)
        } else {
            gles11::LineWidth(val)
        }
    }
    unsafe fn LineWidthx(&mut self, val: GLfixed) {
        gles11::LineWidthx(val)
    }
    unsafe fn StencilFunc(&mut self, func: GLenum, ref_: GLint, mask: GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::StencilFunc(func, ref_, mask)
        } else {
            gles11::StencilFunc(func, ref_, mask)
        }
    }
    unsafe fn StencilFuncSeparate(&mut self, face: GLenum, func: GLenum, ref_: GLint, mask: GLuint) {
        // StencilFuncSeparateNative
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::StencilFuncSeparate(face, func, ref_, mask)
        } else {
            gles11::StencilFunc(func, ref_, mask)
        }
    }
    unsafe fn StencilOp(&mut self, sfail: GLenum, dpfail: GLenum, dppass: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::StencilOp(sfail, dpfail, dppass)
        } else {
            gles11::StencilOp(sfail, dpfail, dppass)
        }
    }
    unsafe fn StencilOpSeparate(&mut self, face: GLenum, sfail: GLenum, dpfail: GLenum, dppass: GLenum) {
        // StencilOpSeparateNative
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::StencilOpSeparate(face, sfail, dpfail, dppass)
        } else {
            gles11::StencilOp(sfail, dpfail, dppass)
        }
    }
    unsafe fn StencilMask(&mut self, mask: GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::StencilMask(mask)
        } else {
            gles11::StencilMask(mask)
        }
    }
    unsafe fn StencilMaskSeparate(&mut self, face: GLenum, mask: GLuint) {
        // StencilMaskSeparateNative
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::StencilMaskSeparate(face, mask)
        } else {
            gles11::StencilMask(mask)
        }
    }
    unsafe fn LogicOp(&mut self, opcode: GLenum) {
        gles11::LogicOp(opcode);
    }

    // Points
    unsafe fn PointSize(&mut self, size: GLfloat) {
        gles11::PointSize(size)
    }
    unsafe fn PointSizex(&mut self, size: GLfixed) {
        gles11::PointSizex(size)
    }
    unsafe fn PointParameterf(&mut self, pname: GLenum, param: GLfloat) {
        gles11::PointParameterf(pname, param)
    }
    unsafe fn PointParameterx(&mut self, pname: GLenum, param: GLfixed) {
        gles11::PointParameterx(pname, param)
    }
    unsafe fn PointParameterfv(&mut self, pname: GLenum, params: *const GLfloat) {
        gles11::PointParameterfv(pname, params)
    }
    unsafe fn PointParameterxv(&mut self, pname: GLenum, params: *const GLfixed) {
        gles11::PointParameterxv(pname, params)
    }

    // Lighting and materials
    unsafe fn Fogf(&mut self, pname: GLenum, param: GLfloat) {
        gles11::Fogf(pname, param)
    }
    unsafe fn Fogx(&mut self, pname: GLenum, param: GLfixed) {
        gles11::Fogx(pname, param)
    }
    unsafe fn Fogfv(&mut self, pname: GLenum, params: *const GLfloat) {
        gles11::Fogfv(pname, params)
    }
    unsafe fn Fogxv(&mut self, pname: GLenum, params: *const GLfixed) {
        gles11::Fogxv(pname, params)
    }
    unsafe fn Lightf(&mut self, light: GLenum, pname: GLenum, param: GLfloat) {
        gles11::Lightf(light, pname, param)
    }
    unsafe fn Lightx(&mut self, light: GLenum, pname: GLenum, param: GLfixed) {
        gles11::Lightx(light, pname, param)
    }
    unsafe fn Lightfv(&mut self, light: GLenum, pname: GLenum, params: *const GLfloat) {
        gles11::Lightfv(light, pname, params)
    }
    unsafe fn Lightxv(&mut self, light: GLenum, pname: GLenum, params: *const GLfixed) {
        gles11::Lightxv(light, pname, params)
    }
    unsafe fn LightModelf(&mut self, pname: GLenum, param: GLfloat) {
        gles11::LightModelf(pname, param)
    }
    unsafe fn LightModelx(&mut self, pname: GLenum, param: GLfixed) {
        gles11::LightModelx(pname, param)
    }
    unsafe fn LightModelfv(&mut self, pname: GLenum, params: *const GLfloat) {
        gles11::LightModelfv(pname, params)
    }
    unsafe fn LightModelxv(&mut self, pname: GLenum, params: *const GLfixed) {
        gles11::LightModelxv(pname, params)
    }
    unsafe fn Materialf(&mut self, face: GLenum, pname: GLenum, param: GLfloat) {
        gles11::Materialf(face, pname, param)
    }
    unsafe fn Materialx(&mut self, face: GLenum, pname: GLenum, param: GLfixed) {
        gles11::Materialx(face, pname, param)
    }
    unsafe fn Materialfv(&mut self, face: GLenum, pname: GLenum, params: *const GLfloat) {
        gles11::Materialfv(face, pname, params)
    }
    unsafe fn Materialxv(&mut self, face: GLenum, pname: GLenum, params: *const GLfixed) {
        gles11::Materialxv(face, pname, params)
    }

    // RouteBuffersFix
    unsafe fn IsBuffer(&mut self, buffer: GLuint) -> GLboolean {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::IsBuffer(buffer)
        } else {
            gles11::IsBuffer(buffer)
        }
    }
    unsafe fn GenBuffers(&mut self, n: GLsizei, buffers: *mut GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GenBuffers(n, buffers)
        } else {
            gles11::GenBuffers(n, buffers)
        }
    }
    unsafe fn DeleteBuffers(&mut self, n: GLsizei, buffers: *const GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DeleteBuffers(n, buffers)
        } else {
            gles11::DeleteBuffers(n, buffers)
        }
    }
    unsafe fn BindBuffer(&mut self, target: GLenum, buffer: GLuint) {
        assert!(target == gles11::ARRAY_BUFFER || target == gles11::ELEMENT_ARRAY_BUFFER);
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BindBuffer(target, buffer)
        } else {
            gles11::BindBuffer(target, buffer)
        }
    }
    unsafe fn BufferData(
        &mut self,
        target: GLenum,
        size: GLsizeiptr,
        data: *const GLvoid,
        usage: GLenum,
    ) {
        assert!(target == gles11::ARRAY_BUFFER || target == gles11::ELEMENT_ARRAY_BUFFER);
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BufferData(target, size, data, usage)
        } else {
            gles11::BufferData(target, size, data, usage)
        }
    }

    unsafe fn BufferSubData(
        &mut self,
        target: GLenum,
        offset: GLintptr,
        size: GLsizeiptr,
        data: *const GLvoid,
    ) {
        assert!(target == gles11::ARRAY_BUFFER || target == gles11::ELEMENT_ARRAY_BUFFER);
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BufferSubData(target, offset, size, data)
        } else {
            gles11::BufferSubData(target, offset, size, data)
        }
    }

    // Non-pointers
    unsafe fn Color4f(&mut self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) {
        gles11::Color4f(red, green, blue, alpha)
    }
    unsafe fn Color4x(&mut self, red: GLfixed, green: GLfixed, blue: GLfixed, alpha: GLfixed) {
        gles11::Color4x(red, green, blue, alpha)
    }
    unsafe fn Color4ub(&mut self, red: GLubyte, green: GLubyte, blue: GLubyte, alpha: GLubyte) {
        gles11::Color4ub(red, green, blue, alpha)
    }
    unsafe fn Normal3f(&mut self, nx: GLfloat, ny: GLfloat, nz: GLfloat) {
        gles11::Normal3f(nx, ny, nz)
    }
    unsafe fn Normal3x(&mut self, nx: GLfixed, ny: GLfixed, nz: GLfixed) {
        gles11::Normal3x(nx, ny, nz)
    }

    // AliasPointersFix
    unsafe fn ColorPointer(
        &mut self,
        size: GLint,
        type_: GLenum,
        stride: GLsizei,
        pointer: *const GLvoid,
    ) {
        if self.is_gles2 {
            // FixPointerNorm
            let n = if type_ == 0x1401 { 1 } else { 0 };
            touchHLE_gl_bindings::gles2::VertexAttribPointer(
                2, size, type_, n as _, stride, pointer,
            );
        } else {
            gles11::ColorPointer(size, type_, stride, pointer)
        }
    }
    unsafe fn NormalPointer(&mut self, type_: GLenum, stride: GLsizei, pointer: *const GLvoid) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::VertexAttribPointer(1, 3, type_, 0, stride, pointer);
        } else {
            gles11::NormalPointer(type_, stride, pointer)
        }
    }
    unsafe fn TexCoordPointer(
        &mut self,
        size: GLint,
        type_: GLenum,
        stride: GLsizei,
        pointer: *const GLvoid,
    ) {
        if self.is_gles2 {
            let mut ct = 0;
            gles11::GetIntegerv(gles11::CLIENT_ACTIVE_TEXTURE, &mut ct);
            let attr = if ct == gles11::TEXTURE1 as GLint {
                4
            } else {
                3
            };
            touchHLE_gl_bindings::gles2::VertexAttribPointer(
                attr, size, type_, 0, stride, pointer,
            );
        } else {
            gles11::TexCoordPointer(size, type_, stride, pointer)
        }
    }
    unsafe fn VertexPointer(
        &mut self,
        size: GLint,
        type_: GLenum,
        stride: GLsizei,
        pointer: *const GLvoid,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::VertexAttribPointer(0, size, type_, 0, stride, pointer);
        } else {
            gles11::VertexPointer(size, type_, stride, pointer)
        }
    }

    // Drawing
    unsafe fn DrawArrays(&mut self, mode: GLenum, first: GLint, count: GLsizei) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DrawArrays(mode, first, count)
        } else {
            gles11::DrawArrays(mode, first, count)
        }
    }
    unsafe fn DrawElements(
        &mut self,
        mode: GLenum,
        count: GLsizei,
        type_: GLenum,
        indices: *const GLvoid,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DrawElements(mode, count, type_, indices)
        } else {
            gles11::DrawElements(mode, count, type_, indices)
        }
    }

    // Clearing
    unsafe fn Clear(&mut self, mask: GLbitfield) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::Clear(mask)
        } else {
            gles11::Clear(mask)
        }
    }
    unsafe fn ClearColor(
        &mut self,
        red: GLclampf,
        green: GLclampf,
        blue: GLclampf,
        alpha: GLclampf,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::ClearColor(red, green, blue, alpha)
        } else {
            gles11::ClearColor(red, green, blue, alpha)
        }
    }
    unsafe fn ClearColorx(
        &mut self,
        red: GLclampx,
        green: GLclampx,
        blue: GLclampx,
        alpha: GLclampx,
    ) {
        gles11::ClearColorx(red, green, blue, alpha)
    }
    unsafe fn ClearDepthf(&mut self, depth: GLclampf) {
        gles11::ClearDepthf(depth)
    }
    unsafe fn ClearDepthx(&mut self, depth: GLclampx) {
        gles11::ClearDepthx(depth)
    }
    unsafe fn ClearStencil(&mut self, s: GLint) {
        gles11::ClearStencil(s)
    }

    // Textures
    // RouteTexturesFix
    unsafe fn PixelStorei(&mut self, pname: GLenum, param: GLint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::PixelStorei(pname, param)
        } else {
            gles11::PixelStorei(pname, param)
        }
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
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::ReadPixels(x, y, width, height, format, type_, pixels)
        } else {
            gles11::ReadPixels(x, y, width, height, format, type_, pixels)
        }
    }
    unsafe fn GenTextures(&mut self, n: GLsizei, textures: *mut GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GenTextures(n, textures)
        } else {
            gles11::GenTextures(n, textures)
        }
    }
    unsafe fn DeleteTextures(&mut self, n: GLsizei, textures: *const GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DeleteTextures(n, textures)
        } else {
            gles11::DeleteTextures(n, textures)
        }
    }
    unsafe fn ActiveTexture(&mut self, texture: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::ActiveTexture(texture)
        } else {
            gles11::ActiveTexture(texture)
        }
    }
    unsafe fn IsTexture(&mut self, texture: GLuint) -> GLboolean {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::IsTexture(texture)
        } else {
            gles11::IsTexture(texture)
        }
    }
    unsafe fn BindTexture(&mut self, target: GLenum, texture: GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BindTexture(target, texture)
        } else {
            gles11::BindTexture(target, texture)
        }
    }
    unsafe fn TexParameteri(&mut self, target: GLenum, pname: GLenum, param: GLint) {
        // StripMipmapsNative
        if self.is_gles2 {
            if pname == 0x8191 || pname == 0x813D {
                return;
            }
            let mut p = param;
            if pname == gles11::TEXTURE_MIN_FILTER {
                if p == 0x2700 || p == 0x2701 {
                    p = 0x2600;
                }
                if p == 0x2702 || p == 0x2703 {
                    p = 0x2601;
                }
            }
            touchHLE_gl_bindings::gles2::TexParameteri(target, pname, p)
        } else {
            gles11::TexParameteri(target, pname, param)
        }
    }
    unsafe fn TexParameterf(&mut self, target: GLenum, pname: GLenum, param: GLfloat) {
        // StripMipmapsNative
        if self.is_gles2 {
            if pname == 0x8191 || pname == 0x813D {
                return;
            }
            let mut p = param;
            if pname == gles11::TEXTURE_MIN_FILTER {
                if p == 0x2700 as f32 || p == 0x2701 as f32 {
                    p = 0x2600 as f32;
                }
                if p == 0x2702 as f32 || p == 0x2703 as f32 {
                    p = 0x2601 as f32;
                }
            }
            touchHLE_gl_bindings::gles2::TexParameterf(target, pname, p)
        } else {
            gles11::TexParameterf(target, pname, param)
        }
    }
    unsafe fn TexParameterx(&mut self, target: GLenum, pname: GLenum, param: GLfixed) {
        gles11::TexParameterx(target, pname, param)
    }
    unsafe fn TexParameteriv(&mut self, target: GLenum, pname: GLenum, params: *const GLint) {
        gles11::TexParameteriv(target, pname, params)
    }
    unsafe fn TexParameterfv(&mut self, target: GLenum, pname: GLenum, params: *const GLfloat) {
        gles11::TexParameterfv(target, pname, params)
    }
    unsafe fn TexParameterxv(&mut self, target: GLenum, pname: GLenum, params: *const GLfixed) {
        gles11::TexParameterxv(target, pname, params)
    }
    unsafe fn TexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        mut internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    ) {
        if format == gles11::BGRA_EXT {
            // This is needed in order to avoid white screen issue on Android!
            // As per BGRA extension specs
            // https://registry.khronos.org/OpenGL/extensions/EXT/EXT_texture_format_BGRA8888.txt,
            // both internalformat and format should be BGRA
            // Tangentially related issue
            // (actually a reverse of what we're doing here)
            // https://android-review.googlesource.com/c/platform/external/qemu/+/974666
            internalformat = gles11::BGRA_EXT as GLint
        }
        // RouteTexImageGles
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::PixelStorei(gles11::UNPACK_ALIGNMENT, 1);
            touchHLE_gl_bindings::gles2::TexImage2D(
                target,
                level,
                internalformat,
                width,
                height,
                border,
                format,
                type_,
                pixels,
            );
            // SafeDefaultFilter
            let p_target = if (0x8515..=0x851A).contains(&target) {
                0x8513
            } else {
                target
            };
            touchHLE_gl_bindings::gles2::TexParameteri(
                p_target,
                gles11::TEXTURE_MIN_FILTER,
                gles11::LINEAR as _,
            );
        } else {
            gles11::TexImage2D(
                target,
                level,
                internalformat,
                width,
                height,
                border,
                format,
                type_,
                pixels,
            )
        }
    }
    unsafe fn TexSubImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    ) {
        // RouteSubImageFix
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::PixelStorei(gles11::UNPACK_ALIGNMENT, 1);
            touchHLE_gl_bindings::gles2::TexSubImage2D(
                target, level, xoffset, yoffset, width, height, format, type_, pixels,
            )
        } else {
            gles11::TexSubImage2D(
                target, level, xoffset, yoffset, width, height, format, type_, pixels,
            )
        }
    }
    unsafe fn CompressedTexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        internalformat: GLenum,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        image_size: GLsizei,
        data: *const GLvoid,
    ) {
        let data = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), image_size as usize) };
        // IMG_texture_compression_pvrtc (only on Imagination/Apple GPUs)
        // TODO: It would be more efficient to use hardware decoding where
        // available (I just don't have a suitable device to try this on)
        if try_decode_pvrtc(
            self,
            target,
            level,
            internalformat,
            width,
            height,
            border,
            data,
        ) {
            log_dbg!("Decoded PVRTC");
            return;
        }

        // OES_compressed_paletted_texture is in the common profile of OpenGL ES
        // 1.1, so we can reasonably assume it's supported.
        if PalettedTextureFormat::get_info(internalformat).is_none() {
            unimplemented!("CompressedTexImage2D internalformat: {:#x}", internalformat);
        }
        log_dbg!("Directly supported texture format: {:#x}", internalformat);
        // RouteCompTexGles
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::PixelStorei(gles11::UNPACK_ALIGNMENT, 1);
            touchHLE_gl_bindings::gles2::CompressedTexImage2D(
                target,
                level,
                internalformat,
                width,
                height,
                border,
                image_size,
                data.as_ptr() as *const _,
            );
            // SafeDefaultFilter
            let p_target = if (0x8515..=0x851A).contains(&target) {
                0x8513
            } else {
                target
            };
            touchHLE_gl_bindings::gles2::TexParameteri(
                p_target,
                gles11::TEXTURE_MIN_FILTER,
                gles11::LINEAR as _,
            );
        } else {
            gles11::CompressedTexImage2D(
                target,
                level,
                internalformat,
                width,
                height,
                border,
                image_size,
                data.as_ptr() as *const _,
            );
        }
    }
    // RouteCopyTex
    unsafe fn CopyTexImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        internalformat: GLenum,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::CopyTexImage2D(
                target,
                level,
                internalformat,
                x,
                y,
                width,
                height,
                border,
            )
        } else {
            gles11::CopyTexImage2D(target, level, internalformat, x, y, width, height, border)
        }
    }
    unsafe fn CopyTexSubImage2D(
        &mut self,
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        x: GLint,
        y: GLint,
        width: GLsizei,
        height: GLsizei,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::CopyTexSubImage2D(
                target, level, xoffset, yoffset, x, y, width, height,
            )
        } else {
            gles11::CopyTexSubImage2D(target, level, xoffset, yoffset, x, y, width, height)
        }
    }
    unsafe fn TexEnvf(&mut self, target: GLenum, pname: GLenum, param: GLfloat) {
        gles11::TexEnvf(target, pname, param)
    }
    unsafe fn TexEnvx(&mut self, target: GLenum, pname: GLenum, param: GLfixed) {
        gles11::TexEnvx(target, pname, param)
    }
    unsafe fn TexEnvi(&mut self, target: GLenum, pname: GLenum, param: GLint) {
        gles11::TexEnvi(target, pname, param)
    }
    unsafe fn TexEnvfv(&mut self, target: GLenum, pname: GLenum, params: *const GLfloat) {
        if target == gles11::TEXTURE_FILTER_CONTROL_EXT {
            assert!(pname == gles11::TEXTURE_LOD_BIAS_EXT);
            unsafe {
                if !CStr::from_ptr(gles11::GetString(gles11::EXTENSIONS) as _)
                    .to_str()
                    .unwrap()
                    .contains("EXT_texture_lod_bias")
                {
                    log_dbg!("GL_EXT_texture_lod_bias is unsupported, skipping TexEnvfv({:#x}, {:#x}, ...) call", target, pname);
                    return;
                }
            };
        }
        gles11::TexEnvfv(target, pname, params)
    }
    unsafe fn TexEnvxv(&mut self, target: GLenum, pname: GLenum, params: *const GLfixed) {
        gles11::TexEnvxv(target, pname, params)
    }
    unsafe fn TexEnviv(&mut self, target: GLenum, pname: GLenum, params: *const GLint) {
        gles11::TexEnviv(target, pname, params)
    }

    unsafe fn MultiTexCoord4f(
        &mut self,
        target: GLenum,
        s: GLfloat,
        t: GLfloat,
        r: GLfloat,
        q: GLfloat,
    ) {
        gles11::MultiTexCoord4f(target, s, t, r, q)
    }
    unsafe fn MultiTexCoord4x(
        &mut self,
        target: GLenum,
        s: GLfixed,
        t: GLfixed,
        r: GLfixed,
        q: GLfixed,
    ) {
        gles11::MultiTexCoord4x(target, s, t, r, q)
    }

    // Matrix stack operations
    unsafe fn MatrixMode(&mut self, mode: GLenum) {
        gles11::MatrixMode(mode)
    }
    unsafe fn LoadIdentity(&mut self) {
        gles11::LoadIdentity()
    }
    unsafe fn LoadMatrixf(&mut self, m: *const GLfloat) {
        gles11::LoadMatrixf(m)
    }
    unsafe fn LoadMatrixx(&mut self, m: *const GLfixed) {
        gles11::LoadMatrixx(m)
    }
    unsafe fn MultMatrixf(&mut self, m: *const GLfloat) {
        gles11::MultMatrixf(m)
    }
    unsafe fn MultMatrixx(&mut self, m: *const GLfixed) {
        gles11::MultMatrixx(m)
    }
    unsafe fn PushMatrix(&mut self) {
        gles11::PushMatrix()
    }
    unsafe fn PopMatrix(&mut self) {
        gles11::PopMatrix();
    }
    unsafe fn Orthof(
        &mut self,
        left: GLfloat,
        right: GLfloat,
        bottom: GLfloat,
        top: GLfloat,
        near: GLfloat,
        far: GLfloat,
    ) {
        gles11::Orthof(left, right, bottom, top, near, far)
    }
    unsafe fn Orthox(
        &mut self,
        left: GLfixed,
        right: GLfixed,
        bottom: GLfixed,
        top: GLfixed,
        near: GLfixed,
        far: GLfixed,
    ) {
        gles11::Orthox(left, right, bottom, top, near, far)
    }
    unsafe fn Frustumf(
        &mut self,
        left: GLfloat,
        right: GLfloat,
        bottom: GLfloat,
        top: GLfloat,
        near: GLfloat,
        far: GLfloat,
    ) {
        gles11::Frustumf(left, right, bottom, top, near, far)
    }
    unsafe fn Frustumx(
        &mut self,
        left: GLfixed,
        right: GLfixed,
        bottom: GLfixed,
        top: GLfixed,
        near: GLfixed,
        far: GLfixed,
    ) {
        gles11::Frustumx(left, right, bottom, top, near, far)
    }
    unsafe fn Rotatef(&mut self, angle: GLfloat, x: GLfloat, y: GLfloat, z: GLfloat) {
        gles11::Rotatef(angle, x, y, z)
    }
    unsafe fn Rotatex(&mut self, angle: GLfixed, x: GLfixed, y: GLfixed, z: GLfixed) {
        gles11::Rotatex(angle, x, y, z)
    }
    unsafe fn Scalef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) {
        gles11::Scalef(x, y, z)
    }
    unsafe fn Scalex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) {
        gles11::Scalex(x, y, z)
    }
    unsafe fn Translatef(&mut self, x: GLfloat, y: GLfloat, z: GLfloat) {
        gles11::Translatef(x, y, z)
    }
    unsafe fn Translatex(&mut self, x: GLfixed, y: GLfixed, z: GLfixed) {
        gles11::Translatex(x, y, z)
    }

    // EsTwoNativeFix
    unsafe fn CreateShader(&mut self, type_: GLenum) -> GLuint {
        touchHLE_gl_bindings::gles2::CreateShader(type_)
    }
    unsafe fn ShaderSource(
        &mut self,
        shader: GLuint,
        count: GLsizei,
        string: *const *const std::ffi::c_char,
        length: *const GLint,
    ) {
        touchHLE_gl_bindings::gles2::ShaderSource(shader, count, string, length)
    }
    unsafe fn CompileShader(&mut self, shader: GLuint) {
        touchHLE_gl_bindings::gles2::CompileShader(shader)
    }
    unsafe fn DeleteShader(&mut self, shader: GLuint) {
        // NativeDeleteShader
        touchHLE_gl_bindings::gles2::DeleteShader(shader)
    }
    unsafe fn GetShaderiv(&mut self, shader: GLuint, pname: GLenum, params: *mut GLint) {
        touchHLE_gl_bindings::gles2::GetShaderiv(shader, pname, params)
    }
    unsafe fn GetShaderInfoLog(
        &mut self,
        shader: GLuint,
        bufSize: GLsizei,
        length: *mut GLsizei,
        infoLog: *mut std::ffi::c_char,
    ) {
        touchHLE_gl_bindings::gles2::GetShaderInfoLog(shader, bufSize, length, infoLog)
    }
    unsafe fn CreateProgram(&mut self) -> GLuint {
        touchHLE_gl_bindings::gles2::CreateProgram()
    }
    unsafe fn DeleteProgram(&mut self, program: GLuint) {
        touchHLE_gl_bindings::gles2::DeleteProgram(program)
    }
    unsafe fn AttachShader(&mut self, program: GLuint, shader: GLuint) {
        touchHLE_gl_bindings::gles2::AttachShader(program, shader)
    }
    unsafe fn BindAttribLocation(
        &mut self,
        program: GLuint,
        index: GLuint,
        name: *const std::ffi::c_char,
    ) {
        touchHLE_gl_bindings::gles2::BindAttribLocation(program, index, name)
    }
    unsafe fn LinkProgram(&mut self, program: GLuint) {
        // RevertAttribHack
        touchHLE_gl_bindings::gles2::LinkProgram(program)
    }
    unsafe fn UseProgram(&mut self, program: GLuint) {
        touchHLE_gl_bindings::gles2::UseProgram(program)
    }
    unsafe fn GetProgramiv(&mut self, program: GLuint, pname: GLenum, params: *mut GLint) {
        touchHLE_gl_bindings::gles2::GetProgramiv(program, pname, params)
    }
    unsafe fn GetProgramInfoLog(
        &mut self,
        program: GLuint,
        bufSize: GLsizei,
        length: *mut GLsizei,
        infoLog: *mut std::ffi::c_char,
    ) {
        touchHLE_gl_bindings::gles2::GetProgramInfoLog(program, bufSize, length, infoLog)
    }
    unsafe fn VertexAttribPointer(
        &mut self,
        indx: GLuint,
        size: GLint,
        type_: GLenum,
        normalized: GLboolean,
        stride: GLsizei,
        ptr: *const GLvoid,
    ) {
        touchHLE_gl_bindings::gles2::VertexAttribPointer(
            indx, size, type_, normalized, stride, ptr,
        )
    }
    unsafe fn DisableVertexAttribArray(&mut self, index: GLuint) {
        touchHLE_gl_bindings::gles2::DisableVertexAttribArray(index)
    }
    unsafe fn EnableVertexAttribArray(&mut self, index: GLuint) {
        touchHLE_gl_bindings::gles2::EnableVertexAttribArray(index)
    }
    // AddAttribNative
    unsafe fn VertexAttrib1f(&mut self, indx: GLuint, x: GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib1f(indx, x)
    }
    unsafe fn VertexAttrib2f(&mut self, indx: GLuint, x: GLfloat, y: GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib2f(indx, x, y)
    }
    unsafe fn VertexAttrib3f(&mut self, indx: GLuint, x: GLfloat, y: GLfloat, z: GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib3f(indx, x, y, z)
    }
    unsafe fn VertexAttrib4f(
        &mut self,
        indx: GLuint,
        x: GLfloat,
        y: GLfloat,
        z: GLfloat,
        w: GLfloat,
    ) {
        touchHLE_gl_bindings::gles2::VertexAttrib4f(indx, x, y, z, w)
    }
    unsafe fn VertexAttrib1fv(&mut self, indx: GLuint, values: *const GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib1fv(indx, values)
    }
    unsafe fn VertexAttrib2fv(&mut self, indx: GLuint, values: *const GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib2fv(indx, values)
    }
    unsafe fn VertexAttrib3fv(&mut self, indx: GLuint, values: *const GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib3fv(indx, values)
    }
    unsafe fn VertexAttrib4fv(&mut self, indx: GLuint, values: *const GLfloat) {
        touchHLE_gl_bindings::gles2::VertexAttrib4fv(indx, values)
    }
    unsafe fn Uniform1i(&mut self, location: GLint, v0: GLint) {
        touchHLE_gl_bindings::gles2::Uniform1i(location, v0)
    }
    unsafe fn Uniform1f(&mut self, location: GLint, v0: GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform1f(location, v0)
    }
    unsafe fn Uniform2f(&mut self, location: GLint, v0: GLfloat, v1: GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform2f(location, v0, v1)
    }
    unsafe fn Uniform3f(&mut self, location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform3f(location, v0, v1, v2)
    }
    // UniformNativeArrays
    unsafe fn Uniform4f(
        &mut self,
        location: GLint,
        v0: GLfloat,
        v1: GLfloat,
        v2: GLfloat,
        v3: GLfloat,
    ) {
        touchHLE_gl_bindings::gles2::Uniform4f(location, v0, v1, v2, v3)
    }
    unsafe fn Uniform1fv(&mut self, location: GLint, count: GLsizei, value: *const GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform1fv(location, count, value)
    }
    unsafe fn Uniform2fv(&mut self, location: GLint, count: GLsizei, value: *const GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform2fv(location, count, value)
    }
    unsafe fn Uniform3fv(&mut self, location: GLint, count: GLsizei, value: *const GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform3fv(location, count, value)
    }
    unsafe fn Uniform4fv(&mut self, location: GLint, count: GLsizei, value: *const GLfloat) {
        touchHLE_gl_bindings::gles2::Uniform4fv(location, count, value)
    }
    unsafe fn Uniform1iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) {
        touchHLE_gl_bindings::gles2::Uniform1iv(location, count, value)
    }
    unsafe fn Uniform2iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) {
        touchHLE_gl_bindings::gles2::Uniform2iv(location, count, value)
    }
    unsafe fn Uniform3iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) {
        touchHLE_gl_bindings::gles2::Uniform3iv(location, count, value)
    }
    unsafe fn Uniform4iv(&mut self, location: GLint, count: GLsizei, value: *const GLint) {
        touchHLE_gl_bindings::gles2::Uniform4iv(location, count, value)
    }
    unsafe fn UniformMatrix2fv(
        &mut self,
        location: GLint,
        count: GLsizei,
        transpose: GLboolean,
        value: *const GLfloat,
    ) {
        touchHLE_gl_bindings::gles2::UniformMatrix2fv(location, count, transpose, value)
    }
    unsafe fn UniformMatrix3fv(
        &mut self,
        location: GLint,
        count: GLsizei,
        transpose: GLboolean,
        value: *const GLfloat,
    ) {
        touchHLE_gl_bindings::gles2::UniformMatrix3fv(location, count, transpose, value)
    }
    unsafe fn UniformMatrix4fv(
        &mut self,
        location: GLint,
        count: GLsizei,
        transpose: GLboolean,
        value: *const GLfloat,
    ) {
        touchHLE_gl_bindings::gles2::UniformMatrix4fv(location, count, transpose, value)
    }
    unsafe fn GetUniformLocation(
        &mut self,
        program: GLuint,
        name: *const std::ffi::c_char,
    ) -> GLint {
        touchHLE_gl_bindings::gles2::GetUniformLocation(program, name)
    }
    unsafe fn GetAttribLocation(
        &mut self,
        program: GLuint,
        name: *const std::ffi::c_char,
    ) -> GLint {
        touchHLE_gl_bindings::gles2::GetAttribLocation(program, name)
    }
    unsafe fn GetActiveUniform(
        &mut self,
        program: GLuint,
        index: GLuint,
        bufSize: GLsizei,
        length: *mut GLsizei,
        size: *mut GLint,
        type_: *mut GLenum,
        name: *mut std::ffi::c_char,
    ) {
        touchHLE_gl_bindings::gles2::GetActiveUniform(
            program, index, bufSize, length, size, type_, name,
        )
    }
    unsafe fn GetActiveAttrib(
        &mut self,
        program: GLuint,
        index: GLuint,
        bufSize: GLsizei,
        length: *mut GLsizei,
        size: *mut GLint,
        type_: *mut GLenum,
        name: *mut std::ffi::c_char,
    ) {
        touchHLE_gl_bindings::gles2::GetActiveAttrib(
            program, index, bufSize, length, size, type_, name,
        )
    }
    unsafe fn BlendColor(&mut self, red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) {
        touchHLE_gl_bindings::gles2::BlendColor(red, green, blue, alpha)
    }
    // AddAttribNative
    unsafe fn GetVertexAttribiv(&mut self, index: GLuint, pname: GLenum, params: *mut GLint) {
        touchHLE_gl_bindings::gles2::GetVertexAttribiv(index, pname, params)
    }

    // OES_framebuffer_object -> EXT_framebuffer_object
    // EsTwoFboGen
    unsafe fn GenFramebuffersOES(&mut self, n: GLsizei, framebuffers: *mut GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GenFramebuffers(n, framebuffers)
        } else {
            gles11::GenFramebuffersOES(n, framebuffers)
        }
    }
    unsafe fn GenRenderbuffersOES(&mut self, n: GLsizei, renderbuffers: *mut GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GenRenderbuffers(n, renderbuffers)
        } else {
            gles11::GenRenderbuffersOES(n, renderbuffers)
        }
    }
    unsafe fn IsFramebufferOES(&mut self, renderbuffer: GLuint) -> GLboolean {
        gles11::IsFramebufferOES(renderbuffer)
    }
    unsafe fn IsRenderbufferOES(&mut self, renderbuffer: GLuint) -> GLboolean {
        gles11::IsRenderbufferOES(renderbuffer)
    }
    // EsTwoFboRest
    unsafe fn BindFramebufferOES(&mut self, target: GLenum, framebuffer: GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BindFramebuffer(target, framebuffer)
        } else {
            gles11::BindFramebufferOES(target, framebuffer)
        }
    }
    unsafe fn BindRenderbufferOES(&mut self, target: GLenum, renderbuffer: GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::BindRenderbuffer(target, renderbuffer)
        } else {
            gles11::BindRenderbufferOES(target, renderbuffer)
        }
    }
    unsafe fn RenderbufferStorageOES(
        &mut self,
        target: GLenum,
        internalformat: GLenum,
        width: GLsizei,
        height: GLsizei,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::RenderbufferStorage(target, internalformat, width, height)
        } else {
            gles11::RenderbufferStorageOES(target, internalformat, width, height)
        }
    }
    unsafe fn FramebufferRenderbufferOES(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        renderbuffertarget: GLenum,
        renderbuffer: GLuint,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::FramebufferRenderbuffer(
                target,
                attachment,
                renderbuffertarget,
                renderbuffer,
            )
        } else {
            gles11::FramebufferRenderbufferOES(target, attachment, renderbuffertarget, renderbuffer)
        }
    }
    unsafe fn FramebufferTexture2DOES(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        textarget: GLenum,
        texture: GLuint,
        level: i32,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::FramebufferTexture2D(
                target, attachment, textarget, texture, level,
            )
        } else {
            gles11::FramebufferTexture2DOES(target, attachment, textarget, texture, level)
        }
    }
    unsafe fn GetFramebufferAttachmentParameterivOES(
        &mut self,
        target: GLenum,
        attachment: GLenum,
        pname: GLenum,
        params: *mut GLint,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetFramebufferAttachmentParameteriv(
                target, attachment, pname, params,
            )
        } else {
            gles11::GetFramebufferAttachmentParameterivOES(target, attachment, pname, params)
        }
    }
    unsafe fn GetRenderbufferParameterivOES(
        &mut self,
        target: GLenum,
        pname: GLenum,
        params: *mut GLint,
    ) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetRenderbufferParameteriv(target, pname, params)
        } else {
            gles11::GetRenderbufferParameterivOES(target, pname, params)
        }
    }
    unsafe fn CheckFramebufferStatusOES(&mut self, target: GLenum) -> GLenum {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::CheckFramebufferStatus(target)
        } else {
            gles11::CheckFramebufferStatusOES(target)
        }
    }
    unsafe fn DeleteFramebuffersOES(&mut self, n: GLsizei, framebuffers: *const GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DeleteFramebuffers(n, framebuffers)
        } else {
            gles11::DeleteFramebuffersOES(n, framebuffers)
        }
    }
    unsafe fn DeleteRenderbuffersOES(&mut self, n: GLsizei, renderbuffers: *const GLuint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::DeleteRenderbuffers(n, renderbuffers)
        } else {
            gles11::DeleteRenderbuffersOES(n, renderbuffers)
        }
    }
    unsafe fn GenerateMipmapOES(&mut self, target: GLenum) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GenerateMipmap(target)
        } else {
            gles11::GenerateMipmapOES(target)
        }
    }
    // RouteGetBuffer
    unsafe fn GetBufferParameteriv(&mut self, target: GLenum, pname: GLenum, params: *mut GLint) {
        if self.is_gles2 {
            touchHLE_gl_bindings::gles2::GetBufferParameteriv(target, pname, params)
        } else {
            gles11::GetBufferParameteriv(target, pname, params)
        }
    }
   unsafe fn MapBufferOES(&mut self, target: GLenum, access: GLenum) -> *mut GLvoid {
        gles11::MapBufferOES(target, access)
    }
    
    unsafe fn UnmapBufferOES(&mut self, target: GLenum) -> GLboolean {
        gles11::UnmapBufferOES(target)
    }
}
