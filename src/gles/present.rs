/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Utilities for presenting frames to the window using an abstract OpenGL ES
//! implementation.

use super::gles11_raw as gles11; // constants and types only
use super::GLES;
use crate::matrix::Matrix;
use std::time::{Duration, Instant};

use std::sync::Mutex;
use std::sync::OnceLock;

pub struct FpsCounter {
    time: std::time::Instant,
    frames: u32,
}

// Global FPS text cache updated by FpsCounter so present_frame can draw it.
static LAST_FPS_TEXT: OnceLock<Mutex<String>> = OnceLock::new();
// Per-process cached GL glyph textures. Created lazily on first overlay draw.
static GLYPH_TEXTURES: OnceLock<Mutex<Option<Vec<u32>>>> = OnceLock::new();
// Runtime-controlled flag to enable the on-screen FPS overlay without requiring
// an environment variable. Use set_onscreen_fps_enabled(true/false) to control it
// from other parts of the runtime (e.g., the app picker or window input).
use std::sync::atomic::{AtomicBool, Ordering};
static ONSCREEN_FPS_ENABLED: OnceLock<AtomicBool> = OnceLock::new();

impl FpsCounter {
    pub fn start() -> Self {
        LAST_FPS_TEXT.get_or_init(|| Mutex::new(String::new()));
        GLYPH_TEXTURES.get_or_init(|| Mutex::new(None));
        FpsCounter {
            time: Instant::now(),
            frames: 0,
        }
    }

    pub fn count_frame(&mut self, label: std::fmt::Arguments<'_>) {
        self.frames += 1;
        let now = Instant::now();
        let duration = now - self.time;
        if duration >= Duration::from_secs(1) {
            self.time = now;
            let fps = std::mem::take(&mut self.frames) as f32 / duration.as_secs_f32();
            echo!("touchHLE: {} FPS: {:.2}", label, fps);
            // Update global text cache for on-screen overlay if enabled via
            // environment variable or the runtime flag.
            let onscreen_env = std::env::var_os("TOUCHHLE_ONSCREEN_FPS").is_some();
            let onscreen_runtime = ONSCREEN_FPS_ENABLED
                .get()
                .map(|b| b.load(Ordering::SeqCst))
                .unwrap_or(false);
            if onscreen_env || onscreen_runtime {
                let text = format!("FPS: {:.1}", fps);
                if let Some(mutex) = LAST_FPS_TEXT.get() {
                    if let Ok(mut s) = mutex.lock() {
                        *s = text;
                    }
                }
            }
        }
    }
}

/// Runtime API: enable/disable the on-screen FPS overlay at runtime.
pub fn set_onscreen_fps_enabled(enabled: bool) {
    ONSCREEN_FPS_ENABLED
        .get_or_init(|| AtomicBool::new(false))
        .store(enabled, Ordering::SeqCst);
}

