//! The common mod class, the command bootstrap, and the per-loader entrypoints
//! and metadata.
//!
//! Almost everything a canvas produces is loader-agnostic, because FrozenLib's
//! `DeferredRegister` and `Event` abstract the difference away. Two things do
//! not, and so live here:
//!
//! * **entrypoints** - Fabric wants a `ModInitializer`, NeoForge wants a class
//!   annotated `@Mod`. Both do nothing but call into the common `init`.
//! * **commands** - FrozenLib has no command-registration event, so each loader
//!   gets a four-line bridge to its own: `CommandRegistrationCallback` on
//!   Fabric, `RegisterCommandsEvent` on NeoForge. The commands themselves are
//!   built once, in common code.

use crate::naming::Names;
use crate::runtime::banner;
use crate::writer::{Source, quote};
use serde_json::json;
use stitchcraft_blocks::{McBlock, ModProject};

/// The `/name` commands the canvas registers, paired with their handler.
fn commands(project: &ModProject) -> Vec<(String, bool, String)> {
    project
        .strands
        .iter()
        .enumerate()
        .filter_map(|(index, strand)| {
            let McBlock::OnCommand { name, op_only } = &strand.instructions.first()?.kind else {
                return None;
            };
            let name = name.trim();
            (!name.is_empty()).then(|| {
                (
                    name.to_string(),
                    *op_only,
                    format!("onCommand_{index}"),
                )
            })
        })
        .collect()
}

