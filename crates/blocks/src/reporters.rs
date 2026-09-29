//! The Minecraft-specific reporter blocks, on top of blockstitch's built-in
//! arithmetic, text, logic and time operators.
//!
//! One table, [`REPORTERS`], is the single source of truth for three
//! consumers, so none of them can drift from the others:
//!
//! * [`register`] hands them to blockstitch-core, which puts them in reach of
//!   the palette and of the editor's drag/drop bookkeeping;
//! * [`operator_rows_json`] is what the QML `BlockRegistry.registerOperators`
//!   call is fed, so the canvas knows how to draw each one;
//! * `stitchcraft-export` matches on [`ReporterSpec::kind`] to emit Java.
//!
//! None of them evaluate. Stitchcraft compiles a canvas rather than running
//! it, so every reporter's `eval` is [`not_evaluable`]: reaching it means
//! something tried to interpret a document, which is a bug rather than a
//! user error.

use blockstitch_core::value::{Evaluated, ExtOperator, Value, register_operators};
use serde_json::{Map, json};
use std::sync::OnceLock;

/// What a reporter hands back, in the three types blockstitch values have.
/// Drives both the palette shape (a hexagon for `Bool`) and the Java
/// coercion the exporter wraps an argument in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultType {
    Number,
    Text,
    Bool,
}

impl ResultType {
    /// The string QML's operator table spells this as.
    pub fn as_qml(self) -> &'static str {
        match self {
            ResultType::Number => "number",
            ResultType::Text => "text",
            ResultType::Bool => "bool",
        }
    }
}

/// One reporter: its wire name, how it draws, and the blanks a fresh one gets.
pub struct ReporterSpec {
    /// Wire name, stored in save files and matched on by the exporter. Also
    /// the palette entry id; the two are always the same here.
    pub kind: &'static str,
    pub arity: usize,
    pub result: ResultType,
    /// Label before the first operand.
    pub prefix: &'static str,
    /// Label between the first and second operands.
    pub infix: &'static str,
    /// Label after the last operand.
    pub suffix: &'static str,
    pub default_args: fn() -> Vec<Value>,
}

fn none() -> Vec<Value> {
    Vec::new()
}

fn one_zero() -> Vec<Value> {
    vec![Value::number(0.0)]
}

fn coords() -> Vec<Value> {
    vec![Value::number(0.0), Value::number(64.0), Value::number(0.0)]
}

fn coords_and_block() -> Vec<Value> {
    vec![
        Value::number(0.0),
        Value::number(64.0),
        Value::number(0.0),
        Value::text("minecraft:stone"),
    ]
}

fn xz() -> Vec<Value> {
    vec![Value::number(0.0), Value::number(0.0)]
}

fn one_chance() -> Vec<Value> {
    vec![Value::number(50.0)]
}

/// Every reporter Stitchcraft adds. `kind` values are part of the save format.
pub const REPORTERS: &[ReporterSpec] = &[
    // ── The event context ─────────────────────────────────────────────────
    // What these resolve to depends on the enclosing hook; `crate::validate`
    // reports a reporter used under a hook that cannot supply it.
    spec("EventPlayerName", 0, ResultType::Text, "event player name", "", "", none),
    spec("EventPlayerHealth", 0, ResultType::Number, "event player health", "", "", none),
    spec("EventPlayerHeldItem", 0, ResultType::Text, "event player held item", "", "", none),
    spec("EventX", 0, ResultType::Number, "event x", "", "", none),
    spec("EventY", 0, ResultType::Number, "event y", "", "", none),
    spec("EventZ", 0, ResultType::Number, "event z", "", "", none),
    spec("EventBlockId", 0, ResultType::Text, "event block id", "", "", none),
    spec("EventEntityType", 0, ResultType::Text, "event entity type", "", "", none),
    spec("EventEntityHealth", 0, ResultType::Number, "event entity health", "", "", none),
    spec("EventDamageAmount", 0, ResultType::Number, "event damage amount", "", "", none),
    // ── The world ─────────────────────────────────────────────────────────
    spec("BlockIdAt", 3, ResultType::Text, "block id at", "", "", coords),
    spec("IsBlockAt", 4, ResultType::Bool, "block at", "is", "", coords_and_block),
    spec("LightAt", 3, ResultType::Number, "light level at", "", "", coords),
    spec("SurfaceHeightAt", 2, ResultType::Number, "surface height at", "", "", xz),
    spec("IsDay", 0, ResultType::Bool, "is daytime?", "", "", none),
    spec("IsRaining", 0, ResultType::Bool, "is raining?", "", "", none),
    spec("TimeOfDay", 0, ResultType::Number, "time of day", "", "", none),
    spec("Difficulty", 0, ResultType::Text, "difficulty", "", "", none),
    spec("LevelName", 0, ResultType::Text, "dimension name", "", "", none),
    // ── The server ────────────────────────────────────────────────────────
    spec("PlayerCount", 0, ResultType::Number, "player count", "", "", none),
    spec("ServerTickCount", 0, ResultType::Number, "ticks since start", "", "", none),
    spec("ModVersion", 0, ResultType::Text, "mod version", "", "", none),
    spec("IsFabric", 0, ResultType::Bool, "running on Fabric?", "", "", none),
    spec("IsNeoForge", 0, ResultType::Bool, "running on NeoForge?", "", "", none),
    // ── Convenience ───────────────────────────────────────────────────────
    spec("RandomChance", 1, ResultType::Bool, "", "", "% chance", one_chance),
    spec("TicksToSeconds", 1, ResultType::Number, "seconds in", "", "ticks", one_zero),
];

