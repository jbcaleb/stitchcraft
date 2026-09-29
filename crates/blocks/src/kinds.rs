//! The small closed choices a block's dropdown fields hold.
//!
//! Every one of these serializes as its bare variant name, because that name
//! is also the `value` a QML dropdown piece sends back - see
//! `Stitchcraft/BlockRows.qml`. Renaming a variant is a save-format break.

use serde::{Deserialize, Serialize};

/// Which entity a command acts on. Values are resolved against the event
/// context the enclosing hook provides, so not every target is meaningful
/// under every hook - [`crate::validate`] reports the mismatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Target {
    /// The player the event is about. The default, and the only target most
    /// hooks have without extra work.
    #[default]
    EventPlayer,
    /// The non-player entity the event is about (a hurt mob, a ticking mob).
    EventEntity,
    /// Every player on the server.
    AllPlayers,
    /// The player closest to the event's position.
    NearestPlayer,
}

impl Target {
    /// True when this target resolves to a single entity rather than a set -
    /// codegen emits a straight-line call for one, a loop for many.
    pub fn is_singular(self) -> bool {
        !matches!(self, Target::AllPlayers)
    }
}

/// Where a message shows up on the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MessageKind {
    #[default]
    Chat,
    ActionBar,
}

/// A starting point for a block's `BlockBehaviour.Properties`. Picking a
/// preset sets the map color, sound type and push reaction; the numeric
/// fields on the block then override hardness, resistance and light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MaterialPreset {
    #[default]
    Stone,
    Wood,
    Metal,
    Glass,
    Wool,
    Dirt,
    Sand,
    Plant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
}

/// Which vanilla `MobCategory` an entity type spawns and despawns under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum MobCategoryKind {
    #[default]
    Creature,
    Monster,
    Ambient,
    WaterCreature,
    Misc,
}

/// Which generated mob class an entity is built on. Each one brings a fixed
/// goal set; the declaration's numeric fields tune it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EntityBase {
    /// Wanders, looks at players, panics when hurt.
    #[default]
    Passive,
    /// Wanders, and targets and melees the nearest player.
    Hostile,
    /// Drifts through the air, no gravity, no pathfinding.
    Flying,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Weather {
    #[default]
    Clear,
    Rain,
    Thunder,
}
