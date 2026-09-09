/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Stub for the CaptiveNetwork bits of `SystemConfiguration.framework`.
//!
//! `CNCopySupportedInterfaces()` / `CNCopyCurrentNetworkInfo()` live inside
//! SystemConfiguration on iOS but are typically reached through their
//! dedicated `kCNNetworkInfo*` key constants. Analytics SDKs bundled with
//! games (e.g. LEGO Ninjago) call these at startup to record Wi-Fi state.
//!
//! touchHLE has no Wi-Fi telemetry, so we model a device whose only
//! interface is the (always present) Wi-Fi interface `en0`, exactly as a
//! real iPhone OS 3.x/4.x device reports, and report it with a placeholder
//! SSID. Apps that only probe for interface presence (the overwhelmingly
//! common case) see the same shape of data as on hardware.

use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::frameworks::core_foundation::cf_array::CFArrayRef;
use crate::frameworks::core_foundation::cf_dictionary::CFDictionaryRef;
use crate::frameworks::core_foundation::cf_string::CFStringRef;
use crate::frameworks::foundation::ns_array;
use crate::frameworks::foundation::ns_dictionary::dict_from_keys_and_objects;
use crate::frameworks::foundation::ns_string::{get_static_str, to_rust_string};
use crate::Environment;

pub const CONSTANTS: ConstantExports = &[
    ("_kCNNetworkInfoKeySSID", HostConstant::NSString("SSID")),
    (
        "_kCNNetworkInfoKeySSIDData",
        HostConstant::NSString("SSIDDATA"),
    ),
    ("_kCNNetworkInfoKeyBSSID", HostConstant::NSString("BSSID")),
];

/// `CFArrayRef CNCopySupportedInterfaces(void);`
///
/// Returns the names of supported network interfaces as CFStrings. On every
/// iPhone OS device of this era this is exactly the Wi-Fi interface `en0`
/// (cellular `pdp_ip0` and Bluetooth PAN only appear on later iOS versions
/// with special entitlements). An empty result would be an empty array, not
/// NULL, per Apple's documentation.
///
/// This is an ownership-transferring ("Copy") function: the caller is
/// responsible for releasing the returned array.
fn CNCopySupportedInterfaces(env: &mut Environment) -> CFArrayRef {
    let en0 = get_static_str(env, "en0");
    ns_array::from_vec(env, vec![en0])
}

/// `CFDictionaryRef CNCopyCurrentNetworkInfo(CFStringRef interfaceName);`
///
/// Returns the current network info dictionary for the given interface, or
/// NULL when the information is unavailable (which is also what a real
/// device returns when Wi-Fi is off or the caller lacks permission).
/// touchHLE has no Wi-Fi telemetry, so any query reports a placeholder
/// network rather than failing the call outright.
fn CNCopyCurrentNetworkInfo(env: &mut Environment, interface: CFStringRef) -> CFDictionaryRef {
    let interface_name = to_rust_string(env, interface.cast());
    log_dbg!("CNCopyCurrentNetworkInfo({interface_name:?}) — reporting placeholder Wi-Fi network",);

    let ssid = get_static_str(env, "touchHLE");
    let bssid = get_static_str(env, "02:00:00:00:00:00");
    let ssid_key = get_static_str(env, "SSID");
    let bssid_key = get_static_str(env, "BSSID");
    dict_from_keys_and_objects(env, &[(ssid_key, ssid), (bssid_key, bssid)])
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(CNCopySupportedInterfaces()),
    export_c_func!(CNCopyCurrentNetworkInfo(_)),
];
