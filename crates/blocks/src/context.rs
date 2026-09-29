//! What each hook can tell its body about the event that fired it.
//!
//! The generated Java passes one `Ctx` object into every handler, with a
//! nullable field per piece of context, so codegen never has to know which
//! hook it is inside - see `stitchcraft-export`. This table is what makes the
//! editor able to say so in advance: a reporter reading a field its hook does
//! not fill is a warning, not a crash, because `Ctx` answers with a default.
//!
//! [`HookContext::UNKNOWN`] is the custom-block case. A "My Blocks" definition
//! can be called from any hook, so nothing about its context is decided until
//! the call site - and every field is treated as possibly present.

use crate::block::McBlock;
use crate::kinds::Target;

/// One piece of event context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextField {
    /// A `ServerPlayer` the event is about.
    Player,
    /// A non-player `LivingEntity` the event is about.
    Entity,
    /// A `BlockPos` the event happened at.
    Position,
    /// The `BlockState` at that position.
    Block,
    /// How much damage the event deals.
    Damage,
    /// A `ServerLevel`. Everything that touches the world needs it.
    Level,
}

impl ContextField {
    /// How the field reads in a diagnostic.
    pub fn label(self) -> &'static str {
        match self {
            ContextField::Player => "a player",
            ContextField::Entity => "an entity",
            ContextField::Position => "a position",
            ContextField::Block => "a block",
            ContextField::Damage => "a damage amount",
            ContextField::Level => "a level",
        }
    }
}

/// Which context fields a hook fills in, and whether its body can cancel it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HookContext {
    pub player: bool,
    pub entity: bool,
    pub position: bool,
    pub block: bool,
    pub damage: bool,
    pub level: bool,
    /// True when the generated handler's `false` return actually stops
    /// something - only then does `CancelEvent` do anything.
    pub cancellable: bool,
    /// True on the client side, where server-side commands do not belong.
    pub client_only: bool,
}

impl HookContext {
    /// Nothing at all - `OnModInit` and friends run before any world exists.
    pub const NONE: Self = Self {
        player: false,
        entity: false,
        position: false,
        block: false,
        damage: false,
        level: false,
        cancellable: false,
        client_only: false,
    };

    /// The custom-block case: assume anything might be there, warn about
    /// nothing, and let the call site decide.
    pub const UNKNOWN: Self = Self {
        player: true,
        entity: true,
        position: true,
        block: true,
        damage: true,
        level: true,
        cancellable: false,
        client_only: false,
    };

    pub fn has(self, field: ContextField) -> bool {
        match field {
            ContextField::Player => self.player,
            ContextField::Entity => self.entity,
            ContextField::Position => self.position,
            ContextField::Block => self.block,
            ContextField::Damage => self.damage,
            ContextField::Level => self.level,
        }
    }

    /// Whether a command's `target` dropdown resolves under this hook.
    pub fn supports(self, target: Target) -> bool {
        match target {
            Target::EventPlayer => self.player,
            Target::EventEntity => self.entity,
            // Both reach for the player list, which needs a running server;
            // the nearest one also needs somewhere to measure from.
            Target::AllPlayers => true,
            Target::NearestPlayer => self.position,
        }
    }
}

/// The context `hook` provides, or `None` if `hook` is not a header block.
pub fn context_of(hook: &McBlock) -> Option<HookContext> {
    let base = HookContext::NONE;
    let level = HookContext { level: true, ..base };
    let at = HookContext {
        position: true,
        ..level
    };
    Some(match hook {
        McBlock::OnModInit | McBlock::OnServerStarted | McBlock::OnServerStopping => base,
        McBlock::OnClientInit => HookContext {
            client_only: true,
            ..base
        },
        McBlock::OnServerTick => base,
        McBlock::OnLevelTick => level,
        McBlock::OnPlayerJoin | McBlock::OnPlayerLeave => HookContext { player: true, ..at },
        // Placement is dispatched from the block's own `onPlace`, which has no
        // player to hand on.
        McBlock::OnBlockPlaced { .. } => HookContext { block: true, ..at },
        McBlock::OnBlockBroken { .. } => HookContext {
            player: true,
            block: true,
            cancellable: true,
            ..at
        },
        McBlock::OnBlockUsed { .. } => HookContext {
            player: true,
            block: true,
            cancellable: true,
            ..at
        },
        McBlock::OnBlockTick { .. } => HookContext { block: true, ..at },
        McBlock::OnItemUsed { .. } => HookContext {
            player: true,
            cancellable: true,
            ..at
        },
        McBlock::OnEntityTick { .. } => HookContext { entity: true, ..at },
        McBlock::OnEntityHurt => HookContext {
            entity: true,
            damage: true,
            cancellable: true,
            ..at
        },
        McBlock::OnEntityDeath => HookContext { entity: true, ..at },
        // A command can come from the console, so the player is nullable -
        // `Ctx` guards it either way.
        McBlock::OnCommand { .. } => HookContext { player: true, ..at },
        McBlock::BlockHeader { .. } => HookContext::UNKNOWN,
        _ if hook.is_declaration() => base,
        _ => return None,
    })
}

/// The context field a reporter reads, for the reporters that read one.
pub fn reporter_needs(kind: &str) -> Option<ContextField> {
    Some(match kind {
        "EventPlayerName" | "EventPlayerHealth" | "EventPlayerHeldItem" => ContextField::Player,
        "EventX" | "EventY" | "EventZ" => ContextField::Position,
        "EventBlockId" => ContextField::Block,
        "EventEntityType" | "EventEntityHealth" => ContextField::Entity,
        "EventDamageAmount" => ContextField::Damage,
        "BlockIdAt" | "IsBlockAt" | "LightAt" | "SurfaceHeightAt" | "IsDay" | "IsRaining"
        | "TimeOfDay" | "Difficulty" | "LevelName" => ContextField::Level,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_hook_knows_nothing_about_players() {
        let ctx = context_of(&McBlock::OnServerTick).unwrap();
        assert!(!ctx.player);
        assert!(!ctx.supports(Target::EventPlayer));
        assert!(
            ctx.supports(Target::AllPlayers),
            "the player list needs no event context"
        );
    }

    #[test]
    fn hurt_is_the_hook_that_carries_a_damage_amount() {
        let hurt = context_of(&McBlock::OnEntityHurt).unwrap();
        assert!(hurt.has(ContextField::Damage));
        assert!(hurt.cancellable);
        let death = context_of(&McBlock::OnEntityDeath).unwrap();
        assert!(!death.has(ContextField::Damage));
        assert!(!death.cancellable);
    }

    #[test]
    fn custom_block_bodies_are_context_agnostic() {
        let ctx = context_of(&McBlock::BlockHeader {
            block_id: "def".into(),
        })
        .unwrap();
        assert_eq!(ctx, HookContext::UNKNOWN);
        assert!(!ctx.cancellable, "a reporter body has no event to cancel");
    }

    #[test]
    fn non_headers_have_no_context_of_their_own() {
        assert!(context_of(&McBlock::CancelEvent).is_none());
    }
}
