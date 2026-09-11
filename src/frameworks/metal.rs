/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Minimal Metal.framework compatibility layer.
//!
//! Metal is a native Apple GPU API, while HyperHLE renders through its
//! portable GLES presentation path. This layer intentionally provides the
//! object and descriptor contract used during app startup and resource setup;
//! it does not pretend to execute Metal command streams on non-Apple hosts.

use crate::dyld::{ConstantExports, HostDylib};
use crate::frameworks::foundation::{ns_string, NSUInteger};
use crate::frameworks::core_animation::ca_layer::CALayerHostObject;
use crate::frameworks::core_graphics::cg_geometry::{CGRect, CGSize};
use crate::mem::{ConstPtr, ConstVoidPtr, GuestUSize, MutPtr, MutVoidPtr};
use crate::frameworks::core_graphics::CGFloat;
use crate::objc::{id, msg, msg_class, nil, objc_classes, ClassExports, HostObject, NSZonePtr};
use crate::Environment;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/Metal.framework/Metal",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[CONSTANTS],
    function_exports: &[FUNCTIONS],
};

const MTL_RESOURCE_STORAGE_MODE_SHARED: NSUInteger = 0;
const MTL_RESOURCE_STORAGE_MODE_MANAGED: NSUInteger = 1;
const MTL_RESOURCE_STORAGE_MODE_PRIVATE: NSUInteger = 2;
const MTL_RESOURCE_STORAGE_MODE_MEMORYLESS: NSUInteger = 3;
const MTL_LOAD_ACTION_DONT_CARE: NSUInteger = 0;
const MTL_LOAD_ACTION_LOAD: NSUInteger = 1;
const MTL_LOAD_ACTION_CLEAR: NSUInteger = 2;
const MTL_STORE_ACTION_DONT_CARE: NSUInteger = 0;
const MTL_STORE_ACTION_STORE: NSUInteger = 1;
const MAX_COLOR_ATTACHMENTS: usize = 8;

#[derive(Default)]
struct MetalObjectHostObject {
    label: id,
    device: id,
    length: NSUInteger,
    contents: MutVoidPtr,
    usage: NSUInteger,
    storage_mode: NSUInteger,
    pixel_format: NSUInteger,
    width: NSUInteger,
    height: NSUInteger,
    depth: NSUInteger,
    mipmap_level_count: NSUInteger,
    sample_count: NSUInteger,
    load_action: NSUInteger,
    store_action: NSUInteger,
    clear_color: [f64; 4],
    command_buffer: id,
    layouts: id,
    attributes: id,
    stride: NSUInteger,
    s_address_mode: NSUInteger,
    t_address_mode: NSUInteger,
    r_address_mode: NSUInteger,
    lod_min_clamp: f32,
    lod_max_clamp: f32,
    max_anisotropy: NSUInteger,
    normalized_coordinates: bool,
    front_stencil: id,
    back_stencil: id,
    step_function: NSUInteger,
    step_rate: NSUInteger,
    stencil_compare_function: NSUInteger,
    stencil_failure_operation: NSUInteger,
    depth_failure_operation: NSUInteger,
    depth_stencil_pass_operation: NSUInteger,
    write_mask: NSUInteger,
    read_mask: NSUInteger,
    frame: CGRect,
    bounds: CGRect,
    vertex_function: id,
    fragment_function: id,
    depth_pixel_format: NSUInteger,
    stencil_pixel_format: NSUInteger,
    blending_enabled: bool,
    source_rgb_blend_factor: NSUInteger,
    destination_rgb_blend_factor: NSUInteger,
    source_alpha_blend_factor: NSUInteger,
    destination_alpha_blend_factor: NSUInteger,
    rgb_blend_operation: NSUInteger,
    alpha_blend_operation: NSUInteger,
    color_attachments: [id; MAX_COLOR_ATTACHMENTS],
}
impl HostObject for MetalObjectHostObject {}

fn metal_string(env: &mut Environment, value: &'static str) -> id {
    ns_string::get_static_str(env, value)
}

fn allocation_size(length: NSUInteger) -> GuestUSize {
    length.max(1)
}

fn constant_uinteger(env: &mut Environment, value: NSUInteger) -> ConstVoidPtr {
    ConstVoidPtr::from_bits(env.mem.alloc_and_write(value).to_bits())
}

const CLASSES: ClassExports = objc_classes! {
(env, this, _cmd);

@implementation MTLDevice: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}

