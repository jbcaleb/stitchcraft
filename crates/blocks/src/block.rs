//! [`McBlock`] - the instruction vocabulary a Stitchcraft canvas is made of,
//! and its [`BlockKind`] implementation.
//!
//! Three families live in one enum, because blockstitch addresses a canvas by
//! a single kind type:
//!
//! * **declarations** (`Register*`) - registry entries. Header-shaped, so each
//!   one sits alone at the top of its own strand. They have no body; anything
//!   stacked under one is ignored, and [`crate::validate`] says so.
//! * **event hooks** (`On*`) - also header-shaped. A hook's handler is the
//!   rest of its strand, the way `WhenRan` works in Blockwork: headers never
//!   own a mouth.
//! * **commands and control flow** - everything else. Only control flow owns
//!   nested bodies, and never more than [`BlockKind::BODY_SLOTS`] of them.
//!
//! Nothing here evaluates. A Stitchcraft document is compiled to Java by
//! `stitchcraft-export`, so the values in these slots are expression trees to
//! translate, not expressions to run.

use crate::kinds::{
    EntityBase, MaterialPreset, MessageKind, MobCategoryKind, Rarity, Target, Weather,
};
use blockstitch_core::graph::{BlockDef, BlockKind, InputValueType, Instruction};
use blockstitch_core::value::Value;
use serde::{Deserialize, Serialize};

/// Shorthand for a nested instruction list, which is most of what the
/// control-flow variants below hold.
pub type Body = Vec<Instruction<McBlock>>;

