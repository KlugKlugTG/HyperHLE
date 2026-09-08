/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Native crash diagnostics.
//!
//! touchHLE's own errors and guest errors are always logged, but a *host*
//! crash (SIGSEGV/SIGBUS/SIGILL from e.g. a JIT bug or a graphics driver)
//! kills the process silently, which makes bug reports undiagnosable ("it
//! just closes with no error"). These handlers write a one-line marker with
//! the fatal signal and faulting address to the same log file the user can
//! share, then restore the default disposition and re-raise so the platform's
//! own crash reporting (Android tombstones etc.) still works.

use std::sync::atomic::{AtomicI32, Ordering};

/// Raw fd of the touchHLE log file, so the signal handler (which cannot
/// safely use the `Mutex<File>` in `log::get_log_file()`) can append to it.
static LOG_FD: AtomicI32 = AtomicI32::new(-1);

/// Register the raw fd of the log file. Called after the log file is
/// created; async-signal-safe `write(2)` then targets it.
pub fn set_log_fd(fd: i32) {
    LOG_FD.store(fd, Ordering::SeqCst);
}

const NAME_SEGV: &[u8] = b"SIGSEGV\0";
const NAME_BUS: &[u8] = b"SIGBUS\0";
const NAME_ILL: &[u8] = b"SIGILL\0";
const NAME_ABORT: &[u8] = b"SIGABRT\0";

extern "C" fn handler(sig: libc::c_int, info: *mut libc::siginfo_t, _uc: *mut libc::c_void) {
    let name: &[u8] = match sig {
        libc::SIGSEGV => NAME_SEGV,
        libc::SIGBUS => NAME_BUS,
        libc::SIGILL => NAME_ILL,
        libc::SIGABRT => NAME_ABORT,
        _ => b"SIGNAL\0",
    };
    // si_addr is the faulting memory address for SIGSEGV/SIGBUS/SIGILL.
    let addr = unsafe {
        match sig {
            libc::SIGABRT => 0,
            _ => (*(info as *const libc::siginfo_t)).si_addr() as usize,
        }
    };
    let msg = format!(
        "touchHLE: FATAL: native host crash: {} at address {:#x} — NOT a guest/app error; this is a touchHLE or driver/JIT bug. The process will now terminate.\n",
        std::str::from_utf8(&name[..name.len() - 1]).unwrap_or("SIGNAL"),
        addr
    );
    let bytes = msg.as_bytes();
    unsafe {
        // Best-effort write to both stderr and the log file. write(2) is
        // async-signal-safe.
        let _ = libc::write(2, bytes.as_ptr() as *const libc::c_void, bytes.len());
        let fd = LOG_FD.load(Ordering::SeqCst);
        if fd >= 0 {
            let _ = libc::write(fd, bytes.as_ptr() as *const libc::c_void, bytes.len());
        }
        // Restore the default disposition and re-raise so the platform's
        // crash reporter (Android tombstone, core dumps) still sees it.
        let mut dfl: libc::sigaction = std::mem::zeroed();
        dfl.sa_sigaction = libc::SIG_DFL;
        libc::sigaction(sig, &dfl, std::ptr::null_mut());
        libc::raise(sig);
    }
}

/// Install the diagnostic handlers for the fatal native signals.
pub fn install() {
    let mut act: libc::sigaction = unsafe { std::mem::zeroed() };
    act.sa_flags = libc::SA_SIGINFO | libc::SA_NODEFER;
    act.sa_sigaction = handler as usize;
    for &sig in &[libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGABRT] {
        unsafe {
            libc::sigaction(sig, &act, std::ptr::null_mut());
        }
    }
}
