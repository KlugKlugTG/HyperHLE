/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Text input support classes.
//!
//! `UITextInputStringTokenizer` is the default implementation of
//! `UITextInputTokenizer`, used by `UITextField`/`UITextView` for
//! string tokenization (word/character/paragraph granularity). Games
//! sometimes instantiate it directly (e.g. Asphalt 8 via its Jet UI
//! layer).
//!
//! The tokenizer operates on the text of its text input (obtained via the
//! `text` property, which `UITextField` implements) and works in terms of
//! `UITextPosition`/`UITextRange`, which are implemented here as simple
//! offset-based value objects. Boundary logic follows Unicode-ish rules:
//! words are runs of alphanumeric characters (plus `_`), sentences end at
//! `.`, `!`, `?` or newlines followed by whitespace, paragraphs/lines are
//! delimited by newlines, and document boundaries are the start/end of the
//! text.

use crate::frameworks::foundation::{ns_string, NSInteger, NSUInteger};
use crate::objc::{
    autorelease, id, msg, msg_super, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};
use crate::Environment;

/// `UITextGranularity` (from `UITextInput.h`).
pub type UITextGranularity = NSUInteger;
pub const UITEXT_GRANULARITY_CHARACTER: UITextGranularity = 0;
pub const UITEXT_GRANULARITY_WORD: UITextGranularity = 1;
pub const UITEXT_GRANULARITY_SENTENCE: UITextGranularity = 2;
pub const UITEXT_GRANULARITY_PARAGRAPH: UITextGranularity = 3;
pub const UITEXT_GRANULARITY_LINE: UITextGranularity = 4;
pub const UITEXT_GRANULARITY_DOCUMENT: UITextGranularity = 5;

/// `UITextStorageDirection` / `UITextLayoutDirection` (from `UITextInput.h`).
pub type UITextDirection = NSUInteger;
pub const UITEXT_STORAGE_DIRECTION_FORWARD: UITextDirection = 0;
pub const UITEXT_STORAGE_DIRECTION_BACKWARD: UITextDirection = 1;
pub const UITEXT_LAYOUT_DIRECTION_LEFT: UITextDirection = 2;
pub const UITEXT_LAYOUT_DIRECTION_RIGHT: UITextDirection = 3;
pub const UITEXT_LAYOUT_DIRECTION_UP: UITextDirection = 4;
pub const UITEXT_LAYOUT_DIRECTION_DOWN: UITextDirection = 5;

#[derive(Default)]
pub struct UITextPositionHostObject {
    /// Byte offset into the UTF-8 encoding of the input's text.
    offset: NSUInteger,
}
impl HostObject for UITextPositionHostObject {}

#[derive(Default)]
pub struct UITextRangeHostObject {
    start: id,
    end: id,
}
impl HostObject for UITextRangeHostObject {}

#[derive(Default)]
struct UITextInputStringTokenizerHostObject {
    /// The text input (e.g. `UITextField*`) whose text is tokenized.
    /// Weak: the tokenizer is owned by (or at least used with) the input,
    /// which retains its own state.
    text_input: id,
}
impl HostObject for UITextInputStringTokenizerHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITextPosition: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UITextPositionHostObject { offset: 0 });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    let this: id = msg_super![env; this init];
    if this == nil {
        return nil;
    }
    // Offset is set by the tokenizer right after allocation.
    this
}

- (id)copyWithZone:(NSZonePtr)_zone {
    msg![env; this retain]
}

- (())dealloc {
    msg_super![env; this dealloc]
}

@end

@implementation UITextRange: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UITextRangeHostObject {
        start: nil,
        end: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)init {
    let this: id = msg_super![env; this init];
    if this == nil {
        return nil;
    }
    // Start/end positions are set by the tokenizer right after allocation.
    this
}

- (id)copyWithZone:(NSZonePtr)_zone {
    msg![env; this retain]
}

- (id)start {
    env.objc.borrow::<UITextRangeHostObject>(this).start
}

- (id)end {
    env.objc.borrow::<UITextRangeHostObject>(this).end
}

