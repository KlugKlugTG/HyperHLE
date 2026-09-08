/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stub for `JavaScriptCore.framework/JavaScriptCore`.
//!
//! On iOS, JavaScriptCore exposes `JSContext` / `JSValue` and the C API
//! (`JSEvaluateScript`, …) for running JavaScript. Ad mediation SDKs (MoPub,
//! Localytics, older Facebook Audience Network builds) commonly link it into
//! their embedded frameworks, but never execute any script until an actual
//! ad is fetched over the network — which never happens in an offline
//! emulator.
//!
//! Registering the dylib with no exports suppresses the
//! `app binary depends on unimplemented or missing dylib` warning. If a guest
//! ever calls into it, the unimplemented-function path will report the exact
//! symbol once, at the call site.

use crate::dyld::HostDylib;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/JavaScriptCore.framework/JavaScriptCore",
    aliases: &[],
    class_exports: &[],
    constant_exports: &[],
    function_exports: &[],
};