- (id)init { this }
- (id)name { metal_string(env, "HyperHLE Metal compatibility device") }
- (bool)hasUnifiedMemory { true }
- (NSUInteger)recommendedMaxWorkingSetSize { 0 }
- (bool)supportsFamily:(NSUInteger)_family { false }
- (bool)supportsFeatureSet:(NSUInteger)_feature_set {
    // Feature sets (iOS 5–11 era GPU capability tiers) — apps like Asphalt 8
    // probe MTLDevice.supportsFeatureSet: to pick a rendering path. All real
    // devices that ran these games support the iOS 8 feature sets, and the
    // app degrades gracefully when told a newer set is available, so
    // reporting true is the compatibility-maximising answer.
    true
}
- (bool)supportsTextureSampleCount:(NSUInteger)count { count == 1 }
- (id)newCommandQueue { msg_class![env; MTLCommandQueue new] }
- (id)newCommandQueueWithMaxCommandBufferCount:(NSUInteger)_count { msg_class![env; MTLCommandQueue new] }
- (id)newBufferWithLength:(NSUInteger)length options:(NSUInteger)options {
    let object = msg_class![env; MTLBuffer alloc];
    let contents = env.mem.alloc(allocation_size(length));
    let host = env.objc.borrow_mut::<MetalObjectHostObject>(object);
    host.length = length;
    host.storage_mode = options & 0xf;
    host.contents = contents;
    object
}
- (id)newBufferWithBytes:(ConstVoidPtr)bytes length:(NSUInteger)length options:(NSUInteger)options {
    let object = msg![env; this newBufferWithLength:length options:options];
    if !bytes.is_null() && length != 0 {
        let source = env.mem.bytes_at(bytes.cast(), length).to_vec();
        let destination_ptr = env.objc.borrow::<MetalObjectHostObject>(object).contents;
        env.mem.bytes_at_mut(destination_ptr.cast(), length).copy_from_slice(&source);
    }
    object
}
- (id)newTextureWithDescriptor:(id)descriptor {
    let object = msg_class![env; MTLTexture alloc];
    let source = env.objc.borrow::<MetalObjectHostObject>(descriptor);
    let device = source.device;
    let pixel_format = source.pixel_format;
    let width = source.width;
    let height = source.height;
    let depth = source.depth;
    let mipmap_level_count = source.mipmap_level_count;
    let sample_count = source.sample_count;
    let host = env.objc.borrow_mut::<MetalObjectHostObject>(object);
    host.device = if device == nil { this } else { device };
    host.pixel_format = pixel_format;
    host.width = width;
    host.height = height;
    host.depth = depth;
    host.mipmap_level_count = mipmap_level_count;
    host.sample_count = sample_count;
    object
}
- (id)newLibraryWithSource:(id)_source options:(id)_options error:(MutPtr<id>)_error {
    // Runtime shader compilation: the game compiles MSL at startup. Without a
    // host GPU to translate it to, we hand back an object that behaves like an
    // empty library — function lookups return real MTLFunction objects whose
    // handles the app can attach to pipeline descriptors.
    msg_class![env; MTLLibrary new]
}
- (id)newLibraryWithData:(ConstVoidPtr)_data error:(MutPtr<id>)_error { msg_class![env; MTLLibrary new] }
- (id)newLibraryWithFile:(ConstPtr<u8>)_path error:(MutPtr<id>)_error { msg_class![env; MTLLibrary new] }
- (id)newSamplerStateWithDescriptor:(id)_descriptor { msg_class![env; MTLSamplerState new] }
- (id)newRenderPipelineStateWithDescriptor:(id)_descriptor error:(MutPtr<id>)_error { msg_class![env; MTLRenderPipelineState new] }
- (id)newDepthStencilStateWithDescriptor:(id)_descriptor { msg_class![env; MTLDepthStencilState new] }
- (bool)supportsFeatureSet:(NSUInteger)_feature_set {
    // Asphalt 9 probes feature sets before creating its Metal device.
    // The iOS 7-era feature sets (1-5) are universally supported by the
    // GLES presentation path; later A9+ feature sets are reported as
    // unsupported so apps pick their legacy pipeline.
    _feature_set <= 5
}

@end

@implementation MTLCommandQueue: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (id)commandBuffer { let buffer = msg_class![env; MTLCommandBuffer new]; env.objc.borrow_mut::<MetalObjectHostObject>(buffer).command_buffer = this; buffer }
- (id)commandBufferWithUnretainedReferences { msg![env; this commandBuffer] }
- (id)newBufferWithLength:(NSUInteger)length options:(NSUInteger)options {
    let device = env.objc.borrow::<MetalObjectHostObject>(this).device;
    if device != nil {
        msg![env; device newBufferWithLength:length options:options]
    } else {
        msg_class![env; MTLDevice newBufferWithLength:length options:options]
    }
}
@end

