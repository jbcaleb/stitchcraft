//! How every [`McBlock`] draws and what a fresh one holds - the data behind
//! the QML canvas's `BlockRegistry.registerRows` and `registerPrefabs` calls.
//!
//! Rows are declared here rather than in QML so that the enum and its drawing
//! cannot drift apart: [`check::rows_cover_every_type`] fails the test suite
//! the moment a variant is added without a row. Prefabs are real [`McBlock`]
//! values serialized through serde, so a fresh palette block is type-checked
//! by construction.
//!
//! Two piece features need a QML-side function that JSON cannot carry, so they
//! are declared here as data and substituted by `Stitchcraft/BlockRows.qml`:
//! [`Options::Source`] (choices that depend on what the document declares) and
//! [`Options::YesNo`] (a boolean field, which has no checkbox piece kind).

use crate::block::McBlock;
use crate::kinds::{
    EntityBase, MaterialPreset, MessageKind, MobCategoryKind, Rarity, Target, Weather,
};
use blockstitch_core::value::Value;
use serde_json::{Map, json};

/// A block's outline. `Stack` is the ordinary notched instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Stack,
    /// Flat top, must be first in its strand.
    Header,
    /// No bottom notch - nothing stacks below.
    Cap,
}

/// Where a dropdown's choices come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Options {
    /// A fixed `[(value, label)]` table.
    Fixed(&'static [(&'static str, &'static str)]),
    /// The document's list names.
    Lists,
    /// The document's dict names.
    Dicts,
    /// A named set the host recomputes as the document changes - see
    /// [`DYNAMIC_SOURCES`].
    Source(&'static str),
    /// A boolean field, drawn as a yes/no dropdown.
    YesNo,
}

/// One piece of a block's row.
#[derive(Debug, Clone, Copy)]
pub enum Piece {
    /// Static text.
    Label(&'static str),
    /// A value slot. `field` is both the field id the editor addresses and the
    /// instruction key it reads, which are always the same in this vocabulary.
    Slot {
        field: &'static str,
        /// True for a boolean slot, drawn as a hexagon.
        boolean: bool,
    },
    Dropdown {
        key: &'static str,
        options: Options,
        placeholder: &'static str,
    },
    /// A free-text field, for the ids Stitchcraft cannot enumerate (vanilla
    /// blocks, items, particles, effects).
    Text {
        key: &'static str,
        placeholder: &'static str,
    },
}

/// The dynamic choice sets a [`Options::Source`] may name. The host answers
/// each one from the current document; `BlockRows.qml` turns them into the
/// functions `BlockRegistry` expects.
pub const DYNAMIC_SOURCES: &[&str] = &[
    "declaredBlocks",
    "declaredItems",
    "declaredEntities",
    "declaredSounds",
    "declaredTabs",
];

/// How one instruction type draws.
pub struct Row {
    pub kind: &'static str,
    pub icon: &'static str,
    pub shape: Shape,
    pub head: &'static [Piece],
    /// The instruction keys holding nested bodies, in slot order.
    pub mouths: &'static [&'static str],
    /// Label drawn between one mouth and the next.
    pub separators: &'static [&'static str],
}

/// A palette section: a heading and the types under it, in the order the
/// sidebar shows them.
pub struct Section {
    pub title: &'static str,
    pub types: &'static [&'static str],
}

// ── Fixed dropdown tables ─────────────────────────────────────────────────
// The `value` in each pair is the serde name of the matching variant in
// `crate::kinds`, so a rename there is a save-format break in both places.

const TARGETS: &[(&str, &str)] = &[
    ("EventPlayer", "the event player"),
    ("EventEntity", "the event entity"),
    ("AllPlayers", "every player"),
    ("NearestPlayer", "the nearest player"),
];

const MESSAGE_KINDS: &[(&str, &str)] = &[("Chat", "in chat"), ("ActionBar", "on the action bar")];

const MATERIALS: &[(&str, &str)] = &[
    ("Stone", "stone"),
    ("Wood", "wood"),
    ("Metal", "metal"),
    ("Glass", "glass"),
    ("Wool", "wool"),
    ("Dirt", "dirt"),
    ("Sand", "sand"),
    ("Plant", "plant"),
];

const RARITIES: &[(&str, &str)] = &[
    ("Common", "common"),
    ("Uncommon", "uncommon"),
    ("Rare", "rare"),
    ("Epic", "epic"),
];

const CATEGORIES: &[(&str, &str)] = &[
    ("Creature", "creature"),
    ("Monster", "monster"),
    ("Ambient", "ambient"),
    ("WaterCreature", "water creature"),
    ("Misc", "misc"),
];

