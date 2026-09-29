//! Compiling a [`Value`] tree into a Java expression.
//!
//! Every expression this module produces has the static Java type `Object`,
//! holding a `Double`, a `String` or a `Boolean` - the same three types
//! [`blockstitch_core::value::Evaluated`] has. Keeping one static type means an
//! operator never has to know what its operands turned out to be, which is
//! exactly the looseness the editor's value slots have.
//!
//! Coercion happens at the point of use, through `Rt.num`, `Rt.str` and
//! `Rt.bool` in the generated runtime. [`number`], [`text`] and [`boolean`]
//! here are the Rust-side shortcuts that skip the call when the value is
//! already a literal of the right kind, which is most of them.
//!
//! The operator semantics mirror blockstitch-core's `Value::eval` exactly -
//! 1-based string indices, degrees for trigonometry, Euclidean modulo - so a
//! canvas means the same thing in the editor and in the exported mod.

use crate::writer::{double, quote};
use blockstitch_core::value::{Op, Value};
use stitchcraft_blocks::reporters;

/// Where a value is being compiled, for the things that depend on it.
#[derive(Debug, Clone, Copy)]
pub struct Ctx<'a> {
    /// The mod id, for namespacing a bare registry id.
    pub mod_id: &'a str,
    /// The generated mod class, which holds the constants a reporter can read.
    pub main_class: &'a str,
    /// The Java expression naming the current context object.
    pub ctx: &'a str,
    /// The Java expression naming the current call's arguments, for a custom
    /// block body reading its inputs.
    pub args: &'a str,
    /// The enclosing custom block's input names, in prototype order - which is
    /// the order a call site's arguments are in. A named input is resolved to
    /// its index here, at export time, rather than looked up at runtime.
    pub params: &'a [String],
}

impl<'a> Ctx<'a> {
    /// The ordinary case: a handler with a `ctx` parameter and an `args` array.
    /// An event hook has no inputs of its own, so `params` starts empty.
    pub fn handler(mod_id: &'a str, main_class: &'a str) -> Self {
        Self {
            mod_id,
            main_class,
            ctx: "ctx",
            args: "args",
            params: &[],
        }
    }
}

/// Compiles `value` to a Java expression of type `Object`.
pub fn object(value: &Value, ctx: Ctx<'_>) -> String {
    match value {
        Value::Number { value } => format!("Double.valueOf({})", double(*value)),
        Value::Text { value } => quote(value),
        // An empty boolean slot is false, the way Scratch's empty hexagon is.
        Value::Bool => "Boolean.FALSE".to_string(),
        Value::Var { name } => format!("Vars.get({})", quote(name)),
        // A parameter read outside a custom block body, or one naming an input
        // the definition no longer has, reads as zero - the same thing the
        // editor shows for an orphaned reporter.
        Value::Param { name } => match ctx.params.iter().position(|param| param == name) {
            Some(index) => format!("Rt.arg({}, {index})", ctx.args),
            None => "Double.valueOf(0.0D)".to_string(),
        },
        Value::Op { op, args, .. } => operator(op, args, ctx),
        Value::Call {
            block_id, args: a, ..
        } => {
            let arguments = a
                .iter()
                .map(|arg| object(arg, ctx))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Handlers.{}({}, new Object[] {{{arguments}}})",
                crate::handlers::custom_block_method(block_id),
                ctx.ctx
            )
        }
    }
}

/// Strips the box [`object`] put on, when the whole expression is one box.
///
/// `object` has to hand back an `Object`, so an arithmetic result comes out as
/// `Double.valueOf(a + b)`. Used where a primitive is wanted, that box would be
/// immediately unwrapped again - and the generated code would read as though it
/// did not trust itself. Taking the wrapper off here is safe because this module
/// wrote it: the check is that the paren opened by the prefix is the one closed
/// at the very end, so a `Double.valueOf(a) + x` is never mistaken for a box.
fn unbox(expression: &str, prefix: &str) -> Option<String> {
    let inner = expression.strip_prefix(prefix)?.strip_suffix(')')?;
    let mut depth = 0i32;
    for c in inner.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    (depth == 0).then(|| inner.to_string())
}