/// One instruction on a Stitchcraft canvas. Serialized internally tagged as
/// `type`, which is the discriminator the QML canvas reads
/// (`instruction.type`), so the tag name is part of the save format.
#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McBlock {
    // ── Declarations ──────────────────────────────────────────────────────
    /// A block registered into the block registry, with an optional
    /// `BlockItem` alongside it.
    RegisterBlock {
        block_id: String,
        display_name: String,
        material: MaterialPreset,
        hardness: Value,
        resistance: Value,
        light: Value,
        requires_tool: bool,
        drops_self: bool,
        give_item: bool,
        creative_tab: String,
    },
    RegisterItem {
        item_id: String,
        display_name: String,
        max_stack: Value,
        rarity: Rarity,
        creative_tab: String,
    },
    /// An entity type plus the generated mob class its `base` names.
    RegisterEntity {
        entity_id: String,
        display_name: String,
        base: EntityBase,
        category: MobCategoryKind,
        width: Value,
        height: Value,
        max_health: Value,
        movement_speed: Value,
        attack_damage: Value,
        spawn_egg: bool,
    },
    RegisterSound {
        sound_id: String,
        subtitle: String,
    },
    RegisterCreativeTab {
        tab_id: String,
        display_name: String,
        icon: String,
    },

    // ── Event hooks ───────────────────────────────────────────────────────
    /// Common mod initialization - the tail runs once, after every registry
    /// has been handed to the loader.
    OnModInit,
    /// Client-only initialization.
    OnClientInit,
    OnServerStarted,
    OnServerStopping,
    /// Every server tick (20/s). Keep the tail cheap.
    OnServerTick,
    /// Every tick of every loaded level.
    OnLevelTick,
    OnPlayerJoin,
    OnPlayerLeave,
    /// After `block_id` is placed. An empty `block_id` matches any block.
    OnBlockPlaced { block_id: String },
    /// After a player breaks `block_id`.
    OnBlockBroken { block_id: String },
    /// A player right-clicks `block_id`. Only a block whose declaration is
    /// targeted by a hook gets a generated subclass to dispatch from.
    OnBlockUsed { block_id: String },
    /// `block_id`'s random tick. The exporter turns on the declaration's
    /// random ticking when this hook exists.
    OnBlockTick { block_id: String },
    /// A player right-clicks holding `item_id`.
    OnItemUsed { item_id: String },
    /// Every tick of every `entity_id` instance.
    OnEntityTick { entity_id: String },
    /// Any living entity takes damage. `event amount` reports how much.
    OnEntityHurt,
    /// Any living entity dies.
    OnEntityDeath,
    /// A registered `/name` command runs.
    OnCommand { name: String, op_only: bool },
    /// The header of a custom ("My Blocks") definition - its body is the rest
    /// of the strand, and `block_id` is the [`BlockDef`] it belongs to.
    BlockHeader { block_id: String },

    // ── Commands ──────────────────────────────────────────────────────────
    Message {
        target: Target,
        kind: MessageKind,
        text: Value,
    },
    Broadcast {
        text: Value,
    },
    LogInfo {
        text: Value,
    },
    /// Runs `command` as the server console.
    RunCommand {
        command: Value,
    },
    SetBlockAt {
        x: Value,
        y: Value,
        z: Value,
        block: String,
    },
    BreakBlockAt {
        x: Value,
        y: Value,
        z: Value,
        drop: bool,
    },
    SpawnEntityAt {
        entity: String,
        x: Value,
        y: Value,
        z: Value,
    },
    GiveItem {
        target: Target,
        item: String,
        count: Value,
    },
    PlaySoundAt {
        sound: String,
        x: Value,
        y: Value,
        z: Value,
        volume: Value,
        pitch: Value,
    },
    SpawnParticleAt {
        particle: String,
        x: Value,
        y: Value,
        z: Value,
        count: Value,
    },
    DamageTarget {
        target: Target,
        amount: Value,
    },
    HealTarget {
        target: Target,
        amount: Value,
    },
    AddEffect {
        target: Target,
        effect: String,
        seconds: Value,
        amplifier: Value,
    },
    TeleportTarget {
        target: Target,
        x: Value,
        y: Value,
        z: Value,
    },
    /// Adds to the target's velocity, the way a knockback does.
    PushTarget {
        target: Target,
        x: Value,
        y: Value,
        z: Value,
    },
    ExplodeAt {
        x: Value,
        y: Value,
        z: Value,
        power: Value,
        fire: bool,
    },
    SetTimeOfDay {
        time: Value,
    },
    SetWeather {
        weather: Weather,
        seconds: Value,
    },
    /// Stops the event from happening, where the hook is a cancellable one.
    CancelEvent,

    // ── Variables, lists, dicts ───────────────────────────────────────────
    SetVariable {
        name: String,
        value: Value,
    },
    ChangeVariable {
        name: String,
        value: Value,
    },
    AddToList {
        name: String,
        value: Value,
    },
    DeleteOfList {
        name: String,
        index: Value,
    },
    DeleteAllOfList {
        name: String,
    },
    InsertIntoList {
        name: String,
        index: Value,
        value: Value,
    },
    ReplaceItemOfList {
        name: String,
        index: Value,
        value: Value,
    },
    DictSet {
        name: String,
        key: Value,
        value: Value,
    },
    DictDelete {
        name: String,
        key: Value,
    },
    DictClear {
        name: String,
    },

    // ── Control flow ──────────────────────────────────────────────────────
    If {
        condition: Value,
        body: Body,
    },
    IfElse {
        condition: Value,
        then_body: Body,
        else_body: Body,
    },
    Repeat {
        count: Value,
        body: Body,
    },
    While {
        condition: Value,
        body: Body,
    },
    ForEachPlayer {
        body: Body,
    },
    /// Runs `body` once, `ticks` server ticks from now.
    ScheduleAfter {
        ticks: Value,
        body: Body,
    },
    EscapeLoop,
    ContinueLoop,
    /// A reporter-shaped custom block's result.
    Return {
        value: Value,
    },
    /// Command-position call of a custom block. `args` line up positionally
    /// against the [`BlockDef`]'s inputs.
    CallBlock {
        block_id: String,
        args: Vec<Value>,
    },
}

/// `Any` unless a field is written `name: Bool` in the table below.
macro_rules! slot_type {
    () => {
        InputValueType::Any
    };
    (Bool) => {
        InputValueType::Bool
    };
}