/// Present the the latest frame (e.g. the app's splash screen or rendering
/// output), provided as a texture bound to `GL_TEXTURE_2D`, by drawing it on
/// the window. It may be rotated, scaled and/or letterboxed as necessary. The
/// virtual cursor is also drawn if it should be currently visible.
///
/// The provided context must be current.
pub unsafe fn present_frame(
    gles: &mut dyn GLES,
    viewport: (u32, u32, u32, u32),
    rotation_matrix: Matrix<2>,
    virtual_cursor_visible_at: Option<(f32, f32, bool)>,
) {
    use gles11::types::*;
    //DebugPresentArgs
    let is_gles2 = gles.is_es2();
    let m = rotation_matrix.columns();

    let mut old_prog: GLint = 0;
    let mut old_array_buf: GLint = 0;
    let mut old_elem_buf: GLint = 0;
    let mut old_cull: GLboolean = 0;
    let mut old_depth: GLboolean = 0;
    let mut old_scissor: GLboolean = 0;
    let mut old_blend: GLboolean = 0;
    let mut old_stencil: GLboolean = 0;
    let mut old_dither: GLboolean = 0;
    let mut old_color_mask = [0u8; 4];
    let mut old_depth_mask: GLboolean = 0;
    let mut old_attribs = [0u8; 8];

    if is_gles2 {
        gles.GetIntegerv(0x8B8D, &mut old_prog);
        gles.GetIntegerv(0x8894, &mut old_array_buf);
        gles.GetIntegerv(0x8895, &mut old_elem_buf);
        gles.GetBooleanv(gles11::CULL_FACE, &mut old_cull);
        gles.GetBooleanv(gles11::DEPTH_TEST, &mut old_depth);
        gles.GetBooleanv(gles11::SCISSOR_TEST, &mut old_scissor);
        gles.GetBooleanv(gles11::BLEND, &mut old_blend);
        gles.GetBooleanv(gles11::STENCIL_TEST, &mut old_stencil);
        gles.GetBooleanv(gles11::DITHER, &mut old_dither);
        gles.GetBooleanv(
            gles11::COLOR_WRITEMASK,
            old_color_mask.as_mut_ptr() as *mut _,
        );
        gles.GetBooleanv(gles11::DEPTH_WRITEMASK, &mut old_depth_mask);
        for i in 0..8 {
            let mut status: GLint = 0;
            gles.GetVertexAttribiv(i, 0x8622, &mut status);
            old_attribs[i as usize] = status as u8;
            gles.DisableVertexAttribArray(i);
        }

        gles.ColorMask(1, 1, 1, 1);
        gles.DepthMask(1);
        gles.Disable(gles11::CULL_FACE);
        gles.Disable(gles11::DEPTH_TEST);
        gles.Disable(gles11::SCISSOR_TEST);
        gles.Disable(gles11::BLEND);
        gles.Disable(gles11::STENCIL_TEST);
        gles.Disable(gles11::DITHER);
        gles.BindBuffer(gles11::ELEMENT_ARRAY_BUFFER, 0);
    }

    gles.Viewport(
        viewport.0 as _,
        viewport.1 as _,
        viewport.2 as _,
        viewport.3 as _,
    );

    // Draw the quad
    let vertices: [f32; 12] = [
        -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0,
    ];
    let tex_coords: [f32; 12] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let matrix = Matrix::<4>::from(&rotation_matrix);

    if is_gles2 {
        gles.ClearColor(0.2, 0.0, 0.0, 1.0);
    } else {
        gles.ClearColor(0.0, 0.0, 0.0, 1.0);
    }
    gles.Clear(gles11::COLOR_BUFFER_BIT | gles11::DEPTH_BUFFER_BIT | gles11::STENCIL_BUFFER_BIT);

    if is_gles2 {
        let vs_src = "attribute vec4 position;\nattribute vec2 texCoord;\nuniform mat4 texMatrix;\nvarying vec2 v_texCoord;\nvoid main() {\n    gl_Position = position;\n    v_texCoord = (texMatrix * vec4(texCoord, 0.0, 1.0)).xy;\n}\0";
        // RemoveDebugTint
        let fs_src = "precision mediump float;\nvarying vec2 v_texCoord;\nuniform sampler2D tex;\nuniform vec4 color;\nvoid main() {\n    vec4 texColor = texture2D(tex, v_texCoord);\n    gl_FragColor = vec4(mix(texColor.rgb, color.rgb, color.a), 1.0);\n}\0";

        let vs = gles.CreateShader(0x8B31);
        let vs_ptr = [vs_src.as_ptr() as *const std::ffi::c_char];
        let vs_len = [vs_src.len() as GLint - 1];
        gles.ShaderSource(vs, 1, vs_ptr.as_ptr(), vs_len.as_ptr());
        gles.CompileShader(vs);

        let fs = gles.CreateShader(0x8B30);
        let fs_ptr = [fs_src.as_ptr() as *const std::ffi::c_char];
        let fs_len = [fs_src.len() as GLint - 1];
        gles.ShaderSource(fs, 1, fs_ptr.as_ptr(), fs_len.as_ptr());
        gles.CompileShader(fs);

        let prog = gles.CreateProgram();
        gles.AttachShader(prog, vs);
        gles.AttachShader(prog, fs);
        // SafeAttribLocation
        gles.BindAttribLocation(prog, 6, c"position".as_ptr() as *const _);
        gles.BindAttribLocation(prog, 7, c"texCoord".as_ptr() as *const _);
        gles.LinkProgram(prog);

        let pos = gles.GetAttribLocation(prog, c"position".as_ptr() as *const _);
        let tex = gles.GetAttribLocation(prog, c"texCoord".as_ptr() as *const _);
        let mat = gles.GetUniformLocation(prog, c"texMatrix".as_ptr() as *const _);
        let col = gles.GetUniformLocation(prog, c"color".as_ptr() as *const _);
        let sampler = gles.GetUniformLocation(prog, c"tex".as_ptr() as *const _);

        gles.UseProgram(prog);
        gles.Uniform4f(col, 0.0, 0.0, 0.0, 0.0);
        gles.UniformMatrix4fv(mat, 1, 0, matrix.columns().as_ptr() as *const _);

        let mut active_tex: GLint = 0;
        gles.GetIntegerv(gles11::ACTIVE_TEXTURE, &mut active_tex);
        let tex_unit = active_tex - gles11::TEXTURE0 as GLint;
        if sampler >= 0 {
            gles.Uniform1i(sampler, tex_unit);
        }

        gles.BindBuffer(gles11::ARRAY_BUFFER, 0);
        if pos >= 0 {
            gles.EnableVertexAttribArray(pos as GLuint);
            gles.VertexAttribPointer(
                pos as GLuint,
                2,
                gles11::FLOAT,
                0,
                0,
                vertices.as_ptr() as *const _,
            );
        }
        if tex >= 0 {
            gles.EnableVertexAttribArray(tex as GLuint);
            gles.VertexAttribPointer(
                tex as GLuint,
                2,
                gles11::FLOAT,
                0,
                0,
                tex_coords.as_ptr() as *const _,
            );
        }

        //DebugDrawArrays
        while gles.GetError() != 0 {}
        gles.DrawArrays(gles11::TRIANGLES, 0, 6);
        let draw_err = gles.GetError();
        if draw_err != 0 {
            log_dbg!("DEBUG_PRESENT: ERROR after DrawArrays: {:#x}", draw_err);
        }

        if let Some((x, y, pressed)) = virtual_cursor_visible_at {
            let (vx, vy, vw, vh) = viewport;
            let x = x - vx as f32;
            let y = y - vy as f32;
            gles.Enable(gles11::BLEND);
            gles.BlendFunc(gles11::ONE, gles11::ONE_MINUS_SRC_ALPHA);
            let radius = 10.0;
            let mut cursor_vertices = vertices;
            for i in (0..cursor_vertices.len()).step_by(2) {
                cursor_vertices[i] = (cursor_vertices[i] * radius + x) / (vw as f32 / 2.0) - 1.0;
                cursor_vertices[i + 1] =
                    1.0 - (cursor_vertices[i + 1] * radius + y) / (vh as f32 / 2.0);
            }
            gles.Uniform4f(
                col,
                0.0,
                0.0,
                0.0,
                if pressed { 2.0 / 3.0 } else { 1.0 / 3.0 },
            );
            if tex >= 0 {
                gles.DisableVertexAttribArray(tex as GLuint);
            }
            if pos >= 0 {
                gles.VertexAttribPointer(
                    pos as GLuint,
                    2,
                    gles11::FLOAT,
                    0,
                    0,
                    cursor_vertices.as_ptr() as *const _,
                );
            }
            gles.DrawArrays(gles11::TRIANGLES, 0, 6);
        }

        if pos >= 0 {
            gles.DisableVertexAttribArray(pos as GLuint);
        }
        if tex >= 0 {
            gles.DisableVertexAttribArray(tex as GLuint);
        }

        gles.UseProgram(old_prog as GLuint);
        gles.DeleteProgram(prog);
        gles.DeleteShader(vs);
        gles.DeleteShader(fs);

        gles.BindBuffer(gles11::ARRAY_BUFFER, old_array_buf as GLuint);
        gles.BindBuffer(gles11::ELEMENT_ARRAY_BUFFER, old_elem_buf as GLuint);
        // StateRestoreFix
        if old_cull != 0 {
            gles.Enable(gles11::CULL_FACE);
        } else {
            gles.Disable(gles11::CULL_FACE);
        }
        if old_depth != 0 {
            gles.Enable(gles11::DEPTH_TEST);
        } else {
            gles.Disable(gles11::DEPTH_TEST);
        }
        if old_scissor != 0 {
            gles.Enable(gles11::SCISSOR_TEST);
        } else {
            gles.Disable(gles11::SCISSOR_TEST);
        }
        if old_blend != 0 {
            gles.Enable(gles11::BLEND);
        } else {
            gles.Disable(gles11::BLEND);
        }
        if old_stencil != 0 {
            gles.Enable(gles11::STENCIL_TEST);
        } else {
            gles.Disable(gles11::STENCIL_TEST);
        }
        if old_dither != 0 {
            gles.Enable(gles11::DITHER);
        } else {
            gles.Disable(gles11::DITHER);
        }
        gles.ColorMask(
            old_color_mask[0],
            old_color_mask[1],
            old_color_mask[2],
            old_color_mask[3],
        );
        gles.DepthMask(old_depth_mask);
        for i in 0..8 {
            if old_attribs[i as usize] != 0 {
                gles.EnableVertexAttribArray(i);
            }
        }
    } else {
        gles.BindBuffer(gles11::ARRAY_BUFFER, 0);
        gles.EnableClientState(gles11::VERTEX_ARRAY);
        gles.VertexPointer(2, gles11::FLOAT, 0, vertices.as_ptr() as *const GLvoid);
        gles.EnableClientState(gles11::TEXTURE_COORD_ARRAY);
        gles.TexCoordPointer(2, gles11::FLOAT, 0, tex_coords.as_ptr() as *const GLvoid);
        gles.MatrixMode(gles11::TEXTURE);
        gles.LoadMatrixf(matrix.columns().as_ptr() as *const _);
        gles.Enable(gles11::TEXTURE_2D);
        gles.DrawArrays(gles11::TRIANGLES, 0, 6);
        gles.LoadIdentity();

        if let Some((x, y, pressed)) = virtual_cursor_visible_at {
            let (vx, vy, vw, vh) = viewport;
            let x = x - vx as f32;
            let y = y - vy as f32;
            gles.Enable(gles11::BLEND);
            gles.BlendFunc(gles11::ONE, gles11::ONE_MINUS_SRC_ALPHA);
            let radius = 10.0;
            let mut cursor_vertices = vertices;
            for i in (0..cursor_vertices.len()).step_by(2) {
                cursor_vertices[i] = (cursor_vertices[i] * radius + x) / (vw as f32 / 2.0) - 1.0;
                cursor_vertices[i + 1] =
                    1.0 - (cursor_vertices[i + 1] * radius + y) / (vh as f32 / 2.0);
            }
            gles.DisableClientState(gles11::TEXTURE_COORD_ARRAY);
            gles.Disable(gles11::TEXTURE_2D);
            gles.Color4f(0.0, 0.0, 0.0, if pressed { 2.0 / 3.0 } else { 1.0 / 3.0 });
            gles.VertexPointer(
                2,
                gles11::FLOAT,
                0,
                cursor_vertices.as_ptr() as *const GLvoid,
            );
            gles.DrawArrays(gles11::TRIANGLES, 0, 6);
        }
    }

    // On-screen FPS overlay (simple bitmap font). Enabled by env var
    // TOUCHHLE_ONSCREEN_FPS=1 or by the runtime flag set via
    // crate::gles::present::set_onscreen_fps_enabled(true).
    let onscreen_env = std::env::var_os("TOUCHHLE_ONSCREEN_FPS").is_some();
    let onscreen_runtime = ONSCREEN_FPS_ENABLED
        .get()
        .map(|b| b.load(Ordering::SeqCst))
        .unwrap_or(false);
    if onscreen_env || onscreen_runtime {
        if let Some(mutex) = LAST_FPS_TEXT.get() {
            if let Ok(s) = mutex.lock() {
                if !s.is_empty() {
                    draw_onscreen_text(gles, viewport, &s);
                }
            }
        }
    }
}