/// Parenthesises an expression that is not a single term.
///
/// The box [`unbox`] takes off was also doing the grouping: `Double.valueOf(a +
/// b)` used as an operand has to become `(a + b)`, or `(2 + 3) * 4` quietly turns
/// into `2 + 3 * 4`. A top-level space is the tell - every operator this module
/// writes is spaced, and every atom (`ctx.x()`, `Rt.div(a, b)`, `!x`, a literal)
/// has none outside its own parentheses.
fn grouped(expression: String) -> String {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for c in expression.chars() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => depth -= 1,
            ' ' if depth == 0 => return format!("({expression})"),
            _ => {}
        }
    }
    expression
}

/// Compiles `value` to a Java `double` expression.
pub fn number(value: &Value, ctx: Ctx<'_>) -> String {
    if let Value::Number { value } = value {
        return double(*value);
    }
    let boxed = object(value, ctx);
    match unbox(&boxed, "Double.valueOf(") {
        Some(inner) => grouped(inner),
        None => format!("Rt.num({boxed})"),
    }
}

/// Compiles `value` to a Java `int` expression, for the slots that only hold
/// whole numbers.
pub fn integer(value: &Value, ctx: Ctx<'_>) -> String {
    if let Value::Number { value } = value {
        return crate::writer::int(*value);
    }
    // `Rt.integer` clamps rather than overflowing, which a plain `(int)` cast
    // would not. An argument position needs no grouping, hence the raw inner.
    let boxed = object(value, ctx);
    match unbox(&boxed, "Double.valueOf(") {
        Some(inner) => format!("Rt.integer({inner})"),
        None => format!("Rt.integer({boxed})"),
    }
}

/// Compiles `value` to a Java `float` expression, for Minecraft's many
/// float-taking signatures.
pub fn float(value: &Value, ctx: Ctx<'_>) -> String {
    match value {
        Value::Number { value } => crate::writer::float(*value),
        // Parenthesised, because a cast binds tighter than the arithmetic the
        // expression may be.
        _ => format!("(float) ({})", number(value, ctx)),
    }
}

/// Compiles `value` to a Java `String` expression.
pub fn text(value: &Value, ctx: Ctx<'_>) -> String {
    if is_text(value) {
        return object(value, ctx);
    }
    format!("Rt.str({})", object(value, ctx))
}

/// True when [`object`] already produces a `String` for this value, so no
/// coercion is needed. Conservative: a `false` only costs an `Rt.str` call.
fn is_text(value: &Value) -> bool {
    match value {
        Value::Text { .. } => true,
        Value::Op { op, .. } => match op {
            Op::Join | Op::NewLine | Op::Tab | Op::Case | Op::LetterOf => true,
            // These two read a stored value, which keeps whichever of the three
            // types it was put in with - so they come back as `Object` in Java
            // however the palette draws them.
            Op::Ext(kind) if matches!(kind.as_ref(), "ListItem" | "DictValue") => false,
            Op::Ext(kind) => reporters::reporter(kind)
                .is_some_and(|spec| spec.result == reporters::ResultType::Text),
            _ => false,
        },
        _ => false,
    }
}

/// Compiles `value` to a Java `boolean` expression.
pub fn boolean(value: &Value, ctx: Ctx<'_>) -> String {
    if let Value::Bool = value {
        return "false".to_string();
    }
    let boxed = object(value, ctx);
    match boxed.as_str() {
        "Boolean.TRUE" => return "true".to_string(),
        "Boolean.FALSE" => return "false".to_string(),
        _ => {}
    }
    match unbox(&boxed, "Boolean.valueOf(") {
        // Grouped for the same reason a number is: `not (a and b)` must not
        // come out as `!a && b`.
        Some(inner) => grouped(inner),
        None => format!("Rt.bool({boxed})"),
    }
}