/// The list and dict reporters.
///
/// blockstitch-core recognizes these wire names when it renames a collection
/// (`graph::lists::list_reporter_name_index` and its dict counterpart) and QML's
/// `BlockRegistry` already knows how to draw them, but neither one registers
/// them as operators - that is the host's job, and without it the palette has
/// no arity to build a fresh one from.
///
/// The name argument sits at the index core expects, and starts blank: the
/// palette rewrites it to the document's first list or dict on the way out.
pub const COLLECTION_REPORTERS: &[ReporterSpec] = &[
    spec("ListItem", 2, ResultType::Text, "item", "of", "", index_then_name),
    spec("ListItemNumber", 2, ResultType::Number, "item # of", "in", "", thing_then_name),
    spec("ListAmount", 2, ResultType::Number, "amount of", "in", "", thing_then_name),
    spec("ListItemExists", 2, ResultType::Bool, "item", "exists in", "", index_then_name),
    spec("ListLength", 1, ResultType::Number, "length of", "", "", just_name),
    spec("ListContains", 2, ResultType::Bool, "", "contains", "", name_then_thing),
    spec("ListIsEmpty", 1, ResultType::Bool, "is", "", "empty?", just_name),
    spec("ListAsJson", 1, ResultType::Text, "", "", "as JSON", just_name),
    spec("DictValue", 2, ResultType::Text, "value", "in", "", key_then_name),
    spec("DictHasKey", 2, ResultType::Bool, "", "has key", "", name_then_key),
    spec("DictSize", 1, ResultType::Number, "size of", "", "", just_name),
    spec("DictKeys", 1, ResultType::Text, "keys of", "", "", just_name),
    spec("DictIsEmpty", 1, ResultType::Bool, "is", "", "empty?", just_name),
    spec("DictAsJson", 1, ResultType::Text, "", "", "as JSON", just_name),
];

fn just_name() -> Vec<Value> {
    vec![Value::text("")]
}

fn index_then_name() -> Vec<Value> {
    vec![Value::number(1.0), Value::text("")]
}

fn thing_then_name() -> Vec<Value> {
    vec![Value::text("thing"), Value::text("")]
}

fn name_then_thing() -> Vec<Value> {
    vec![Value::text(""), Value::text("thing")]
}

fn key_then_name() -> Vec<Value> {
    vec![Value::text("key"), Value::text("")]
}

fn name_then_key() -> Vec<Value> {
    vec![Value::text(""), Value::text("key")]
}

/// Keeps the tables above readable - the fields are positional there and named
/// here, in the order [`ReporterSpec`] declares them.
const fn spec(
    kind: &'static str,
    arity: usize,
    result: ResultType,
    prefix: &'static str,
    infix: &'static str,
    suffix: &'static str,
    default_args: fn() -> Vec<Value>,
) -> ReporterSpec {
    ReporterSpec {
        kind,
        arity,
        result,
        prefix,
        infix,
        suffix,
        default_args,
    }
}

/// Every reporter this crate registers, Minecraft-specific and collection.
pub fn all() -> impl Iterator<Item = &'static ReporterSpec> {
    REPORTERS.iter().chain(COLLECTION_REPORTERS)
}

/// The reporter `kind` names, or `None` if there is no such reporter.
pub fn reporter(kind: &str) -> Option<&'static ReporterSpec> {
    all().find(|spec| spec.kind == kind)
}

/// Every reporter's `eval`. Stitchcraft exports a canvas instead of running
/// one, so nothing should ever call this.
fn not_evaluable(_args: &[Evaluated]) -> Result<Evaluated, String> {
    Err("Stitchcraft reporters describe generated Java and cannot be evaluated in the editor"
        .to_string())
}

/// Adds every reporter to blockstitch-core's operator registry. Call once at
/// startup, before any saved document is loaded: an unregistered wire name
/// still round-trips as `Op::Ext`, but the editor would not know its arity.
pub fn register() {
    static OPERATORS: OnceLock<&'static [ExtOperator]> = OnceLock::new();
    let operators = OPERATORS.get_or_init(|| {
        let built: Vec<ExtOperator> = all()
            .map(|spec| ExtOperator {
                kind: spec.kind,
                op: spec.kind,
                arity: spec.arity,
                default_args: spec.default_args,
                eval: not_evaluable,
            })
            .collect();
        // `register_operators` wants a `'static` slice and the registry keeps
        // it for the life of the process, so leaking once is the whole cost.
        Box::leak(built.into_boxed_slice())
    });
    register_operators(operators);
}

