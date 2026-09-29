//! Compiling a stack of instructions into Java statements.
//!
//! Every command becomes one call into the generated runtime rather than a
//! direct Minecraft call, so `Rt.java` is the only generated file that touches
//! the game's API. When a Minecraft signature changes between versions, that is
//! the one file to fix - not every call site the canvas produced.
//!
//! Two constructs cannot be written inline and become methods of their own:
//! `ScheduleAfter`, whose body runs later, and a custom block, whose body is
//! shared by its call sites. [`Emitter`] queues those while it writes and hands
//! them back through [`Emitter::take_pending`].

use crate::expr::{self, Ctx};
use crate::naming::{self, Names};
use crate::writer::{Source, quote};
use blockstitch_core::graph::{BlockKind, Instruction};
use stitchcraft_blocks::kinds::{MessageKind, Target, Weather};
use stitchcraft_blocks::{McBlock, ModProject};

/// What a `return` means in the method being written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnMode {
    /// An event handler: `true` carries on, `false` cancels.
    Event,
    /// A custom block body, which reports an `Object`.
    Block,
    /// A delayed body, which reports nothing.
    Delayed,
}

/// A method the emitter needs written once it has finished the current one.
#[derive(Debug, Clone)]
pub struct Pending {
    pub method: String,
    pub body: Vec<Instruction<McBlock>>,
    /// The mode the body was written under - a delayed body reports nothing
    /// whatever its parent did.
    pub mode: ReturnMode,
    /// The enclosing custom block's inputs, carried through so a delayed body
    /// inside one can still read them.
    pub params: Vec<String>,
}

/// Writes statements for one document.
pub struct Emitter<'a> {
    names: &'a Names,
    project: &'a ModProject,
    pending: Vec<Pending>,
    /// Suffix for the next generated local or method name, unique per export.
    counter: usize,
}

impl<'a> Emitter<'a> {
    pub fn new(names: &'a Names, project: &'a ModProject) -> Self {
        Self {
            names,
            project,
            pending: Vec::new(),
            counter: 0,
        }
    }

    fn fresh(&mut self, stem: &str) -> String {
        self.counter += 1;
        format!("{stem}{}", self.counter)
    }

    /// The methods queued while writing, emptying the queue. Writing those may
    /// queue more, so callers drain in a loop.
    pub fn take_pending(&mut self) -> Vec<Pending> {
        std::mem::take(&mut self.pending)
    }

    /// Writes `instructions` as the body of a method in `mode`. `params` is the
    /// enclosing custom block's inputs, empty for an event hook.
    pub fn body(
        &mut self,
        source: &mut Source,
        instructions: &[Instruction<McBlock>],
        mode: ReturnMode,
        params: &[String],
    ) {
        let ctx = Ctx {
            params,
            ..Ctx::handler(&self.project.mod_id, &self.names.main_class)
        };
        self.statements(source, instructions, mode, ctx);
    }

    fn statements(
        &mut self,
        source: &mut Source,
        instructions: &[Instruction<McBlock>],
        mode: ReturnMode,
        ctx: Ctx<'_>,
    ) {
        for instruction in instructions {
            self.statement(source, &instruction.kind, mode, ctx);
        }
    }