@implementation MTLCommandBuffer: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (id)renderCommandEncoderWithDescriptor:(id)_descriptor { msg_class![env; RMTLRenderCommandEncoder new] }
- (())enqueue {}
- (())commit {}
- (())waitUntilCompleted {}
- (())presentDrawable:(id)_drawable {}
- (id)device { env.objc.borrow::<MetalObjectHostObject>(this).command_buffer }
- (NSUInteger)status { 0 }
@end

@implementation MTLRenderCommandEncoder: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (())endEncoding {}
- (())setRenderPipelineState:(id)_state {}
- (())setVertexBuffer:(id)_buffer offset:(NSUInteger)_offset atIndex:(NSUInteger)_index {}
- (())setFragmentBuffer:(id)_buffer offset:(NSUInteger)_offset atIndex:(NSUInteger)_index {}
- (())setVertexTexture:(id)_texture atIndex:(NSUInteger)_index {}
- (())setFragmentTexture:(id)_texture atIndex:(NSUInteger)_index {}
- (())drawPrimitives:(NSUInteger)_type vertexStart:(NSUInteger)_start vertexCount:(NSUInteger)_count {}
- (())drawIndexedPrimitives:(NSUInteger)_type indexCount:(NSUInteger)_count indexType:(NSUInteger)_index_type indexBuffer:(id)_buffer indexBufferOffset:(NSUInteger)_offset {}
@end

@implementation MTLRenderPassDescriptor: NSObject
+ (id)renderPassDescriptor { msg_class![env; MTLRenderPassDescriptor new] }
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (id)colorAttachments { msg_class![env; MTLRenderPassColorAttachmentDescriptorArray new] }
- (id)depthAttachment { msg_class![env; MTLRenderPassDepthAttachmentDescriptor new] }
- (id)stencilAttachment { msg_class![env; MTLRenderPassStencilAttachmentDescriptor new] }
@end

@implementation MTLRenderPassColorAttachmentDescriptorArray: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (id)objectAtIndexedSubscript:(NSUInteger)index {
    if (index as usize) >= MAX_COLOR_ATTACHMENTS {
        return msg_class![env; MTLRenderPassColorAttachmentDescriptor new];
    }
    let stored = env.objc.borrow::<MetalObjectHostObject>(this).color_attachments[index as usize];
    if stored != nil {
        return stored;
    }
    let attachment = msg_class![env; MTLRenderPassColorAttachmentDescriptor new];
    env.objc.borrow_mut::<MetalObjectHostObject>(this).color_attachments[index as usize] = attachment;
    attachment
}
- (id)objectAtIndex:(NSUInteger)index { msg![env; this objectAtIndexedSubscript:index] }
- (())setObject:(id)obj atIndexedSubscript:(NSUInteger)index {
    if (index as usize) < MAX_COLOR_ATTACHMENTS {
        env.objc.borrow_mut::<MetalObjectHostObject>(this).color_attachments[index as usize] = obj;
    }
}
@end

@implementation MTLRenderPassColorAttachmentDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (())setTexture:(id)texture { env.objc.borrow_mut::<MetalObjectHostObject>(this).device = texture }
- (id)texture { env.objc.borrow::<MetalObjectHostObject>(this).device }
- (())setLoadAction:(NSUInteger)action { env.objc.borrow_mut::<MetalObjectHostObject>(this).load_action = action }
- (NSUInteger)loadAction { env.objc.borrow::<MetalObjectHostObject>(this).load_action }
- (())setStoreAction:(NSUInteger)action { env.objc.borrow_mut::<MetalObjectHostObject>(this).store_action = action }
- (NSUInteger)storeAction { env.objc.borrow::<MetalObjectHostObject>(this).store_action }
- (())setClearColor:(id)color {
    let host = env.objc.borrow_mut::<MetalObjectHostObject>(this);
    for component in 0usize..4 {
        let value: f64 = if color == nil {
            0.0
        } else {
            env.mem.read(color.cast::<f64>() + component as u32)
        };
        host.clear_color[component] = value;
    }
}
- (id)clearColor { nil }
- (())setResolveTexture:(id)texture { env.objc.borrow_mut::<MetalObjectHostObject>(this).device = texture }
- (id)resolveTexture { env.objc.borrow::<MetalObjectHostObject>(this).device }
@end

@implementation MTLTexture: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (id)device { env.objc.borrow::<MetalObjectHostObject>(this).device }
- (NSUInteger)width { env.objc.borrow::<MetalObjectHostObject>(this).width }
- (NSUInteger)height { env.objc.borrow::<MetalObjectHostObject>(this).height }
- (NSUInteger)depth { env.objc.borrow::<MetalObjectHostObject>(this).depth }
- (NSUInteger)mipmapLevelCount { env.objc.borrow::<MetalObjectHostObject>(this).mipmap_level_count }
- (NSUInteger)sampleCount { env.objc.borrow::<MetalObjectHostObject>(this).sample_count }
- (NSUInteger)pixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).pixel_format }
- (())replaceRegion:(id)_region mipmapLevel:(NSUInteger)_level withBytes:(ConstVoidPtr)_bytes bytesPerRow:(NSUInteger)_row {}
@end

