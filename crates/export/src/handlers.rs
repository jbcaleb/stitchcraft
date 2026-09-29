//! `Handlers.java` and `Hooks.java`: one method per strand, and the code that
//! wires those methods to FrozenLib's cross-platform events.
//!
//! Every handler has the same shape - `(Ctx ctx, Object[] args)` - so a hook, a
//! delayed body and a custom block are all callable the same way, and the
//! scheduler needs one functional interface rather than three. The return type
//! is what differs: a hook reports whether the event should carry on, a custom
//! block reports its value, and a delayed body reports nothing.
//!
//! Hooks that FrozenLib has no event for - a block being right-clicked, an item
//! being used, one of the mod's own mobs ticking - are dispatched from the
//! generated content classes instead, through the tables at the bottom of
//! `Handlers`. `content.rs` writes the other end of that.

use crate::naming::{self, Names};
use crate::runtime::banner;
use crate::stmt::{Emitter, ReturnMode};
use crate::writer::{Source, quote};
use blockstitch_core::graph::{BlockKind, Instruction, Strand};
use stitchcraft_blocks::{McBlock, ModProject};

/// The Java method name for a custom block's body. Derived from the definition
/// id rather than its label, because the label can be renamed and the id cannot.
pub fn custom_block_method(block_id: &str) -> String {
    format!("block_{}", naming::identifier(block_id))
}

/// One strand that runs code, resolved into what the emitters need.
struct Handler<'a> {
    /// The generated method name.
    method: String,
    /// The header that decides where it is wired up.
    hook: &'a McBlock,
    /// The strand tail - the handler's actual body.
    body: &'a [Instruction<McBlock>],
    mode: ReturnMode,
    /// A custom block's inputs, in prototype order.
    params: Vec<String>,
}

/// Collects every strand that produces a method, in document order. Declaration
/// strands and headerless ones produce nothing; `validate` has already said so.
fn handlers(project: &ModProject) -> Vec<Handler<'_>> {
    let mut out = Vec::new();
    for (index, strand) in project.strands.iter().enumerate() {
        let Some(first) = strand.instructions.first() else {
            continue;
        };
        if !first.kind.is_header() || first.kind.is_declaration() {
            continue;
        }
        let body = &strand.instructions[1..];
        let (method, mode, params) = match &first.kind {
            McBlock::BlockHeader { block_id } => {
                let def = project.block_def(block_id);
                let params = def
                    .map(|def| def.input_names().map(str::to_string).collect())
                    .unwrap_or_default();
                (custom_block_method(block_id), ReturnMode::Block, params)
            }
            // The strand index keeps two hooks of the same kind apart, and keeps
            // the name stable as long as the strand order is.
            other => (
                format!("{}_{index}", naming::identifier(hook_stem(other))),
                ReturnMode::Event,
                Vec::new(),
            ),
        };
        out.push(Handler {
            method,
            hook: &first.kind,
            body,
            mode,
            params,
        });
    }
    out
}

/// The method-name stem for a hook - its variant name, lowercased at the front.
fn hook_stem(hook: &McBlock) -> &'static str {
    match hook {
        McBlock::OnModInit => "onModInit",
        McBlock::OnClientInit => "onClientInit",
        McBlock::OnServerStarted => "onServerStarted",
        McBlock::OnServerStopping => "onServerStopping",
        McBlock::OnServerTick => "onServerTick",
        McBlock::OnLevelTick => "onLevelTick",
        McBlock::OnPlayerJoin => "onPlayerJoin",
        McBlock::OnPlayerLeave => "onPlayerLeave",
        McBlock::OnBlockPlaced { .. } => "onBlockPlaced",
        McBlock::OnBlockBroken { .. } => "onBlockBroken",
        McBlock::OnBlockUsed { .. } => "onBlockUsed",
        McBlock::OnBlockTick { .. } => "onBlockTick",
        McBlock::OnItemUsed { .. } => "onItemUsed",
        McBlock::OnEntityTick { .. } => "onEntityTick",
        McBlock::OnEntityHurt => "onEntityHurt",
        McBlock::OnEntityDeath => "onEntityDeath",
        McBlock::OnCommand { .. } => "onCommand",
        McBlock::BlockHeader { .. } => "block",
        _ => "handler",
    }
}