/// The common mod class: the constants, the id helper, and the init both loaders
/// call.
pub fn main_class(project: &ModProject, names: &Names) -> String {
    let main = &names.main_class;
    let has_entities = !crate::content::declared_bases(project).is_empty();
    let mut s = Source::new();
    s.line(format!("package {};", names.package));
    s.blank();
    banner(
        &mut s,
        names,
        "The common half of the mod: what both loaders call into.\n\nRegistration \
         order matters. Blocks come before the items that wrap them, and both come \
         before the creative tabs that list them, because a tab's contents are \
         resolved when the tab is built.",
    );
    s.lines([
        "import com.mojang.logging.LogUtils;",
        "import net.minecraft.resources.Identifier;",
        "import org.slf4j.Logger;",
        format!("import {}.ModBlocks;", names.registry_package()).as_str(),
        format!("import {}.ModEntities;", names.registry_package()).as_str(),
        format!("import {}.ModItems;", names.registry_package()).as_str(),
        format!("import {}.ModSounds;", names.registry_package()).as_str(),
        format!("import {}.ModTabs;", names.registry_package()).as_str(),
        format!("import {}.Handlers;", names.script_package()).as_str(),
        format!("import {}.Hooks;", names.script_package()).as_str(),
        format!("import {}.Ctx;", names.script_package()).as_str(),
        format!("import {}.Rt;", names.script_package()).as_str(),
        format!("import {}.Vars;", names.script_package()).as_str(),
    ]);
    s.blank();
    s.braced(format!("public final class {main}"), |s| {
        s.line(format!(
            "public static final String MOD_ID = {};",
            quote(&project.mod_id)
        ));
        s.line(format!(
            "public static final String VERSION = {};",
            quote(&project.version)
        ));
        s.line("public static final Logger LOGGER = LogUtils.getLogger();");
        s.blank();
        s.line(format!("private {main}() {{}}"));
        s.blank();
        s.doc("An id in this mod's namespace.");
        s.braced("public static Identifier id(String path)", |s| {
            s.line("return Identifier.fromNamespaceAndPath(MOD_ID, path);");
        });
        s.blank();
        s.doc(
            "Called from each loader's entrypoint. Everything the canvas declared \
             is registered here, then its hooks are wired up, then the \
             `when the mod loads` stacks run.",
        );
        s.braced("public static void init()", |s| {
            s.line("ModSounds.REGISTRY.register();");
            s.line("ModBlocks.REGISTRY.register();");
            s.line("ModItems.REGISTRY.register();");
            s.line("ModEntities.REGISTRY.register();");
            s.line("ModTabs.REGISTRY.register();");
            if has_entities {
                s.blank();
                s.comment("Without these a declared mob throws the first time it spawns.");
                s.line("ModEntities.registerAttributes();");
            }
            s.blank();
            s.line("declareState();");
            s.line("Hooks.register();");
            s.blank();
            let init = handler_methods(project, |hook| matches!(hook, McBlock::OnModInit));
            if init.is_empty() {
                s.comment("The canvas has no `when the mod loads` stack.");
            }
            for method in init {
                s.line(format!("Handlers.{method}(Ctx.of((net.minecraft.server.MinecraftServer) null), Rt.NO_ARGS);"));
            }
        });
        s.blank();
        s.doc(
            "Declares the canvas's variables, lists and dicts with the values it \
             was saved holding. `Vars.load` later replaces these with whatever the \
             last session ended on, keeping only the names still declared.",
        );
        s.braced("private static void declareState()", |s| {
            if project.variables.is_empty() && project.lists.is_empty() && project.dicts.is_empty() {
                s.comment("The canvas declares no variables, lists or dicts.");
                return;
            }
            for variable in &project.variables {
                s.line(format!(
                    "Vars.declareVariable({}, {});",
                    quote(&variable.name),
                    evaluated_literal(&variable.value)
                ));
            }
            for list in &project.lists {
                let items = list
                    .items
                    .iter()
                    .map(list_item_literal)
                    .collect::<Vec<_>>()
                    .join(", ");
                s.line(format!(
                    "Vars.declareList({}{}{items});",
                    quote(&list.name),
                    if list.items.is_empty() { "" } else { ", " }
                ));
            }
            for dict in &project.dicts {
                s.line(format!("Vars.declareDict({});", quote(&dict.name)));
                for entry in &dict.entries {
                    s.line(format!(
                        "Vars.declareDictEntry({}, {}, {});",
                        quote(&dict.name),
                        quote(&entry.key),
                        dict_item_literal(&entry.value)
                    ));
                }
            }
        });
        s.blank();
        s.doc("Called from the client entrypoint only.");
        s.braced("public static void initClient()", |s| {
            if has_entities {
                s.line(format!("{}.ModRenderers.register();", names.content_package()));
            }
            let client = handler_methods(project, |hook| matches!(hook, McBlock::OnClientInit));
            if client.is_empty() && !has_entities {
                s.comment("The canvas has no `when the client loads` stack.");
            }
            for method in client {
                s.line(format!("Handlers.{method}(Ctx.of((net.minecraft.server.MinecraftServer) null), Rt.NO_ARGS);"));
            }
        });
    });
    s.finish()
}

/// A persisted variable value as a Java literal.
fn evaluated_literal(value: &blockstitch_core::value::Evaluated) -> String {
    use blockstitch_core::value::Evaluated;
    match value {
        Evaluated::Number(n) => format!("Double.valueOf({})", crate::writer::double(*n)),
        Evaluated::Text(text) => quote(text),
        Evaluated::Bool(b) => {
            if *b {
                "Boolean.TRUE".to_string()
            } else {
                "Boolean.FALSE".to_string()
            }
        }
    }
}

fn list_item_literal(item: &blockstitch_core::graph::ListItem) -> String {
    use blockstitch_core::graph::ListItem;
    match item {
        ListItem::Number(n) => format!("Double.valueOf({})", crate::writer::double(*n)),
        ListItem::Text(text) => quote(text),
    }
}

fn dict_item_literal(item: &blockstitch_core::graph::DictItem) -> String {
    use blockstitch_core::graph::DictItem;
    match item {
        DictItem::Number(n) => format!("Double.valueOf({})", crate::writer::double(*n)),
        DictItem::Text(text) => quote(text),
    }
}

