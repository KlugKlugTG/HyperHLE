/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ptrace.h`

use crate::dyld::FunctionExports;
use crate::libc::errno::{set_errno, EINVAL};
use crate::libc::unistd::pid_t;
use crate::mem::MutPtr;
use crate::{export_c_func, Environment};

// Специфичный для Apple флаг защиты от отладки
const PT_DENY_ATTACH: i32 = 31;

fn ptrace(env: &mut Environment, request: i32, pid: pid_t, addr: MutPtr<u8>, data: i32) -> i32 {
    match request {
        PT_DENY_ATTACH => {
            log_dbg!("ptrace(PT_DENY_ATTACH) called by app for anti-debugging.");

            // Честная реализация: если GDB-сервер
            // GDB-сервер в этой конфигурации не отслеживается; отладчика нет,
            // вызов успешен (как на реальном устройстве без отладчика).
            log_dbg!("ptrace(PT_DENY_ATTACH): no debugger attached, succeeding.");

            // Если отладчика нет, вызов успешен.
            // Возвращаем 0.
            // загрузку.
            0
        }
        _ => {
            log!(
                "Warning: unhandled ptrace({}, {}, {:?}, {}) called, returning -1",
                request,
                pid,
                addr,
                data
            );
            // По стандарту POSIX, если request
            // неизвестен, нужно выставить
            set_errno(env, EINVAL);
            -1
        }
    }
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(ptrace(_, _, _, _))];
