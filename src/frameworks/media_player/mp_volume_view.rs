/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `MPVolumeView`.
//!
//! Apple documentation:
//! <https://developer.apple.com/documentation/mediaplayer/mpvolumeview>

use std::collections::HashMap;

use crate::frameworks::audio_toolbox::audio_session;
use crate::frameworks::core_graphics::{CGRect, CGSize};
use crate::frameworks::uikit::ui_view::ui_control::UIControlEventValueChanged;
use crate::objc::{
    autorelease, id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes,
    release, retain, ClassExports, NSZonePtr,
};
use crate::Environment;

#[derive(Default)]
struct MPVolumeViewHostObject {
    superclass: crate::frameworks::uikit::ui_view::UIViewHostObject,
    volume_slider: id,
    shows_volume_slider: bool,
    shows_route_button: bool,
    volume_thumb_images: HashMap<u32, id>,
    minimum_volume_slider_images: HashMap<u32, id>,
    maximum_volume_slider_images: HashMap<u32, id>,
    route_button_images: HashMap<u32, id>,
}
impl_HostObject_with_superclass!(MPVolumeViewHostObject);

fn volume(env: &mut Environment) -> f32 {
    audio_session::current_output_volume(env)
}

fn set_volume(env: &mut Environment, value: f32) {
    audio_session::set_current_output_volume(env, value);
}

fn update_slider(env: &mut Environment, this: id) {
    let slider = env
        .objc
        .borrow::<MPVolumeViewHostObject>(this)
        .volume_slider;
    if slider != nil {
        let _: () = msg![env; slider setValue:(volume(env))];
    }
}

fn replace_state_image(env: &mut Environment, this: id, state: u32, image: id, kind: u32) {
    retain(env, image);
    let old = match kind {
        0 => {
            let host = env.objc.borrow_mut::<MPVolumeViewHostObject>(this);
            host.volume_thumb_images.insert(state, image).unwrap_or(nil)
        }
        1 => {
            let host = env.objc.borrow_mut::<MPVolumeViewHostObject>(this);
            host.minimum_volume_slider_images
                .insert(state, image)
                .unwrap_or(nil)
        }
        2 => {
            let host = env.objc.borrow_mut::<MPVolumeViewHostObject>(this);
            host.maximum_volume_slider_images
                .insert(state, image)
                .unwrap_or(nil)
        }
        _ => {
            let host = env.objc.borrow_mut::<MPVolumeViewHostObject>(this);
            host.route_button_images.insert(state, image).unwrap_or(nil)
        }
    };
    release(env, old);
}

fn state_image(images: &HashMap<u32, id>, state: u32) -> id {
    images
        .get(&state)
        .copied()
        .or_else(|| images.get(&0).copied())
        .unwrap_or(nil)
}

fn release_images(env: &mut Environment, images: HashMap<u32, id>) {
    for image in images.into_values() {
        release(env, image);
    }
}

fn init_common(env: &mut Environment, this: id) -> id {
    let slider: id = msg_class![env; UISlider alloc];
    let slider: id = msg![env; slider initWithFrame:(CGRect::default())];
    let _: () = msg![env; slider setMinimumValue:0.0_f32];
    let _: () = msg![env; slider setMaximumValue:1.0_f32];
    let _: () = msg![env; slider setValue:(volume(env))];

    let action = env
        .objc
        .lookup_selector("volumeSliderValueChanged:")
        .expect("MPVolumeView action selector was not registered");
    let _: () =
        msg![env; slider addTarget:this action:action forControlEvents:UIControlEventValueChanged];
    let _: () = msg![env; this addSubview:slider];
    release(env, slider);
    env.objc
        .borrow_mut::<MPVolumeViewHostObject>(this)
        .volume_slider = slider;
    let _: () = msg![env; this setShowsVolumeSlider:true];
    let _: () = msg![env; this setShowsRouteButton:true];
    this
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation MPVolumeView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host = Box::<MPVolumeViewHostObject>::default();
    env.objc.alloc_object(this, host, &mut env.mem)
}

- (id)init {
    msg![env; this initWithFrame:(CGRect::default())]
}