/// `args[index]`, or a blank of the right kind when a hand-edited save is short
/// an argument. An operator's arity is enforced by the editor, so this is a
/// guard against a broken file rather than a case the UI can produce.
fn arg(args: &[Value], index: usize) -> Value {
    args.get(index).cloned().unwrap_or(Value::Number { value: 0.0 })
}

fn operator(op: &Op, args: &[Value], ctx: Ctx<'_>) -> String {
    let num = |index: usize| number(&arg(args, index), ctx);
    let str_ = |index: usize| text(&arg(args, index), ctx);
    let obj = |index: usize| object(&arg(args, index), ctx);
    let boxed_number = |expression: String| format!("Double.valueOf({expression})");
    let boxed_bool = |expression: String| format!("Boolean.valueOf({expression})");

    match op {
        Op::Add => boxed_number(format!("{} + {}", num(0), num(1))),
        Op::Sub => boxed_number(format!("{} - {}", num(0), num(1))),
        Op::Mul => boxed_number(format!("{} * {}", num(0), num(1))),
        // Division and modulo go through the runtime so a zero divisor reads as
        // 0 instead of throwing out of an event handler.
        Op::Div => boxed_number(format!("Rt.div({}, {})", num(0), num(1))),
        Op::Mod => boxed_number(format!("Rt.mod({}, {})", num(0), num(1))),
        Op::Round => boxed_number(format!("Math.round({}) * 1.0D", num(0))),
        // The enum argument is the function name, and the operand follows it.
        Op::Math => boxed_number(format!("Rt.math({}, {})", str_(0), num(1))),
        Op::Random => boxed_number(format!("Rt.random({}, {})", num(0), num(1))),
        // Join takes two or three operands from the palette and any number from
        // a save file, so it is written as a fold rather than a fixed pair.
        Op::Join => {
            if args.is_empty() {
                quote("")
            } else {
                args.iter()
                    .map(|value| text(value, ctx))
                    .collect::<Vec<_>>()
                    .join(" + ")
            }
        }
        Op::NewLine => quote("\n"),
        Op::Tab => quote("\t"),
        Op::IndexOf => boxed_number(format!("Rt.indexOf({}, {})", str_(0), str_(1))),
        Op::LastIndexOf => boxed_number(format!("Rt.lastIndexOf({}, {})", str_(0), str_(1))),
        Op::LetterOf => format!("Rt.letterOf({}, {})", num(0), str_(1)),
        Op::Length => boxed_number(format!("Rt.length({})", str_(0))),
        // Here the enum argument is second: `[text] to [uppercase]`.
        Op::Case => format!("Rt.changeCase({}, {})", str_(0), str_(1)),
        Op::Eq => boxed_bool(format!("Rt.eq({}, {})", obj(0), obj(1))),
        Op::Neq => boxed_bool(format!("!Rt.eq({}, {})", obj(0), obj(1))),
        Op::Gt => boxed_bool(format!("{} > {}", num(0), num(1))),
        Op::Lt => boxed_bool(format!("{} < {}", num(0), num(1))),
        Op::Gte => boxed_bool(format!("{} >= {}", num(0), num(1))),
        Op::Lte => boxed_bool(format!("{} <= {}", num(0), num(1))),
        // `&&`/`||` keep their short-circuiting, which is the one place the
        // generated code is more forgiving than `Value::eval`.
        Op::And => boxed_bool(format!(
            "{} && {}",
            boolean(&arg(args, 0), ctx),
            boolean(&arg(args, 1), ctx)
        )),
        Op::Or => boxed_bool(format!(
            "{} || {}",
            boolean(&arg(args, 0), ctx),
            boolean(&arg(args, 1), ctx)
        )),
        Op::Not => boxed_bool(format!("!{}", boolean(&arg(args, 0), ctx))),
        Op::True => "Boolean.TRUE".to_string(),
        Op::False => "Boolean.FALSE".to_string(),
        Op::CurrentTime => boxed_number(format!("Rt.currentTime({})", str_(0))),
        Op::Ext(kind) => reporter(kind, args, ctx),
    }
}

