/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIRefreshControl` (iOS 6+): the pull-to-refresh control used with
//! table/collection views.
//!
//! touchHLE does not simulate pull-to-refresh gestures, so the control
//! exists, round-trips its state (`refreshing`, `attributedTitle`) and
//! never fires its `valueChanged` handler. Apps that only check
//! `isRefreshing` after calling `beginRefreshing`/`endRefreshing` work
//! exactly as on a device.

use super::ui_control::UIControlHostObject;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg_super, nil, objc_classes, release, retain,
    ClassExports, NSZonePtr,
};

#[derive(Default)]
struct UIRefreshControlHostObject {
    superclass: UIControlHostObject,
    /// Whether `-beginRefreshing` was called and `-endRefreshing` has not
    /// been called since.
    refreshing: bool,
    /// NSAttributedString* shown next to the spinner, or nil.
    attributed_title: id,
}
impl_HostObject_with_superclass!(UIRefreshControlHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIRefreshControl: UIControl

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UIRefreshControlHostObject {
        superclass: UIControlHostObject::default(),
        refreshing: false,
        attributed_title: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())dealloc {
    let title = env.objc.borrow::<UIRefreshControlHostObject>(this).attributed_title;
    if title != nil {
        release(env, title);
    }
    msg_super![env; this dealloc]
}

// MARK: - Refreshing state

- (bool)isRefreshing {
    env.objc.borrow::<UIRefreshControlHostObject>(this).refreshing
}

- (())beginRefreshing {
    env.objc.borrow_mut::<UIRefreshControlHostObject>(this).refreshing = true;
}

- (())endRefreshing {
    env.objc.borrow_mut::<UIRefreshControlHostObject>(this).refreshing = false;
}

// MARK: - Title

- (id)attributedTitle {
    env.objc.borrow::<UIRefreshControlHostObject>(this).attributed_title
}

- (())setAttributedTitle:(id)title {
    let old = {
        let host = env.objc.borrow_mut::<UIRefreshControlHostObject>(this);
        if title == host.attributed_title {
            return;
        }
        std::mem::replace(&mut host.attributed_title, title)
    };
    if title != nil {
        retain(env, title);
    }
    if old != nil {
        release(env, old);
    }
}

@end

};
