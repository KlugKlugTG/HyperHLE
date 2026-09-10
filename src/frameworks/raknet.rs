/*
 * RakNet memory allocation support for touchHLE.
 */

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, MutVoidPtr};
use crate::Environment;

// Используем атрибут, чтобы Rust не ругался на
// манглированное C++ имя функции
fn __ZN6RakNet6OP_NEWINS_7RakPeerEEEPT_PKcj(
    env: &mut Environment,
    _file: ConstPtr<u8>,
    _line: u32,
) -> MutVoidPtr {
    // Класс RakPeer довольно большой. В старых
    // версиях RakNet (iOS ARMv6/v7)
    // Выделяем память с запасом, чтобы
    // конструктор не затер чужие данные.
    let ptr = env.mem.alloc(size);
    
    if !ptr.is_null() {
        // C++ объекты часто ожидают чистую
        // память перед вызовом конструктора
        log_dbg!("RakNet::OP_NEW<RakPeer> allocated {} bytes at {:?}", size, ptr);
    } else {
        log!("Error: RakNet::OP_NEW failed to allocate memory!");
    }
    
    ptr
}

// Экспортируем функцию в стиле touchHLE
pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(__ZN6RakNet6OP_NEWINS_7RakPeerEEEPT_PKcj(_, _, _)),
];