    fn statement(
        &mut self,
        source: &mut Source,
        kind: &McBlock,
        mode: ReturnMode,
        ctx: Ctx<'_>,
    ) {
        // Copying the two shared references out ends the borrow of `self`, so
        // the closures below can coexist with the recursive `self.statements`.
        let project = self.project;
        let names = self.names;
        let mod_id = &project.mod_id;
        // Registry ids that name one of the mod's own content are namespaced
        // here, so the runtime only ever sees a fully qualified id.
        let id = |raw: &str| quote(&naming::qualify(mod_id, raw));
        let num = |value| expr::number(value, ctx);
        let int = |value| expr::integer(value, ctx);
        let flt = |value| expr::float(value, ctx);
        let obj = |value| expr::object(value, ctx);
        let txt = |value| expr::text(value, ctx);
        let cond = |value| expr::boolean(value, ctx);
        let c = ctx.ctx;

        match kind {
            // ── Talking ───────────────────────────────────────────────────
            McBlock::Message { target, kind, text } => source.line(format!(
                "Rt.message({c}, {}, {}, {});",
                target_constant(*target),
                txt(text),
                matches!(kind, MessageKind::ActionBar)
            )),
            McBlock::Broadcast { text } => {
                source.line(format!("Rt.broadcast({c}, {});", txt(text)));
            }
            McBlock::LogInfo { text } => {
                source.line(format!("{}.LOGGER.info({});", names.main_class, txt(text)));
            }
            McBlock::RunCommand { command } => {
                source.line(format!("Rt.runCommand({c}, {});", txt(command)));
            }

            // ── The world ─────────────────────────────────────────────────
            McBlock::SetBlockAt { x, y, z, block } => source.line(format!(
                "Rt.setBlock({c}, {}, {}, {}, {});",
                num(x),
                num(y),
                num(z),
                id(block)
            )),
            McBlock::BreakBlockAt { x, y, z, drop } => source.line(format!(
                "Rt.breakBlock({c}, {}, {}, {}, {drop});",
                num(x),
                num(y),
                num(z)
            )),
            McBlock::SpawnEntityAt { entity, x, y, z } => source.line(format!(
                "Rt.spawnEntity({c}, {}, {}, {}, {});",
                id(entity),
                num(x),
                num(y),
                num(z)
            )),
            McBlock::PlaySoundAt {
                sound,
                x,
                y,
                z,
                volume,
                pitch,
            } => source.line(format!(
                "Rt.playSound({c}, {}, {}, {}, {}, {}, {});",
                id(sound),
                num(x),
                num(y),
                num(z),
                flt(volume),
                flt(pitch)
            )),
            McBlock::SpawnParticleAt {
                particle,
                x,
                y,
                z,
                count,
            } => source.line(format!(
                "Rt.spawnParticle({c}, {}, {}, {}, {}, {});",
                id(particle),
                num(x),
                num(y),
                num(z),
                int(count)
            )),
            McBlock::ExplodeAt { x, y, z, power, fire } => source.line(format!(
                "Rt.explode({c}, {}, {}, {}, {}, {fire});",
                num(x),
                num(y),
                num(z),
                flt(power)
            )),
            McBlock::SetTimeOfDay { time } => {
                source.line(format!("Rt.setTimeOfDay({c}, {});", int(time)));
            }
            McBlock::SetWeather { weather, seconds } => source.line(format!(
                "Rt.setWeather({c}, {}, {});",
                weather_constant(*weather),
                int(seconds)
            )),

            // ── Entities ──────────────────────────────────────────────────
            McBlock::GiveItem {
                target,
                item,
                count,
            } => source.line(format!(
                "Rt.giveItem({c}, {}, {}, {});",
                target_constant(*target),
                id(item),
                int(count)
            )),
            McBlock::DamageTarget { target, amount } => source.line(format!(
                "Rt.damage({c}, {}, {});",
                target_constant(*target),
                flt(amount)
            )),
            McBlock::HealTarget { target, amount } => source.line(format!(
                "Rt.heal({c}, {}, {});",
                target_constant(*target),
                flt(amount)
            )),
            McBlock::AddEffect {
                target,
                effect,
                seconds,
                amplifier,
            } => source.line(format!(
                "Rt.addEffect({c}, {}, {}, {}, {});",
                target_constant(*target),
                id(effect),
                int(seconds),
                int(amplifier)
            )),
            McBlock::TeleportTarget { target, x, y, z } => source.line(format!(
                "Rt.teleport({c}, {}, {}, {}, {});",
                target_constant(*target),
                num(x),
                num(y),
                num(z)
            )),
            McBlock::PushTarget { target, x, y, z } => source.line(format!(
                "Rt.push({c}, {}, {}, {}, {});",
                target_constant(*target),
                num(x),
                num(y),
                num(z)
            )),

            // ── Variables, lists, dicts ───────────────────────────────────
            McBlock::SetVariable { name, value } => {
                source.line(format!("Vars.set({}, {});", quote(name), obj(value)));
            }
            McBlock::ChangeVariable { name, value } => {
                source.line(format!("Vars.change({}, {});", quote(name), num(value)));
            }
            McBlock::AddToList { name, value } => {
                source.line(format!("Vars.listAdd({}, {});", quote(name), obj(value)));
            }
            McBlock::DeleteOfList { name, index } => {
                source.line(format!("Vars.listDelete({}, {});", quote(name), num(index)));
            }
            McBlock::DeleteAllOfList { name } => {
                source.line(format!("Vars.listClear({});", quote(name)));
            }
            McBlock::InsertIntoList { name, index, value } => source.line(format!(
                "Vars.listInsert({}, {}, {});",
                quote(name),
                num(index),
                obj(value)
            )),
            McBlock::ReplaceItemOfList { name, index, value } => source.line(format!(
                "Vars.listReplace({}, {}, {});",
                quote(name),
                num(index),
                obj(value)
            )),
            McBlock::DictSet { name, key, value } => source.line(format!(
                "Vars.dictSet({}, {}, {});",
                quote(name),
                txt(key),
                obj(value)
            )),
            McBlock::DictDelete { name, key } => {
                source.line(format!("Vars.dictDelete({}, {});", quote(name), txt(key)));
            }
            McBlock::DictClear { name } => {
                source.line(format!("Vars.dictClear({});", quote(name)));
            }

            // ── Control flow ──────────────────────────────────────────────
            McBlock::If { condition, body } => {
                source.braced(format!("if ({})", cond(condition)), |source| {
                    self.statements(source, body, mode, ctx);
                });
            }
            McBlock::IfElse {
                condition,
                then_body,
                else_body,
            } => {
                source.open(format!("if ({}) {{", cond(condition)));
                self.statements(source, then_body, mode, ctx);
                source.pivot("} else {");
                self.statements(source, else_body, mode, ctx);
                source.close("}");
            }
            McBlock::Repeat { count, body } => {
                // The count is read once, so changing a variable inside the
                // body cannot lengthen the loop - the same as Scratch.
                let times = self.fresh("times");
                let index = self.fresh("i");
                source.line(format!("final int {times} = {};", int(count)));
                source.braced(
                    format!("for (int {index} = 0; {index} < {times}; {index}++)"),
                    |source| self.statements(source, body, mode, ctx),
                );
            }
            McBlock::While { condition, body } => {
                // Guarded: a handler runs inside the server tick, so a
                // condition that never goes false would hang the game rather
                // than just spin. `Rt.LOOP_LIMIT` passes are enough for any
                // sane loop and bounded for the rest.
                let guard = self.fresh("pass");
                source.braced(
                    format!(
                        "for (int {guard} = 0; {guard} < Rt.LOOP_LIMIT && ({}); {guard}++)",
                        cond(condition)
                    ),
                    |source| self.statements(source, body, mode, ctx),
                );
            }
            McBlock::ForEachPlayer { body } => {
                // Each pass rebinds the context's player, so `the event player`
                // inside the body means this pass's player.
                let player = self.fresh("player");
                let inner_name = self.fresh("ctx");
                source.braced(
                    format!("for (ServerPlayer {player} : Rt.players({c}))"),
                    |source| {
                        source.line(format!(
                            "final Ctx {inner_name} = {c}.withPlayer({player});"
                        ));
                        let inner = Ctx {
                            ctx: &inner_name,
                            ..ctx
                        };
                        // `inner_name` is a local, so the borrow cannot outlive
                        // this block - hence the nested call rather than
                        // rebinding `ctx` for the rest of the method.
                        self.statements(source, body, mode, inner);
                    },
                );
            }
            McBlock::ScheduleAfter { ticks, body } => {
                let method = self.fresh("delayed");
                source.line(format!(
                    "Scheduler.after({}, {c}, {}, Handlers::{method});",
                    int(ticks),
                    ctx.args
                ));
                self.pending.push(Pending {
                    method,
                    body: body.clone(),
                    mode: ReturnMode::Delayed,
                    params: ctx.params.to_vec(),
                });
            }
            McBlock::EscapeLoop => source.line("break;"),
            McBlock::ContinueLoop => source.line("continue;"),
            McBlock::Return { value } => match mode {
                ReturnMode::Block => source.line(format!("return {};", obj(value))),
                // Nothing here reports a value, so this just stops the handler.
                // The editor warns about it; the generated code honours the
                // "stop" half rather than dropping the block.
                ReturnMode::Event => source.line("return true;"),
                ReturnMode::Delayed => source.line("return;"),
            },
            McBlock::CancelEvent => match mode {
                ReturnMode::Event => source.line("return false;"),
                ReturnMode::Block => source.line("return null;"),
                ReturnMode::Delayed => source.line("return;"),
            },
            McBlock::CallBlock { block_id, args } => {
                let arguments = args
                    .iter()
                    .map(|value| obj(value))
                    .collect::<Vec<_>>()
                    .join(", ");
                source.line(format!(
                    "Handlers.{}({c}, new Object[] {{{arguments}}});",
                    crate::handlers::custom_block_method(block_id)
                ));
            }

            // Headers are only ever the first instruction of a strand, and
            // `handlers` strips that one before calling in here. Listing them
            // rather than falling through to a wildcard is what makes adding a
            // command to `McBlock` fail to compile until it is emitted.
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
            | McBlock::BlockHeader { .. } => {
                debug_assert!(
                    kind.is_header(),
                    "this arm is meant to cover exactly the header blocks"
                );
            }
        }
    }
}

