/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! glob? glob. glob-glob

use super::dirent::{closedir, dirent, opendir, readdir, DIR};
use super::fnmatch::fnmatch;
use super::string::strlen;
use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{guest_size_of, ConstPtr, GuestUSize, MutPtr, SafeRead};
use crate::Environment;

// Our internal type.
type GlobFlagType = i32;
const GLOB_DOOFFS: GlobFlagType = 0x2;
const GLOB_NOSORT: GlobFlagType = 0x20;
const GLOB_MAGCHAR: GlobFlagType = 0x100;
const GLOB_NOESCAPE: GlobFlagType = 0x2000;

// POSIX glob() flags
const GLOB_APPEND: GlobFlagType = 0x1;
const GLOB_MARK: GlobFlagType = 0x4;
const GLOB_NOCHECK: GlobFlagType = 0x10;

// Error codes follow the BSD/Darwin glob.h convention (touchHLE targets
// iOS): GLOB_NOSPACE (-1), GLOB_ABORTED (-2), GLOB_NOMATCH (-3).
// (-4 is GLOB_NOSYS on Darwin, not GLOB_NOSPACE.)
const GLOB_NOMATCH: i32 = -3;
const GLOB_NOSPACE: i32 = -1;

/// Sanity cap on the size of the results list (reserved slots included).
/// A guest-supplied `gl_offs` (with GLOB_DOOFFS) or a garbage `gl_pathc`
/// (with GLOB_APPEND) must not make us attempt an absurd allocation, which
/// could abort the process in the allocator.
const MAX_GLOB_RESULTS: u64 = 0x10_0000;

#[repr(C, packed)]
struct glob_t {
    gl_pathc: GuestUSize,
    gl_matchc: i32,
    gl_offs: GuestUSize,
    gl_flags: i32,
    gl_pathv: MutPtr<MutPtr<u8>>,
    gl_errfunc: GuestFunction,  // TODO
    gl_closedir: GuestFunction, // TODO
    gl_readdir: GuestFunction,  // TODO
    gl_opendir: GuestFunction,  // TODO
    gl_lstat: GuestFunction,    // TODO
    gl_stat: GuestFunction,     // TODO
}
unsafe impl SafeRead for glob_t {}