// --- Tiny bitmap font & overlay drawing implementation ---
const GLYPH_W: u32 = 8;
const GLYPH_H: u32 = 8;

// Glyphs available in this tiny font: "0123456789:.FPS"
const GLYPH_CHARS: &str = "0123456789:.FPS";
// Each glyph is 8 bytes, each bit is a pixel (MSB left).
const GLYPH_BITMAPS: &[[u8; 8]] = &[
    // 0
    [0x3C, 0x66, 0x6E, 0x7E, 0x76, 0x66, 0x3C, 0x00],
    // 1
    [0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00],
    // 2
    [0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x7E, 0x00],
    // 3
    [0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00],
    // 4
    [0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C, 0x1E, 0x00],
    // 5
    [0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00],
    // 6
    [0x3C, 0x66, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00],
    // 7
    [0x7E, 0x66, 0x0C, 0x18, 0x18, 0x18, 0x18, 0x00],
    // 8
    [0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00],
    // 9
    [0x3C, 0x66, 0x66, 0x3E, 0x06, 0x66, 0x3C, 0x00],
    // : (colon)
    [0x00, 0x18, 0x18, 0x00, 0x00, 0x18, 0x18, 0x00],
    // . (dot)
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00],
    // F
    [0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00],
    // P
    [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00],
    // S
    [0x3C, 0x66, 0x30, 0x1C, 0x06, 0x66, 0x3C, 0x00],
];

