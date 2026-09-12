//! `GKMatchmaker` — matchmaking singleton.
//!
//! touchHLE has no Game Center connectivity, so matchmaking can never
//! actually find peers. The singleton is nevertheless a real object:
//! games call `[GKMatchmaker sharedMatchmaker]` during start-up and then
//! register matchmaking handlers / query properties. Returning `nil`
//! (the previous unimplemented-class behaviour) made some games conclude
//! Game Center is broken and skip otherwise-working features.
//!
//! Asynchronous APIs (`startMatchableRequestWithCompletionHandler:`,
//! `findMatchForRequest:withCompletionHandler:` etc.) invoke their
//! completion block with `nil` match / a GKError so games take their
//! "no match available" path promptly instead of waiting forever.

use crate::abi::{CallFromHost, GuestFunction};
use crate::frameworks::game_kit::gk_local_player::make_not_authenticated_error;
use crate::mem::{ConstVoidPtr, MutPtr, Ptr};
use crate::objc::{autorelease, id, msg_class, nil, objc_classes, ClassExports, HostObject};
use crate::Environment;
use crate::objc::msg;

/// Apple Block ABI: word offset 3 (== byte offset 12) of a block struct
/// holds its `invoke` function pointer.
/// <https://clang.llvm.org/docs/Block-ABI-Apple.html>
const BLOCK_INVOKE_WORD_OFFSET: u32 = 3;

fn block_invoke(env: &mut Environment, block: id) -> Option<GuestFunction> {
    if block == nil {
        return None;
    }
    let block_ptr: MutPtr<u32> = Ptr::from_bits(block.to_bits());
    let invoke_addr: u32 = env.mem.read(block_ptr + BLOCK_INVOKE_WORD_OFFSET);
    if invoke_addr == 0 {
        log!(
            "Warning: GKMatchmaker block {:?} has NULL invoke pointer; not calling.",
            block
        );
        return None;
    }
    Some(GuestFunction::from_addr_with_thumb_bit(invoke_addr))
}

/// Invoke an ObjC block with the signature `void (^)(GKMatch *, NSError *)`
/// or `void (^)(GKMatchRequest *, NSError *)` — second argument is a
/// not-authenticated `NSError`, first is `nil`.
fn invoke_match_error_block(env: &mut Environment, block: id) {
    let Some(invoke) = block_invoke(env, block) else {
        return;
    };
    let error = make_not_authenticated_error(env);
    let block_arg: ConstVoidPtr = Ptr::from_bits(block.to_bits()).cast_const();
    <GuestFunction as CallFromHost<(), (ConstVoidPtr, id, id)>>::call_from_host(
        &invoke,
        env,
        (block_arg, nil, error),
    );
}

/// Invoke an ObjC block with the signature `void (^)(NSError *)` —
/// a not-authenticated `NSError`.
fn invoke_error_block(env: &mut Environment, block: id) {
    let Some(invoke) = block_invoke(env, block) else {
        return;
    };
    let error = make_not_authenticated_error(env);
    let block_arg: ConstVoidPtr = Ptr::from_bits(block.to_bits()).cast_const();
    <GuestFunction as CallFromHost<(), (ConstVoidPtr, id)>>::call_from_host(
        &invoke,
        env,
        (block_arg, error),
    );
}

#[derive(Default)]
struct GKMatchmakerHostObject;

impl HostObject for GKMatchmakerHostObject {}

/// Singleton cache for `[GKMatchmaker sharedMatchmaker]`.
#[derive(Default)]
pub struct State {
    matchmaker: Option<id>,
}