/// Compiles one of Stitchcraft's own reporters. An unknown wire name - a save
/// from a newer version, or a hand-edited one - becomes a zero rather than
/// invalid Java, and the exporter has already warned about it.
fn reporter(kind: &str, args: &[Value], ctx: Ctx<'_>) -> String {
    let num = |index: usize| number(&arg(args, index), ctx);
    let str_ = |index: usize| text(&arg(args, index), ctx);
    let obj = |index: usize| object(&arg(args, index), ctx);
    let c = ctx.ctx;
    let boxed_number = |expression: String| format!("Double.valueOf({expression})");
    let boxed_bool = |expression: String| format!("Boolean.valueOf({expression})");

    match kind {
        // ── The event context ─────────────────────────────────────────────
        // Every one of these is null-safe inside `Ctx`, so a reporter under a
        // hook that cannot answer it reads as empty rather than crashing.
        "EventPlayerName" => format!("{c}.playerName()"),
        "EventPlayerHealth" => boxed_number(format!("{c}.playerHealth()")),
        "EventPlayerHeldItem" => format!("{c}.playerHeldItem()"),
        "EventX" => boxed_number(format!("{c}.x()")),
        "EventY" => boxed_number(format!("{c}.y()")),
        "EventZ" => boxed_number(format!("{c}.z()")),
        "EventBlockId" => format!("{c}.blockId()"),
        "EventEntityType" => format!("{c}.entityType()"),
        "EventEntityHealth" => boxed_number(format!("{c}.entityHealth()")),
        "EventDamageAmount" => boxed_number(format!("{c}.damage()")),
        // ── The world ─────────────────────────────────────────────────────
        "BlockIdAt" => format!("Rt.blockIdAt({c}, {}, {}, {})", num(0), num(1), num(2)),
        "IsBlockAt" => boxed_bool(format!(
            "Rt.isBlockAt({c}, {}, {}, {}, {})",
            num(0),
            num(1),
            num(2),
            qualified(&arg(args, 3), ctx)
        )),
        "LightAt" => boxed_number(format!(
            "Rt.lightAt({c}, {}, {}, {})",
            num(0),
            num(1),
            num(2)
        )),
        "SurfaceHeightAt" => {
            boxed_number(format!("Rt.surfaceHeightAt({c}, {}, {})", num(0), num(1)))
        }
        "IsDay" => boxed_bool(format!("Rt.isDay({c})")),
        "IsRaining" => boxed_bool(format!("Rt.isRaining({c})")),
        "TimeOfDay" => boxed_number(format!("Rt.timeOfDay({c})")),
        "Difficulty" => format!("Rt.difficulty({c})"),
        "LevelName" => format!("Rt.levelName({c})"),
        // ── The server ────────────────────────────────────────────────────
        "PlayerCount" => boxed_number(format!("Rt.playerCount({c})")),
        "ServerTickCount" => boxed_number("Scheduler.tickCount()".to_string()),
        "ModVersion" => format!("{}.VERSION", ctx.main_class),
        "IsFabric" => "Boolean.valueOf(ModLoader.isFabric())".to_string(),
        "IsNeoForge" => "Boolean.valueOf(ModLoader.isNeoForge())".to_string(),
        // ── Convenience ───────────────────────────────────────────────────
        "RandomChance" => boxed_bool(format!("Rt.chance({})", num(0))),
        "TicksToSeconds" => boxed_number(format!("({}) / 20.0D", num(0))),
        // ── Collections ───────────────────────────────────────────────────
        // The name argument's index is blockstitch-core's, not ours - see
        // `reporters::COLLECTION_REPORTERS`.
        "ListItem" => format!("Vars.listItem({}, {})", str_(1), num(0)),
        "ListItemNumber" => boxed_number(format!("Vars.listIndexOf({}, {})", str_(1), obj(0))),
        "ListAmount" => boxed_number(format!("Vars.listCount({}, {})", str_(1), obj(0))),
        "ListItemExists" => boxed_bool(format!("Vars.listHas({}, {})", str_(1), num(0))),
        "ListLength" => boxed_number(format!("Vars.listSize({})", str_(0))),
        "ListContains" => boxed_bool(format!("Vars.listContains({}, {})", str_(0), obj(1))),
        "ListIsEmpty" => boxed_bool(format!("Vars.listSize({}) == 0", str_(0))),
        "ListAsJson" => format!("Vars.listAsJson({})", str_(0)),
        "DictValue" => format!("Vars.dictGet({}, {})", str_(1), str_(0)),
        "DictHasKey" => boxed_bool(format!("Vars.dictHas({}, {})", str_(0), str_(1))),
        "DictSize" => boxed_number(format!("Vars.dictSize({})", str_(0))),
        "DictKeys" => format!("Vars.dictKeys({})", str_(0)),
        "DictIsEmpty" => boxed_bool(format!("Vars.dictSize({}) == 0", str_(0))),
        "DictAsJson" => format!("Vars.dictAsJson({})", str_(0)),
        _ => {
            debug_assert!(
                reporters::reporter(kind).is_none(),
                "{kind} is a registered reporter with no Java translation"
            );
            "Double.valueOf(0.0D)".to_string()
        }
    }
}

