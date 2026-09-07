/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `Accounts.framework` — Apple's iOS-5/6 unified credential store.
//!
//! Apps that link the framework reach the `ACAccountTypeIdentifier*` and
//! `ACFacebook*` (later: `ACTencentWeibo*`, `ACSinaWeibo*`,
//! `ACLinkedIn*`) string constants by Mach-O symbol lookup. Without
//! a [`crate::dyld::HostDylib`] entry the slots remain NULL and any
//! guest-side `[NSString isEqualToString:ACAccountTypeIdentifierFacebook]`
//! dereferences NULL.
//!
//! touchHLE has no real ACAccountStore implementation — the OAuth flow
//! it would drive doesn't exist on Android — but resolving the
//! constants is enough to keep apps that defensively link Accounts (via
//! Facebook SDK / Flurry / Talking Carl's sharing hooks) past the dyld
//! warning and into their normal "account isn't configured" code path.
//!
//! Reference: Apple `Accounts.framework` headers (`ACAccountType.h`,
//! `ACAccountStore.h`, `ACFacebookAccountType.h`).

use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{ConstantExports, HostConstant, HostDylib};
use crate::frameworks::foundation::ns_array;
use crate::objc::{id, nil, objc_classes, ClassExports};

pub const CONSTANTS: ConstantExports = &[
    // ACAccountType identifiers — apps pass these to
    // -[ACAccountStore accountTypeWithAccountTypeIdentifier:].
    // Literal values come from Apple's headers (com.apple.* reverse
    // DNS namespace).
    (
        "_ACAccountTypeIdentifierTwitter",
        HostConstant::NSString("com.apple.twitter"),
    ),
    (
        "_ACAccountTypeIdentifierFacebook",
        HostConstant::NSString("com.apple.facebook"),
    ),
    (
        "_ACAccountTypeIdentifierSinaWeibo",
        HostConstant::NSString("com.apple.sinaweibo"),
    ),
    (
        "_ACAccountTypeIdentifierTencentWeibo",
        HostConstant::NSString("com.apple.tencentweibo"),
    ),
    (
        "_ACAccountTypeIdentifierLinkedIn",
        HostConstant::NSString("com.apple.linkedin"),
    ),
    // Facebook-specific options dictionary keys used with
    // -[ACAccountStore requestAccessToAccountsWithType:options:completion:].
    (
        "_ACFacebookAppIdKey",
        HostConstant::NSString("ACFacebookAppIdKey"),
    ),
    (
        "_ACFacebookPermissionsKey",
        HostConstant::NSString("ACFacebookPermissionsKey"),
    ),
    (
        "_ACFacebookAudienceKey",
        HostConstant::NSString("ACFacebookAudienceKey"),
    ),
    // Audience-value singletons. Apple ships these as `NSString *
    // const`. Apps compare with `isEqualToString:`; literals match
    // Apple's documented values.
    (
        "_ACFacebookAudienceEveryone",
        HostConstant::NSString("ACFacebookAudienceEveryone"),
    ),
    (
        "_ACFacebookAudienceFriends",
        HostConstant::NSString("ACFacebookAudienceFriends"),
    ),
    (
        "_ACFacebookAudienceOnlyMe",
        HostConstant::NSString("ACFacebookAudienceOnlyMe"),
    ),
    // Tencent / Sina Weibo / LinkedIn options dictionary keys exported
    // alongside Facebook's.
    (
        "_ACTencentWeiboAppIdKey",
        HostConstant::NSString("ACTencentWeiboAppIdKey"),
    ),
    (
        "_ACLinkedInAppIdKey",
        HostConstant::NSString("ACLinkedInAppIdKey"),
    ),
    (
        "_ACLinkedInPermissionsKey",
        HostConstant::NSString("ACLinkedInPermissionsKey"),
    ),
    // Notification name posted when ACAccountStore detects an external
    // change to its database.
    (
        "_ACAccountStoreDidChangeNotification",
        HostConstant::NSString("ACAccountStoreDidChangeNotification"),
    ),
    // Error domain used by all -[ACAccountStore *] failures.
    (
        "_ACErrorDomain",
        HostConstant::NSString("com.apple.accounts"),
    ),
];