- (id)initWithFrame:(CGRect)frame {
    let this: id = msg_super![env; this initWithFrame:frame];
    init_common(env, this)
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    init_common(env, this)
}

- (())dealloc {
    let host = std::mem::take(env.objc.borrow_mut::<MPVolumeViewHostObject>(this));
    release_images(env, host.volume_thumb_images);
    release_images(env, host.minimum_volume_slider_images);
    release_images(env, host.maximum_volume_slider_images);
    release_images(env, host.route_button_images);
    msg_super![env; this dealloc]
}

- (())volumeSliderValueChanged:(id)sender {
    let value: f32 = msg![env; sender value];
    set_volume(env, value);
    update_slider(env, this);
}

- (bool)showsVolumeSlider {
    env.objc.borrow::<MPVolumeViewHostObject>(this).shows_volume_slider
}

- (())setShowsVolumeSlider:(bool)shows {
    let slider = {
        let host = env.objc.borrow_mut::<MPVolumeViewHostObject>(this);
        host.shows_volume_slider = shows;
        host.volume_slider
    };
    if slider != nil {
        let _: () = msg![env; slider setHidden:(!shows)];
    }
}

- (bool)showsRouteButton {
    env.objc.borrow::<MPVolumeViewHostObject>(this).shows_route_button
}

- (())setShowsRouteButton:(bool)shows {
    env.objc.borrow_mut::<MPVolumeViewHostObject>(this).shows_route_button = shows;
}

- (bool)areWirelessRoutesAvailable {
    false
}

- (id)volumeSlider {
    let slider = env.objc.borrow::<MPVolumeViewHostObject>(this).volume_slider;
    retain(env, slider);
    autorelease(env, slider)
}

- (())setVolumeSliderValue:(f32)value {
    set_volume(env, value);
    update_slider(env, this);
}

- (f32)volumeSliderValue {
    volume(env)
}

- (CGSize)sizeThatFits:(CGSize)_size {
    CGSize { width: 200.0, height: 23.0 }
}

- (CGRect)minimumVolumeSliderImageRectForBounds:(CGRect)bounds {
    bounds
}

- (CGRect)maximumVolumeSliderImageRectForBounds:(CGRect)bounds {
    bounds
}

- (CGRect)volumeThumbRectForBounds:(CGRect)bounds
                 volumeSliderRect:(CGRect)slider_rect
                             value:(f32)value {
    let slider = env.objc.borrow::<MPVolumeViewHostObject>(this).volume_slider;
    if slider == nil {
        return slider_rect;
    }
    let _: () = msg![env; slider setFrame:slider_rect];
    let _: () = msg![env; slider setValue:value];
    slider_rect
}

- (())setVolumeThumbImage:(id)image forState:(u32)state {
    replace_state_image(env, this, state, image, 0);
}

- (id)volumeThumbImageForState:(u32)state {
    let images = &env.objc.borrow::<MPVolumeViewHostObject>(this).volume_thumb_images;
    state_image(images, state)
}

- (())setMinimumVolumeSliderImage:(id)image forState:(u32)state {
    replace_state_image(env, this, state, image, 1);
}

- (id)minimumVolumeSliderImageForState:(u32)state {
    let images = &env.objc.borrow::<MPVolumeViewHostObject>(this).minimum_volume_slider_images;
    state_image(images, state)
}

- (())setMaximumVolumeSliderImage:(id)image forState:(u32)state {
    replace_state_image(env, this, state, image, 2);
}

- (id)maximumVolumeSliderImageForState:(u32)state {
    let images = &env.objc.borrow::<MPVolumeViewHostObject>(this).maximum_volume_slider_images;
    state_image(images, state)
}

- (())setRouteButtonImage:(id)image forState:(u32)state {
    replace_state_image(env, this, state, image, 3);
}

- (id)routeButtonImageForState:(u32)state {
    let images = &env.objc.borrow::<MPVolumeViewHostObject>(this).route_button_images;
    state_image(images, state)
}

- (())layoutSubviews {
    let bounds: CGRect = msg![env; this bounds];
    let slider = env.objc.borrow::<MPVolumeViewHostObject>(this).volume_slider;
    if slider != nil {
        let _: () = msg![env; slider setFrame:bounds];
    }
    update_slider(env, this);
}

@end

};