fn glyph_index(ch: char) -> Option<usize> {
    GLYPH_CHARS.chars().position(|c| c == ch)
}

unsafe fn ensure_glyph_textures(gles: &mut dyn GLES) -> Option<Vec<u32>> {
    use gles11::types::*;
    let lock = GLYPH_TEXTURES.get().unwrap().lock().unwrap();
    if lock.is_some() {
        return lock.clone();
    }
    drop(lock);

    let mut guard = GLYPH_TEXTURES.get().unwrap().lock().unwrap();
    if guard.is_some() {
        return guard.clone();
    }

    let count = GLYPH_BITMAPS.len();
    let mut texs = Vec::with_capacity(count);
    for i in 0..count {
        let mut tex: GLuint = 0;
        gles.GenTextures(1, &mut tex);
        gles.BindTexture(gles11::TEXTURE_2D, tex);
        // Build RGBA data from bitmap
        let mut data = vec![0u8; (GLYPH_W * GLYPH_H * 4) as usize];
        let bmp = GLYPH_BITMAPS[i];
        for y in 0..GLYPH_H {
            let row = bmp[y as usize];
            for x in 0..GLYPH_W {
                let bit = (row >> (7 - x)) & 1;
                let idx = ((y * GLYPH_W + x) * 4) as usize;
                if bit != 0 {
                    data[idx] = 255; // R
                    data[idx + 1] = 255;
                    data[idx + 2] = 255;
                    data[idx + 3] = 255; // A
                } else {
                    data[idx] = 0;
                    data[idx + 1] = 0;
                    data[idx + 2] = 0;
                    data[idx + 3] = 0;
                }
            }
        }
        gles.TexImage2D(
            gles11::TEXTURE_2D,
            0,
            gles11::RGBA as _,
            GLYPH_W as _,
            GLYPH_H as _,
            0,
            gles11::RGBA,
            gles11::UNSIGNED_BYTE,
            data.as_ptr() as *const _,
        );
        gles.TexParameteri(
            gles11::TEXTURE_2D,
            gles11::TEXTURE_MIN_FILTER,
            gles11::NEAREST as _,
        );
        gles.TexParameteri(
            gles11::TEXTURE_2D,
            gles11::TEXTURE_MAG_FILTER,
            gles11::NEAREST as _,
        );
        gles.TexParameteri(
            gles11::TEXTURE_2D,
            gles11::TEXTURE_WRAP_S,
            gles11::CLAMP_TO_EDGE as _,
        );
        gles.TexParameteri(
            gles11::TEXTURE_2D,
            gles11::TEXTURE_WRAP_T,
            gles11::CLAMP_TO_EDGE as _,
        );
        texs.push(tex);
    }
    *guard = Some(texs.clone());
    Some(texs)
}

