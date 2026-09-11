/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! EAGL.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_animation::ca_eagl_layer::{
    find_fullscreen_eagl_layer, get_pixels_vec_for_presenting, present_pixels,
};
use crate::frameworks::core_graphics::{CGFloat, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::get_static_str;
use crate::frameworks::foundation::NSUInteger;
use crate::gles::gles11_raw as gles11; // constants only
use crate::gles::gles11_raw::types::*;
use crate::gles::present::{present_frame, FpsCounter};
use crate::gles::{create_gles1_ctx, gles1_on_gl2, GLESContext, GLES};
use crate::mem::MutPtr;
use crate::objc::{id, msg, nil, objc_classes, release, retain, ClassExports, HostObject};
use crate::options::Options;
use crate::dyld::{export_c_func, FunctionExports};
use crate::Environment;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const kEAGLDrawablePropertyColorFormat: &str = "ColorFormat";
pub const kEAGLDrawablePropertyRetainedBacking: &str = "RetainedBacking";
pub const kEAGLColorFormatRGBA8: &str = "RGBA8";
pub const kEAGLColorFormatRGB565: &str = "RGB565";

pub const CONSTANTS: ConstantExports = &[
    ("_kEAGLDrawablePropertyColorFormat", HostConstant::NSString(kEAGLDrawablePropertyColorFormat)),
    ("_kEAGLDrawablePropertyRetainedBacking", HostConstant::NSString(kEAGLDrawablePropertyRetainedBacking)),
    ("_kEAGLColorFormatRGBA8", HostConstant::NSString(kEAGLColorFormatRGBA8)),
    ("_kEAGLColorFormatRGB565", HostConstant::NSString(kEAGLColorFormatRGB565)),
];

pub(super) type EAGLRenderingAPI = u32; 
const kEAGLRenderingAPIOpenGLES1: EAGLRenderingAPI = 1;
const kEAGLRenderingAPIOpenGLES2: EAGLRenderingAPI = 2;

#[derive(Default)]
pub(super) struct EAGLContextHostObject {
    pub(super) api: EAGLRenderingAPI, 
    pub(super) gles_ctx: Option<Box<dyn GLESContext>>,
    renderbuffer_drawable_bindings: Rc<RefCell<HashMap<GLuint, id>>>,
    fps_counter: Option<FpsCounter>,
    next_frame_due: Option<Instant>,
    pub mapped_buffers: HashMap<(GLenum, GLuint), (MutPtr<GLvoid>, *mut GLvoid, usize)>,
    pub retina_scale: f32, 
    /// Programs for which the guest app explicitly bound attribute locations
    /// via `glBindAttribLocation` before linking.
    pub guest_bound_attribs: HashMap<GLuint, std::collections::HashSet<String>>,
}
impl HostObject for EAGLContextHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation EAGLContext: NSObject

+ (id)alloc {
    let host_object = Box::new(EAGLContextHostObject {
        api: kEAGLRenderingAPIOpenGLES1,
        gles_ctx: None,
        renderbuffer_drawable_bindings: Rc::new(RefCell::new(HashMap::new())),
        fps_counter: None,
        next_frame_due: None,
        mapped_buffers: HashMap::new(),
        retina_scale: 1.0,
        guest_bound_attribs: HashMap::new(),
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (id)currentContext {
    env.framework_state.opengles.current_ctx_for_thread(env.current_thread).unwrap_or(nil)
}

+ (bool)setCurrentContext:(id)context {
    retain(env, context);
    let current_ctx = env.framework_state.opengles.current_ctx_for_thread(env.current_thread);

    if let Some(old_ctx) = std::mem::take(current_ctx) {
        release(env, old_ctx);
    }

    let current_ctx = env.framework_state.opengles.current_ctx_for_thread(env.current_thread);
    if context != nil {
        *current_ctx = Some(context);
    }

    true
}

- (id)initWithAPI:(EAGLRenderingAPI)api sharegroup:(id)group {
    if api != kEAGLRenderingAPIOpenGLES1 && api != kEAGLRenderingAPIOpenGLES2 {
        return nil;
    }

    if group == nil {
        return msg![env; this initWithAPI:api];
    }

    {
        let prev_context_gles = env.objc.borrow_mut::<EAGLContextHostObject>(group).gles_ctx.as_mut().unwrap();
        let _prev_ctx = prev_context_gles.make_current(env.window.as_mut().unwrap());
    }
    
    env.window.as_mut().unwrap().set_share_with_current_context(true);

    let mut gles_ins = create_gles1_ctx(env);

    {
        let gles_ctx = gles_ins.make_current(env.window.as_mut().unwrap());
        log!("Driver info: {}", unsafe { gles_ctx.driver_description() });
    }

    let host_obj = env.objc.borrow_mut::<EAGLContextHostObject>(this);
    host_obj.api = api;
    host_obj.gles_ctx = Some(gles_ins);

    env.window.as_mut().unwrap().set_share_with_current_context(false);
    env.objc.borrow_mut::<EAGLContextHostObject>(this).renderbuffer_drawable_bindings = 
        env.objc.borrow::<EAGLContextHostObject>(group).renderbuffer_drawable_bindings.clone();
    
    this
}

- (id)initWithAPI:(EAGLRenderingAPI)api {
    if api != kEAGLRenderingAPIOpenGLES1 && api != kEAGLRenderingAPIOpenGLES2 {
        return nil;
    }

    let mut gles_ins = create_gles1_ctx(env);

    {
        let gles_ctx = gles_ins.make_current(env.window.as_mut().expect("OpenGL ES is not supported in headless mode"));
        log!("Driver info: {}", unsafe { gles_ctx.driver_description() });
    }

    let host_obj = env.objc.borrow_mut::<EAGLContextHostObject>(this);
    host_obj.api = api;
    host_obj.gles_ctx = Some(gles_ins);
    this
}

- (EAGLRenderingAPI)API {
    env.objc.borrow::<EAGLContextHostObject>(this).api
}

- (id)sharegroup {
    this
}

- (())dealloc {
    let host_obj = env.objc.borrow_mut::<EAGLContextHostObject>(this);
    for &(_guest_buf, _host_buf, _size) in host_obj.mapped_buffers.values() {
        env.mem.free(_guest_buf);
    }
    if Rc::strong_count(&host_obj.renderbuffer_drawable_bindings) == 1 {
        let bindings = std::mem::take(&mut host_obj.renderbuffer_drawable_bindings);
        for (_renderbuffer, drawable) in bindings.take() {
            release(env, drawable);
        }
    }
    env.objc.dealloc_object(this, &mut env.mem);
}

- (bool)renderbufferStorage:(NSUInteger)target
               fromDrawable:(id)drawable {
    assert!(drawable != nil);
    assert!(target == gles11::RENDERBUFFER_OES);

    let props: id = msg![env; drawable drawableProperties];
    let format_key = get_static_str(env, kEAGLDrawablePropertyColorFormat);
    let format_rgba8 = get_static_str(env, kEAGLColorFormatRGBA8);
    let format_rgb565 = get_static_str(env, kEAGLColorFormatRGB565);

    let format: id = msg![env; props objectForKey:format_key];
    if !msg![env; format isEqual:format_rgba8] && !msg![env; format isEqual:format_rgb565] {
        log!("[renderbufferStorage:{:?} fromDrawable:{:?}] Warning: unhandled format {:?}, using RGBA8", target, drawable, format);
    }
    let internalformat = gles11::RGBA8_OES;

    let scale: CGFloat = {
        let drawable_class = msg![env; drawable class];
        if env.objc.class_has_method_named(drawable_class, "contentsScale") {
            msg![env; drawable contentsScale]
        } else {
            1.0
        }
    };
    env.objc.borrow_mut::<EAGLContextHostObject>(this).retina_scale = scale as f32;

    let (width, height) = {
        let bounds: CGRect = msg![env; drawable bounds];
        let CGSize { width, height } = bounds.size;
        let scale_hack = env.options.scale_hack.get() as f32;
        let final_w = (width * scale * scale_hack).round() as u32;
        let final_h = (height * scale * scale_hack).round() as u32;
        //DebugRenderSize
        log!("DEBUG_EAGL: renderbufferStorage:fromDrawable: {:?} Bounds: w={}, h={} | scale={}, scale_hack={} | Final FBO: {}x{}", drawable, width, height, scale, scale_hack, final_w, final_h);
        (final_w, final_h)
    };

    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
    let renderbuffer = {
        let mut gles = super::sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread).unwrap();
        unsafe {
            gles.RenderbufferStorageOES(target, internalformat, width.try_into().unwrap(), height.try_into().unwrap());
            let mut renderbuffer = 0;
            gles.GetIntegerv(gles11::RENDERBUFFER_BINDING_OES, &mut renderbuffer);
            renderbuffer as _
        }
    };

    retain(env, drawable);
    let host_obj = env.objc.borrow_mut::<EAGLContextHostObject>(this);
    let maybe_old_drawable = host_obj.renderbuffer_drawable_bindings.borrow_mut().insert(
        renderbuffer,
        drawable
    );
    if let Some(old_drawable) = maybe_old_drawable {
        release(env, old_drawable);
    }

    true
}

- (bool)presentRenderbuffer:(NSUInteger)target {
    assert!(target == gles11::RENDERBUFFER_OES);
    let sleep_for = limit_framerate(&mut env.objc.borrow_mut::<EAGLContextHostObject>(this).next_frame_due, &env.options);

    if env.options.print_fps {
        env
            .objc
            .borrow_mut::<EAGLContextHostObject>(this)
            .fps_counter
            .get_or_insert_with(FpsCounter::start)
            .count_frame(format_args!("EAGLContext {this:?}"));
    }

    let fullscreen_layer = find_fullscreen_eagl_layer(env);
    let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
    let mut gles = super::sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread).unwrap();
    let renderbuffer: GLuint = unsafe {
        let mut renderbuffer = 0;
        gles.GetIntegerv(gles11::RENDERBUFFER_BINDING_OES, &mut renderbuffer);
        renderbuffer as _
    };

    std::mem::drop(gles);
    let Some(&drawable) = env
        .objc
        .borrow::<EAGLContextHostObject>(this)
        .renderbuffer_drawable_bindings
        .borrow()
        .get(&renderbuffer) else {
        log_dbg!("Can't present a renderbuffer {:?} not bound to a drawable!", renderbuffer);
        return false;
    };

    // We're presenting to the opaque CAEAGLLayer that covers the screen.
    // We can use the fast path where we skip composition and present directly.
    //DebugPresentPath
    log!("DEBUG_EAGL: presentRenderbuffer: target={}, drawable={:?}, fullscreen_layer={:?}", target, drawable, fullscreen_layer);
    if drawable == fullscreen_layer {
        log!(
            "DEBUG_EAGL: Layer {:?} IS fullscreen layer. Fast path ACTIVE. renderbuffer: {:?}",
            drawable,
            renderbuffer,
        );
        // re-borrow
        unsafe {
            present_renderbuffer(env);
        }
    } else {
        if fullscreen_layer != nil {
            log!("DEBUG_EAGL: Layer {:?} is NOT fullscreen layer {:?}. Rendering to RAM (SLOW PATH) or skipped!", drawable, fullscreen_layer);
            // If there's a single layer that covers the screen, and this isn't
            // it, there's no point in presenting the output because it won't be
            // seen. Using a noisy log because it's a weird scenario and might
            // indicate a bug.
            log!(
                "Layer {:?} is not the fullscreen layer {:?}, skipping presentation of renderbuffer {:?}!",
                drawable,
                fullscreen_layer,
                renderbuffer,
            );
            if let Some(sleep_for) = sleep_for {
                env.sleep(sleep_for);
            }
            return true;
        }

        let pixels_vec = get_pixels_vec_for_presenting(env, drawable);
        let (pixels_vec, width, height) = {
            let mut gles = super::sync_context(&mut env.framework_state.opengles, &mut env.objc, env.window.as_mut().unwrap(), env.current_thread).unwrap();
            unsafe {
                read_renderbuffer(gles.as_mut(), pixels_vec)
            }
        };
        present_pixels(env, drawable, pixels_vec, width, height);
    }

    if let Some(sleep_for) = sleep_for {
        env.sleep(sleep_for);
    }

    true
}

@end

};

fn limit_framerate(next_frame_due: &mut Option<Instant>, options: &Options) -> Option<Duration> {
    let interval = if let Some(fps) = options.fps_limit {
        1.0 / fps
    } else {
        return None;
    };
    let interval_rust = Duration::from_secs_f64(interval);

    let &mut Some(current_frame_due) = next_frame_due else {
        *next_frame_due = Some(Instant::now() + interval_rust);
        return None;
    };

    let now = Instant::now();
    *next_frame_due = if now > current_frame_due + interval_rust {
        log_dbg!("Too much slop accumulated, skipping an interval.");
        Some(
            current_frame_due
                + Duration::from_secs_f64(
                    interval * (((now - current_frame_due).as_secs_f64() / interval).ceil()),
                ),
        )
    } else {
        Some(current_frame_due + interval_rust)
    };

    if now < current_frame_due {
        Some(current_frame_due.saturating_duration_since(now))
    } else {
        None
    }
}

unsafe fn get_ptr(gles: &mut dyn GLES, pname: GLenum) -> *const GLvoid {
    let mut ptr = std::ptr::null();
    gles.GetPointerv(pname, &mut ptr);
    ptr
}

unsafe fn get_ints<const N: usize>(gles: &mut dyn GLES, pname: GLenum) -> [GLint; N] {
    let mut res = [0; N];
    gles.GetIntegerv(pname, res.as_mut_ptr());
    res
}

unsafe fn get_int(gles: &mut dyn GLES, pname: GLenum) -> GLint {
    get_ints::<1>(gles, pname)[0]
}

unsafe fn get_tex_env_ints<const N: usize>(
    gles: &mut dyn GLES,
    target: GLenum,
    pname: GLenum,
) -> [GLint; N] {
    let mut res = [0; N];
    gles.GetTexEnviv(target, pname, res.as_mut_ptr());
    res
}

unsafe fn get_tex_env_int(gles: &mut dyn GLES, target: GLenum, pname: GLenum) -> GLint {
    get_tex_env_ints::<1>(gles, target, pname)[0]
}

unsafe fn get_floats<const N: usize>(gles: &mut dyn GLES, pname: GLenum) -> [GLfloat; N] {
    let mut res = [0.0; N];
    gles.GetFloatv(pname, res.as_mut_ptr());
    res
}

unsafe fn get_renderbuffer_size(gles: &mut dyn GLES) -> (GLsizei, GLsizei) {
    let mut width: GLint = 0;
    let mut height: GLint = 0;
    gles.GetRenderbufferParameterivOES(
        gles11::RENDERBUFFER_OES,
        gles11::RENDERBUFFER_WIDTH_OES,
        &mut width,
    );
    gles.GetRenderbufferParameterivOES(
        gles11::RENDERBUFFER_OES,
        gles11::RENDERBUFFER_HEIGHT_OES,
        &mut height,
    );
    (width, height)
}

unsafe fn read_renderbuffer(gles: &mut dyn GLES, mut pixel_buffer: Vec<u8>) -> (Vec<u8>, u32, u32) {
    let renderbuffer: GLuint = get_int(gles, gles11::RENDERBUFFER_BINDING_OES) as _;
    let (width, height) = get_renderbuffer_size(gles);
    let width_u32: u32 = width.try_into().unwrap();
    let height_u32: u32 = height.try_into().unwrap();
    let old_framebuffer: GLuint = get_int(gles, gles11::FRAMEBUFFER_BINDING_OES) as _;

    let mut src_framebuffer = 0;
    gles.GenFramebuffersOES(1, &mut src_framebuffer);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, src_framebuffer);
    gles.FramebufferRenderbufferOES(
        gles11::FRAMEBUFFER_OES,
        gles11::COLOR_ATTACHMENT0_OES,
        gles11::RENDERBUFFER_OES,
        renderbuffer,
    );

    let size = (width_u32 as usize)
        .checked_mul(height_u32 as usize)
        .unwrap()
        .checked_mul(4)
        .unwrap();

    pixel_buffer.clear();
    pixel_buffer.reserve_exact(size);
    let before = Instant::now();
    gles.ReadPixels(
        0,
        0,
        width,
        height,
        gles11::RGBA,
        gles11::UNSIGNED_BYTE,
        pixel_buffer.as_mut_ptr() as *mut _,
    );
    log_dbg!(
        "glReadPixels(0, 0, {}, {}, …) took {:?}",
        width,
        height,
        Instant::now().saturating_duration_since(before)
    );
    pixel_buffer.set_len(size);

    gles.DeleteFramebuffersOES(1, &src_framebuffer);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, old_framebuffer);
    (pixel_buffer, width_u32, height_u32)
}