/// `Handlers.java` - every strand's body, plus the per-id dispatch tables the
/// content classes call into.
pub fn handlers_java(project: &ModProject, names: &Names) -> String {
    let handlers = handlers(project);
    let mut s = Source::new();
    s.line(format!("package {};", names.script_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "One method per stack on the canvas.\n\nAn event handler reports whether \
         the event should carry on: `false` is the `cancel the event` block, and \
         only the hooks that can be cancelled pay attention to it. A custom \
         block reports its value, and a delayed body reports nothing.",
    );
    s.lines([
        "import java.util.Map;",
        "import net.frozenblock.lib.platform.ModLoader;",
        "import net.minecraft.server.level.ServerPlayer;",
        format!("import {}.{};", names.package, names.main_class).as_str(),
    ]);
    s.blank();
    s.braced("public final class Handlers", |s| {
        s.line("private Handlers() {}");

        let mut emitter = Emitter::new(names, project);
        for handler in &handlers {
            s.blank();
            write_method(s, &mut emitter, handler, project);
        }

        // Delayed bodies queue more delayed bodies, so this drains rather than
        // iterating once.
        let mut pending = emitter.take_pending();
        while !pending.is_empty() {
            for delayed in pending {
                s.blank();
                s.doc("A delayed body, from an `after N ticks` block.");
                s.braced(
                    format!(
                        "public static void {}(Ctx ctx, Object[] args)",
                        delayed.method
                    ),
                    |s| {
                        emitter.body(s, &delayed.body, delayed.mode, &delayed.params);
                    },
                );
            }
            pending = emitter.take_pending();
        }

        write_dispatch_tables(s, &handlers, names);
    });
    s.finish()
}

fn write_method(
    s: &mut Source,
    emitter: &mut Emitter<'_>,
    handler: &Handler<'_>,
    project: &ModProject,
) {
    let signature = match handler.mode {
        ReturnMode::Event => format!("public static boolean {}(Ctx ctx, Object[] args)", handler.method),
        ReturnMode::Block => format!("public static Object {}(Ctx ctx, Object[] args)", handler.method),
        ReturnMode::Delayed => format!("public static void {}(Ctx ctx, Object[] args)", handler.method),
    };
    s.doc(&describe(handler.hook, project));
    s.braced(signature, |s| {
        emitter.body(s, handler.body, handler.mode, &handler.params);
        match handler.mode {
            // Falling off the end of a handler means "nothing cancelled it".
            ReturnMode::Event => s.line("return true;"),
            // A reporter-shaped block with no `report` block reports nothing.
            ReturnMode::Block => s.line("return Double.valueOf(0.0D);"),
            ReturnMode::Delayed => {}
        }
    });
}

/// The doc comment a handler gets, so the generated file reads as a map back to
/// the canvas rather than a list of opaque methods.
fn describe(hook: &McBlock, project: &ModProject) -> String {
    match hook {
        McBlock::OnBlockPlaced { block_id } if block_id.trim().is_empty() => {
            "Runs after any block is placed.".to_string()
        }
        McBlock::OnBlockPlaced { block_id } => format!("Runs after `{block_id}` is placed."),
        McBlock::OnBlockBroken { block_id } if block_id.trim().is_empty() => {
            "Runs before any block is broken by a player.".to_string()
        }
        McBlock::OnBlockBroken { block_id } => {
            format!("Runs before a player breaks `{block_id}`. Cancellable.")
        }
        McBlock::OnBlockUsed { block_id } => {
            format!("Runs when a player right-clicks `{block_id}`. Cancellable.")
        }
        McBlock::OnBlockTick { block_id } => format!("Runs on `{block_id}`'s random tick."),
        McBlock::OnItemUsed { item_id } => {
            format!("Runs when a player uses `{item_id}`. Cancellable.")
        }
        McBlock::OnEntityTick { entity_id } => format!("Runs every tick of a `{entity_id}`."),
        McBlock::OnCommand { name, op_only } => format!(
            "Runs on `/{name}`{}.",
            if *op_only {
                ", for operators only"
            } else {
                ""
            }
        ),
        McBlock::BlockHeader { block_id } => match project.block_def(block_id) {
            Some(def) => format!(
                "The custom block `{}`.",
                def.pieces
                    .iter()
                    .map(|piece| match piece {
                        blockstitch_core::graph::BlockPiece::Label { text, .. } => text.clone(),
                        blockstitch_core::graph::BlockPiece::Input { name, .. }
                        | blockstitch_core::graph::BlockPiece::Branch { name, .. } =>
                            format!("({name})"),
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            None => "A custom block whose definition is missing.".to_string(),
        },
        McBlock::OnModInit => "Runs once, after every registry is handed to the loader.".to_string(),
        McBlock::OnClientInit => "Runs once on the client.".to_string(),
        McBlock::OnServerStarted => "Runs when the server has started.".to_string(),
        McBlock::OnServerStopping => "Runs as the server shuts down.".to_string(),
        McBlock::OnServerTick => "Runs every server tick.".to_string(),
        McBlock::OnLevelTick => "Runs every tick of every loaded level.".to_string(),
        McBlock::OnPlayerJoin => "Runs when a player joins.".to_string(),
        McBlock::OnPlayerLeave => "Runs when a player leaves.".to_string(),
        McBlock::OnEntityHurt => {
            "Runs before any living entity takes damage. Cancellable.".to_string()
        }
        McBlock::OnEntityDeath => "Runs after any living entity dies.".to_string(),
        _ => "Generated handler.".to_string(),
    }
}

/// The tables the generated content classes look their handler up in. A block
/// that no hook targets is absent, which is how `ScriptedBlock` knows it has
/// nothing to dispatch.
fn write_dispatch_tables(s: &mut Source, handlers: &[Handler<'_>], names: &Names) {
    s.blank();
    s.comment(
        "── Dispatch ──────────────────────────────────────────────────────────",
    );
    s.doc(
        "What a generated block, item or mob calls when something happens to it. \
         FrozenLib has no cross-platform event for these, so the content classes \
         below override the game's own methods and come here.",
    );
    s.braced("public interface Hook", |s| {
        s.line("boolean run(Ctx ctx, Object[] args);");
    });

    for (name, table) in [
        ("BLOCK_USED", targets(handlers, &names.mod_id, |hook| match hook {
            McBlock::OnBlockUsed { block_id } => Some(block_id.clone()),
            _ => None,
        })),
        ("BLOCK_PLACED", targets(handlers, &names.mod_id, |hook| match hook {
            McBlock::OnBlockPlaced { block_id } => Some(block_id.clone()),
            _ => None,
        })),
        ("BLOCK_TICKED", targets(handlers, &names.mod_id, |hook| match hook {
            McBlock::OnBlockTick { block_id } => Some(block_id.clone()),
            _ => None,
        })),
        ("ITEM_USED", targets(handlers, &names.mod_id, |hook| match hook {
            McBlock::OnItemUsed { item_id } => Some(item_id.clone()),
            _ => None,
        })),
        ("ENTITY_TICKED", targets(handlers, &names.mod_id, |hook| match hook {
            McBlock::OnEntityTick { entity_id } => Some(entity_id.clone()),
            _ => None,
        })),
    ] {
        s.blank();
        if table.is_empty() {
            s.line(format!(
                "public static final Map<String, Hook> {name} = Map.of();"
            ));
            continue;
        }
        s.comment(
            "Several hooks can target one id; they run in canvas order, and the \
             first to cancel stops the rest.",
        );
        s.open(format!("public static final Map<String, Hook> {name} = Map.ofEntries("));
        let entries: Vec<String> = group(&table)
            .into_iter()
            .map(|(id, methods)| {
                let chain = methods
                    .iter()
                    .map(|method| format!("Handlers::{method}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Map.entry({}, chain({chain}))", quote(&id))
            })
            .collect();
        for (index, entry) in entries.iter().enumerate() {
            let comma = if index + 1 < entries.len() { "," } else { "" };
            s.line(format!("{entry}{comma}"));
        }
        s.close(");");
    }

    s.blank();
    s.doc(
        "Runs several hooks in order, stopping at the first that cancels. One \
         `Hook` per id keeps the content classes from having to know how many \
         there are.",
    );
    s.braced("private static Hook chain(Hook... hooks)", |s| {
        s.line("if (hooks.length == 1) return hooks[0];");
        s.open("return (ctx, args) -> {");
        s.open("for (Hook hook : hooks) {");
        s.line("if (!hook.run(ctx, args)) return false;");
        s.close("}");
        s.line("return true;");
        s.close("};");
    });
    s.blank();
    s.doc(
        "Runs the hook registered for `id`, if any. `true` when nothing was \
         registered or nothing cancelled - so a caller can always treat it as \
         \"carry on\".",
    );
    s.braced("public static boolean dispatch(Map<String, Hook> table, String id, Ctx ctx)", |s| {
        s.line("final Hook hook = table.get(id);");
        s.line("if (hook == null) return true;");
        s.open("try {");
        s.line("return hook.run(ctx, Rt.NO_ARGS);");
        s.pivot("} catch (Exception e) {");
        s.line(format!(
            "{}.LOGGER.error(\"Stitchcraft: a handler for {{}} failed\", id, e);",
            names.main_class
        ));
        s.line("return true;");
        s.close("}");
    });
}

/// `(id, method)` pairs for every handler whose hook `pick` matches. Ids are
/// qualified, because the content classes look themselves up by the fully
/// qualified key the registry gives them.
fn targets(
    handlers: &[Handler<'_>],
    mod_id: &str,
    pick: impl Fn(&McBlock) -> Option<String>,
) -> Vec<(String, String)> {
    handlers
        .iter()
        .filter_map(|handler| {
            let id = pick(handler.hook)?;
            let id = id.trim();
            (!id.is_empty()).then(|| (naming::qualify(mod_id, id), handler.method.clone()))
        })
        .collect()
}

/// Groups `(id, method)` pairs by id, keeping document order within each.
fn group(pairs: &[(String, String)]) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for (id, method) in pairs {
        match out.iter_mut().find(|(existing, _)| existing == id) {
            Some((_, methods)) => methods.push(method.clone()),
            None => out.push((id.clone(), vec![method.clone()])),
        }
    }
    out
}

/// `Hooks.java` - registering the handlers that FrozenLib does have an event for.
pub fn hooks_java(project: &ModProject, names: &Names) -> String {
    let handlers = handlers(project);
    let main = &names.main_class;
    let mut s = Source::new();
    s.line(format!("package {};", names.script_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "Wiring the canvas's event hooks to FrozenLib's cross-platform events, \
         which is what lets one set of handlers serve both Fabric and NeoForge.\n\n\
         The hooks with no FrozenLib event - a block being used, an item being \
         used, one of the mod's own mobs ticking - are dispatched from the \
         generated content classes instead; see `Handlers`.",
    );
    s.lines([
        "import net.frozenblock.lib.event.api.events.LifecycleEvents;",
        "import net.frozenblock.lib.event.api.events.PlayerBlockBreakEvents;",
        "import net.frozenblock.lib.event.api.events.ServerLivingEntityEvents;",
        "import net.frozenblock.lib.event.api.events.ServerPlayerEvents;",
        "import net.frozenblock.lib.event.api.events.TickEvents;",
        "import net.minecraft.core.registries.BuiltInRegistries;",
        "import net.minecraft.server.level.ServerLevel;",
        "import net.minecraft.server.level.ServerPlayer;",
        format!("import {}.{main};", names.package).as_str(),
    ]);
    s.blank();
    s.braced("public final class Hooks", |s| {
        s.line("private Hooks() {}");
        s.blank();
        s.doc(
            "Called from the common mod init, after every registry is handed to \
             the loader. The server tick is always registered, because the \
             scheduler behind `after N ticks` rides on it.",
        );
        s.braced("public static void register()", |s| {
            s.comment("The scheduler needs the tick whether or not the canvas asked for it.");
            s.open("TickEvents.END_SERVER_TICK.register(server -> {");
            s.line("Scheduler.tick(server);");
            for handler in with_hook(&handlers, |hook| matches!(hook, McBlock::OnServerTick)) {
                s.line(format!("run(Ctx.of(server), Handlers::{handler});"));
            }
            s.close("});");

            let level_tick = with_hook(&handlers, |hook| matches!(hook, McBlock::OnLevelTick));
            if !level_tick.is_empty() {
                s.blank();
                s.open("TickEvents.END_LEVEL_TICK.register(level -> {");
                for handler in level_tick {
                    s.line(format!("run(Ctx.of(level), Handlers::{handler});"));
                }
                s.close("});");
            }

            for (event, matcher, context) in [
                (
                    "LifecycleEvents.SERVER_STARTED",
                    Matcher::Started,
                    "Ctx.of(server)",
                ),
                (
                    "LifecycleEvents.SERVER_STOPPING",
                    Matcher::Stopping,
                    "Ctx.of(server)",
                ),
            ] {
                let picked = with_hook(&handlers, |hook| matcher.matches(hook));
                s.blank();
                s.open(format!("{event}.register(server -> {{"));
                if matcher == Matcher::Started {
                    s.comment("Saved variables, lists and dicts come back before anything reads them.");
                    s.line("Vars.load();");
                }
                for handler in picked {
                    s.line(format!("run({context}, Handlers::{handler});"));
                }
                if matcher == Matcher::Stopping {
                    s.comment("...and go back to disk once nothing can change them again.");
                    s.line("Vars.save();");
                    s.line("Scheduler.clear();");
                }
                s.close("});");
            }

            let join = with_hook(&handlers, |hook| matches!(hook, McBlock::OnPlayerJoin));
            if !join.is_empty() {
                s.blank();
                s.open("ServerPlayerEvents.JOIN.register((server, player) -> {");
                for handler in join {
                    s.line(format!("run(Ctx.of(player), Handlers::{handler});"));
                }
                s.close("});");
            }

            let leave = with_hook(&handlers, |hook| matches!(hook, McBlock::OnPlayerLeave));
            if !leave.is_empty() {
                s.blank();
                s.open("ServerPlayerEvents.LEAVE.register((server, player) -> {");
                for handler in leave {
                    s.line(format!("run(Ctx.of(player), Handlers::{handler});"));
                }
                s.close("});");
            }

            let broken = filtered(&handlers, &project.mod_id, |hook| match hook {
                McBlock::OnBlockBroken { block_id } => Some(block_id.as_str()),
                _ => None,
            });
            if !broken.is_empty() {
                s.blank();
                s.comment(
                    "BEFORE rather than AFTER, so `cancel the event` can actually \
                     stop the break: returning false cancels it. One listener \
                     serves every \"when X is broken\" strand, each checking its \
                     own filter - a strand with no block named matches any.",
                );
                s.open("PlayerBlockBreakEvents.BEFORE.register((level, player, pos, state, blockEntity) -> {");
                s.line("if (!(level instanceof ServerLevel serverLevel)) return true;");
                s.line("final String id = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();");
                s.line("final Ctx ctx = Ctx.of(serverLevel, pos, state).withPlayer(player instanceof ServerPlayer breaker ? breaker : null);");
                for (method, filter) in broken {
                    let guard = match filter {
                        Some(id) => format!("id.equals({}) && ", quote(&id)),
                        None => String::new(),
                    };
                    s.line(format!(
                        "if ({guard}!Handlers.{method}(ctx, Rt.NO_ARGS)) return false;"
                    ));
                }
                s.line("return true;");
                s.close("});");
            }

            let hurt = with_hook(&handlers, |hook| matches!(hook, McBlock::OnEntityHurt));
            if !hurt.is_empty() {
                s.blank();
                s.open("ServerLivingEntityEvents.ALLOW_DAMAGE.register((entity, source, amount) -> {");
                s.line("final Ctx ctx = Ctx.of(entity).withDamage(amount);");
                for handler in hurt {
                    s.line(format!(
                        "if (!Handlers.{handler}(ctx, Rt.NO_ARGS)) return false;"
                    ));
                }
                s.line("return true;");
                s.close("});");
            }

            let death = with_hook(&handlers, |hook| matches!(hook, McBlock::OnEntityDeath));
            if !death.is_empty() {
                s.blank();
                s.open("ServerLivingEntityEvents.AFTER_DEATH.register((entity, source) -> {");
                for handler in death {
                    s.line(format!("run(Ctx.of(entity), Handlers::{handler});"));
                }
                s.close("});");
            }

        });
        s.blank();
        s.doc(
            "Runs a handler for its side effects, swallowing whatever it throws. \
             A handler runs inside the server tick, and an exception there would \
             take the world down rather than just this block.",
        );
        s.braced("private static void run(Ctx ctx, Handlers.Hook hook)", |s| {
            s.open("try {");
            s.line("hook.run(ctx, Rt.NO_ARGS);");
            s.pivot("} catch (Exception e) {");
            s.line(format!(
                "{main}.LOGGER.error(\"Stitchcraft: a handler failed\", e);"
            ));
            s.close("}");
        });
    });
    s.finish()
}

/// Which hook an event registration is looking for, where `matches!` in a
/// closure would need naming a variant twice.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Matcher {
    Started,
    Stopping,
}

impl Matcher {
    fn matches(self, hook: &McBlock) -> bool {
        match self {
            Matcher::Started => matches!(hook, McBlock::OnServerStarted),
            Matcher::Stopping => matches!(hook, McBlock::OnServerStopping),
        }
    }
}

/// `(method, filter)` for each handler `pick` matches, where the filter is the
/// fully qualified id the hook is for - or `None` when it named nothing and so
/// matches everything.
fn filtered<'a>(
    handlers: &[Handler<'a>],
    mod_id: &str,
    pick: impl Fn(&'a McBlock) -> Option<&'a str>,
) -> Vec<(String, Option<String>)> {
    handlers
        .iter()
        .filter_map(|handler| {
            let id = pick(handler.hook)?.trim();
            let filter = (!id.is_empty()).then(|| naming::qualify(mod_id, id));
            Some((handler.method.clone(), filter))
        })
        .collect()
}

/// The method names of the handlers whose hook matches.
fn with_hook(handlers: &[Handler<'_>], matches: impl Fn(&McBlock) -> bool) -> Vec<String> {
    handlers
        .iter()
        .filter(|handler| matches(handler.hook))
        .map(|handler| handler.method.clone())
        .collect()
}

/// The strands a document would compile to methods for - used by the tests and
/// by `lib.rs`'s summary.
pub fn handler_count(project: &ModProject) -> usize {
    handlers(project).len()
}

/// The declaration strands, paired with their header, for the registry emitters.
pub fn declarations(project: &ModProject) -> Vec<(&Strand<McBlock>, &McBlock)> {
    project
        .strands
        .iter()
        .filter_map(|strand| {
            let first = strand.instructions.first()?;
            first.kind.is_declaration().then_some((strand, &first.kind))
        })
        .collect()
}