/// The generated method names of the handlers whose hook `matches`. Mirrors the
/// naming in `handlers.rs`, which keys off the strand's index.
fn handler_methods(project: &ModProject, matches: impl Fn(&McBlock) -> bool) -> Vec<String> {
    project
        .strands
        .iter()
        .enumerate()
        .filter_map(|(index, strand)| {
            let kind = &strand.instructions.first()?.kind;
            matches(kind).then(|| {
                let stem = match kind {
                    McBlock::OnModInit => "onModInit",
                    McBlock::OnClientInit => "onClientInit",
                    _ => "handler",
                };
                format!("{stem}_{index}")
            })
        })
        .collect()
}

/// `Commands.java` - the command tree, built once in common code so both loaders
/// register the same thing.
pub fn commands_java(project: &ModProject, names: &Names) -> String {
    let commands = commands(project);
    let mut s = Source::new();
    s.line(format!("package {};", names.script_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "The commands the canvas registers.\n\nFrozenLib has no \
         command-registration event, so this is the one piece of behaviour each \
         loader has to bridge to itself - see the `fabric` and `neoforge` \
         entrypoints. The tree itself is built here, once.",
    );
    s.lines([
        "import com.mojang.brigadier.CommandDispatcher;",
        "import net.minecraft.commands.CommandSourceStack;",
        "import net.minecraft.commands.Commands;",
        "import net.minecraft.server.level.ServerPlayer;",
    ]);
    s.blank();
    s.braced("public final class ModCommands", |s| {
        s.line("private ModCommands() {}");
        s.blank();
        s.doc(
            "Adds every generated command to `dispatcher`. Each one reports \
             success unless its stack cancelled, which is what makes \
             `cancel the event` show as a failed command.",
        );
        s.braced(
            "public static void register(CommandDispatcher<CommandSourceStack> dispatcher)",
            |s| {
                if commands.is_empty() {
                    s.comment("The canvas registers no commands.");
                    return;
                }
                for (name, op_only, method) in &commands {
                    s.open("dispatcher.register(");
                    s.open(format!("Commands.literal({})", quote(name)));
                    if *op_only {
                        s.line(".requires(source -> source.hasPermission(2))");
                    }
                    s.open(".executes(context -> {");
                    s.line("final CommandSourceStack source = context.getSource();");
                    s.comment(
                        "A command can come from the console, so the player is \
                         whatever the source happens to have.",
                    );
                    s.line("final ServerPlayer player = source.getPlayer();");
                    s.line("final Ctx ctx = player != null ? Ctx.of(player) : Ctx.of(source.getLevel());");
                    s.line(format!(
                        "return Handlers.{method}(ctx, Rt.NO_ARGS) ? 1 : 0;"
                    ));
                    s.close("})");
                    s.close("");
                    s.close(");");
                }
            },
        );
    });
    s.finish()
}

/// The Fabric subproject: two entrypoints and `fabric.mod.json`.
pub fn fabric_files(project: &ModProject, names: &Names) -> Vec<(String, String)> {
    let main = &names.main_class;
    let has_commands = !commands(project).is_empty();

    let mut s = Source::new();
    s.line(format!("package {}.fabric;", names.package));
    s.blank();
    banner(
        &mut s,
        names,
        "The Fabric entrypoint. It does nothing the common code does not, except \
         bridge command registration, which FrozenLib has no event for.",
    );
    s.lines([
        "import net.fabricmc.api.ModInitializer;",
        format!("import {}.{main};", names.package).as_str(),
    ]);
    if has_commands {
        s.line("import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;");
        s.line(format!("import {}.ModCommands;", names.script_package()));
    }
    s.blank();
    s.braced(format!("public final class {main}Fabric implements ModInitializer"), |s| {
        s.line("@Override");
        s.braced("public void onInitialize()", |s| {
            s.line(format!("{main}.init();"));
            if has_commands {
                s.open("CommandRegistrationCallback.EVENT.register((dispatcher, registry, environment) -> {");
                s.line("ModCommands.register(dispatcher);");
                s.close("});");
            }
        });
    });
    let entrypoint = s.finish();

    let mut c = Source::new();
    c.line(format!("package {}.fabric;", names.package));
    c.blank();
    banner(&mut c, names, "The Fabric client entrypoint.");
    c.lines([
        "import net.fabricmc.api.ClientModInitializer;",
        format!("import {}.{main};", names.package).as_str(),
    ]);
    c.blank();
    c.braced(
        format!("public final class {main}FabricClient implements ClientModInitializer"),
        |c| {
            c.line("@Override");
            c.braced("public void onInitializeClient()", |c| {
                c.line(format!("{main}.initClient();"));
            });
        },
    );
    let client = c.finish();

    let metadata = json!({
        "schemaVersion": 1,
        "id": project.mod_id,
        "version": project.version,
        "name": project.name,
        "description": if project.description.trim().is_empty() {
            format!("{} - built with Stitchcraft.", project.name)
        } else {
            project.description.clone()
        },
        "authors": project.authors,
        "license": project.license,
        "environment": "*",
        "entrypoints": {
            "main": [format!("{}.fabric.{main}Fabric", names.package)],
            "client": [format!("{}.fabric.{main}FabricClient", names.package)],
        },
        "depends": {
            "fabricloader": format!(">={}", project.targets.fabric_loader_version),
            "minecraft": format!("~{}", project.targets.minecraft_version),
            "java": format!(">={}", project.targets.java_version),
            "fabric-api": "*",
            "frozenlib": "*",
        },
    });

    vec![
        (names.fabric(&format!("{main}Fabric.java")), entrypoint),
        (names.fabric(&format!("{main}FabricClient.java")), client),
        (
            "fabric/src/main/resources/fabric.mod.json".to_string(),
            {
                let mut text = serde_json::to_string_pretty(&metadata).unwrap_or_default();
                text.push('\n');
                text
            },
        ),
    ]
}

/// The NeoForge subproject: one `@Mod` class and `neoforge.mods.toml`.
pub fn neoforge_files(project: &ModProject, names: &Names) -> Vec<(String, String)> {
    let main = &names.main_class;
    let has_commands = !commands(project).is_empty();

    let mut s = Source::new();
    s.line(format!("package {}.neoforge;", names.package));
    s.blank();
    banner(
        &mut s,
        names,
        "The NeoForge entrypoint.\n\nThe common `init` runs from the constructor, \
         which is where NeoForge expects registration to happen: FrozenLib's \
         `DeferredRegister.register()` needs the mod event bus, and that is only \
         available while the mod is being constructed.",
    );
    s.lines([
        "import net.neoforged.fml.common.Mod;",
        format!("import {}.{main};", names.package).as_str(),
    ]);
    if has_commands {
        s.lines([
            "import net.neoforged.bus.api.SubscribeEvent;",
            "import net.neoforged.fml.common.EventBusSubscriber;",
            "import net.neoforged.neoforge.event.RegisterCommandsEvent;",
        ]);
        s.line(format!("import {}.ModCommands;", names.script_package()));
    }
    s.blank();
    s.line(format!("@Mod({main}.MOD_ID)"));
    s.braced(format!("public final class {main}NeoForge"), |s| {
        s.braced(format!("public {main}NeoForge()"), |s| {
            s.line(format!("{main}.init();"));
            s.comment(
                "The client half runs from a dist-specific listener rather than \
                 here, so a dedicated server never touches client classes.",
            );
        });
        if has_commands {
            s.blank();
            s.doc("Bridges NeoForge's command event to the shared command tree.");
            s.line(format!("@EventBusSubscriber(modid = {main}.MOD_ID)"));
            s.braced("public static final class Commands", |s| {
                s.line("private Commands() {}");
                s.blank();
                s.line("@SubscribeEvent");
                s.braced("public static void onRegisterCommands(RegisterCommandsEvent event)", |s| {
                    s.line("ModCommands.register(event.getDispatcher());");
                });
            });
        }
    });
    let entrypoint = s.finish();

    let mut c = Source::new();
    c.line(format!("package {}.neoforge;", names.package));
    c.blank();
    banner(
        &mut c,
        names,
        "The NeoForge client entrypoint, kept separate so a dedicated server \
         never loads it.",
    );
    c.lines([
        "import net.neoforged.api.distmarker.Dist;",
        "import net.neoforged.bus.api.SubscribeEvent;",
        "import net.neoforged.fml.common.EventBusSubscriber;",
        "import net.neoforged.fml.event.lifecycle.FMLClientSetupEvent;",
        format!("import {}.{main};", names.package).as_str(),
    ]);
    c.blank();
    c.line(format!(
        "@EventBusSubscriber(modid = {main}.MOD_ID, value = Dist.CLIENT, bus = EventBusSubscriber.Bus.MOD)"
    ));
    c.braced(format!("public final class {main}NeoForgeClient"), |c| {
        c.line(format!("private {main}NeoForgeClient() {{}}"));
        c.blank();
        c.line("@SubscribeEvent");
        c.braced("public static void onClientSetup(FMLClientSetupEvent event)", |c| {
            c.line(format!("event.enqueueWork({main}::initClient);"));
        });
    });
    let client = c.finish();

    vec![
        (names.neoforge(&format!("{main}NeoForge.java")), entrypoint),
        (
            names.neoforge(&format!("{main}NeoForgeClient.java")),
            client,
        ),
        (
            "neoforge/src/main/resources/META-INF/neoforge.mods.toml".to_string(),
            mods_toml(project, names),
        ),
    ]
}

/// `neoforge.mods.toml`. Written by hand rather than through a TOML crate because
/// the file is a fixed shape and the only variable parts are already escaped.
fn mods_toml(project: &ModProject, names: &Names) -> String {
    let escape = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let description = if project.description.trim().is_empty() {
        format!("{} - built with Stitchcraft.", project.name)
    } else {
        project.description.clone()
    };
    let authors = project.authors.join(", ");
    let mut out = String::new();
    out.push_str("modLoader = \"javafml\"\n");
    out.push_str("loaderVersion = \"[4,)\"\n");
    out.push_str(&format!("license = \"{}\"\n\n", escape(&project.license)));
    out.push_str("[[mods]]\n");
    out.push_str(&format!("modId = \"{}\"\n", escape(&project.mod_id)));
    out.push_str(&format!("version = \"{}\"\n", escape(&project.version)));
    out.push_str(&format!("displayName = \"{}\"\n", escape(&project.name)));
    if !authors.is_empty() {
        out.push_str(&format!("authors = \"{}\"\n", escape(&authors)));
    }
    // A triple-quoted string keeps a multi-line description from needing escapes.
    out.push_str(&format!("description = '''\n{description}\n'''\n\n"));
    out.push_str(&format!(
        "[[dependencies.{}]]\n",
        escape(&project.mod_id)
    ));
    out.push_str("modId = \"neoforge\"\n");
    out.push_str("type = \"required\"\n");
    out.push_str(&format!(
        "versionRange = \"[{},)\"\n",
        escape(&project.targets.neoforge_version)
    ));
    out.push_str("ordering = \"NONE\"\n");
    out.push_str("side = \"BOTH\"\n\n");
    out.push_str(&format!(
        "[[dependencies.{}]]\n",
        escape(&project.mod_id)
    ));
    out.push_str("modId = \"frozenlib\"\n");
    out.push_str("type = \"required\"\n");
    out.push_str("versionRange = \"[0,)\"\n");
    out.push_str("ordering = \"BEFORE\"\n");
    out.push_str("side = \"BOTH\"\n");
    let _ = names;
    out
}