unsafe fn draw_onscreen_text(gles: &mut dyn GLES, viewport: (u32, u32, u32, u32), text: &str) {
    use gles11::types::*;
    let (vx, vy, vw, vh) = viewport;
    // Pixel size per glyph
    let scale = 2u32; // 8x8 * 2 = 16px high font
    let gw = (GLYPH_W * scale) as f32;
    let gh = (GLYPH_H * scale) as f32;

    // Ensure textures
    let texs_opt = ensure_glyph_textures(gles);
    if texs_opt.is_none() {
        return;
    }
    let texs = texs_opt.unwrap();

    // Save state
    let mut old_active_texture: GLint = 0;
    gles.GetIntegerv(gles11::ACTIVE_TEXTURE, &mut old_active_texture);
    let mut old_texture: GLint = 0;
    gles.GetIntegerv(gles11::TEXTURE_BINDING_2D, &mut old_texture);

    // Setup orthographic projection in pixels
    gles.MatrixMode(gles11::PROJECTION);
    gles.PushMatrix();
    gles.LoadIdentity();
    gles.Orthof(0.0, vw as _, vh as _, 0.0, -1.0, 1.0);
    gles.MatrixMode(gles11::MODELVIEW);
    gles.PushMatrix();
    gles.LoadIdentity();

    // Prepare arrays
    gles.EnableClientState(gles11::VERTEX_ARRAY);
    gles.EnableClientState(gles11::TEXTURE_COORD_ARRAY);
    gles.Enable(gles11::TEXTURE_2D);
    gles.Enable(gles11::BLEND);
    gles.BlendFunc(gles11::SRC_ALPHA, gles11::ONE_MINUS_SRC_ALPHA);

    // Draw text at top-left with small margin
    let mut x_px = vx as f32 + 8.0;
    let y_px = vy as f32 + 8.0;

    for ch in text.chars() {
        if let Some(idx) = glyph_index(ch) {
            let tex = texs[idx] as GLint;
            gles.BindTexture(gles11::TEXTURE_2D, tex as _);

            // Quad: two triangles
            let x0 = x_px;
            let y0 = y_px;
            let x1 = x_px + gw;
            let y1 = y_px + gh;
            let verts: [f32; 8] = [x0, y0, x0, y1, x1, y0, x1, y1];
            let texcoords: [f32; 8] = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0];
            gles.VertexPointer(2, gles11::FLOAT, 0, verts.as_ptr() as *const GLvoid);
            gles.TexCoordPointer(2, gles11::FLOAT, 0, texcoords.as_ptr() as *const GLvoid);
            gles.DrawArrays(gles11::TRIANGLE_STRIP, 0, 4);

            x_px += gw + 2.0;
        } else {
            // Unknown char -> space
            x_px += gw / 2.0;
        }
    }

    // Restore state
    gles.BindTexture(gles11::TEXTURE_2D, old_texture as _);
    gles.ActiveTexture(old_active_texture as _);
    gles.Disable(gles11::BLEND);
    gles.Disable(gles11::TEXTURE_2D);
    gles.DisableClientState(gles11::TEXTURE_COORD_ARRAY);
    gles.DisableClientState(gles11::VERTEX_ARRAY);

    gles.MatrixMode(gles11::MODELVIEW);
    gles.PopMatrix();
    gles.MatrixMode(gles11::PROJECTION);
    gles.PopMatrix();
    gles.MatrixMode(gles11::TEXTURE);
    gles.LoadIdentity();
}