@implementation MTLBuffer: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone {
    env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem)
}
- (id)device { env.objc.borrow::<MetalObjectHostObject>(this).device }
- (NSUInteger)length { env.objc.borrow::<MetalObjectHostObject>(this).length }
- (MutVoidPtr)contents { env.objc.borrow::<MetalObjectHostObject>(this).contents }
- (NSUInteger)storageMode { env.objc.borrow::<MetalObjectHostObject>(this).storage_mode }
@end

@implementation MTLRenderPipelineState: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)device { env.objc.borrow::<MetalObjectHostObject>(this).device }
@end

@implementation MTLDepthStencilState: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
@end

@implementation MTLLibrary: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (id)label { env.objc.borrow::<MetalObjectHostObject>(this).label }
- (())setLabel:(id)label { env.objc.borrow_mut::<MetalObjectHostObject>(this).label = label }
- (id)newFunctionWithName:(id)name {
    let object = msg_class![env; MTLFunction new];
    env.objc.borrow_mut::<MetalObjectHostObject>(object).label = name;
    object
}
@end

@implementation MTLFunction: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (id)name { env.objc.borrow::<MetalObjectHostObject>(this).label }
- (id)label { env.objc.borrow::<MetalObjectHostObject>(this).label }
- (())setLabel:(id)label { env.objc.borrow_mut::<MetalObjectHostObject>(this).label = label }
- (id)vertexFunction { env.objc.borrow::<MetalObjectHostObject>(this).vertex_function }
- (())setVertexFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).vertex_function = function }
- (id)fragmentFunction { env.objc.borrow::<MetalObjectHostObject>(this).fragment_function }
- (())setFragmentFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).fragment_function = function }
- (NSUInteger)depthAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).depth_pixel_format }
- (())setDepthAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).depth_pixel_format = format }
- (NSUInteger)stencilAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).stencil_pixel_format }
- (())setStencilAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).stencil_pixel_format = format }
@end

@implementation MTLRenderPipelineColorAttachmentDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (NSUInteger)pixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).pixel_format }
- (())setPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).pixel_format = format }
- (bool)blendingEnabled { env.objc.borrow::<MetalObjectHostObject>(this).blending_enabled }
- (())setBlendingEnabled:(bool)enabled { env.objc.borrow_mut::<MetalObjectHostObject>(this).blending_enabled = enabled }
- (NSUInteger)sourceRGBBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).source_rgb_blend_factor }
- (())setSourceRGBBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).source_rgb_blend_factor = factor }
- (NSUInteger)destinationRGBBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).destination_rgb_blend_factor }
- (())setDestinationRGBBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).destination_rgb_blend_factor = factor }
- (NSUInteger)sourceAlphaBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).source_alpha_blend_factor }
- (())setSourceAlphaBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).source_alpha_blend_factor = factor }
- (NSUInteger)destinationAlphaBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).destination_alpha_blend_factor }
- (())setDestinationAlphaBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).destination_alpha_blend_factor = factor }
- (NSUInteger)rgbBlendOperation { env.objc.borrow::<MetalObjectHostObject>(this).rgb_blend_operation }
- (())setRgbBlendOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).rgb_blend_operation = operation }
- (NSUInteger)alphaBlendOperation { env.objc.borrow::<MetalObjectHostObject>(this).alpha_blend_operation }
- (())setAlphaBlendOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).alpha_blend_operation = operation }
- (NSUInteger)writeMask { env.objc.borrow::<MetalObjectHostObject>(this).write_mask }
- (())setWriteMask:(NSUInteger)mask { env.objc.borrow_mut::<MetalObjectHostObject>(this).write_mask = mask }
@end