/// A registry id argument, namespaced to the mod when it is a bare path.
fn qualified(value: &Value, ctx: Ctx<'_>) -> String {
    match value {
        Value::Text { value } => quote(&crate::naming::qualify(ctx.mod_id, value)),
        // A computed id cannot be namespaced here, so the runtime does it.
        _ => format!("Rt.qualify({})", text(value, ctx)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Ctx<'static> {
        Ctx::handler("demo", "Demo")
    }

    fn compile(value: &Value) -> String {
        object(value, ctx())
    }

    #[test]
    fn literals_compile_to_java_literals() {
        assert_eq!(compile(&Value::number(3.0)), "Double.valueOf(3.0D)");
        assert_eq!(compile(&Value::text("hi")), "\"hi\"");
        assert_eq!(compile(&Value::Bool), "Boolean.FALSE");
    }

    #[test]
    fn coercion_is_skipped_for_a_literal_of_the_right_kind() {
        assert_eq!(number(&Value::number(2.5), ctx()), "2.5D");
        assert_eq!(text(&Value::text("hi"), ctx()), "\"hi\"");
        assert_eq!(boolean(&Value::Bool, ctx()), "false");
        // ...and applied when it is not.
        assert_eq!(number(&Value::text("2"), ctx()), "Rt.num(\"2\")");
    }

    #[test]
    fn arithmetic_nests_without_losing_its_shape() {
        // (2 + 3) * 4
        let value = Value::op(
            Op::Mul,
            vec![
                Value::op(Op::Add, vec![Value::number(2.0), Value::number(3.0)]),
                Value::number(4.0),
            ],
        );
        assert_eq!(compile(&value), "Double.valueOf((2.0D + 3.0D) * 4.0D)");
    }

    #[test]
    fn unboxing_an_operand_keeps_its_grouping() {
        // Taking `Double.valueOf(...)` off an operand loses the grouping it was
        // also providing, so anything that is not a single term gets parens.
        let sum = Value::op(Op::Add, vec![Value::number(2.0), Value::number(3.0)]);
        assert_eq!(number(&sum, ctx()), "(2.0D + 3.0D)");
        // ...and anything that is one term does not.
        let chance = Value::op(Op::Ext("RandomChance".into()), vec![Value::number(10.0)]);
        assert_eq!(boolean(&chance, ctx()), "Rt.chance(10.0D)");
        // `not (a and b)` must not become `!a && b`.
        let conjunction = Value::op(
            Op::Not,
            vec![Value::op(
                Op::And,
                vec![Value::op(Op::True, vec![]), Value::op(Op::False, vec![])],
            )],
        );
        assert_eq!(compile(&conjunction), "Boolean.valueOf(!(true && false))");
        // A string literal holding a space is not a top-level operator.
        let letter = Value::op(
            Op::Length,
            vec![Value::text("two words")],
        );
        assert_eq!(number(&letter, ctx()), "Rt.length(\"two words\")");
    }

    #[test]
    fn division_and_modulo_go_through_the_runtime() {
        let value = Value::op(Op::Div, vec![Value::number(1.0), Value::number(0.0)]);
        assert_eq!(compile(&value), "Double.valueOf(Rt.div(1.0D, 0.0D))");
    }

    #[test]
    fn logic_short_circuits() {
        let value = Value::op(Op::And, vec![Value::Bool, Value::op(Op::True, vec![])]);
        assert_eq!(compile(&value), "Boolean.valueOf(false && true)");
    }

    #[test]
    fn join_folds_however_many_operands_it_has() {
        let three = Value::op(
            Op::Join,
            vec![Value::text("a"), Value::text("b"), Value::text("c")],
        );
        assert_eq!(compile(&three), "\"a\" + \"b\" + \"c\"");
        assert_eq!(compile(&Value::op(Op::Join, vec![])), "\"\"");
    }

    #[test]
    fn the_enum_argument_keeps_its_declared_position() {
        // Math puts the function first, Case puts it second.
        let math = Value::op(Op::Math, vec![Value::text("Sqrt"), Value::number(9.0)]);
        assert_eq!(compile(&math), "Double.valueOf(Rt.math(\"Sqrt\", 9.0D))");
        let case = Value::op(Op::Case, vec![Value::text("hi"), Value::text("Upper")]);
        assert_eq!(compile(&case), "Rt.changeCase(\"hi\", \"Upper\")");
    }

    #[test]
    fn variables_read_through_the_generated_store() {
        assert_eq!(
            compile(&Value::Var {
                name: "score".into()
            }),
            "Vars.get(\"score\")"
        );
    }

    #[test]
    fn context_reporters_read_off_the_context_object() {
        let value = Value::op(Op::Ext("EventPlayerName".into()), vec![]);
        assert_eq!(compile(&value), "ctx.playerName()");
    }

    #[test]
    fn a_list_reporter_passes_the_name_where_core_keeps_it() {
        // ListItem is `item [index] of [name]`, name at index 1.
        let value = Value::op(
            Op::Ext("ListItem".into()),
            vec![Value::number(1.0), Value::text("scores")],
        );
        assert_eq!(compile(&value), "Vars.listItem(\"scores\", 1.0D)");
    }

    #[test]
    fn a_bare_registry_id_gets_namespaced_at_compile_time() {
        let value = Value::op(
            Op::Ext("IsBlockAt".into()),
            vec![
                Value::number(0.0),
                Value::number(0.0),
                Value::number(0.0),
                Value::text("chime"),
            ],
        );
        assert!(compile(&value).contains("\"demo:chime\""));
    }

    #[test]
    fn an_unknown_reporter_still_produces_valid_java() {
        let value = Value::op(Op::Ext("SomethingNewer".into()), vec![]);
        assert_eq!(compile(&value), "Double.valueOf(0.0D)");
    }

    #[test]
    fn a_short_argument_list_does_not_panic() {
        // Arity is enforced by the editor; a hand-edited save is not.
        let value = Value::op(Op::Add, vec![Value::number(1.0)]);
        assert_eq!(compile(&value), "Double.valueOf(1.0D + 0.0D)");
    }
}
