/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPMediaPlaylist`.

use crate::dyld::{ConstantExports, HostConstant};
use crate::objc::{objc_classes, ClassExports};

// `MPMediaPlaylist` property keys. Per Apple's "Playlist property keys"
// documentation (`MPMediaPlaylist.h`), each of these is an `NSString *`
// constant whose value is the documented Cocoa property name:
//
// https://developer.apple.com/documentation/mediaplayer/playlist-property-keys?language=objc
pub const MPMediaPlaylistPropertyAuthorDisplayName: &str = "authorDisplayName";
pub const MPMediaPlaylistPropertyName: &str = "name";
pub const MPMediaPlaylistPropertyDescriptionText: &str = "descriptionText";
pub const MPMediaPlaylistPropertyPersistentID: &str = "persistentID";
pub const MPMediaPlaylistPropertyPlaylistAttributes: &str = "playlistAttributes";
pub const MPMediaPlaylistPropertyCloudGlobalID: &str = "cloudGlobalID";
pub const MPMediaPlaylistPropertySeedItems: &str = "seedItems";

pub const CONSTANTS: ConstantExports = &[
    (
        "_MPMediaPlaylistPropertyAuthorDisplayName",
        HostConstant::NSString(MPMediaPlaylistPropertyAuthorDisplayName),
    ),
    (
        "_MPMediaPlaylistPropertyName",
        HostConstant::NSString(MPMediaPlaylistPropertyName),
    ),
    (
        "_MPMediaPlaylistPropertyDescriptionText",
        HostConstant::NSString(MPMediaPlaylistPropertyDescriptionText),
    ),
    (
        "_MPMediaPlaylistPropertyPersistentID",
        HostConstant::NSString(MPMediaPlaylistPropertyPersistentID),
    ),
    (
        "_MPMediaPlaylistPropertyPlaylistAttributes",
        HostConstant::NSString(MPMediaPlaylistPropertyPlaylistAttributes),
    ),
    (
        "_MPMediaPlaylistPropertyCloudGlobalID",
        HostConstant::NSString(MPMediaPlaylistPropertyCloudGlobalID),
    ),
    (
        "_MPMediaPlaylistPropertySeedItems",
        HostConstant::NSString(MPMediaPlaylistPropertySeedItems),
    ),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPMediaPlaylist: MPMediaItemCollection
// TODO
@end

};
