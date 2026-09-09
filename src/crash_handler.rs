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

/// Append a message to the log file (and stderr). Safe to call from a panic
/// hook; uses file-level locking via try_lock so re-entrant panics don't
/// deadlock — on contention the message is dropped rather than deadlocked.
pub fn append_to_log(msg: &str) {
    use std::io::Write;
    if let Ok(mut log_file) = crate::log::get_log_file().try_lock() {
        let _ = log_file.write_all(msg.as_bytes());
        let _ = log_file.write_all(b"\n");
        let _ = log_file.flush();
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = std::io::stderr().write_all(msg.as_bytes());
        let _ = std::io::stderr().write_all(b"\n");
    }
}

/// Install a Rust panic hook that mirrors panic messages into the touchHLE
/// log file. The default hook only writes to stderr/logcat, which users
/// rarely capture, so panics look like silent aborts (especially on Android,
/// where a panic unwinding out of a guest-thread coroutine ends in
/// SIGABRT — see the FATAL SIGNAL marker in the log).
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        let msg = format!(
            "touchHLE: PANIC in thread \"{}\" at {}: {}\n(panic is followed by unwinding; if this appears right before a FATAL SIGNAL line, the panic crossed a coroutine boundary and aborted the process)",
            thread_name,
            info.location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "<unknown>".to_string()),
            info.payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_string()),
        );
        append_to_log(&msg);
    }));
}

#[cfg(unix)]
mod imp {
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
        let msg = format!(
            "{}last guest PC: {:#x} (see CPU loop)\n",
            msg,
            crate::environment::LAST_GUEST_PC.load(Ordering::Relaxed)
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
}

#[cfg(unix)]
pub use imp::{install, set_log_fd};

#[cfg(not(unix))]
pub fn set_log_fd(_fd: i32) {}

#[cfg(not(unix))]
pub fn install() {}