fn glob(
    env: &mut Environment,
    pattern: ConstPtr<u8>,
    flags: GlobFlagType,
    err_func: GuestFunction,
    pglob: MutPtr<glob_t>,
) -> i32 {
    let pattern_str = env.mem.cstr_at_utf8(pattern);
    log_dbg!(
        "glob({:?}, {}, {:?}, {:?})",
        pattern_str,
        flags,
        err_func,
        pglob
    );
    if !err_func.to_ptr().is_null() {
        // POSIX: errfunc is only consulted for directory-open failures, which
        // our implementation cannot hit (opendir of the literal directory
        // prefix). Accept non-NULL values instead of asserting so apps that
        // pass one keep working.
        log_dbg!("glob(): ignoring non-NULL errfunc");
    }
    let do_offs = flags & GLOB_DOOFFS != 0;

    // Reject (with a warning) flag combinations that change the meaning of
    // results in ways we do not implement, per POSIX these would be honored.
    let unsupported = flags
        & !(GLOB_DOOFFS | GLOB_NOSORT | GLOB_NOESCAPE | GLOB_APPEND | GLOB_NOCHECK | GLOB_MARK);
    if unsupported != 0 {
        log!(
            "glob(): unsupported flags {:#x}, results may be incomplete",
            unsupported
        );
    }
    let no_check = flags & GLOB_NOCHECK != 0;

    // Guest-reachable: a pattern that is not valid UTF-8 must not crash the
    // host. Degrade to a lossy conversion, which at worst fails to match.
    let pattern_str: String = match pattern_str {
        Ok(s) => s.to_owned(),
        Err(bytes) => {
            log!("glob(): pattern is not valid UTF-8, using lossy conversion");
            String::from_utf8_lossy(bytes).into_owned()
        }
    };
    // Backslash escaping is not supported; treat a backslash literally, like
    // GLOB_NOESCAPE does, rather than panicking.
    let (directory, subpattern) = match pattern_str.rsplit_once('/') {
        Some(parts) => parts,
        None => ("", pattern_str.as_str()),
    };
    // The last path segment holds the pattern; everything before it is a
    // literal directory. Support '*' only in the final segment, which covers
    // the app-bundle scanning patterns seen in practice.
    let has_star_wildcard = subpattern.contains('*');

    let directory_c_str = env.mem.alloc_and_write_cstr(directory.as_bytes());
    let dirp: MutPtr<DIR> = opendir(env, directory_c_str.cast_const());
    env.mem.free(directory_c_str.cast());
    if dirp.is_null() {
        // The directory could not be opened (e.g. it does not exist). POSIX
        // says glob() returns GLOB_NOMATCH in this case rather than failing.
        if no_check {
            return glob_no_check(env, pglob, flags, do_offs, &pattern_str);
        }
        return GLOB_NOMATCH;
    }

    let subpattern_c_str: ConstPtr<u8> = env
        .mem
        .alloc_and_write_cstr(subpattern.as_bytes())
        .cast_const();

    let dirent_name_offset = std::mem::offset_of!(dirent, d_name) as GuestUSize;

    let mut next_dir_entry = readdir(env, dirp);
    let mut tmp_vec: Vec<MutPtr<u8>> = vec![];
    while !next_dir_entry.is_null() {
        let name_c_str: ConstPtr<u8> = next_dir_entry.cast().cast_const() + dirent_name_offset;

        // POSIX: glob() never returns "." or ".." entries (unless explicitly
        // included in the pattern, which we don't support — leading dots also
        // require GLOB_PERIOD, which is not honored here).
        {
            let name: &[u8] = &env.mem.cstr_at(name_c_str);
            if name == b"." || name == b".." {
                next_dir_entry = readdir(env, dirp);
                continue;
            }
        }

        // TODO: should we match on the whole path or just the filename?
        if fnmatch(env, subpattern_c_str, name_c_str, 0) == 0 {
            // TODO: use `lstat` and/or `stat` to get information on names found
            let name_len: GuestUSize = strlen(env, name_c_str);
            let dir_len = directory.len() as GuestUSize;
            let size = dir_len + 1 + name_len + 1;

            let buf = env.mem.calloc(size).cast::<u8>();
            env.mem
                .bytes_at_mut(buf, dir_len)
                .copy_from_slice(directory.as_bytes());
            env.mem.bytes_at_mut(buf + dir_len, 1).copy_from_slice(b"/");
            let name_bytes = env.mem.bytes_at(name_c_str, name_len).to_vec();
            env.mem
                .bytes_at_mut(buf + dir_len + 1, name_len)
                .copy_from_slice(&name_bytes);

            tmp_vec.push(buf);
        }

        next_dir_entry = readdir(env, dirp);
    }

    env.mem.free(subpattern_c_str.cast_mut().cast());
    closedir(env, dirp);

    if tmp_vec.is_empty() && no_check {
        // GLOB_NOCHECK: return the pattern itself instead of an empty list.
        // Do not build the (empty) result list first so that nothing leaks.
        return glob_no_check(env, pglob, flags, do_offs, &pattern_str);
    }

    let mut tmp_glob = env.mem.read(pglob);
    tmp_glob.gl_matchc = tmp_vec.len() as GuestUSize as i32;
    tmp_glob.gl_flags = if has_star_wildcard {
        flags | GLOB_MAGCHAR
    } else {
        flags & !GLOB_MAGCHAR
    };
    env.mem.write(pglob, tmp_glob);

    let res = write_glob_results(env, pglob, flags, do_offs, &tmp_vec);
    if res != 0 {
        // The results list was not built, so the matched strings are not
        // referenced anywhere; free them instead of leaking guest memory.
        for entry in &tmp_vec {
            env.mem.free(entry.cast());
        }
        return res;
    }
    if tmp_vec.is_empty() {
        GLOB_NOMATCH
    } else {
        0 // success and match
    }
}