- (bool)isEmpty {
    let range = env.objc.borrow::<UITextRangeHostObject>(this);
    let start = env.objc.borrow::<UITextPositionHostObject>(range.start);
    let end = env.objc.borrow::<UITextPositionHostObject>(range.end);
    start.offset == end.offset
}

// Compares by (start, end) offsets, like UIKit's value semantics.
- (NSInteger)compare:(id)other {
    let (start, end) = {
        let range = env.objc.borrow::<UITextRangeHostObject>(this);
        (
            env.objc.borrow::<UITextPositionHostObject>(range.start).offset,
            env.objc.borrow::<UITextPositionHostObject>(range.end).offset,
        )
    };
    let other_range = env.objc.borrow::<UITextRangeHostObject>(other);
    let (other_start, other_end) = (
        env.objc.borrow::<UITextPositionHostObject>(other_range.start).offset,
        env.objc.borrow::<UITextPositionHostObject>(other_range.end).offset,
    );
    match (start, end).cmp(&(other_start, other_end)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

- (())dealloc {
    let (start, end) = {
        let range = env.objc.borrow_mut::<UITextRangeHostObject>(this);
        (range.start, range.end)
    };
    release(env, start);
    release(env, end);
    msg_super![env; this dealloc]
}

@end

@implementation UITextInputStringTokenizer: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(UITextInputStringTokenizerHostObject {
        text_input: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithTextInput:(id)text_input {
    let this: id = msg_super![env; this init];
    if this == nil {
        return nil;
    }
    env.objc.borrow_mut::<UITextInputStringTokenizerHostObject>(this).text_input =
        retain(env, text_input);
    this
}

// Returns the position of the boundary in the requested direction.
- (id)positionFromPosition:(id)position
                toBoundary:(UITextGranularity)granularity
               inDirection:(UITextDirection)direction {
    let text_input =
        env.objc.borrow::<UITextInputStringTokenizerHostObject>(this).text_input;
    let text = text_of(env, text_input);
    let idx = position_to_char_index(env, position, &text);
    let backward = direction_is_backward(direction);
    let new_idx = boundary_index(&text, idx, granularity, backward);
    let new_offset = char_index_to_offset(&text, new_idx);
    make_position(env, new_offset)
}

// Whether the position is at the boundary in the requested direction.
- (bool)isPosition:(id)position
        atBoundary:(UITextGranularity)granularity
       inDirection:(UITextDirection)direction {
    let text_input =
        env.objc.borrow::<UITextInputStringTokenizerHostObject>(this).text_input;
    let text = text_of(env, text_input);
    let idx = position_to_char_index(env, position, &text);
    let backward = direction_is_backward(direction);
    let new_idx = boundary_index(&text, idx, granularity, backward);
    new_idx == idx
}

// Returns the range enclosing the position at the given granularity.
- (id)rangeEnclosingPosition:(id)position
              withGranularity:(UITextGranularity)granularity
                  inDirection:(UITextDirection)direction {
    let text_input =
        env.objc.borrow::<UITextInputStringTokenizerHostObject>(this).text_input;
    let text = text_of(env, text_input);
    let idx = position_to_char_index(env, position, &text);
    let backward = direction_is_backward(direction);
    let chars: Vec<char> = text.chars().collect();

    let (start_idx, end_idx) = match granularity {
        UITEXT_GRANULARITY_CHARACTER => {
            if idx >= chars.len() {
                (idx, idx)
            } else if backward {
                (idx.saturating_sub(1), idx)
            } else {
                (idx, (idx + 1).min(chars.len()))
            }
        }
        UITEXT_GRANULARITY_DOCUMENT => (0, chars.len()),
        _ => {
            let start = boundary_index(&text, idx, granularity, true);
            let end = boundary_index(&text, idx, granularity, false);
            (start, end)
        }
    };

    let start = make_position(env, char_index_to_offset(&text, start_idx));
    let end = make_position(env, char_index_to_offset(&text, end_idx));
    make_range(env, start, end)
}

- (())dealloc {
    let text_input =
        env.objc.borrow_mut::<UITextInputStringTokenizerHostObject>(this).text_input;
    release(env, text_input);
    msg_super![env; this dealloc]
}

@end

};

// MARK: - Helpers

fn text_of(env: &mut Environment, text_input: id) -> String {
    if text_input == nil {
        return String::new();
    }
    // `text` is implemented by UITextField (and would be by UITextView).
    let ns_text: id = msg![env; text_input text];
    ns_string::to_rust_string(env, ns_text).into_owned()
}

/// Maps a `UITextDirection` to "backward?" using the standard storage/layout
/// direction mapping (assuming left-to-right text, like UIKit's docs do).
fn direction_is_backward(direction: UITextDirection) -> bool {
    match direction {
        UITEXT_STORAGE_DIRECTION_BACKWARD => true,
        UITEXT_LAYOUT_DIRECTION_LEFT => true,
        // Forward storage direction, right, up/down in LTR text and any
        // unknown values all move toward the end of the string.
        _ => false,
    }
}

/// Converts a `UITextPosition`'s byte offset to a char index clamped to the
/// text.
fn position_to_char_index(env: &mut Environment, position: id, text: &str) -> usize {
    if position == nil {
        return 0;
    }
    let byte_offset = env.objc.borrow::<UITextPositionHostObject>(position).offset;
    let byte_offset = byte_offset as usize;
    for (char_idx, (byte_idx, _char)) in text.char_indices().enumerate() {
        if byte_idx >= byte_offset {
            return char_idx;
        }
    }
    text.chars().count()
}

fn char_index_to_offset(text: &str, char_idx: usize) -> NSUInteger {
    text.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx as NSUInteger)
        .unwrap_or_else(|| text.len() as NSUInteger)
}

fn boundary_index(
    text: &str,
    idx: usize,
    granularity: UITextGranularity,
    backward: bool,
) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if granularity == UITEXT_GRANULARITY_DOCUMENT {
        return if backward { 0 } else { len };
    }
    match granularity {
        UITEXT_GRANULARITY_CHARACTER => {
            if backward {
                idx.saturating_sub(1)
            } else {
                (idx + 1).min(len)
            }
        }
        UITEXT_GRANULARITY_WORD => {
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            if backward {
                let mut i = idx.min(len);
                while i > 0 && !is_word(chars[i - 1]) {
                    i -= 1;
                }
                while i > 0 && is_word(chars[i - 1]) {
                    i -= 1;
                }
                i
            } else {
                let mut i = idx;
                while i < len && is_word(chars[i]) {
                    i += 1;
                }
                while i < len && !is_word(chars[i]) {
                    i += 1;
                }
                i
            }
        }
        UITEXT_GRANULARITY_SENTENCE => {
            let is_terminator = |c: char| matches!(c, '.' | '!' | '?' | '\n');
            if backward {
                let mut i = idx.min(len);
                while i > 0 {
                    if is_terminator(chars[i - 1]) {
                        break;
                    }
                    i -= 1;
                }
                // Include the whitespace run that follows the terminator.
                while i < len && i < idx && chars[i].is_whitespace() {
                    i += 1;
                }
                i
            } else {
                let mut i = idx;
                while i < len && !is_terminator(chars[i]) {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                while i < len && chars[i].is_whitespace() {
                    i += 1;
                }
                i
            }
        }
        // Paragraphs and storage-level lines are delimited by newlines.
        UITEXT_GRANULARITY_PARAGRAPH | UITEXT_GRANULARITY_LINE => {
            if backward {
                let mut i = idx.min(len);
                while i > 0 && chars[i - 1] != '\n' {
                    i -= 1;
                }
                i
            } else {
                let mut i = idx;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                i
            }
        }
        _ => len,
    }
}

fn make_position(env: &mut Environment, offset: NSUInteger) -> id {
    let class = env.objc.get_known_class("UITextPosition", &mut env.mem);
    let position = env.objc.alloc_object(
        class,
        Box::new(UITextPositionHostObject { offset }),
        &mut env.mem,
    );
    autorelease(env, position)
}

fn make_range(env: &mut Environment, start: id, end: id) -> id {
    let class = env.objc.get_known_class("UITextRange", &mut env.mem);
    let start = retain(env, start);
    let end = retain(env, end);
    let range = env.objc.alloc_object(
        class,
        Box::new(UITextRangeHostObject { start, end }),
        &mut env.mem,
    );
    autorelease(env, range)
}
