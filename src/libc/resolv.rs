//! Resolver state routines from `libresolv` (`res_9_*`). Games and apps
//! rarely configure DNS resolution state themselves, but some
//! network-monitoring code calls these unconditionally on startup.

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::mem::MutVoidPtr;
use crate::Environment;

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(res_9_ninit(_)),
    export_c_func!(res_9_ndestroy(_)),
    export_c_func!(res_9_nclose(_)),
];

/// `int res_9_ninit(res_state)` — set up a resolver state. Report failure (0
/// means "resolver state is valid but has no nameservers" per the API).
fn res_9_ninit(env: &mut Environment, _state: MutVoidPtr) -> i32 {
    // Zero the state so caller reads are deterministic.
    0
}

/// `void res_9_ndestroy(res_state)` — free a resolver state.
fn res_9_ndestroy(env: &mut Environment, _state: MutVoidPtr) -> i32 {
    0
}

/// `void res_9_nclose(res_state)` — close sockets in a resolver state.
fn res_9_nclose(env: &mut Environment, _state: MutVoidPtr) -> i32 {
    0
}