const ENTITY_BASES: &[(&str, &str)] = &[
    ("Passive", "passive"),
    ("Hostile", "hostile"),
    ("Flying", "flying"),
];

const WEATHERS: &[(&str, &str)] = &[
    ("Clear", "clear"),
    ("Rain", "rain"),
    ("Thunder", "thunder"),
];

// ── Row shorthands ────────────────────────────────────────────────────────

const fn label(text: &'static str) -> Piece {
    Piece::Label(text)
}

const fn slot(field: &'static str) -> Piece {
    Piece::Slot {
        field,
        boolean: false,
    }
}

const fn boolean_slot(field: &'static str) -> Piece {
    Piece::Slot {
        field,
        boolean: true,
    }
}

const fn text(key: &'static str, placeholder: &'static str) -> Piece {
    Piece::Text { key, placeholder }
}

const fn choose(
    key: &'static str,
    options: Options,
    placeholder: &'static str,
) -> Piece {
    Piece::Dropdown {
        key,
        options,
        placeholder,
    }
}

const fn yes_no(key: &'static str) -> Piece {
    Piece::Dropdown {
        key,
        options: Options::YesNo,
        placeholder: "no",
    }
}

const fn row(kind: &'static str, icon: &'static str, head: &'static [Piece]) -> Row {
    Row {
        kind,
        icon,
        shape: Shape::Stack,
        head,
        mouths: &[],
        separators: &[],
    }
}

const fn header(kind: &'static str, icon: &'static str, head: &'static [Piece]) -> Row {
    Row {
        kind,
        icon,
        shape: Shape::Header,
        head,
        mouths: &[],
        separators: &[],
    }
}

const fn cap(kind: &'static str, icon: &'static str, head: &'static [Piece]) -> Row {
    Row {
        kind,
        icon,
        shape: Shape::Cap,
        head,
        mouths: &[],
        separators: &[],
    }
}

const fn wrap(
    kind: &'static str,
    icon: &'static str,
    head: &'static [Piece],
    mouths: &'static [&'static str],
    separators: &'static [&'static str],
) -> Row {
    Row {
        kind,
        icon,
        shape: Shape::Stack,
        head,
        mouths,
        separators,
    }
}