impl State {
    fn get(env: &mut Environment) -> &mut State {
        &mut env.framework_state.game_kit.matchmaker
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation GKMatchmaker: NSObject

// + (GKMatchmaker *)sharedMatchmaker
+ (id)sharedMatchmaker {
    if let Some(matchmaker) = State::get(env).matchmaker {
        return matchmaker;
    }
    let new: id = msg_class![env; GKMatchmaker alloc];
    let matchmaker: id = msg![env; new init];
    State::get(env).matchmaker = Some(matchmaker);
    matchmaker
}

// The singleton is intentionally immortal (like GKLocalPlayer's), so
// `-release` is a no-op.
- (())release {
}

- (id)retain {
    this
}

// - (void)startBrowsingForPlayers:(void (^)(NSArray<GKPlayer *> *, ...))...
- (())startBrowsingForPlayers:(id)_handler {
    log!("GKMatchmaker startBrowsingForPlayers: stubbed (no peers in offline mode)");
}

- (())stopBrowsingForPlayers {
}

// - (void)startMatchableRequest... (iOS 6+, private-ish but used by some
// engines); completion signature varies, so we only accept the common
// `void (^)(GKMatch *, NSError *)` shape via findMatchForRequest below.

// - (void)findMatchForRequest:(GKMatchRequest *)request
//       withCompletionHandler:(void (^)(GKMatch *, NSError *))handler
- (())findMatchForRequest:(id)_request
    withCompletionHandler:(id)handler {
    log!("GKMatchmaker findMatchForRequest:withCompletionHandler: stubbed (offline)");
    if handler != nil {
        invoke_match_error_block(env, handler);
    }
}

// - (void)findPlayersForHostedMatchRequest:(GKMatchRequest *)request
//                    withCompletionHandler:(void (^)(NSArray *, NSError *))handler
- (())findPlayersForHostedMatchRequest:(id)_request
                 withCompletionHandler:(id)handler {
    log!("GKMatchmaker findPlayersForHostedMatchRequest: stubbed (offline)");
    if handler != nil {
        invoke_match_error_block(env, handler);
    }
}

// - (void)cancel {
- (())cancel {
}

// - (void)queryPlayerGroupActivity:(NSUInteger)group
//            withCompletionHandler:(void (^)(NSInteger, NSError *))handler
- (())queryPlayerGroupActivity:(u32)_group
         withCompletionHandler:(id)handler {
    log!("GKMatchmaker queryPlayerGroupActivity:withCompletionHandler: stubbed");
    if handler != nil {
        invoke_error_block(env, handler);
    }
}

// - (void)queryActivityStartingWithInvite:(GKInvite *)invite
//                   withCompletionHandler:(void (^)(NSInteger, NSError *))handler
- (())queryActivityStartingWithInvite:(id)_invite
                withCompletionHandler:(id)handler {
    log!("GKMatchmaker queryActivityStartingWithInvite: stubbed");
    if handler != nil {
        invoke_error_block(env, handler);
    }
}

// - (void)invitePlayer:(GKPlayer *)player { ... } (block-tail variants
// exist across SDK versions; ignore them all)
- (())invitePlayer:(id)_player {
}

- (())cancelInvite:(id)_player {
}

// - (void)loadMatchDataWithCompletionHandler:(void (^)(NSData *, NSError *))handler
- (())loadMatchDataWithCompletionHandler:(id)handler {
    log!("GKMatchmaker loadMatchDataWithCompletionHandler: stubbed (empty)");
    if handler == nil {
        return;
    }
    let empty_data: id = msg_class![env; NSData data];
    autorelease(env, empty_data);
    let Some(invoke) = block_invoke(env, handler) else {
        return;
    };
    let error = make_not_authenticated_error(env);
    let block_arg: ConstVoidPtr = Ptr::from_bits(handler.to_bits()).cast_const();
    <GuestFunction as CallFromHost<(), (ConstVoidPtr, id, id)>>::call_from_host(
        &invoke,
        env,
        (block_arg, empty_data, error),
    );
}

// - (BOOL)isInvitable:(GKPlayer *)player
- (bool)isInvitable:(id)_player {
    false
}

// - (void)addPlayersToMatch:(GKMatch *)match
//   players:(NSArray *)playersToInvite
//   withCompletionHandler:(void (^)(NSError *))handler
- (())addPlayersToMatch:(id)_match
                players:(id)_players
  withCompletionHandler:(id)handler {
    log!("GKMatchmaker addPlayersToMatch: stubbed");
    if handler != nil {
        invoke_error_block(env, handler);
    }
}

// - (void)finishMatchmakingForMatch:(GKMatch *)match
- (())finishMatchmakingForMatch:(id)_match {
}

// - (void)match:(GKMatch *)match didFail:(NSError *)error (delegate-style)
- (())match:(id)_match didFail:(id)_error {
}

@end

};