/// Declares, once, which fields of which variants hold a [`Value`], and
/// generates the three lookups over that table that [`BlockKind`] needs: walk
/// them all, resolve one by field id, and report one's declared type.
///
/// A variant with no value fields is simply left out.
macro_rules! value_fields {
    ( $( $variant:ident { $( $field:ident $( : $ty:ident )? ),+ $(,)? } )+ ) => {
        fn walk_own_values(&mut self, f: &mut dyn FnMut(&mut Value, InputValueType)) {
            match self {
                $( McBlock::$variant { $( $field, )+ .. } => {
                    $( f($field, slot_type!($($ty)?)); )+
                } )+
                McBlock::CallBlock { args, .. } => {
                    // Declared types live on the BlockDef, not here; a call
                    // site's blanks are repaired by BlockGraph::migrate_bool_slots.
                    for arg in args {
                        f(arg, InputValueType::Any);
                    }
                }
                _ => {}
            }
        }

        fn own_slot_mut(&mut self, field: &str) -> Option<&mut Value> {
            match self {
                $( McBlock::$variant { $( $field, )+ .. } => match field {
                    $( stringify!($field) => Some($field), )+
                    _ => None,
                } )+
                McBlock::CallBlock { args, .. } => args.get_mut(arg_index(field)?),
                _ => None,
            }
        }

        fn own_slot_type(&self, field: &str) -> Option<InputValueType> {
            match self {
                $( McBlock::$variant { .. } => match field {
                    $( stringify!($field) => Some(slot_type!($($ty)?)), )+
                    _ => None,
                } )+
                _ => None,
            }
        }
    };
}

/// The index an `argN` field id names on a [`McBlock::CallBlock`].
fn arg_index(field: &str) -> Option<usize> {
    field.strip_prefix("arg")?.parse().ok()
}

/// Fields typed as whole numbers, so a typed edit is parsed as an integer
/// rather than left as `3.0`. Matched on field id alone: every one of these
/// names means the same thing wherever it appears.
const INTEGER_FIELDS: &[&str] = &[
    "light",
    "max_stack",
    "count",
    "amplifier",
    "index",
    "ticks",
    "time",
    "seconds",
];

impl McBlock {
    value_fields! {
        RegisterBlock { hardness, resistance, light }
        RegisterItem { max_stack }
        RegisterEntity { width, height, max_health, movement_speed, attack_damage }
        Message { text }
        Broadcast { text }
        LogInfo { text }
        RunCommand { command }
        SetBlockAt { x, y, z }
        BreakBlockAt { x, y, z }
        SpawnEntityAt { x, y, z }
        GiveItem { count }
        PlaySoundAt { x, y, z, volume, pitch }
        SpawnParticleAt { x, y, z, count }
        DamageTarget { amount }
        HealTarget { amount }
        AddEffect { seconds, amplifier }
        TeleportTarget { x, y, z }
        PushTarget { x, y, z }
        ExplodeAt { x, y, z, power }
        SetTimeOfDay { time }
        SetWeather { seconds }
        SetVariable { value }
        ChangeVariable { value }
        AddToList { value }
        DeleteOfList { index }
        InsertIntoList { index, value }
        ReplaceItemOfList { index, value }
        DictSet { key, value }
        DictDelete { key }
        If { condition: Bool }
        IfElse { condition: Bool }
        Repeat { count }
        While { condition: Bool }
        ScheduleAfter { ticks }
        Return { value }
    }

    /// The registry id this declaration defines, or `None` for anything that
    /// is not a declaration.
    pub fn declared_id(&self) -> Option<&str> {
        match self {
            McBlock::RegisterBlock { block_id, .. } => Some(block_id),
            McBlock::RegisterItem { item_id, .. } => Some(item_id),
            McBlock::RegisterEntity { entity_id, .. } => Some(entity_id),
            McBlock::RegisterSound { sound_id, .. } => Some(sound_id),
            McBlock::RegisterCreativeTab { tab_id, .. } => Some(tab_id),
            _ => None,
        }
    }

    /// True for the `Register*` family - header blocks that declare a registry
    /// entry rather than run anything.
    pub fn is_declaration(&self) -> bool {
        self.declared_id().is_some()
    }

    /// True for the `On*` family plus [`McBlock::BlockHeader`] - the headers
    /// whose strand tail is a body of code.
    pub fn is_hook(&self) -> bool {
        self.is_header() && !self.is_declaration()
    }
}

impl BlockKind for McBlock {
    /// Two, for [`McBlock::IfElse`]. Nothing here nests deeper in one block.
    const BODY_SLOTS: u8 = 2;

    const HEADER_LABEL: &'static str = "event";

    fn visit_values_mut(&mut self, f: &mut dyn FnMut(&mut Value, InputValueType)) {
        self.walk_own_values(f);
    }