fn target_constant(target: Target) -> &'static str {
    match target {
        Target::EventPlayer => "Rt.Target.EVENT_PLAYER",
        Target::EventEntity => "Rt.Target.EVENT_ENTITY",
        Target::AllPlayers => "Rt.Target.ALL_PLAYERS",
        Target::NearestPlayer => "Rt.Target.NEAREST_PLAYER",
    }
}

fn weather_constant(weather: Weather) -> &'static str {
    match weather {
        Weather::Clear => "Rt.Weather.CLEAR",
        Weather::Rain => "Rt.Weather.RAIN",
        Weather::Thunder => "Rt.Weather.THUNDER",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockstitch_core::value::Value;
    use stitchcraft_blocks::kinds::MessageKind;

    fn emit(instructions: Vec<McBlock>, mode: ReturnMode) -> (String, Vec<Pending>) {
        let project = ModProject::new("demo");
        let names = Names::of(&project);
        let mut emitter = Emitter::new(&names, &project);
        let mut source = Source::new();
        let wrapped: Vec<_> = instructions.into_iter().map(Instruction::new).collect();
        emitter.body(&mut source, &wrapped, mode, &[]);
        (source.finish(), emitter.take_pending())
    }

    #[test]
    fn a_command_becomes_one_runtime_call() {
        let (java, _) = emit(
            vec![McBlock::Message {
                target: Target::EventPlayer,
                kind: MessageKind::Chat,
                text: Value::text("hi"),
            }],
            ReturnMode::Event,
        );
        assert_eq!(
            java,
            "Rt.message(ctx, Rt.Target.EVENT_PLAYER, \"hi\", false);\n"
        );
    }

    #[test]
    fn bare_ids_are_namespaced_and_qualified_ones_are_left_alone() {
        let (java, _) = emit(
            vec![
                McBlock::SetBlockAt {
                    x: Value::number(0.0),
                    y: Value::number(0.0),
                    z: Value::number(0.0),
                    block: "chime".into(),
                },
                McBlock::SetBlockAt {
                    x: Value::number(0.0),
                    y: Value::number(0.0),
                    z: Value::number(0.0),
                    block: "minecraft:stone".into(),
                },
            ],
            ReturnMode::Event,
        );
        assert!(java.contains("\"demo:chime\""));
        assert!(java.contains("\"minecraft:stone\""));
    }

    #[test]
    fn a_repeat_reads_its_count_once() {
        let (java, _) = emit(
            vec![McBlock::Repeat {
                count: Value::Var {
                    name: "n".into(),
                },
                body: vec![Instruction::new(McBlock::ContinueLoop)],
            }],
            ReturnMode::Event,
        );
        assert!(
            java.contains("final int times1 = Rt.integer(Vars.get(\"n\"));"),
            "{java}"
        );
        assert!(java.contains("for (int i2 = 0; i2 < times1; i2++) {"));
        assert!(java.contains("continue;"));
    }

    #[test]
    fn a_while_loop_is_bounded() {
        let (java, _) = emit(
            vec![McBlock::While {
                condition: Value::op(blockstitch_core::value::Op::True, vec![]),
                body: vec![],
            }],
            ReturnMode::Event,
        );
        assert!(java.contains("Rt.LOOP_LIMIT && (true)"), "{java}");
    }

    #[test]
    fn if_else_writes_both_arms() {
        let (java, _) = emit(
            vec![McBlock::IfElse {
                condition: Value::Bool,
                then_body: vec![Instruction::new(McBlock::CancelEvent)],
                else_body: vec![Instruction::new(McBlock::EscapeLoop)],
            }],
            ReturnMode::Event,
        );
        assert_eq!(
            java,
            "if (false) {\n    return false;\n} else {\n    break;\n}\n"
        );
    }

    #[test]
    fn for_each_player_rebinds_the_context() {
        let (java, _) = emit(
            vec![McBlock::ForEachPlayer {
                body: vec![Instruction::new(McBlock::Message {
                    target: Target::EventPlayer,
                    kind: MessageKind::Chat,
                    text: Value::text("hi"),
                })],
            }],
            ReturnMode::Event,
        );
        assert!(java.contains("for (ServerPlayer player1 : Rt.players(ctx)) {"));
        assert!(java.contains("final Ctx ctx2 = ctx.withPlayer(player1);"));
        assert!(
            java.contains("Rt.message(ctx2,"),
            "the body must talk to the rebound context: {java}"
        );
    }

    #[test]
    fn a_delayed_body_becomes_a_method_of_its_own() {
        let (java, pending) = emit(
            vec![McBlock::ScheduleAfter {
                ticks: Value::number(40.0),
                body: vec![Instruction::new(McBlock::Broadcast {
                    text: Value::text("later"),
                })],
            }],
            ReturnMode::Event,
        );
        assert_eq!(
            java,
            "Scheduler.after(40, ctx, args, Handlers::delayed1);\n"
        );
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].method, "delayed1");
        assert_eq!(pending[0].mode, ReturnMode::Delayed);
    }

    #[test]
    fn return_means_something_different_in_each_mode() {
        let report = |mode| {
            emit(
                vec![McBlock::Return {
                    value: Value::number(7.0),
                }],
                mode,
            )
            .0
        };
        assert_eq!(report(ReturnMode::Block), "return Double.valueOf(7.0D);\n");
        assert_eq!(report(ReturnMode::Event), "return true;\n");
        assert_eq!(report(ReturnMode::Delayed), "return;\n");
    }
}
