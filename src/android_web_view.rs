/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Bridge from the guest's `UIWebView` to a *real* native Android WebView.
//!
//! On Android, each `UIWebView` is backed by a genuine `android.webkit.WebView`
//! layered on top of the SDL surface, so pages load with a real engine
//! (HTML/CSS/JS, HTTPS, links, scrolling, back/forward). All calls go through
//! hand-rolled JNI (no `jni` crate dependency) into static methods on
//! `MainActivity` — see `android/app/src/main/java/org/touchhle/android/`.
//!
//! On other platforms every function here is a harmless no-op; the desktop
//! build renders webviews with the headless-Chromium snapshot bridge in
//! `ui_web_view.rs` instead.


/// A live overlay id, or `-1` when no overlay could be created.
pub type OverlayId = i32;

#[cfg(target_os = "android")]
mod imp {
    use super::*;
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_void};

    extern "C" {
        // Exported by libSDL2.so on Android (SDL_system.h). Returns the
        // JNIEnv* for the calling thread, attaching it to the JVM if needed.
        fn SDL_AndroidGetJNIEnv() -> *mut c_void;
    }

    /// JNI function-table slot indices. The table starts with 4 reserved
    /// pointers, hence every index below is `4 + <position in jni.h>`.
    mod slots {
        pub const FIND_CLASS: usize = 6;
        pub const EXCEPTION_OCCURRED: usize = 15;
        pub const EXCEPTION_CLEAR: usize = 17;
        pub const NEW_STRING_UTF: usize = 167;
        pub const GET_STRING_UTF_CHARS: usize = 169;
        pub const RELEASE_STRING_UTF_CHARS: usize = 170;
        pub const GET_STATIC_METHOD_ID: usize = 113 + 4;
        pub const CALL_STATIC_OBJECT_METHOD_A: usize = 110 + 2;
        pub const CALL_STATIC_BOOLEAN_METHOD_A: usize = 113 + 2;
        pub const CALL_STATIC_INT_METHOD_A: usize = 125 + 2;
        pub const CALL_STATIC_VOID_METHOD_A: usize = 137 + 2;
        pub const DELETE_LOCAL_REF: usize = 23;
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    union JValue {
        l: *mut c_void,
        i: c_int,
        z: u8,
        _pad: u64,
    }

    /// One JNI call context: the JNIEnv and a couple of vtable fns.
    struct Jni {
        env: *mut c_void,
    }

    impl Jni {
        fn attach() -> Option<Jni> {
            unsafe {
                let env = SDL_AndroidGetJNIEnv();
                if env.is_null() {
                    return None;
                }
                Some(Jni { env })
            }
        }

        fn slot<F>(&self, index: usize) -> F {
            unsafe {
                let table = self.env as *mut *mut c_void;
                std::mem::transmute_copy::<*mut c_void, F>(&*table.add(index))
            }
        }

        fn exception_pending(&self) -> bool {
            let f: unsafe extern "C" fn(*mut c_void) -> *mut c_void =
                self.slot(slots::EXCEPTION_OCCURRED);
            let thrown = unsafe { f(self.env) };
            !thrown.is_null()
        }

        fn clear_exception(&self) {
            let f: unsafe extern "C" fn(*mut c_void) = self.slot(slots::EXCEPTION_CLEAR);
            unsafe { f(self.env) }
        }

        fn find_main_activity_class(&self) -> Option<*mut c_void> {
            let name = CString::new("org/touchhle/android/MainActivity").ok()?;
            let f: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void =
                self.slot(slots::FIND_CLASS);
            let class = unsafe { f(self.env, name.as_ptr()) };
            if class.is_null() {
                self.clear_exception();
                return None;
            }
            Some(class)
        }

        fn get_static_method(
            &self,
            class: *mut c_void,
            name: &str,
            sig: &str,
        ) -> Option<*mut c_void> {
            let name = CString::new(name).ok()?;
            let sig = CString::new(sig).ok()?;
            let f: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *const c_char,
                *const c_char,
            ) -> *mut c_void = self.slot(slots::GET_STATIC_METHOD_ID);
            let method = unsafe { f(self.env, class, name.as_ptr(), sig.as_ptr()) };
            if method.is_null() {
                self.clear_exception();
                return None;
            }
            Some(method)
        }

        fn new_jstring(&self, s: &str) -> *mut c_void {
            let c = CString::new(s.replace('\0', "")).unwrap_or_default();
            let f: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void =
                self.slot(slots::NEW_STRING_UTF);
            unsafe { f(self.env, c.as_ptr()) }
        }

        fn jstring_to_rust(&self, js: *mut c_void) -> Option<String> {
            if js.is_null() {
                return None;
            }
            let get: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u8) -> *const c_char =
                self.slot(slots::GET_STRING_UTF_CHARS);
            let release: unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) =
                self.slot(slots::RELEASE_STRING_UTF_CHARS);
            let chars = unsafe { get(self.env, js, std::ptr::null_mut()) };
            if chars.is_null() {
                return None;
            }
            let s = unsafe { std::ffi::CStr::from_ptr(chars) }
                .to_string_lossy()
                .into_owned();
            unsafe { release(self.env, js, chars) };
            Some(s)
        }

        fn delete_local_ref(&self, obj: *mut c_void) {
            if obj.is_null() {
                return;
            }
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) =
                self.slot(slots::DELETE_LOCAL_REF);
            unsafe { f(self.env, obj) }
        }

        fn call_static_void(&self, class: *mut c_void, method: *mut c_void, args: &[JValue]) {
            let f: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *const JValue,
            ) = self.slot(slots::CALL_STATIC_VOID_METHOD_A);
            unsafe { f(self.env, class, method, args.as_ptr()) };
            if self.exception_pending() {
                self.clear_exception();
            }
        }

        fn call_static_bool(
            &self,
            class: *mut c_void,
            method: *mut c_void,
            args: &[JValue],
        ) -> bool {
            let f: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *const JValue,
            ) -> u8 = self.slot(slots::CALL_STATIC_BOOLEAN_METHOD_A);
            let r = unsafe { f(self.env, class, method, args.as_ptr()) };
            if self.exception_pending() {
                self.clear_exception();
            }
            r != 0
        }

        fn call_static_int(
            &self,
            class: *mut c_void,
            method: *mut c_void,
            args: &[JValue],
        ) -> c_int {
            let f: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *const JValue,
            ) -> c_int = self.slot(slots::CALL_STATIC_INT_METHOD_A);
            let r = unsafe { f(self.env, class, method, args.as_ptr()) };
            if self.exception_pending() {
                self.clear_exception();
            }
            r
        }

        fn call_static_object(
            &self,
            class: *mut c_void,
            method: *mut c_void,
            args: &[JValue],
        ) -> *mut c_void {
            let f: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *const JValue,
            ) -> *mut c_void = self.slot(slots::CALL_STATIC_OBJECT_METHOD_A);
            let r = unsafe { f(self.env, class, method, args.as_ptr()) };
            if self.exception_pending() {
                self.clear_exception();
            }
            r
        }

        fn with_class<T>(&self, f: impl FnOnce(&Jni, *mut c_void) -> Option<T>) -> Option<T> {
            let class = self.find_main_activity_class()?;
            let result = f(self, class);
            self.delete_local_ref(class);
            result
        }

        fn call_static_bool_method(&self, method_name: &str, id: OverlayId) -> Option<bool> {
            self.with_class(|jni, class| {
                let method = jni.get_static_method(class, method_name, "(I)Z")?;
                Some(jni.call_static_bool(class, method, &int_args(id)))
            })
        }

        fn call_static_void_method(&self, method_name: &str, id: OverlayId) {
            if let Some(class) = self.find_main_activity_class() {
                if let Some(method) = self.get_static_method(class, method_name, "(I)V") {
                    self.call_static_void(class, method, &int_args(id));
                }
                self.delete_local_ref(class);
            }
        }
    }

    fn int_args(id: OverlayId) -> [JValue; 1] {
        [JValue { i: id }]
    }

    pub fn show(url: &str, x: i32, y: i32, w: i32, h: i32) -> OverlayId {
        let Some(jni) = Jni::attach() else {
            return -1;
        };
        jni.with_class(|jni, class| {
            let Some(method) =
                jni.get_static_method(class, "showWebOverlay", "(Ljava/lang/String;IIII)I")
            else {
                return None;
            };
            let url_j = jni.new_jstring(url);
            let args = [
                JValue { l: url_j },
                JValue { i: x },
                JValue { i: y },
                JValue { i: w },
                JValue { i: h },
            ];
            let id = jni.call_static_int(class, method, &args);
            jni.delete_local_ref(url_j);
            Some(id)
        })
        .unwrap_or(-1)
    }

    pub fn navigate(id: OverlayId, url: &str) {
        let Some(jni) = Jni::attach() else {
            return;
        };
        jni.with_class(|jni, class| {
            let method =
                jni.get_static_method(class, "navigateWebOverlay", "(ILjava/lang/String;)V")?;
            let url_j = jni.new_jstring(url);
            let args = [JValue { i: id }, JValue { l: url_j }];
            jni.call_static_void(class, method, &args);
            jni.delete_local_ref(url_j);
            Some(())
        });
    }

    pub fn load_data(id: OverlayId, data: &str, mime: &str) {
        let Some(jni) = Jni::attach() else {
            return;
        };
        jni.with_class(|jni, class| {
            let method = jni.get_static_method(
                class,
                "loadDataWebOverlay",
                "(ILjava/lang/String;Ljava/lang/String;)V",
            )?;
            let data_j = jni.new_jstring(data);
            let mime_j = jni.new_jstring(mime);
            let args = [JValue { i: id }, JValue { l: data_j }, JValue { l: mime_j }];
            jni.call_static_void(class, method, &args);
            jni.delete_local_ref(data_j);
            jni.delete_local_ref(mime_j);
            Some(())
        });
    }

    pub fn set_bounds(id: OverlayId, x: i32, y: i32, w: i32, h: i32) {
        let Some(jni) = Jni::attach() else {
            return;
        };
        jni.with_class(|jni, class| {
            let method = jni.get_static_method(class, "setWebOverlayBounds", "(IIIII)V")?;
            let args = [
                JValue { i: id },
                JValue { i: x },
                JValue { i: y },
                JValue { i: w },
                JValue { i: h },
            ];
            jni.call_static_void(class, method, &args);
            Some(())
        });
    }

    pub fn hide(id: OverlayId) {
        if let Some(jni) = Jni::attach() {
            jni.call_static_void_method("hideWebOverlay", id);
        }
    }

    pub fn go_back(id: OverlayId) {
        if let Some(jni) = Jni::attach() {
            jni.call_static_void_method("goBackWebOverlay", id);
        }
    }

    pub fn go_forward(id: OverlayId) {
        if let Some(jni) = Jni::attach() {
            jni.call_static_void_method("goForwardWebOverlay", id);
        }
    }

    pub fn can_go_back(id: OverlayId) -> bool {
        Jni::attach()
            .and_then(|jni| jni.call_static_bool_method("canGoBackWebOverlay", id))
            .unwrap_or(false)
    }

    pub fn can_go_forward(id: OverlayId) -> bool {
        Jni::attach()
            .and_then(|jni| jni.call_static_bool_method("canGoForwardWebOverlay", id))
            .unwrap_or(false)
    }

    pub fn eval_js(id: OverlayId, script: &str) -> Option<String> {
        let jni = Jni::attach()?;
        jni.with_class(|jni, class| {
            let method = jni.get_static_method(
                class,
                "evalJsWebOverlay",
                "(ILjava/lang/String;)Ljava/lang/String;",
            )?;
            let script_j = jni.new_jstring(script);
            let args = [JValue { i: id }, JValue { l: script_j }];
            let out = jni.call_static_object(class, method, &args);
            let s = jni.jstring_to_rust(out);
            jni.delete_local_ref(script_j);
            jni.delete_local_ref(out);
            s
        })
    }

    pub fn stop_loading(id: OverlayId) {
        if let Some(jni) = Jni::attach() {
            jni.call_static_void_method("stopLoadingWebOverlay", id);
        }
    }

    /// Whether a real overlay has been created (used by `ui_web_view.rs` to
    /// decide between the native path and the desktop Chromium fallback).
    pub fn native_webview_available() -> bool {
        Jni::attach()
            .and_then(|jni| jni.find_main_activity_class())
            .is_some()
    }
}

#[cfg(not(target_os = "android"))]
mod imp {
    use super::OverlayId;

    pub fn show(_url: &str, _x: i32, _y: i32, _w: i32, _h: i32) -> OverlayId {
        -1
    }
    pub fn navigate(_id: OverlayId, _url: &str) {}
    pub fn load_data(_id: OverlayId, _data: &str, _mime: &str) {}
    pub fn set_bounds(_id: OverlayId, _x: i32, _y: i32, _w: i32, _h: i32) {}
    pub fn hide(_id: OverlayId) {}
    pub fn go_back(_id: OverlayId) {}
    pub fn go_forward(_id: OverlayId) {}
    pub fn can_go_back(_id: OverlayId) -> bool {
        false
    }
    pub fn can_go_forward(_id: OverlayId) -> bool {
        false
    }
    pub fn eval_js(_id: OverlayId, _script: &str) -> Option<String> {
        None
    }
    pub fn stop_loading(_id: OverlayId) {}
    pub fn native_webview_available() -> bool {
        false
    }
}

pub use imp::*;