    fn is_header(&self) -> bool {
        matches!(
            self,
            McBlock::RegisterBlock { .. }
                | McBlock::RegisterItem { .. }
                | McBlock::RegisterEntity { .. }
                | McBlock::RegisterSound { .. }
                | McBlock::RegisterCreativeTab { .. }
                | McBlock::OnModInit
                | McBlock::OnClientInit
                | McBlock::OnServerStarted
                | McBlock::OnServerStopping
                | McBlock::OnServerTick
                | McBlock::OnLevelTick
                | McBlock::OnPlayerJoin
                | McBlock::OnPlayerLeave
                | McBlock::OnBlockPlaced { .. }
                | McBlock::OnBlockBroken { .. }
                | McBlock::OnBlockUsed { .. }
                | McBlock::OnBlockTick { .. }
                | McBlock::OnItemUsed { .. }
                | McBlock::OnEntityTick { .. }
                | McBlock::OnEntityHurt
                | McBlock::OnEntityDeath
                | McBlock::OnCommand { .. }
                | McBlock::BlockHeader { .. }
        )
    }

    fn body(&self, slot: u8) -> Option<&Body> {
        match (self, slot) {
            (McBlock::If { body, .. }, 0)
            | (McBlock::Repeat { body, .. }, 0)
            | (McBlock::While { body, .. }, 0)
            | (McBlock::ForEachPlayer { body }, 0)
            | (McBlock::ScheduleAfter { body, .. }, 0) => Some(body),
            (McBlock::IfElse { then_body, .. }, 0) => Some(then_body),
            (McBlock::IfElse { else_body, .. }, 1) => Some(else_body),
            _ => None,
        }
    }

    fn body_mut(&mut self, slot: u8) -> Option<&mut Body> {
        match (self, slot) {
            (McBlock::If { body, .. }, 0)
            | (McBlock::Repeat { body, .. }, 0)
            | (McBlock::While { body, .. }, 0)
            | (McBlock::ForEachPlayer { body }, 0)
            | (McBlock::ScheduleAfter { body, .. }, 0) => Some(body),
            (McBlock::IfElse { then_body, .. }, 0) => Some(then_body),
            (McBlock::IfElse { else_body, .. }, 1) => Some(else_body),
            _ => None,
        }
    }

    fn variable_target_mut(&mut self) -> Option<&mut String> {
        match self {
            McBlock::SetVariable { name, .. } | McBlock::ChangeVariable { name, .. } => Some(name),
            _ => None,
        }
    }

    fn list_target_mut(&mut self) -> Option<&mut String> {
        match self {
            McBlock::AddToList { name, .. }
            | McBlock::DeleteOfList { name, .. }
            | McBlock::DeleteAllOfList { name }
            | McBlock::InsertIntoList { name, .. }
            | McBlock::ReplaceItemOfList { name, .. } => Some(name),
            _ => None,
        }
    }

    fn dict_target_mut(&mut self) -> Option<&mut String> {
        match self {
            McBlock::DictSet { name, .. }
            | McBlock::DictDelete { name, .. }
            | McBlock::DictClear { name } => Some(name),
            _ => None,
        }
    }

    fn calls_block(&self, block_id: &str) -> bool {
        matches!(self, McBlock::CallBlock { block_id: id, .. } if id == block_id)
    }

    fn call_args_mut(&mut self, block_id: &str) -> Option<&mut Vec<Value>> {
        match self {
            McBlock::CallBlock { block_id: id, args } if id == block_id => Some(args),
            _ => None,
        }
    }

    fn block_header_id(&self) -> Option<&str> {
        match self {
            McBlock::BlockHeader { block_id } => Some(block_id),
            _ => None,
        }
    }

    fn value_slot_mut(&mut self, field: &str) -> Option<&mut Value> {
        self.own_slot_mut(field)
    }

    fn blank_field_value(&self, field: &str, blocks: &[BlockDef]) -> Option<Value> {
        if let McBlock::CallBlock { block_id, .. } = self {
            let index = arg_index(field)?;
            let declared = blocks
                .iter()
                .find(|def| def.id == *block_id)?
                .input_types()
                .nth(index)?;
            return (declared == InputValueType::Bool).then_some(Value::Bool);
        }
        match self.own_slot_type(field)? {
            InputValueType::Bool => Some(Value::Bool),
            // `None` means "a plain zero", which is what an `Any` slot wants.
            InputValueType::Any => None,
        }
    }