/// The reporter table as QML's `BlockRegistry.registerOperators` wants it:
/// `{ Kind: { prefix, infix, suffix, result } }`. Empty labels are left out so
/// the canvas does not draw a blank gap.
///
/// [`COLLECTION_REPORTERS`] are deliberately absent: `BlockRegistry` already
/// has rows for them, and its rows mark the name argument as an `enumArg` over
/// the document's lists or dicts, which is a QML-side concern this table cannot
/// express. Overriding them here would turn that dropdown back into a slot.
pub fn operator_rows_json() -> serde_json::Value {
    let mut rows = Map::new();
    for spec in REPORTERS {
        let mut row = Map::new();
        for (key, label) in [
            ("prefix", spec.prefix),
            ("infix", spec.infix),
            ("suffix", spec.suffix),
        ] {
            if !label.is_empty() {
                row.insert(key.to_string(), json!(label));
            }
        }
        row.insert("result".to_string(), json!(spec.result.as_qml()));
        rows.insert(spec.kind.to_string(), serde_json::Value::Object(row));
    }
    serde_json::Value::Object(rows)
}

/// The palette's reporter listing: `[[kind, result, arity], ...]`, matching the
/// shape `PalettePanel` uses for blockstitch's own operators.
pub fn palette_entries_json() -> serde_json::Value {
    entries_json(REPORTERS.iter())
}

/// The list reporters, in the same shape, for the palette's list section.
pub fn list_palette_entries_json() -> serde_json::Value {
    entries_json(
        COLLECTION_REPORTERS
            .iter()
            .filter(|spec| spec.kind.starts_with("List")),
    )
}

/// The dict reporters, for the palette's dict section.
pub fn dict_palette_entries_json() -> serde_json::Value {
    entries_json(
        COLLECTION_REPORTERS
            .iter()
            .filter(|spec| spec.kind.starts_with("Dict")),
    )
}

fn entries_json<'a>(specs: impl Iterator<Item = &'a ReporterSpec>) -> serde_json::Value {
    json!(
        specs
            .map(|spec| json!([spec.kind, spec.result.as_qml(), spec.arity]))
            .collect::<Vec<_>>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockstitch_core::value::{Op, operator_kind};

    #[test]
    fn kinds_are_unique() {
        let mut kinds: Vec<&str> = all().map(|spec| spec.kind).collect();
        let total = kinds.len();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), total, "two reporters share a wire name");
    }

    #[test]
    fn collection_reporters_put_the_name_where_core_looks_for_it() {
        use blockstitch_core::graph::{dict_reporter_name_index, list_reporter_name_index};
        for spec in COLLECTION_REPORTERS {
            let index = list_reporter_name_index(spec.kind)
                .or_else(|| dict_reporter_name_index(spec.kind))
                .unwrap_or_else(|| {
                    panic!("{} is not a name blockstitch-core recognizes", spec.kind)
                });
            assert!(
                index < spec.arity,
                "{}'s name argument is outside its arity",
                spec.kind
            );
            assert_eq!(
                (spec.default_args)()[index],
                Value::text(""),
                "{}'s name argument must be a blank Text leaf for renames to find it",
                spec.kind
            );
        }
    }

    #[test]
    fn default_args_match_the_declared_arity() {
        for spec in all() {
            assert_eq!(
                (spec.default_args)().len(),
                spec.arity,
                "{} hands out the wrong number of blanks",
                spec.kind
            );
        }
    }

    #[test]
    fn registering_puts_them_in_reach_of_the_palette() {
        register();
        let entry = operator_kind("IsBlockAt").expect("registered reporter");
        assert_eq!(entry.arity, 4);
        assert_eq!(entry.op, Op::Ext("IsBlockAt".into()));
        // Registering twice keeps the first, so startup order cannot matter.
        register();
        assert_eq!(operator_kind("IsBlockAt").unwrap().arity, 4);
    }

    #[test]
    fn a_builtin_name_is_never_shadowed() {
        for spec in all() {
            assert_ne!(
                Op::from_name(spec.kind),
                Op::Add,
                "sanity: from_name should not collapse"
            );
            assert!(
                matches!(Op::from_name(spec.kind), Op::Ext(_)),
                "{} collides with a built-in operator, which always wins the lookup",
                spec.kind
            );
        }
    }

    #[test]
    fn qml_rows_omit_empty_labels() {
        let rows = operator_rows_json();
        let chance = &rows["RandomChance"];
        assert_eq!(chance["suffix"], "% chance");
        assert_eq!(chance["result"], "bool");
        assert!(chance.get("prefix").is_none());
    }
}