@implementation CAMetalLayer: NSObject
+ (id)layer { msg_class![env; CAMetalLayer new] }
+ (id)allocWithZone:(NSZonePtr)_zone {
    // CAMetalLayer subclasses CALayer, so it must carry a CALayerHostObject.
    // Using the generic Metal host object here breaks every CALayer borrow
    // made by the CoreAnimation compositor (the "SUPER HACK! Faking borrow"
    // warnings in the logs).
    env.objc.alloc_object(this, Box::new(crate::frameworks::core_animation::ca_layer::CALayerHostObject::default()), &mut env.mem)
}
- (id)init { this }
- (id)device { env.objc.borrow::<CALayerHostObject>(this).metal_device }
- (())setDevice:(id)device { env.objc.borrow_mut::<CALayerHostObject>(this).metal_device = device }
- (NSUInteger)pixelFormat { env.objc.borrow::<CALayerHostObject>(this).metal_pixel_format }
- (())setPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<CALayerHostObject>(this).metal_pixel_format = format }
- (id)nextDrawable { nil }
- (CGSize)drawableSize { env.objc.borrow::<CALayerHostObject>(this).metal_drawable_size }
- (())setDrawableSize:(CGSize)size { env.objc.borrow_mut::<CALayerHostObject>(this).metal_drawable_size = size }
- (bool)isHidden { false }
- (())setHidden:(bool)_hidden {}
- (bool)framebufferOnly { env.objc.borrow::<CALayerHostObject>(this).metal_framebuffer_only }
- (())setFramebufferOnly:(bool)only { env.objc.borrow_mut::<CALayerHostObject>(this).metal_framebuffer_only = only }
- (CGRect)frame { msg![env; this bounds] }
- (())setFrame:(CGRect)frame { env.objc.borrow_mut::<CALayerHostObject>(this).set_frame_metal(frame) }
- (CGRect)bounds { env.objc.borrow::<CALayerHostObject>(this).get_bounds_frame() }
- (())setBounds:(CGRect)bounds { env.objc.borrow_mut::<CALayerHostObject>(this).set_bounds_metal(bounds) }
- (CGSize)boundsSize { env.objc.borrow::<CALayerHostObject>(this).get_bounds_frame().size }
- (CGFloat)contentsScale { 1.0 }
- (())setContentsScale:(CGFloat)_scale {}
@end

@implementation MTLSamplerState: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
@end

@implementation MTLCompileOptions: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
// Properties on MTLCompileOptions are only consulted host-side; accept
// the common ones so shader-compilation paths proceed.
- (())setPreprocessorMacros:(id)_macros {}
- (id)preprocessorMacros { nil }
- (bool)fastMathEnabled { true }
- (())setFastMathEnabled:(bool)_enabled {}
- (())setLanguageVersion:(f32)_version {}
- (f32)languageVersion { 1.0 }
@end

@implementation MTLVertexDescriptor: NSObject
+ (id)vertexDescriptor { msg_class![env; MTLVertexDescriptor new] }
+ (id)allocWithZone:(NSZonePtr)_zone {
    let object = env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem);
    let host = env.objc.borrow_mut::<MetalObjectHostObject>(object);
    host.layouts = nil;
    host.attributes = nil;
    object
}
- (id)init { this }
- (id)layouts {
    let existing = env.objc.borrow::<MetalObjectHostObject>(this).layouts;
    if existing != nil { return existing; }
    let layouts = msg_class![env; MTLVertexBufferLayoutDescriptorArray new];
    env.objc.borrow_mut::<MetalObjectHostObject>(this).layouts = layouts;
    layouts
}
- (id)attributes {
    let existing = env.objc.borrow::<MetalObjectHostObject>(this).attributes;
    if existing != nil { return existing; }
    let attributes = msg_class![env; MTLVertexAttributeDescriptorArray new];
    env.objc.borrow_mut::<MetalObjectHostObject>(this).attributes = attributes;
    attributes
}
- (NSUInteger)stride { env.objc.borrow::<MetalObjectHostObject>(this).stride }
- (())setStride:(NSUInteger)stride { env.objc.borrow_mut::<MetalObjectHostObject>(this).stride = stride }
@end

@implementation MTLVertexBufferLayoutDescriptorArray: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (id)objectAtIndexedSubscript:(NSUInteger)_index { msg_class![env; MTLVertexBufferLayoutDescriptor new] }
- (id)objectAtIndex:(NSUInteger)_index { msg![env; this objectAtIndexedSubscript:_index] }
- (())setObject:(id)_object atIndexedSubscript:(NSUInteger)_index {}
@end

@implementation MTLVertexBufferLayoutDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (NSUInteger)stride { env.objc.borrow::<MetalObjectHostObject>(this).stride }
- (())setStride:(NSUInteger)stride { env.objc.borrow_mut::<MetalObjectHostObject>(this).stride = stride }
- (NSUInteger)stepFunction { env.objc.borrow::<MetalObjectHostObject>(this).step_function }
- (())setStepFunction:(NSUInteger)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).step_function = function }
- (NSUInteger)stepRate { env.objc.borrow::<MetalObjectHostObject>(this).step_rate }
- (())setStepRate:(NSUInteger)rate { env.objc.borrow_mut::<MetalObjectHostObject>(this).step_rate = rate }
@end

@implementation MTLVertexAttributeDescriptorArray: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (id)objectAtIndexedSubscript:(NSUInteger)_index { msg_class![env; MTLVertexAttributeDescriptor new] }
- (id)objectAtIndex:(NSUInteger)_index { msg![env; this objectAtIndexedSubscript:_index] }
- (())setObject:(id)_object atIndexedSubscript:(NSUInteger)_index {}
@end