unsafe fn present_renderbuffer(env: &mut Environment) {
    let viewport = env.window.as_mut().unwrap().viewport();
    let mut rotation_matrix = env.window.as_mut().unwrap().rotation_matrix();
    let virtual_cursor_visible_at = env.window.as_mut().unwrap().virtual_cursor_visible_at();

    let gles_ctx = super::get_thread_context(
        &mut env.framework_state.opengles,
        &mut env.objc,
        env.current_thread,
    )
    .unwrap();
    let mut gles_boxed = gles_ctx.make_current(env.window.as_mut().unwrap());
    let gles = gles_boxed.as_mut();

    let renderbuffer: GLuint = get_int(gles, gles11::RENDERBUFFER_BINDING_OES) as _;
    let (width, height) = get_renderbuffer_size(gles);
    let old_framebuffer: GLuint = get_int(gles, gles11::FRAMEBUFFER_BINDING_OES) as _;
    let old_texture_2d: GLuint = get_int(gles, gles11::TEXTURE_BINDING_2D) as _;
    let active_texture: GLint = get_int(gles, gles11::ACTIVE_TEXTURE);
    //DebugPRBState
    log!("DEBUG_PRB: Start. RB={}, w={}, h={}. OLD_FB={}, OLD_TEX2D={}, ACTIVE_TEX={:#x}", renderbuffer, width, height, old_framebuffer, old_texture_2d, active_texture);

    let mut src_framebuffer = 0;
    gles.GenFramebuffersOES(1, &mut src_framebuffer);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, src_framebuffer);
    gles.FramebufferRenderbufferOES(
        gles11::FRAMEBUFFER_OES,
        gles11::COLOR_ATTACHMENT0_OES,
        gles11::RENDERBUFFER_OES,
        renderbuffer,
    );

    // DebugFboStatusExt
    let fbo_status = gles.CheckFramebufferStatusOES(gles11::FRAMEBUFFER_OES);
    let mut px = [0u8; 20];
    let hw = width.saturating_sub(1);
    let hh = height.saturating_sub(1);
    gles.ReadPixels(0, 0, 1, 1, gles11::RGBA, gles11::UNSIGNED_BYTE, px[0..4].as_mut_ptr() as *mut _);
    gles.ReadPixels(hw, 0, 1, 1, gles11::RGBA, gles11::UNSIGNED_BYTE, px[4..8].as_mut_ptr() as *mut _);
    gles.ReadPixels(0, hh, 1, 1, gles11::RGBA, gles11::UNSIGNED_BYTE, px[8..12].as_mut_ptr() as *mut _);
    gles.ReadPixels(hw, hh, 1, 1, gles11::RGBA, gles11::UNSIGNED_BYTE, px[12..16].as_mut_ptr() as *mut _);
    gles.ReadPixels(width / 2, height / 2, 1, 1, gles11::RGBA, gles11::UNSIGNED_BYTE, px[16..20].as_mut_ptr() as *mut _);
    log!("DEBUG_PRB: FBO={:#x}. w={}, h={}. Pixels: BL[{},{},{},{}] BR[{},{},{},{}] TL[{},{},{},{}] TR[{},{},{},{}] C[{},{},{},{}]",
        fbo_status, width, height,
        px[0], px[1], px[2], px[3], px[4], px[5], px[6], px[7],
        px[8], px[9], px[10], px[11], px[12], px[13], px[14], px[15],
        px[16], px[17], px[18], px[19]
    );

    // Create a texture with a copy of the pixels in the framebuffer
    let mut texture: GLuint = 0;
    gles.GenTextures(1, &mut texture);
    gles.BindTexture(gles11::TEXTURE_2D, texture);
    
    // Clear error
    while gles.GetError() != 0 {}
    
    gles.CopyTexImage2D(
        gles11::TEXTURE_2D,
        0,
        gles11::RGB as _,
        0,
        0,
        width,
        height,
        0,
    );
    
    //DebugCopyTex
    let err_after_copy = gles.GetError();
    if err_after_copy != 0 {
        log!("DEBUG_PRB: ERROR after CopyTexImage2D: {:#x}", err_after_copy);
    }
    
    // The texture will not have any mip levels so we must ensure the filter
    // does not use them, else rendering will fail.
    gles.TexParameteri(
        gles11::TEXTURE_2D,
        gles11::TEXTURE_MIN_FILTER,
        gles11::LINEAR as _,
    );
    gles.DeleteFramebuffersOES(1, &src_framebuffer);

    let is_gles2 = gles.is_es2();
    let old_arrays = {
        let mut old_arrays = [gles11::FALSE; gles1_on_gl2::ARRAYS.len()];
        if !is_gles2 {
            for (is_enabled, info) in old_arrays.iter_mut().zip(gles1_on_gl2::ARRAYS.iter()) {
                gles.GetBooleanv(info.name, is_enabled);
                gles.DisableClientState(info.name);
            }
        }
        old_arrays
    };
    let old_capabilities = {
        let mut old_capabilities = [gles11::FALSE; gles1_on_gl2::CAPABILITIES.len()];
        if !is_gles2 {
            for (is_enabled, &name) in old_capabilities
                .iter_mut()
                .zip(gles1_on_gl2::CAPABILITIES.iter())
            {
                gles.GetBooleanv(name, is_enabled);
                gles.Disable(name);
            }
        }
        old_capabilities
    };
    let old_matrix_mode: GLenum = if !is_gles2 {
        get_int(gles, gles11::MATRIX_MODE) as _
    } else {
        0
    };
    let old_color: [GLfloat; 4] = if !is_gles2 {
        get_floats(gles, gles11::CURRENT_COLOR)
    } else {
        [0.0; 4]
    };
    if !is_gles2 {
        for mode in [gles11::MODELVIEW, gles11::PROJECTION, gles11::TEXTURE] {
            gles.MatrixMode(mode);
            gles.PushMatrix();
            gles.LoadIdentity();
        }
        gles.Color4f(1.0, 1.0, 1.0, 1.0);
    }

    let old_viewport: (GLint, GLint, GLsizei, GLsizei) = {
        let [x, y, width, height] = get_ints(gles, gles11::VIEWPORT);
        (x, y, width as _, height as _)
    };
    let old_clear_color: [GLfloat; 4] = get_floats(gles, gles11::COLOR_CLEAR_VALUE);
    let old_array_buffer: GLuint = get_int(gles, gles11::ARRAY_BUFFER_BINDING) as _;
    let old_vertex_array_binding: GLuint = get_int(gles, gles11::VERTEX_ARRAY_BUFFER_BINDING) as _;
    let old_vertex_array_size: GLint = get_int(gles, gles11::VERTEX_ARRAY_SIZE);
    let old_vertex_array_type: GLenum = get_int(gles, gles11::VERTEX_ARRAY_TYPE) as _;
    let old_vertex_array_stride: GLsizei = get_int(gles, gles11::VERTEX_ARRAY_STRIDE) as _;
    let old_vertex_array_pointer = get_ptr(gles, gles11::VERTEX_ARRAY_POINTER);
    let old_tex_coord_array_binding: GLuint =
        get_int(gles, gles11::TEXTURE_COORD_ARRAY_BUFFER_BINDING) as _;
    let old_tex_coord_array_size: GLint = get_int(gles, gles11::TEXTURE_COORD_ARRAY_SIZE);
    let old_tex_coord_array_type: GLenum = get_int(gles, gles11::TEXTURE_COORD_ARRAY_TYPE) as _;
    let old_tex_coord_array_stride: GLsizei =
        get_int(gles, gles11::TEXTURE_COORD_ARRAY_STRIDE) as _;
    let old_tex_coord_array_pointer = get_ptr(gles, gles11::TEXTURE_COORD_ARRAY_POINTER);
    let old_blend_sfactor: GLenum = get_int(gles, gles11::BLEND_SRC) as _;
    let old_blend_dfactor: GLenum = get_int(gles, gles11::BLEND_DST) as _;
    let old_tex_env_mode = get_tex_env_int(gles, gles11::TEXTURE_ENV, gles11::TEXTURE_ENV_MODE);
    let tex_env_mode_arr = [gles11::REPLACE; 1];
    gles.TexEnviv(
        gles11::TEXTURE_ENV,
        gles11::TEXTURE_ENV_MODE,
        tex_env_mode_arr.as_ptr().cast(),
    );

    // SmartRotationFix
    let rb_w = width as f32;
    let rb_h = height as f32;
    let cols = rotation_matrix.columns();
    let is_rotated = cols[0][0].abs() < 0.1 && cols[0][1].abs() > 0.9;
    
    if is_rotated && rb_w > rb_h {
        unsafe {
            let m_ptr = &mut rotation_matrix as *mut _ as *mut [[f32; 2]; 2];
            *m_ptr = [
                [1.0, 0.0],
                [0.0, 1.0],
            ];
        }
        log!("DEBUG_EAGL: SmartRotationFix bypassed matrix! rb_w={}, rb_h={}", rb_w, rb_h);
    }

    // Draw the quad
    present_frame(gles, viewport, rotation_matrix, virtual_cursor_visible_at);
    gles.DeleteTextures(1, &texture);

    if !is_gles2 {
        for (&is_enabled, info) in old_arrays.iter().zip(gles1_on_gl2::ARRAYS.iter()) {
            match is_enabled {
                gles11::TRUE => gles.EnableClientState(info.name),
                gles11::FALSE => gles.DisableClientState(info.name),
                _ => unreachable!(),
            }
        }
        for (&is_enabled, &name) in old_capabilities
            .iter()
            .zip(gles1_on_gl2::CAPABILITIES.iter())
        {
            match is_enabled {
                gles11::TRUE => gles.Enable(name),
                gles11::FALSE => gles.Disable(name),
                _ => unreachable!(),
            }
        }
        for mode in [gles11::MODELVIEW, gles11::PROJECTION, gles11::TEXTURE] {
            gles.MatrixMode(mode);
            gles.PopMatrix();
        }
        gles.MatrixMode(old_matrix_mode);
        gles.Color4f(old_color[0], old_color[1], old_color[2], old_color[3]);
    }
    gles.Viewport(
        old_viewport.0,
        old_viewport.1,
        old_viewport.2,
        old_viewport.3,
    );
    gles.ClearColor(
        old_clear_color[0],
        old_clear_color[1],
        old_clear_color[2],
        old_clear_color[3],
    );
    gles.BindBuffer(gles11::ARRAY_BUFFER, old_vertex_array_binding);
    gles.VertexPointer(
        old_vertex_array_size,
        old_vertex_array_type,
        old_vertex_array_stride,
        old_vertex_array_pointer,
    );
    gles.BindBuffer(gles11::ARRAY_BUFFER, old_tex_coord_array_binding);
    gles.TexCoordPointer(
        old_tex_coord_array_size,
        old_tex_coord_array_type,
        old_tex_coord_array_stride,
        old_tex_coord_array_pointer,
    );
    gles.BindBuffer(gles11::ARRAY_BUFFER, old_array_buffer);
    gles.BlendFunc(old_blend_sfactor, old_blend_dfactor);

    let old_tex_env_mode_arr = [old_tex_env_mode; 1];
    gles.TexEnviv(
        gles11::TEXTURE_ENV,
        gles11::TEXTURE_ENV_MODE,
        old_tex_env_mode_arr.as_ptr().cast(),
    );

    std::mem::drop(gles_boxed);
    env.window.as_mut().unwrap().swap_window();

    let mut gles_boxed = gles_ctx.make_current(env.window.as_mut().unwrap());
    let gles = gles_boxed.as_mut();
    gles.BindTexture(gles11::TEXTURE_2D, old_texture_2d);
    gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, old_framebuffer);
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(EAGLGetVersion(_, _))];

pub fn EAGLGetVersion(env: &mut Environment, major: MutPtr<u32>, minor: MutPtr<u32>) {
    if !major.is_null() {
        env.mem.write(major, 1);
    }
    if !minor.is_null() {
        env.mem.write(minor, 5);
    }
    log!("EAGLGetVersion called");
}
