/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ifaddrs.h` and `net/if.h` (interface addresses and interface naming)

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::errno::{set_errno, ENOENT, ENXIO};
use crate::mem::{ConstPtr, GuestUSize, MutPtr, MutVoidPtr, SafeRead};
use crate::Environment;

// Mirrors the POSIX `struct ifaddrs` layout as seen by 32-bit ARM guests.
// All pointer fields are 4-byte guest pointers.
#[allow(non_camel_case_types)]
#[repr(C, packed)]
pub struct ifaddrs {
    /// Next node in the linked list (NULL = end).
    pub ifa_next: MutPtr<ifaddrs>,
    /// NUL-terminated interface name, e.g. "en0".
    pub ifa_name: ConstPtr<u8>,
    /// Interface flags (IFF_UP, IFF_LOOPBACK, …).
    pub ifa_flags: u32,
    /// Primary address (may be NULL).
    pub ifa_addr: u32, // guest ptr to sockaddr – typed as u32 to avoid pulling in socket types
    /// Netmask (may be NULL).
    pub ifa_netmask: u32,
    /// Broadcast or point-to-point destination address (may be NULL).
    pub ifa_broadaddr: u32,
    /// Protocol-specific data (may be NULL).
    pub ifa_data: u32,
}
// SAFETY: the struct is plain data; every field is either a scalar or a guest
// pointer that touchHLE's pointer type already validates.
unsafe impl SafeRead for ifaddrs {}

// ---------------------------------------------------------------------------
// getifaddrs / freeifaddrs
// ---------------------------------------------------------------------------

/// `int getifaddrs(struct ifaddrs **ifap)`
///
/// Returns success (0) with an empty interface list (*ifap = NULL).
/// Network-aware apps interpret an empty list as "no network interfaces
/// available" and gracefully fall back to offline mode, which is the
/// correct behavior for an emulator that doesn't expose host networking.
fn getifaddrs(env: &mut Environment, ifap: MutPtr<MutPtr<ifaddrs>>) -> i32 {
    // Write NULL into *ifap — an empty linked list means no interfaces.
    if !ifap.is_null() {
        env.mem.write(ifap, MutPtr::null());
    }

    log_dbg!("getifaddrs() => 0 (empty list, no interfaces exposed to guest)");
    0 // success
}

/// `void freeifaddrs(struct ifaddrs *ifa)`
///
/// Since our `getifaddrs` never allocates anything, this is a no-op. If a
/// future implementation does allocate, the deallocation logic belongs here.
fn freeifaddrs(_env: &mut Environment, ifa: MutPtr<ifaddrs>) {
    if !ifa.is_null() {
        // Future: walk the linked list and free each node + name string.
        log!(
            "TODO: freeifaddrs({:#x}) – list was not allocated by us, ignoring",
            ifa.to_bits()
        );
    }
}

// ---------------------------------------------------------------------------
// net/if.h – interface index / name mapping
// (commonly used together with ifaddrs by network-aware apps)
// ---------------------------------------------------------------------------

/// Maximum length of an interface name including the NUL terminator.
const IF_NAMESIZE: usize = 16;

// ---------------------------------------------------------------------------
// Fake interface table
// ---------------------------------------------------------------------------
//
// touchHLE doesn't expose host networking to the guest (`getifaddrs()`
// returns an empty list), but a completely empty interface table breaks
// some apps: e.g. Turbo Dismount's prime31 SocialNetworking plugin calls
// `if_nametoindex()` on a Wi-Fi/3G probe, treats a 0 return as a hard
// failure and crashes with an unhandled Mono NullReferenceException.
//
// On a real iPhone OS device, `lo0` and `en0` (Wi-Fi) always exist with
// well-known indices, so we present a minimal virtual table that matches
// that expectation. Interface *data* is still absent (no addresses, no
// routes), so apps that actually open sockets get the usual
// "network not supported" error path instead of a crash.

/// Fake loopback interface (`lo0`), index 1 — always exists on BSD/iOS.
const FAKE_LOOPBACK_INDEX: u32 = 1;
const FAKE_LOOPBACK_NAME: &[u8] = b"lo0";
/// Fake Wi-Fi interface (`en0`), index 2 — always exists on iOS devices.
const FAKE_WIFI_INDEX: u32 = 2;
const FAKE_WIFI_NAME: &[u8] = b"en0";
/// Index assigned to any other probed interface name (pdp_ip0, utun0, …).
/// Returning a valid index for unknown names is deliberately permissive:
/// the guest is told the interface exists, which keeps network-availability
/// probes happy even when they look for a name we don't model.
const FAKE_OTHER_INDEX: u32 = 3;

/// `unsigned int if_nametoindex(const char *ifname)`
///
/// Returns the index for the named interface, or 0 on error (per POSIX,
/// which also documents `errno` getting set to `ENXIO`).
///
/// Unlike real kernels, unknown-but-plausible names map to a stable fake
/// index ([FAKE_OTHER_INDEX]) so availability probes succeed. A NULL or
/// empty name still returns 0, as on real OSes.
fn if_nametoindex(env: &mut Environment, ifname: ConstPtr<u8>) -> u32 {
    let name = env.mem.cstr_at_utf8(ifname).unwrap_or("");
    let index = if name.is_empty() {
        0
    } else if name.eq_ignore_ascii_case("lo0") {
        FAKE_LOOPBACK_INDEX
    } else if name.eq_ignore_ascii_case("en0") {
        FAKE_WIFI_INDEX
    } else {
        FAKE_OTHER_INDEX
    };
    if index == 0 {
        set_errno(env, ENXIO);
    } else {
        log_dbg!(
            "if_nametoindex(\"{}\") => {} (fake virtual interface)",
            name,
            index
        );
    }
    index
}

/// `char *if_indextoname(unsigned int ifindex, char *ifname)`
///
/// Writes the name of interface `ifindex` into `ifname` (at least
/// `IF_NAMESIZE` bytes) and returns `ifname`, or NULL on error.
/// Inverse of [if_nametoindex]: only the modeled fake interfaces resolve.
fn if_indextoname(env: &mut Environment, ifindex: u32, ifname: MutPtr<u8>) -> MutPtr<u8> {
    let name: &[u8] = match ifindex {
        FAKE_LOOPBACK_INDEX => FAKE_LOOPBACK_NAME,
        FAKE_WIFI_INDEX => FAKE_WIFI_NAME,
        FAKE_OTHER_INDEX => b"en1",
        _ => {
            set_errno(env, ENXIO);
            return MutPtr::null();
        }
    };
    if ifname.is_null() {
        set_errno(env, ENXIO);
        return MutPtr::null();
    }
    if name.len() + 1 > IF_NAMESIZE {
        set_errno(env, ENXIO);
        return MutPtr::null();
    }
    for (i, &byte) in name.iter().chain(std::iter::once(&0)).enumerate() {
        env.mem.write(ifname + i as u32, byte);
    }
    log_dbg!(
        "if_indextoname({}) => \"{}\"",
        ifindex,
        std::str::from_utf8(name).unwrap_or("?")
    );
    ifname
}

// `struct if_nameindex` used by if_nameindex() / if_freenameindex().
#[allow(non_camel_case_types)]
#[repr(C, packed)]
pub struct if_nameindex {
    pub if_index: u32,
    pub if_name: ConstPtr<u8>,
}
unsafe impl SafeRead for if_nameindex {}

// Layout of the guest-allocated if_nameindex() result:
// [lo0 entry][en0 entry][other entry][terminator][lo0 name][en0 name][en1 name]
const IF_NAMEINDEX_TABLE_BYTES: GuestUSize = (std::mem::size_of::<if_nameindex>() as GuestUSize)
    * 4
    + (FAKE_LOOPBACK_NAME.len() as GuestUSize + 1)
    + (FAKE_WIFI_NAME.len() as GuestUSize + 1)
    + (3 + 1); // "en1"

/// `struct if_nameindex *if_nameindex(void)`
///
/// Returns a guest-allocated array of all interface name/index pairs
/// terminated by an entry with `if_index == 0` and `if_name == NULL`.
/// The array (and the referenced names) live in a single guest allocation
/// and are freed by [if_freenameindex].
fn if_nameindex(env: &mut Environment) -> MutPtr<if_nameindex> {
    let base: MutPtr<u8> = env.mem.alloc(IF_NAMEINDEX_TABLE_BYTES).cast();
    if base.is_null() {
        set_errno(env, ENOENT);
        return MutPtr::null();
    }

    let entry_size = std::mem::size_of::<if_nameindex>() as u32;
    let name_area: MutPtr<u8> = MutPtr::from_bits(base.to_bits() + entry_size * 4);

    let mut write_entry = |slot: u32, index: u32, name: &[u8], name_off: u32| {
        let entry: MutPtr<if_nameindex> = MutPtr::from_bits(base.to_bits() + slot * entry_size);
        let name_ptr: MutPtr<u8> = MutPtr::from_bits(name_area.to_bits() + name_off);
        for (i, &byte) in name.iter().chain(std::iter::once(&0)).enumerate() {
            env.mem.write(
                MutPtr::from_bits(name_area.to_bits() + name_off + i as u32),
                byte,
            );
        }
        env.mem.write(
            entry,
            if_nameindex {
                if_index: index,
                if_name: name_ptr.cast_const(),
            },
        );
    };

    let mut name_off: u32 = 0;
    let names: [(u32, &[u8]); 3] = [
        (FAKE_LOOPBACK_INDEX, FAKE_LOOPBACK_NAME),
        (FAKE_WIFI_INDEX, FAKE_WIFI_NAME),
        (FAKE_OTHER_INDEX, b"en1"),
    ];
    for (slot, (index, name)) in names.iter().enumerate() {
        write_entry(slot as u32, *index, name, name_off);
        name_off += name.len() as u32 + 1;
    }
    // Terminator entry: index 0, NULL name.
    let terminator: MutPtr<if_nameindex> = MutPtr::from_bits(base.to_bits() + 3 * entry_size);
    env.mem.write(
        terminator,
        if_nameindex {
            if_index: 0,
            if_name: ConstPtr::null(),
        },
    );

    log_dbg!("if_nameindex() => table with 3 fake interfaces (lo0, en0, en1)");
    base.cast()
}

/// `void if_freenameindex(struct if_nameindex *ptr)`
///
/// Frees the single guest allocation backing the array returned by
/// [if_nameindex].
fn if_freenameindex(env: &mut Environment, ptr: MutPtr<if_nameindex>) {
    if !ptr.is_null() {
        let base: MutVoidPtr = ptr.cast();
        env.mem.free(base);
    }
}

// ---------------------------------------------------------------------------
// Export table
// ---------------------------------------------------------------------------

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(getifaddrs(_)),
    export_c_func!(freeifaddrs(_)),
    export_c_func!(if_nametoindex(_)),
    export_c_func!(if_indextoname(_, _)),
    export_c_func!(if_nameindex()),
    export_c_func!(if_freenameindex(_)),
];