@implementation MTLVertexAttributeDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (NSUInteger)format { 0 }
- (())setFormat:(NSUInteger)_format {}
- (NSUInteger)offset { 0 }
- (())setOffset:(NSUInteger)_offset {}
- (NSUInteger)bufferIndex { 0 }
- (())setBufferIndex:(NSUInteger)_index {}
@end

@implementation MTLSamplerDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (NSUInteger)minFilter { env.objc.borrow::<MetalObjectHostObject>(this).usage }
- (())setMinFilter:(NSUInteger)filter { env.objc.borrow_mut::<MetalObjectHostObject>(this).usage = filter }
- (NSUInteger)magFilter { env.objc.borrow::<MetalObjectHostObject>(this).storage_mode }
- (())setMagFilter:(NSUInteger)filter { env.objc.borrow_mut::<MetalObjectHostObject>(this).storage_mode = filter }
- (NSUInteger)mipFilter { env.objc.borrow::<MetalObjectHostObject>(this).load_action }
- (())setMipFilter:(NSUInteger)filter { env.objc.borrow_mut::<MetalObjectHostObject>(this).load_action = filter }
- (NSUInteger)addressModeS { env.objc.borrow::<MetalObjectHostObject>(this).s_address_mode }
- (())setAddressModeS:(NSUInteger)mode { env.objc.borrow_mut::<MetalObjectHostObject>(this).s_address_mode = mode }
// The SDK also spells the address-mode properties with S/T/R prefixes.
- (NSUInteger)sAddressMode { env.objc.borrow::<MetalObjectHostObject>(this).s_address_mode }
- (())setSAddressMode:(NSUInteger)mode { env.objc.borrow_mut::<MetalObjectHostObject>(this).s_address_mode = mode }
- (NSUInteger)addressModeT { env.objc.borrow::<MetalObjectHostObject>(this).t_address_mode }
- (())setAddressModeT:(NSUInteger)mode { env.objc.borrow_mut::<MetalObjectHostObject>(this).t_address_mode = mode }
- (NSUInteger)tAddressMode { env.objc.borrow::<MetalObjectHostObject>(this).t_address_mode }
- (())setTAddressMode:(NSUInteger)mode { env.objc.borrow_mut::<MetalObjectHostObject>(this).t_address_mode = mode }
- (NSUInteger)addressModeR { env.objc.borrow::<MetalObjectHostObject>(this).r_address_mode }
- (())setAddressModeR:(NSUInteger)mode { env.objc.borrow_mut::<MetalObjectHostObject>(this).r_address_mode = mode }
- (NSUInteger)rAddressMode { env.objc.borrow::<MetalObjectHostObject>(this).r_address_mode }
- (())setRAddressMode:(NSUInteger)mode { env.objc.borrow_mut::<MetalObjectHostObject>(this).r_address_mode = mode }
- (NSUInteger)addressModeW { 0 }
- (())setAddressModeW:(NSUInteger)_mode {}
- (NSUInteger)compareFunction { 0 }
- (())setCompareFunction:(NSUInteger)_function {}
- (f32)lodMinClamp { env.objc.borrow::<MetalObjectHostObject>(this).lod_min_clamp }
- (())setLodMinClamp:(f32)clamp { env.objc.borrow_mut::<MetalObjectHostObject>(this).lod_min_clamp = clamp }
- (f32)lodMaxClamp { env.objc.borrow::<MetalObjectHostObject>(this).lod_max_clamp }
- (())setLodMaxClamp:(f32)clamp { env.objc.borrow_mut::<MetalObjectHostObject>(this).lod_max_clamp = clamp }
- (NSUInteger)maxAnisotropy { env.objc.borrow::<MetalObjectHostObject>(this).max_anisotropy }
- (())setMaxAnisotropy:(NSUInteger)anisotropy { env.objc.borrow_mut::<MetalObjectHostObject>(this).max_anisotropy = anisotropy }
- (bool)normalizedCoordinates { env.objc.borrow::<MetalObjectHostObject>(this).normalized_coordinates }
- (())setNormalizedCoordinates:(bool)normalized { env.objc.borrow_mut::<MetalObjectHostObject>(this).normalized_coordinates = normalized }
- (id)label { env.objc.borrow::<MetalObjectHostObject>(this).label }
- (())setLabel:(id)label { env.objc.borrow_mut::<MetalObjectHostObject>(this).label = label }
- (id)vertexFunction { env.objc.borrow::<MetalObjectHostObject>(this).vertex_function }
- (())setVertexFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).vertex_function = function }
- (id)fragmentFunction { env.objc.borrow::<MetalObjectHostObject>(this).fragment_function }
- (())setFragmentFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).fragment_function = function }
- (NSUInteger)depthAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).depth_pixel_format }
- (())setDepthAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).depth_pixel_format = format }
- (NSUInteger)stencilAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).stencil_pixel_format }
- (())setStencilAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).stencil_pixel_format = format }
@end


@implementation MTLRenderPipelineColorAttachmentDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (NSUInteger)pixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).pixel_format }
- (())setPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).pixel_format = format }
- (bool)blendingEnabled { env.objc.borrow::<MetalObjectHostObject>(this).blending_enabled }
- (())setBlendingEnabled:(bool)enabled { env.objc.borrow_mut::<MetalObjectHostObject>(this).blending_enabled = enabled }
- (NSUInteger)sourceRGBBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).source_rgb_blend_factor }
- (())setSourceRGBBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).source_rgb_blend_factor = factor }
- (NSUInteger)destinationRGBBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).destination_rgb_blend_factor }
- (())setDestinationRGBBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).destination_rgb_blend_factor = factor }
- (NSUInteger)sourceAlphaBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).source_alpha_blend_factor }
- (())setSourceAlphaBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).source_alpha_blend_factor = factor }
- (NSUInteger)destinationAlphaBlendFactor { env.objc.borrow::<MetalObjectHostObject>(this).destination_alpha_blend_factor }
- (())setDestinationAlphaBlendFactor:(NSUInteger)factor { env.objc.borrow_mut::<MetalObjectHostObject>(this).destination_alpha_blend_factor = factor }
- (NSUInteger)rgbBlendOperation { env.objc.borrow::<MetalObjectHostObject>(this).rgb_blend_operation }
- (())setRgbBlendOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).rgb_blend_operation = operation }
- (NSUInteger)alphaBlendOperation { env.objc.borrow::<MetalObjectHostObject>(this).alpha_blend_operation }
- (())setAlphaBlendOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).alpha_blend_operation = operation }
@end

@implementation MTLDepthStencilDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (NSUInteger)depthCompareFunction { env.objc.borrow::<MetalObjectHostObject>(this).usage }
- (())setDepthCompareFunction:(NSUInteger)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).usage = function }
- (bool)depthWriteEnabled { env.objc.borrow::<MetalObjectHostObject>(this).sample_count != 0 }
- (())setDepthWriteEnabled:(bool)enabled { env.objc.borrow_mut::<MetalObjectHostObject>(this).sample_count = enabled as NSUInteger }
- (id)frontFaceStencil {
    let existing = env.objc.borrow::<MetalObjectHostObject>(this).front_stencil;
    if existing != nil { return existing; }
    let stencil = msg_class![env; MTLStencilDescriptor new];
    env.objc.borrow_mut::<MetalObjectHostObject>(this).front_stencil = stencil;
    stencil
}
- (id)backFaceStencil {
    let existing = env.objc.borrow::<MetalObjectHostObject>(this).back_stencil;
    if existing != nil { return existing; }
    let stencil = msg_class![env; MTLStencilDescriptor new];
    env.objc.borrow_mut::<MetalObjectHostObject>(this).back_stencil = stencil;
    stencil
}
- (id)label { env.objc.borrow::<MetalObjectHostObject>(this).label }
- (())setLabel:(id)label { env.objc.borrow_mut::<MetalObjectHostObject>(this).label = label }
- (id)vertexFunction { env.objc.borrow::<MetalObjectHostObject>(this).vertex_function }
- (())setVertexFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).vertex_function = function }
- (id)fragmentFunction { env.objc.borrow::<MetalObjectHostObject>(this).fragment_function }
- (())setFragmentFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).fragment_function = function }
- (NSUInteger)depthAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).depth_pixel_format }
- (())setDepthAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).depth_pixel_format = format }
- (NSUInteger)stencilAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).stencil_pixel_format }
- (())setStencilAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).stencil_pixel_format = format }
@end


@implementation MTLStencilDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (NSUInteger)stencilCompareFunction { env.objc.borrow::<MetalObjectHostObject>(this).stencil_compare_function }
- (())setStencilCompareFunction:(NSUInteger)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).stencil_compare_function = function }
- (NSUInteger)stencilFailureOperation { env.objc.borrow::<MetalObjectHostObject>(this).stencil_failure_operation }
- (())setStencilFailureOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).stencil_failure_operation = operation }
- (NSUInteger)depthFailureOperation { env.objc.borrow::<MetalObjectHostObject>(this).depth_failure_operation }
- (())setDepthFailureOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).depth_failure_operation = operation }
- (NSUInteger)depthStencilPassOperation { env.objc.borrow::<MetalObjectHostObject>(this).depth_stencil_pass_operation }
- (())setDepthStencilPassOperation:(NSUInteger)operation { env.objc.borrow_mut::<MetalObjectHostObject>(this).depth_stencil_pass_operation = operation }
- (NSUInteger)writeMask { env.objc.borrow::<MetalObjectHostObject>(this).write_mask }
- (())setWriteMask:(NSUInteger)mask { env.objc.borrow_mut::<MetalObjectHostObject>(this).write_mask = mask }
- (NSUInteger)readMask { env.objc.borrow::<MetalObjectHostObject>(this).read_mask }
- (())setReadMask:(NSUInteger)mask { env.objc.borrow_mut::<MetalObjectHostObject>(this).read_mask = mask }
@end