/// Host object backing `ACAccountType` instances handed out by
/// `-[ACAccountStore accountTypeWithAccountTypeIdentifier:]`.
#[derive(Default)]
struct ACAccountTypeHostObject {
    identifier: id,
}
impl crate::objc::HostObject for ACAccountTypeHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation ACAccountStore: NSObject

// `+alloc`/`-init` come from `NSObject`.

// `- (NSArray<ACAccount *> *)accounts;`
//
// touchHLE has no account database, so the store is always empty. This
// mirrors a device with no Twitter/Facebook/etc. accounts configured.
- (id)accounts {
    let empty = ns_array::from_vec(env, Vec::new());
    crate::objc::autorelease(env, empty)
}

// `- (ACAccountType *)accountTypeWithAccountTypeIdentifier:
//                                    (NSString *)typeIdentifier;`
- (id)accountTypeWithAccountTypeIdentifier:(id)type_identifier {
    let class = env.objc.get_known_class("ACAccountType", &mut env.mem);
    let host_object = Box::new(ACAccountTypeHostObject {
        identifier: type_identifier,
    });
    let new = env.objc.alloc_object(class, host_object, &mut env.mem);
    crate::objc::autorelease(env, new)
}

// `- (NSArray<ACAccount *> *)accountsWithAccountType:
//                                    (ACAccountType *)accountType;`
//
// Always empty, there is no account database.
- (id)accountsWithAccountType:(id)_account_type {
    let empty = ns_array::from_vec(env, Vec::new());
    crate::objc::autorelease(env, empty)
}

// `- (BOOL)saveAccount:(ACAccount *)account error:(NSError **)error;`
//
// Nothing can be saved: report failure, like the real store does when
// the request is missing required properties.
- (bool)saveAccount:(id)_account error:(id)_error {
    false
}

// `- (BOOL)removeAccount:(ACAccount *)account error:(NSError **)error;`
- (bool)removeAccount:(id)_account error:(id)_error {
    false
}

// `- (void)requestAccessToAccountsWithType:
//               (ACAccountType *)accountType
//               options:(NSDictionary *)options
//               completion:(void (^)(BOOL granted, NSError *error))
//               completion;`
//
// With no accounts there is nothing to authorize, so like on a real
// device with no matching account the completion handler is called
// with `granted = NO`. The error is left nil; apps key off the granted
// flag (ACErrorAccountNotFound is what the real framework reports,
// but a nil error keeps stub handling simpler on the guest side).
- (())requestAccessToAccountsWithType:(id)_account_type
                              options:(id)_options
                           completion:(id)completion
{
    if completion == nil {
        return;
    }
    // Block layout on 32-bit ARM: the invoke function pointer lives at
    // word 3 (same as in GameController.framework handling).
    let invoke_ptr: u32 = env.mem.read(completion.cast::<u32>() + 3u32);
    if invoke_ptr == 0 {
        return;
    }
    let invoke = GuestFunction::from_addr_with_thumb_bit(invoke_ptr);
    let _: () = invoke.call_from_host(env, (false, nil));
}

@end

@implementation ACAccountType: NSObject

// `- (NSString *)identifier;`
- (id)identifier {
    env.objc.borrow::<ACAccountTypeHostObject>(this).identifier
}

// `- (Class)accountClass;`
- (id)accountClass {
    env.objc.get_known_class("ACAccount", &mut env.mem)
}

@end

@implementation ACAccount: NSObject

// There are never any accounts, so instances of this class only exist
// if an app constructs one itself. All accessors return empty values.
- (id)username {
    nil
}

- (id)credential {
    nil
}

@end

};

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/Accounts.framework/Accounts",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};