/// Every instruction type's row. One entry per [`McBlock`] variant.
pub const ROWS: &[Row] = &[
    // ── Declarations ──────────────────────────────────────────────────────
    header(
        "RegisterBlock",
        "box",
        &[
            label("register block"),
            text("block_id", "block id"),
            label("named"),
            text("display_name", "Display Name"),
            label("as"),
            choose("material", Options::Fixed(MATERIALS), "stone"),
            label("hardness"),
            slot("hardness"),
            label("blast resistance"),
            slot("resistance"),
            label("light"),
            slot("light"),
            label("needs a tool"),
            yes_no("requires_tool"),
            label("drops itself"),
            yes_no("drops_self"),
            label("give an item"),
            yes_no("give_item"),
            label("in tab"),
            choose("creative_tab", Options::Source("declaredTabs"), "main"),
        ],
    ),
    header(
        "RegisterItem",
        "package",
        &[
            label("register item"),
            text("item_id", "item id"),
            label("named"),
            text("display_name", "Display Name"),
            label("stacks to"),
            slot("max_stack"),
            label("rarity"),
            choose("rarity", Options::Fixed(RARITIES), "common"),
            label("in tab"),
            choose("creative_tab", Options::Source("declaredTabs"), "main"),
        ],
    ),
    header(
        "RegisterEntity",
        "ghost",
        &[
            label("register entity"),
            text("entity_id", "entity id"),
            label("named"),
            text("display_name", "Display Name"),
            label("as a"),
            choose("base", Options::Fixed(ENTITY_BASES), "passive"),
            choose("category", Options::Fixed(CATEGORIES), "creature"),
            label("size"),
            slot("width"),
            label("x"),
            slot("height"),
            label("health"),
            slot("max_health"),
            label("speed"),
            slot("movement_speed"),
            label("attack"),
            slot("attack_damage"),
            label("spawn egg"),
            yes_no("spawn_egg"),
        ],
    ),
    header(
        "RegisterSound",
        "volume-2",
        &[
            label("register sound"),
            text("sound_id", "sound id"),
            label("subtitled"),
            text("subtitle", "Subtitle"),
        ],
    ),
    header(
        "RegisterCreativeTab",
        "layout-grid",
        &[
            label("register creative tab"),
            text("tab_id", "tab id"),
            label("named"),
            text("display_name", "Display Name"),
            label("showing"),
            text("icon", "item id"),
        ],
    ),
    // ── Event hooks ───────────────────────────────────────────────────────
    header("OnModInit", "play", &[label("when the mod loads")]),
    header("OnClientInit", "monitor", &[label("when the client loads")]),
    header("OnServerStarted", "boxes", &[label("when the server starts")]),
    header("OnServerStopping", "monitor-x", &[label("when the server stops")]),
    header("OnServerTick", "clock", &[label("every server tick")]),
    header("OnLevelTick", "layers", &[label("every level tick")]),
    header("OnPlayerJoin", "user", &[label("when a player joins")]),
    header("OnPlayerLeave", "user", &[label("when a player leaves")]),
    header(
        "OnBlockPlaced",
        "hammer",
        &[
            label("when"),
            text("block_id", "any block"),
            label("is placed"),
        ],
    ),
    header(
        "OnBlockBroken",
        "hammer",
        &[
            label("when"),
            text("block_id", "any block"),
            label("is broken"),
        ],
    ),
    header(
        "OnBlockUsed",
        "hand",
        &[
            label("when"),
            choose("block_id", Options::Source("declaredBlocks"), "a block"),
            label("is right-clicked"),
        ],
    ),
    header(
        "OnBlockTick",
        "shapes",
        &[
            label("on"),
            choose("block_id", Options::Source("declaredBlocks"), "a block"),
            label("random tick"),
        ],
    ),
    header(
        "OnItemUsed",
        "mouse-pointer-click",
        &[
            label("when"),
            choose("item_id", Options::Source("declaredItems"), "an item"),
            label("is used"),
        ],
    ),
    header(
        "OnEntityTick",
        "zap",
        &[
            label("every tick of"),
            choose("entity_id", Options::Source("declaredEntities"), "an entity"),
        ],
    ),
    header("OnEntityHurt", "triangle-alert", &[label("when an entity is hurt")]),
    header("OnEntityDeath", "ghost", &[label("when an entity dies")]),
    header(
        "OnCommand",
        "terminal",
        &[
            label("on command /"),
            text("name", "name"),
            label("operators only"),
            yes_no("op_only"),
        ],
    ),
    header("BlockHeader", "blocks", &[label("define")]),
    // ── Commands ──────────────────────────────────────────────────────────
    row(
        "Message",
        "message-square",
        &[
            label("tell"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            slot("text"),
            choose("kind", Options::Fixed(MESSAGE_KINDS), "in chat"),
        ],
    ),
    row("Broadcast", "volume-2", &[label("announce"), slot("text")]),
    row("LogInfo", "file-text", &[label("log"), slot("text")]),
    row("RunCommand", "terminal", &[label("run command"), slot("command")]),
    row(
        "SetBlockAt",
        "layers",
        &[
            label("set block at"),
            slot("x"),
            slot("y"),
            slot("z"),
            label("to"),
            text("block", "minecraft:stone"),
        ],
    ),
    row(
        "BreakBlockAt",
        "hammer",
        &[
            label("break block at"),
            slot("x"),
            slot("y"),
            slot("z"),
            label("dropping items"),
            yes_no("drop"),
        ],
    ),
    row(
        "SpawnEntityAt",
        "ghost",
        &[
            label("spawn"),
            text("entity", "minecraft:pig"),
            label("at"),
            slot("x"),
            slot("y"),
            slot("z"),
        ],
    ),
    row(
        "GiveItem",
        "package",
        &[
            label("give"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            slot("count"),
            label("x"),
            text("item", "minecraft:diamond"),
        ],
    ),
    row(
        "PlaySoundAt",
        "volume-2",
        &[
            label("play sound"),
            text("sound", "minecraft:entity.player.levelup"),
            label("at"),
            slot("x"),
            slot("y"),
            slot("z"),
            label("volume"),
            slot("volume"),
            label("pitch"),
            slot("pitch"),
        ],
    ),
    row(
        "SpawnParticleAt",
        "sparkles",
        &[
            label("spawn"),
            slot("count"),
            text("particle", "minecraft:flame"),
            label("at"),
            slot("x"),
            slot("y"),
            slot("z"),
        ],
    ),
    row(
        "DamageTarget",
        "target",
        &[
            label("damage"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            label("by"),
            slot("amount"),
        ],
    ),
    row(
        "HealTarget",
        "plus",
        &[
            label("heal"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            label("by"),
            slot("amount"),
        ],
    ),
    row(
        "AddEffect",
        "pipette",
        &[
            label("give"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            text("effect", "minecraft:speed"),
            label("for"),
            slot("seconds"),
            label("seconds at level"),
            slot("amplifier"),
        ],
    ),
    row(
        "TeleportTarget",
        "move-3d",
        &[
            label("teleport"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            label("to"),
            slot("x"),
            slot("y"),
            slot("z"),
        ],
    ),
    row(
        "PushTarget",
        "wind",
        &[
            label("push"),
            choose("target", Options::Fixed(TARGETS), "the event player"),
            label("by"),
            slot("x"),
            slot("y"),
            slot("z"),
        ],
    ),
    row(
        "ExplodeAt",
        "zap",
        &[
            label("explode at"),
            slot("x"),
            slot("y"),
            slot("z"),
            label("with power"),
            slot("power"),
            label("setting fires"),
            yes_no("fire"),
        ],
    ),
    row("SetTimeOfDay", "sun", &[label("set time of day to"), slot("time")]),
    row(
        "SetWeather",
        "cloud",
        &[
            label("set weather to"),
            choose("weather", Options::Fixed(WEATHERS), "clear"),
            label("for"),
            slot("seconds"),
            label("seconds"),
        ],
    ),
    row("CancelEvent", "x", &[label("cancel the event")]),
    // ── Variables, lists, dicts ───────────────────────────────────────────
    row(
        "SetVariable",
        "equal",
        &[
            label("set"),
            choose("name", Options::Source("variables"), "variable"),
            label("to"),
            slot("value"),
        ],
    ),
    row(
        "ChangeVariable",
        "trending-up",
        &[
            label("change"),
            choose("name", Options::Source("variables"), "variable"),
            label("by"),
            slot("value"),
        ],
    ),
    row(
        "AddToList",
        "plus",
        &[
            label("add"),
            slot("value"),
            label("to"),
            choose("name", Options::Lists, "list"),
        ],
    ),
    row(
        "DeleteOfList",
        "trash",
        &[
            label("delete item"),
            slot("index"),
            label("of"),
            choose("name", Options::Lists, "list"),
        ],
    ),
    row(
        "DeleteAllOfList",
        "trash",
        &[
            label("delete all of"),
            choose("name", Options::Lists, "list"),
        ],
    ),
    row(
        "InsertIntoList",
        "plus",
        &[
            label("insert"),
            slot("value"),
            label("at"),
            slot("index"),
            label("of"),
            choose("name", Options::Lists, "list"),
        ],
    ),
    row(
        "ReplaceItemOfList",
        "repeat",
        &[
            label("replace item"),
            slot("index"),
            label("of"),
            choose("name", Options::Lists, "list"),
            label("with"),
            slot("value"),
        ],
    ),
    row(
        "DictSet",
        "equal",
        &[
            label("set key"),
            slot("key"),
            label("of"),
            choose("name", Options::Dicts, "dict"),
            label("to"),
            slot("value"),
        ],
    ),
    row(
        "DictDelete",
        "trash",
        &[
            label("delete key"),
            slot("key"),
            label("of"),
            choose("name", Options::Dicts, "dict"),
        ],
    ),
    row(
        "DictClear",
        "trash",
        &[label("clear"), choose("name", Options::Dicts, "dict")],
    ),
    // ── Control flow ──────────────────────────────────────────────────────
    wrap(
        "If",
        "git-branch",
        &[label("if"), boolean_slot("condition"), label("then")],
        &["body"],
        &[],
    ),
    wrap(
        "IfElse",
        "git-fork",
        &[label("if"), boolean_slot("condition"), label("then")],
        &["then_body", "else_body"],
        &["else"],
    ),
    wrap(
        "Repeat",
        "repeat",
        &[label("repeat"), slot("count")],
        &["body"],
        &[],
    ),
    wrap(
        "While",
        "rotate-cw",
        &[label("while"), boolean_slot("condition")],
        &["body"],
        &[],
    ),
    wrap(
        "ForEachPlayer",
        "users",
        &[label("for each player")],
        &["body"],
        &[],
    ),
    wrap(
        "ScheduleAfter",
        "clock",
        &[label("after"), slot("ticks"), label("ticks")],
        &["body"],
        &[],
    ),
    cap("EscapeLoop", "log-out", &[label("break out of the loop")]),
    cap("ContinueLoop", "skip-forward", &[label("next loop pass")]),
    cap("Return", "undo", &[label("report"), slot("value")]),
    row("CallBlock", "blocks", &[]),
];

/// The palette, top to bottom.
pub const SECTIONS: &[Section] = &[
    Section {
        title: "Registry",
        types: &[
            "RegisterBlock",
            "RegisterItem",
            "RegisterEntity",
            "RegisterSound",
            "RegisterCreativeTab",
        ],
    },
    Section {
        title: "Lifecycle",
        types: &[
            "OnModInit",
            "OnClientInit",
            "OnServerStarted",
            "OnServerStopping",
            "OnServerTick",
            "OnLevelTick",
            "OnCommand",
        ],
    },
    Section {
        title: "World events",
        types: &[
            "OnBlockPlaced",
            "OnBlockBroken",
            "OnBlockUsed",
            "OnBlockTick",
            "OnItemUsed",
        ],
    },
    Section {
        title: "Entity events",
        types: &[
            "OnPlayerJoin",
            "OnPlayerLeave",
            "OnEntityTick",
            "OnEntityHurt",
            "OnEntityDeath",
        ],
    },
    Section {
        title: "Talk",
        types: &["Message", "Broadcast", "LogInfo", "RunCommand"],
    },
    Section {
        title: "World",
        types: &[
            "SetBlockAt",
            "BreakBlockAt",
            "SpawnEntityAt",
            "PlaySoundAt",
            "SpawnParticleAt",
            "ExplodeAt",
            "SetTimeOfDay",
            "SetWeather",
        ],
    },
    Section {
        title: "Entities",
        types: &[
            "GiveItem",
            "DamageTarget",
            "HealTarget",
            "AddEffect",
            "TeleportTarget",
            "PushTarget",
        ],
    },
    Section {
        title: "Control",
        types: &[
            "If",
            "IfElse",
            "Repeat",
            "While",
            "ForEachPlayer",
            "ScheduleAfter",
            "EscapeLoop",
            "ContinueLoop",
            "Return",
            "CancelEvent",
        ],
    },
];

/// The list-manipulation commands the palette shows beside the document's
/// lists, as `PalettePanel.listInstructionTypes` wants them.
pub const LIST_TYPES: &[&str] = &[
    "AddToList",
    "DeleteOfList",
    "DeleteAllOfList",
    "InsertIntoList",
    "ReplaceItemOfList",
];

/// The dict-manipulation commands, as `PalettePanel.dictInstructionTypes`
/// wants them.
pub const DICT_TYPES: &[&str] = &["DictSet", "DictDelete", "DictClear"];

/// A fresh instruction of `kind`, as the palette hands it to a drag.
///
/// Returning a real [`McBlock`] rather than a JSON literal is what keeps the
/// palette honest: a field renamed on the enum stops compiling here.
pub fn prefab(kind: &str) -> Option<McBlock> {
    let number = Value::number;
    let text = Value::text;
    Some(match kind {
        // ── Declarations ──────────────────────────────────────────────────
        "RegisterBlock" => McBlock::RegisterBlock {
            block_id: String::new(),
            display_name: String::new(),
            material: MaterialPreset::Stone,
            hardness: number(1.5),
            resistance: number(6.0),
            light: number(0.0),
            requires_tool: true,
            drops_self: true,
            give_item: true,
            creative_tab: String::new(),
        },
        "RegisterItem" => McBlock::RegisterItem {
            item_id: String::new(),
            display_name: String::new(),
            max_stack: number(64.0),
            rarity: Rarity::Common,
            creative_tab: String::new(),
        },
        "RegisterEntity" => McBlock::RegisterEntity {
            entity_id: String::new(),
            display_name: String::new(),
            base: EntityBase::Passive,
            category: MobCategoryKind::Creature,
            width: number(0.8),
            height: number(1.4),
            max_health: number(10.0),
            movement_speed: number(0.25),
            attack_damage: number(2.0),
            spawn_egg: true,
        },
        "RegisterSound" => McBlock::RegisterSound {
            sound_id: String::new(),
            subtitle: String::new(),
        },
        "RegisterCreativeTab" => McBlock::RegisterCreativeTab {
            tab_id: "main".to_string(),
            display_name: String::new(),
            icon: String::new(),
        },
        // ── Event hooks ───────────────────────────────────────────────────
        "OnModInit" => McBlock::OnModInit,
        "OnClientInit" => McBlock::OnClientInit,
        "OnServerStarted" => McBlock::OnServerStarted,
        "OnServerStopping" => McBlock::OnServerStopping,
        "OnServerTick" => McBlock::OnServerTick,
        "OnLevelTick" => McBlock::OnLevelTick,
        "OnPlayerJoin" => McBlock::OnPlayerJoin,
        "OnPlayerLeave" => McBlock::OnPlayerLeave,
        "OnBlockPlaced" => McBlock::OnBlockPlaced {
            block_id: String::new(),
        },
        "OnBlockBroken" => McBlock::OnBlockBroken {
            block_id: String::new(),
        },
        "OnBlockUsed" => McBlock::OnBlockUsed {
            block_id: String::new(),
        },
        "OnBlockTick" => McBlock::OnBlockTick {
            block_id: String::new(),
        },
        "OnItemUsed" => McBlock::OnItemUsed {
            item_id: String::new(),
        },
        "OnEntityTick" => McBlock::OnEntityTick {
            entity_id: String::new(),
        },
        "OnEntityHurt" => McBlock::OnEntityHurt,
        "OnEntityDeath" => McBlock::OnEntityDeath,
        "OnCommand" => McBlock::OnCommand {
            name: String::new(),
            op_only: false,
        },
        // `BlockHeader` is never dragged from the palette - the Make a Block
        // dialog creates it along with its strand.
        "BlockHeader" => McBlock::BlockHeader {
            block_id: String::new(),
        },
        // ── Commands ──────────────────────────────────────────────────────
        "Message" => McBlock::Message {
            target: Target::EventPlayer,
            kind: MessageKind::Chat,
            text: text("Hello!"),
        },
        "Broadcast" => McBlock::Broadcast {
            text: text("Hello, everyone!"),
        },
        "LogInfo" => McBlock::LogInfo {
            text: text("here"),
        },
        "RunCommand" => McBlock::RunCommand {
            command: text("time set day"),
        },
        "SetBlockAt" => McBlock::SetBlockAt {
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
            block: "minecraft:stone".to_string(),
        },
        "BreakBlockAt" => McBlock::BreakBlockAt {
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
            drop: true,
        },
        "SpawnEntityAt" => McBlock::SpawnEntityAt {
            entity: "minecraft:pig".to_string(),
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
        },
        "GiveItem" => McBlock::GiveItem {
            target: Target::EventPlayer,
            item: "minecraft:diamond".to_string(),
            count: number(1.0),
        },
        "PlaySoundAt" => McBlock::PlaySoundAt {
            sound: "minecraft:entity.player.levelup".to_string(),
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
            volume: number(1.0),
            pitch: number(1.0),
        },
        "SpawnParticleAt" => McBlock::SpawnParticleAt {
            particle: "minecraft:flame".to_string(),
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
            count: number(8.0),
        },
        "DamageTarget" => McBlock::DamageTarget {
            target: Target::EventPlayer,
            amount: number(1.0),
        },
        "HealTarget" => McBlock::HealTarget {
            target: Target::EventPlayer,
            amount: number(1.0),
        },
        "AddEffect" => McBlock::AddEffect {
            target: Target::EventPlayer,
            effect: "minecraft:speed".to_string(),
            seconds: number(10.0),
            amplifier: number(0.0),
        },
        "TeleportTarget" => McBlock::TeleportTarget {
            target: Target::EventPlayer,
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
        },
        "PushTarget" => McBlock::PushTarget {
            target: Target::EventPlayer,
            x: number(0.0),
            y: number(0.5),
            z: number(0.0),
        },
        "ExplodeAt" => McBlock::ExplodeAt {
            x: number(0.0),
            y: number(64.0),
            z: number(0.0),
            power: number(2.0),
            fire: false,
        },
        "SetTimeOfDay" => McBlock::SetTimeOfDay {
            time: number(1000.0),
        },
        "SetWeather" => McBlock::SetWeather {
            weather: Weather::Rain,
            seconds: number(300.0),
        },
        "CancelEvent" => McBlock::CancelEvent,
        // ── Variables, lists, dicts ───────────────────────────────────────
        // The palette rewrites `name` to the document's first variable, list
        // or dict before the drag starts.
        "SetVariable" => McBlock::SetVariable {
            name: String::new(),
            value: number(0.0),
        },
        "ChangeVariable" => McBlock::ChangeVariable {
            name: String::new(),
            value: number(1.0),
        },
        "AddToList" => McBlock::AddToList {
            name: String::new(),
            value: text("thing"),
        },
        "DeleteOfList" => McBlock::DeleteOfList {
            name: String::new(),
            index: number(1.0),
        },
        "DeleteAllOfList" => McBlock::DeleteAllOfList {
            name: String::new(),
        },
        "InsertIntoList" => McBlock::InsertIntoList {
            name: String::new(),
            index: number(1.0),
            value: text("thing"),
        },
        "ReplaceItemOfList" => McBlock::ReplaceItemOfList {
            name: String::new(),
            index: number(1.0),
            value: text("thing"),
        },
        "DictSet" => McBlock::DictSet {
            name: String::new(),
            key: text("key"),
            value: text("value"),
        },
        "DictDelete" => McBlock::DictDelete {
            name: String::new(),
            key: text("key"),
        },
        "DictClear" => McBlock::DictClear {
            name: String::new(),
        },
        // ── Control flow ──────────────────────────────────────────────────
        "If" => McBlock::If {
            condition: Value::Bool,
            body: Vec::new(),
        },
        "IfElse" => McBlock::IfElse {
            condition: Value::Bool,
            then_body: Vec::new(),
            else_body: Vec::new(),
        },
        "Repeat" => McBlock::Repeat {
            count: number(10.0),
            body: Vec::new(),
        },
        "While" => McBlock::While {
            condition: Value::Bool,
            body: Vec::new(),
        },
        "ForEachPlayer" => McBlock::ForEachPlayer { body: Vec::new() },
        "ScheduleAfter" => McBlock::ScheduleAfter {
            ticks: number(20.0),
            body: Vec::new(),
        },
        "EscapeLoop" => McBlock::EscapeLoop,
        "ContinueLoop" => McBlock::ContinueLoop,
        "Return" => McBlock::Return {
            value: number(0.0),
        },
        "CallBlock" => McBlock::CallBlock {
            block_id: String::new(),
            args: Vec::new(),
        },
        _ => return None,
    })
}

/// The `type` tag of every row, which is also every type the palette can drop.
pub fn all_types() -> Vec<&'static str> {
    ROWS.iter().map(|row| row.kind).collect()
}

// ── JSON for the QML side ─────────────────────────────────────────────────

fn piece_json(piece: &Piece) -> serde_json::Value {
    match piece {
        Piece::Label(text) => json!({ "kind": "label", "text": text }),
        Piece::Slot { field, boolean } => {
            // `field` addresses the value for the editor and `key` reads it off
            // the instruction; this vocabulary always names them the same.
            json!({ "kind": "value", "field": field, "key": field, "bool": boolean })
        }
        Piece::Dropdown {
            key,
            options,
            placeholder,
        } => {
            let options = match options {
                Options::Fixed(pairs) => json!(
                    pairs
                        .iter()
                        .map(|(value, label)| json!({ "value": value, "label": label }))
                        .collect::<Vec<_>>()
                ),
                Options::Lists => json!("lists"),
                Options::Dicts => json!("dicts"),
                Options::Source(name) => json!({ "source": name }),
                Options::YesNo => json!("yesno"),
            };
            json!({
                "kind": "dropdown",
                "key": key,
                "options": options,
                "placeholder": placeholder,
            })
        }
        Piece::Text { key, placeholder } => {
            json!({ "kind": "text", "key": key, "placeholder": placeholder })
        }
    }
}

/// The row table as `BlockRegistry.registerRows` wants it. `Options::Source`
/// and `Options::YesNo` come through as data for `BlockRows.qml` to replace
/// with functions.
pub fn rows_json() -> serde_json::Value {
    let mut rows = Map::new();
    for row in ROWS {
        let mut entry = Map::new();
        entry.insert("icon".to_string(), json!(row.icon));
        match row.shape {
            Shape::Stack => {}
            Shape::Header => {
                entry.insert("shape".to_string(), json!("header"));
            }
            Shape::Cap => {
                entry.insert("shape".to_string(), json!("cap"));
            }
        }
        entry.insert(
            "head".to_string(),
            json!(row.head.iter().map(piece_json).collect::<Vec<_>>()),
        );
        if !row.mouths.is_empty() {
            entry.insert("mouths".to_string(), json!(row.mouths));
        }
        if !row.separators.is_empty() {
            entry.insert("separators".to_string(), json!(row.separators));
        }
        rows.insert(row.kind.to_string(), serde_json::Value::Object(entry));
    }
    serde_json::Value::Object(rows)
}

/// Every prefab as `BlockRegistry.registerPrefabs` wants it: the instruction
/// fields a fresh palette block starts with, keyed by type.
pub fn prefabs_json() -> serde_json::Value {
    let mut prefabs = Map::new();
    for row in ROWS {
        let Some(block) = prefab(row.kind) else {
            continue;
        };
        let mut fields = match serde_json::to_value(&block) {
            Ok(serde_json::Value::Object(fields)) => fields,
            // `McBlock` is an internally tagged enum, so every variant is an
            // object; anything else would be a serde bug rather than bad data.
            _ => continue,
        };
        // The canvas sets `type` itself from the registry key.
        fields.remove("type");
        prefabs.insert(row.kind.to_string(), serde_json::Value::Object(fields));
    }
    serde_json::Value::Object(prefabs)
}

/// The palette sections as JSON: `[{title, types}, ...]`.
pub fn sections_json() -> serde_json::Value {
    json!(
        SECTIONS
            .iter()
            .map(|section| json!({ "title": section.title, "types": section.types }))
            .collect::<Vec<_>>()
    )
}

#[cfg(test)]
mod check {
    use super::*;
    use std::collections::HashSet;

    /// Every type in the catalogue, from the serde tag of every prefab, so the
    /// two tables are checked against each other rather than against a list.
    #[test]
    fn rows_cover_every_type() {
        for row in ROWS {
            let block = prefab(row.kind)
                .unwrap_or_else(|| panic!("{} has a row but no prefab", row.kind));
            let json = serde_json::to_value(&block).unwrap();
            assert_eq!(
                json["type"], row.kind,
                "{}'s prefab builds a different variant",
                row.kind
            );
        }
    }

    #[test]
    fn row_kinds_are_unique() {
        let kinds: HashSet<&str> = ROWS.iter().map(|row| row.kind).collect();
        assert_eq!(kinds.len(), ROWS.len(), "two rows share a type");
    }

    #[test]
    fn mouth_counts_stay_within_the_body_slot_budget() {
        use blockstitch_core::graph::BlockKind;
        for row in ROWS {
            assert!(
                row.mouths.len() <= McBlock::BODY_SLOTS as usize,
                "{} declares more mouths than McBlock::BODY_SLOTS",
                row.kind
            );
            if !row.separators.is_empty() {
                assert_eq!(
                    row.separators.len(),
                    row.mouths.len().saturating_sub(1),
                    "{} needs one separator between each pair of mouths",
                    row.kind
                );
            }
        }
    }

    #[test]
    fn mouth_keys_reach_a_real_body() {
        use blockstitch_core::graph::BlockKind;
        for row in ROWS {
            for slot in 0..row.mouths.len() as u8 {
                let mut block = prefab(row.kind).unwrap();
                assert!(
                    block.body_mut(slot).is_some(),
                    "{} declares mouth {slot} but McBlock has no body there",
                    row.kind
                );
            }
        }
    }

    #[test]
    fn value_pieces_address_a_real_slot() {
        use blockstitch_core::graph::BlockKind;
        for row in ROWS {
            for piece in row.head {
                let Piece::Slot { field, .. } = piece else {
                    continue;
                };
                let mut block = prefab(row.kind).unwrap();
                assert!(
                    block.value_slot_mut(field).is_some(),
                    "{}'s row draws a slot for `{field}`, which McBlock has not got",
                    row.kind
                );
            }
        }
    }

    #[test]
    fn dropdown_and_text_pieces_address_a_real_field() {
        for row in ROWS {
            let fields = match serde_json::to_value(prefab(row.kind).unwrap()).unwrap() {
                serde_json::Value::Object(fields) => fields,
                other => panic!("{} serialized as {other}", row.kind),
            };
            for piece in row.head {
                let key = match piece {
                    Piece::Dropdown { key, .. } | Piece::Text { key, .. } => *key,
                    _ => continue,
                };
                assert!(
                    fields.contains_key(key),
                    "{}'s row draws `{key}`, which is not a field of the variant",
                    row.kind
                );
            }
        }
    }

    #[test]
    fn header_rows_match_the_block_kinds_own_answer() {
        use blockstitch_core::graph::BlockKind;
        for row in ROWS {
            let block = prefab(row.kind).unwrap();
            assert_eq!(
                row.shape == Shape::Header,
                block.is_header(),
                "{}'s row shape and McBlock::is_header disagree",
                row.kind
            );
        }
    }

    #[test]
    fn every_dynamic_source_is_one_the_host_answers() {
        let known: HashSet<&str> = DYNAMIC_SOURCES
            .iter()
            .copied()
            .chain(["variables"])
            .collect();
        for row in ROWS {
            for piece in row.head {
                if let Piece::Dropdown {
                    options: Options::Source(name),
                    ..
                } = piece
                {
                    assert!(known.contains(name), "unknown dynamic source `{name}`");
                }
            }
        }
    }

    #[test]
    fn sections_only_name_types_that_exist() {
        let kinds: HashSet<&str> = ROWS.iter().map(|row| row.kind).collect();
        for section in SECTIONS {
            for kind in section.types {
                assert!(
                    kinds.contains(kind),
                    "palette section {:?} names unknown type `{kind}`",
                    section.title
                );
            }
        }
        for kind in LIST_TYPES.iter().chain(DICT_TYPES) {
            assert!(kinds.contains(kind), "collection type `{kind}` has no row");
        }
    }

    #[test]
    fn prefab_json_leaves_the_type_tag_to_the_canvas() {
        let prefabs = prefabs_json();
        let repeat = &prefabs["Repeat"];
        assert!(repeat.get("type").is_none());
        assert_eq!(repeat["count"]["value"], 10.0);
        assert_eq!(repeat["body"], json!([]));
    }
}