@implementation MTLRenderPipelineDescriptor: NSObject
+ (id)allocWithZone:(NSZonePtr)_zone { env.objc.alloc_object(this, Box::new(MetalObjectHostObject::default()), &mut env.mem) }
- (id)init { this }
- (id)vertexDescriptor { env.objc.borrow::<MetalObjectHostObject>(this).layouts }
- (())setVertexDescriptor:(id)descriptor { env.objc.borrow_mut::<MetalObjectHostObject>(this).layouts = descriptor }
- (id)colorAttachments { msg_class![env; MTLRenderPassColorAttachmentDescriptorArray new] }
- (NSUInteger)sampleCount { env.objc.borrow::<MetalObjectHostObject>(this).sample_count }
- (())setSampleCount:(NSUInteger)count { env.objc.borrow_mut::<MetalObjectHostObject>(this).sample_count = count }
- (id)label { env.objc.borrow::<MetalObjectHostObject>(this).label }
- (())setLabel:(id)label { env.objc.borrow_mut::<MetalObjectHostObject>(this).label = label }
- (id)vertexFunction { env.objc.borrow::<MetalObjectHostObject>(this).vertex_function }
- (())setVertexFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).vertex_function = function }
- (id)fragmentFunction { env.objc.borrow::<MetalObjectHostObject>(this).fragment_function }
- (())setFragmentFunction:(id)function { env.objc.borrow_mut::<MetalObjectHostObject>(this).fragment_function = function }
- (NSUInteger)depthAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).depth_pixel_format }
- (())setDepthAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).depth_pixel_format = format }
- (NSUInteger)stencilAttachmentPixelFormat { env.objc.borrow::<MetalObjectHostObject>(this).stencil_pixel_format }
- (())setStencilAttachmentPixelFormat:(NSUInteger)format { env.objc.borrow_mut::<MetalObjectHostObject>(this).stencil_pixel_format = format }
@end



};

fn MTLCreateSystemDefaultDevice(env: &mut Environment) -> id {
    msg_class![env; MTLDevice new]
}

pub const FUNCTIONS: crate::dyld::FunctionExports =
    &[crate::dyld::export_c_func!(MTLCreateSystemDefaultDevice())];

pub const CONSTANTS: ConstantExports = &[
    (
        "_MTLResourceStorageModeShared",
        crate::dyld::HostConstant::Custom(|env| {
            constant_uinteger(env, MTL_RESOURCE_STORAGE_MODE_SHARED)
        }),
    ),
    (
        "_MTLResourceStorageModeManaged",
        crate::dyld::HostConstant::Custom(|env| {
            constant_uinteger(env, MTL_RESOURCE_STORAGE_MODE_MANAGED)
        }),
    ),
    (
        "_MTLResourceStorageModePrivate",
        crate::dyld::HostConstant::Custom(|env| {
            constant_uinteger(env, MTL_RESOURCE_STORAGE_MODE_PRIVATE)
        }),
    ),
    (
        "_MTLResourceStorageModeMemoryless",
        crate::dyld::HostConstant::Custom(|env| {
            constant_uinteger(env, MTL_RESOURCE_STORAGE_MODE_MEMORYLESS)
        }),
    ),
    (
        "_MTLLoadActionDontCare",
        crate::dyld::HostConstant::Custom(|env| constant_uinteger(env, MTL_LOAD_ACTION_DONT_CARE)),
    ),
    (
        "_MTLLoadActionLoad",
        crate::dyld::HostConstant::Custom(|env| constant_uinteger(env, MTL_LOAD_ACTION_LOAD)),
    ),
    (
        "_MTLLoadActionClear",
        crate::dyld::HostConstant::Custom(|env| constant_uinteger(env, MTL_LOAD_ACTION_CLEAR)),
    ),
    (
        "_MTLStoreActionDontCare",
        crate::dyld::HostConstant::Custom(|env| constant_uinteger(env, MTL_STORE_ACTION_DONT_CARE)),
    ),
    (
        "_MTLStoreActionStore",
        crate::dyld::HostConstant::Custom(|env| constant_uinteger(env, MTL_STORE_ACTION_STORE)),
    ),
];