    fn field_requires_integer(&self, field: &str) -> bool {
        INTEGER_FIELDS.contains(&field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockstitch_core::graph::{BlockPiece, BlockShape};

    fn number(n: f64) -> Value {
        Value::number(n)
    }

    #[test]
    fn hooks_and_declarations_are_both_headers() {
        assert!(McBlock::OnServerTick.is_header());
        assert!(McBlock::OnServerTick.is_hook());
        let declaration = McBlock::RegisterSound {
            sound_id: "chime".into(),
            subtitle: "A chime".into(),
        };
        assert!(declaration.is_header());
        assert!(declaration.is_declaration());
        assert!(!declaration.is_hook());
        assert!(!McBlock::CancelEvent.is_header());
    }

    #[test]
    fn field_ids_resolve_the_slot_they_name() {
        let mut block = McBlock::PlaySoundAt {
            sound: "chime".into(),
            x: number(1.0),
            y: number(2.0),
            z: number(3.0),
            volume: number(1.0),
            pitch: number(1.0),
        };
        *block.value_slot_mut("y").expect("y is a slot") = number(64.0);
        assert_eq!(block.value_slot_mut("y").cloned(), Some(number(64.0)));
        assert!(block.value_slot_mut("nope").is_none());
    }

    #[test]
    fn boolean_slots_are_declared_as_such() {
        let branch = McBlock::If {
            condition: Value::Bool,
            body: vec![],
        };
        assert_eq!(branch.blank_field_value("condition", &[]), Some(Value::Bool));
        let repeat = McBlock::Repeat {
            count: number(10.0),
            body: vec![],
        };
        // An `Any` slot blanks to a plain zero, which the trait spells `None`.
        assert_eq!(repeat.blank_field_value("count", &[]), None);
    }

    #[test]
    fn visit_values_reaches_every_declared_slot() {
        let mut block = McBlock::ExplodeAt {
            x: number(1.0),
            y: number(2.0),
            z: number(3.0),
            power: number(4.0),
            fire: false,
        };
        let mut seen = 0;
        block.visit_values_mut(&mut |_, _| seen += 1);
        assert_eq!(seen, 4);
    }

    #[test]
    fn call_sites_address_their_arguments_by_index() {
        let mut call = McBlock::CallBlock {
            block_id: "def".into(),
            args: vec![number(1.0), Value::Bool],
        };
        assert_eq!(call.value_slot_mut("arg1").cloned(), Some(Value::Bool));
        assert!(call.value_slot_mut("arg7").is_none());
        assert!(call.calls_block("def"));
        assert!(!call.calls_block("other"));

        let def = BlockDef {
            id: "def".into(),
            pieces: vec![
                BlockPiece::Label {
                    id: "l".into(),
                    text: "do".into(),
                },
                BlockPiece::Input {
                    id: "i0".into(),
                    name: "amount".into(),
                    value_type: InputValueType::Any,
                },
                BlockPiece::Input {
                    id: "i1".into(),
                    name: "loudly".into(),
                    value_type: InputValueType::Bool,
                },
            ],
            shape: BlockShape::Normal,
            color: "#4C97FF".into(),
        };
        assert_eq!(call.blank_field_value("arg0", &[def.clone()]), None);
        assert_eq!(
            call.blank_field_value("arg1", &[def]),
            Some(Value::Bool),
            "a boolean input blanks back to an empty hexagon"
        );
    }

    #[test]
    fn nested_bodies_are_reachable_by_slot() {
        let mut branch = McBlock::IfElse {
            condition: Value::Bool,
            then_body: vec![Instruction::new(McBlock::CancelEvent)],
            else_body: vec![],
        };
        assert_eq!(branch.body(0).map(Vec::len), Some(1));
        assert_eq!(branch.body(1).map(Vec::len), Some(0));
        assert!(branch.body(2).is_none());
        branch
            .body_mut(1)
            .expect("else body")
            .push(Instruction::new(McBlock::EscapeLoop));
        assert_eq!(branch.body(1).map(Vec::len), Some(1));
    }

    #[test]
    fn the_type_tag_is_the_wire_discriminator() {
        let json = serde_json::to_value(&McBlock::OnBlockUsed {
            block_id: "chime_block".into(),
        })
        .unwrap();
        assert_eq!(json["type"], "OnBlockUsed");
        assert_eq!(json["block_id"], "chime_block");
    }
}
