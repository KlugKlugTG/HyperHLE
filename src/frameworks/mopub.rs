/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Host implementation for the MoPub ads SDK (`MoPubSDKFramework`).
//!
//! MoPub was a widely used mobile ad mediation SDK (acquired by AppLovin;
//! shut down in 2022). Games that shipped with it embed
//! `MoPubSDKFramework.framework` inside the app bundle and link against its
//! exported C symbols — chiefly the ad size constants and the string keys of
//! the ad data dictionary from `MPAdConstants.h`:
//!
//! ```c
//! extern CGSize const MOPUB_BANNER_SIZE;        // {320, 50}
//! extern CGSize const MOPUB_MEDIUM_RECT_SIZE;   // {300, 250}
//! extern CGSize const MOPUB_LEADERBOARD_SIZE;   // {728, 90}
//! extern CGSize const MOPUB_WIDE_SKYSCRAPER_SIZE; // {160, 600}
//! extern NSString * const kAdTitleKey;          // "adTitle"
//! extern NSString * const kAdTextKey;           // "adBodyText"
//! extern NSString * const kAdCTATextKey;        // "adCTAText"
//! extern NSString * const kAdIconImageKey;      // "adIconImage"
//! extern NSString * const kAdIconImageViewKey;  // "adIconImageView"
//! extern NSString * const kAdMainImageKey;      // "adMainImage"
//! extern NSString * const kAdMainMediaViewKey;  // "adMainMediaView"
//! extern NSString * const kAdStarRatingKey;     // "adStarRating"
//! extern const CGFloat kStarRatingMinValue;     // 1.0
//! extern const CGFloat kStarRatingMaxValue;     // 5.0
//! ```
//!
//! plus the consent-manager notification and the rewarded-video constants
//! from `MPConsentManager.h` / `MPRewardedVideoController.h`:
//!
//! ```c
//! extern NSString * const kMPConsentChangedNotification;
//! extern NSString * const kMPConsentChangedInfoCanCollectPersonalInfoKey;
//! extern NSString * const kMPConsentChangedInfoNewConsentStatusKey;
//! extern NSString * const kMPConsentChangedInfoPreviousConsentStatusKey;
//! extern NSString * const kMPRewardedVideoRewardCurrencyAmountUnspecified;
//! extern NSString * const kMPRewardedVideoRewardCurrencyTypeUnspecified;
//! extern NSString * const MoPubRewardedVideoAdsSDKDomain;
//! ```
//!
//! Without these exports the loader leaves the relocations pointing at
//! garbage and spams the log with `unhandled external relocation` /
//! `unhandled non-lazy symbol` warnings at startup.
//!
//! Note that we deliberately do NOT provide the Objective-C classes
//! (`MPAdView`, `MPInterstitialAdController`, …): ad SDKs are useless in an
//! offline emulator and the class lookup error would surface once, at the
//! point where the app actually tries to show an ad, instead of breaking
//! symbol binding at load time.

use crate::dyld::{ConstantExports, HostConstant, HostDylib};
use crate::frameworks::core_graphics::CGSize;
use crate::mem::ConstVoidPtr;
use crate::Environment;

fn write_cg_size(env: &mut Environment, width: f32, height: f32) -> ConstVoidPtr {
    let size = CGSize { width, height };
    env.mem.alloc_and_write(size).cast().cast_const()
}

fn write_cg_float(env: &mut Environment, value: f32) -> ConstVoidPtr {
    env.mem.alloc_and_write(value).cast().cast_const()
}

fn banner_size(env: &mut Environment) -> ConstVoidPtr {
    write_cg_size(env, 320.0, 50.0)
}

fn medium_rect_size(env: &mut Environment) -> ConstVoidPtr {
    write_cg_size(env, 300.0, 250.0)
}

fn leaderboard_size(env: &mut Environment) -> ConstVoidPtr {
    write_cg_size(env, 728.0, 90.0)
}

fn wide_skyscraper_size(env: &mut Environment) -> ConstVoidPtr {
    write_cg_size(env, 160.0, 600.0)
}

fn star_rating_min_value(env: &mut Environment) -> ConstVoidPtr {
    write_cg_float(env, 1.0)
}

fn star_rating_max_value(env: &mut Environment) -> ConstVoidPtr {
    write_cg_float(env, 5.0)
}

pub const CONSTANTS: ConstantExports = &[
    ("_MOPUB_BANNER_SIZE", HostConstant::Custom(banner_size)),
    (
        "_MOPUB_MEDIUM_RECT_SIZE",
        HostConstant::Custom(medium_rect_size),
    ),
    (
        "_MOPUB_LEADERBOARD_SIZE",
        HostConstant::Custom(leaderboard_size),
    ),
    (
        "_MOPUB_WIDE_SKYSCRAPER_SIZE",
        HostConstant::Custom(wide_skyscraper_size),
    ),
    ("_kAdTitleKey", HostConstant::NSString("adTitle")),
    ("_kAdTextKey", HostConstant::NSString("adBodyText")),
    ("_kAdCTATextKey", HostConstant::NSString("adCTAText")),
    ("_kAdIconImageKey", HostConstant::NSString("adIconImage")),
    (
        "_kAdIconImageViewKey",
        HostConstant::NSString("adIconImageView"),
    ),
    ("_kAdMainImageKey", HostConstant::NSString("adMainImage")),
    (
        "_kAdMainMediaViewKey",
        HostConstant::NSString("adMainMediaView"),
    ),
    ("_kAdStarRatingKey", HostConstant::NSString("adStarRating")),
    (
        "_kStarRatingMinValue",
        HostConstant::Custom(star_rating_min_value),
    ),
    (
        "_kStarRatingMaxValue",
        HostConstant::Custom(star_rating_max_value),
    ),
    (
        "_kMPConsentChangedNotification",
        HostConstant::NSString("MPConsentChangedNotification"),
    ),
    (
        "_kMPConsentChangedInfoCanCollectPersonalInfoKey",
        HostConstant::NSString("canCollectPersonalInfo"),
    ),
    (
        "_kMPConsentChangedInfoNewConsentStatusKey",
        HostConstant::NSString("newConsentStatus"),
    ),
    (
        "_kMPConsentChangedInfoPreviousConsentStatusKey",
        HostConstant::NSString("previousConsentStatus"),
    ),
    (
        "_kMPRewardedVideoRewardCurrencyAmountUnspecified",
        HostConstant::NSString("currency_amount_unspecified"),
    ),
    (
        "_kMPRewardedVideoRewardCurrencyTypeUnspecified",
        HostConstant::NSString("currency_type_unspecified"),
    ),
    (
        "_MoPubRewardedVideoAdsSDKDomain",
        HostConstant::NSString("ads.mopub.com"),
    ),
];

pub const DYLIB: HostDylib = HostDylib {
    path: "@rpath/MoPubSDKFramework.framework/MoPubSDKFramework",
    aliases: &[
        "MoPubSDKFramework.framework/MoPubSDKFramework",
        "@rpath/MoPubSDK.framework/MoPubSDK",
    ],
    class_exports: &[],
    constant_exports: &[CONSTANTS],
    function_exports: &[],
};