/// Write `entries` into a freshly allocated results list (merging in any
/// previous results when GLOB_APPEND is set) and store it in `*pglob`.
/// Returns GLOB_NOSPACE on overflow, leaving `*pglob` untouched.
fn write_glob_results(
    env: &mut Environment,
    pglob: MutPtr<glob_t>,
    flags: GlobFlagType,
    do_offs: bool,
    entries: &[MutPtr<u8>],
) -> i32 {
    let tmp_glob = env.mem.read(pglob);
    let offs: u64 = if do_offs { tmp_glob.gl_offs as u64 } else { 0 };

    // GLOB_APPEND: the previous results belong to us; move their entries
    // into the new list and release the old array (POSIX keeps the matches
    // and requires appending to them).
    let (old_pathv, old_pathc) =
        if flags & GLOB_APPEND != 0 && !tmp_glob.gl_pathv.is_null() && tmp_glob.gl_pathc > 0 {
            (tmp_glob.gl_pathv, tmp_glob.gl_pathc as u64)
        } else {
            (MutPtr::null(), 0u64)
        };
    let new_matches = entries.len() as u64;
    // Slots reserved by GLOB_DOOFFS plus the previous matches are copied
    // over to their original indices.
    let kept_count = if old_pathc > 0 {
        offs.saturating_add(old_pathc)
    } else {
        0
    };
    let total_count = kept_count.saturating_add(new_matches);
    if total_count > MAX_GLOB_RESULTS {
        log!(
            "glob(): refusing huge result list ({} entries, offs {})",
            total_count,
            offs
        );
        return GLOB_NOSPACE;
    }
    let list_size = (total_count + 1) * guest_size_of::<MutPtr<u8>>() as u64;
    let list_out: MutPtr<MutPtr<u8>> = env.mem.calloc(list_size as GuestUSize).cast();
    for i in 0..kept_count {
        let old_entry: MutPtr<u8> = env.mem.read(old_pathv + i as GuestUSize);
        env.mem.write(list_out + i as GuestUSize, old_entry);
    }
    if !old_pathv.is_null() {
        // The entries were moved into the new list; only the array itself
        // is freed, so repeated GLOB_APPEND calls do not leak it.
        env.mem.free(old_pathv.cast());
    }
    for (idx, entry) in entries.iter().enumerate() {
        env.mem.write(
            list_out + (kept_count as GuestUSize) + idx as GuestUSize,
            *entry,
        );
    }
    let mut tmp_glob = env.mem.read(pglob);
    tmp_glob.gl_pathc = (old_pathc + new_matches) as GuestUSize;
    tmp_glob.gl_pathv = list_out;
    env.mem.write(pglob, tmp_glob);
    0
}

/// GLOB_NOCHECK: if nothing matched, return the pattern itself as the sole
/// "match", exactly as if it were a literal path (POSIX 2.13.2).
fn glob_no_check(
    env: &mut Environment,
    pglob: MutPtr<glob_t>,
    flags: GlobFlagType,
    do_offs: bool,
    pattern_str: &str,
) -> i32 {
    let name_len = pattern_str.len() as GuestUSize;
    let buf = env.mem.calloc(name_len as u32 + 1).cast::<u8>();
    env.mem
        .bytes_at_mut(buf, name_len as GuestUSize)
        .copy_from_slice(pattern_str.as_bytes());

    let res = write_glob_results(env, pglob, flags, do_offs, &[buf]);
    if res != 0 {
        env.mem.free(buf.cast());
        return res;
    }
    let mut tmp_glob = env.mem.read(pglob);
    tmp_glob.gl_matchc = 1;
    tmp_glob.gl_flags = flags | GLOB_MAGCHAR;
    env.mem.write(pglob, tmp_glob);
    0
}

fn globfree(env: &mut Environment, pglob: MutPtr<glob_t>) {
    let tmp_glob = env.mem.read(pglob);
    let offs = if tmp_glob.gl_flags & GLOB_DOOFFS != 0 {
        tmp_glob.gl_offs
    } else {
        0
    };
    for i in 0..tmp_glob.gl_pathc as GuestUSize {
        let match_ = env.mem.read(tmp_glob.gl_pathv + offs + i);
        env.mem.free(match_.cast());
    }
    env.mem.free(tmp_glob.gl_pathv.cast());
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(glob(_, _, _, _)),
    export_c_func!(globfree(_)),
];
