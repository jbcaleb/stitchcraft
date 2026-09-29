//! Compiling a Stitchcraft canvas into a Minecraft mod for Fabric and NeoForge.
//!
//! [`export`] turns a [`ModProject`] into a [`Bundle`] - a whole source tree in
//! memory - which [`Bundle::write_to`] then puts on disk. Nothing is written
//! until the document validates, so a half-exported project is not a state this
//! can leave you in.
//!
//! # How one set of sources serves two loaders
//!
//! FrozenLib does the work that would otherwise be duplicated:
//!
//! * [`registry`] builds on its `DeferredRegister`, which registers immediately
//!   on Fabric and defers to NeoForge's own deferred register on NeoForge;
//! * [`handlers`] wires the canvas's hooks to its cross-platform `Event`s;
//! * entity attributes go through its `DefaultAttributeRegistry`, and client-side
//!   renderers through its `EntityRendererRegistry`.
//!
//! What FrozenLib has no answer for is generated per loader, and there is very
//! little of it: an entrypoint class each, and a four-line command bridge, both
//! in [`loaders`].
//!
//! # What the generated code looks like
//!
//! One canvas stack becomes one Java method. Every command becomes a call into
//! `Rt.java`, which is the only generated file that touches Minecraft's own API -
//! so a game update that renames a method is a fix in one file rather than
//! everywhere the canvas used it.
//!
//! ```no_run
//! use stitchcraft_blocks::{McBlock, ModProject, catalog};
//! use blockstitch_core::graph::Instruction;
//!
//! let mut project = ModProject::new("wonder_blocks");
//! project.graph.add_strand(
//!     0,
//!     0,
//!     vec![
//!         Instruction::new(McBlock::OnPlayerJoin),
//!         Instruction::new(catalog::prefab("Broadcast").unwrap()),
//!     ],
//! );
//!
//! let bundle = stitchcraft_export::export(&project).expect("a valid canvas");
//! bundle.write_to("out/wonder_blocks").unwrap();
//! ```

pub mod assets;
pub mod content;
pub mod expr;
pub mod gradle;
pub mod handlers;
pub mod loaders;
pub mod naming;
pub mod registry;
pub mod runtime;
pub mod stmt;
pub mod writer;

use naming::Names;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use stitchcraft_blocks::{ModProject, Report, validate};

/// A complete mod source tree, held in memory.
///
/// Paths are relative, `/`-separated and sorted, so two exports of the same
/// document produce byte-identical output and a diff between versions reads.
#[derive(Debug, Clone, Default)]
pub struct Bundle {
    pub files: BTreeMap<String, String>,
    /// Everything worth saying about the export: the document's own warnings,
    /// plus the ones only the exporter can see.
    pub warnings: Vec<String>,
}

impl Bundle {
    /// Writes every file under `root`, creating directories as needed.
    ///
    /// Existing files are overwritten and nothing else is removed, so a stale
    /// class from a renamed declaration survives - deliberately, because
    /// deleting files under a directory the user chose is not this function's
    /// call. [`Bundle::stale_files`] finds them.
    pub fn write_to(&self, root: impl AsRef<Path>) -> io::Result<()> {
        let root = root.as_ref();
        for (path, contents) in &self.files {
            let target = root.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, contents)?;
        }
        Ok(())
    }

    /// Generated-looking files already under `root` that this export would not
    /// write - what a renamed or deleted declaration leaves behind. Only paths
    /// inside the directories the exporter owns are considered.
    pub fn stale_files(&self, root: impl AsRef<Path>) -> Vec<String> {
        let root = root.as_ref();
        let owned = ["common/src/main", "fabric/src/main", "neoforge/src/main"];
        let mut out = Vec::new();
        for prefix in owned {
            collect(&root.join(prefix), prefix, &self.files, &mut out);
        }
        out.sort();
        out
    }

    /// Total size of the export, for the summary the app shows.
    pub fn byte_count(&self) -> usize {
        self.files.values().map(String::len).sum()
    }
}

fn collect(
    directory: &Path,
    prefix: &str,
    known: &BTreeMap<String, String>,
    out: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let relative = format!("{prefix}/{name}");
        if entry.path().is_dir() {
            collect(&entry.path(), &relative, known, out);
        } else if !known.contains_key(&relative) {
            out.push(relative);
        }
    }
}

/// Compiles `project`. Returns the document's [`Report`] unchanged when it has
/// errors, so the caller can show the same diagnostics the editor does.
pub fn export(project: &ModProject) -> Result<Bundle, Report> {
    let report = validate::check(project);
    if report.has_errors() {
        return Err(report);
    }
    let names = Names::of(project);
    let mut warnings: Vec<String> = report
        .warnings()
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    let mut files = BTreeMap::new();

    let mut add = |path: String, contents: String| {
        files.insert(path, contents);
    };

    // ── Common ────────────────────────────────────────────────────────────
    add(
        names.common(&format!("{}.java", names.main_class)),
        loaders::main_class(project, &names),
    );
    for (path, contents) in runtime::files(&names) {
        add(path, contents);
    }
    add(
        names.common("script/Handlers.java"),
        handlers::handlers_java(project, &names),
    );
    add(
        names.common("script/Hooks.java"),
        handlers::hooks_java(project, &names),
    );
    add(
        names.common("script/ModCommands.java"),
        loaders::commands_java(project, &names),
    );
    add(
        names.common("registry/ModBlocks.java"),
        registry::blocks_java(project, &names, &mut warnings),
    );
    add(
        names.common("registry/ModItems.java"),
        registry::items_java(project, &names, &mut warnings),
    );
    add(
        names.common("registry/ModEntities.java"),
        registry::entities_java(project, &names, &mut warnings),
    );
    add(
        names.common("registry/ModSounds.java"),
        registry::sounds_java(project, &names),
    );
    add(
        names.common("registry/ModTabs.java"),
        registry::tabs_java(project, &names),
    );
    for (path, contents) in content::files(project, &names) {
        add(path, contents);
    }
    if !content::declared_bases(project).is_empty() {
        add(
            names.common("content/ModRenderers.java"),
            content::renderers_java(project, &names),
        );
    }
    for (path, contents) in assets::files(project, &names) {
        add(path, contents);
    }

    // ── Loaders ───────────────────────────────────────────────────────────
    if project.loaders.fabric {
        for (path, contents) in loaders::fabric_files(project, &names) {
            add(path, contents);
        }
    }
    if project.loaders.neoforge {
        for (path, contents) in loaders::neoforge_files(project, &names) {
            add(path, contents);
        }
    }

    // ── Build ─────────────────────────────────────────────────────────────
    for (path, contents) in gradle::files(project, &names) {
        add(path, contents);
    }
    let missing_art = assets::missing_art(project, &names);
    add(
        "README.md".to_string(),
        gradle::readme(project, &names, &missing_art),
    );

    if !missing_art.is_empty() {
        warnings.push(format!(
            "{} texture or sound file(s) still have to be added by hand - the \
             generated README lists them.",
            missing_art.len()
        ));
    }
    if handlers::handler_count(project) == 0 {
        warnings.push(
            "Nothing on the canvas runs: the mod will load and do nothing. Add an \
             event block and stack something under it."
                .to_string(),
        );
    }

    Ok(Bundle { files, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockstitch_core::graph::Instruction;
    use blockstitch_core::value::Value;
    use stitchcraft_blocks::kinds::{MaterialPreset, MessageKind, Target};
    use stitchcraft_blocks::{McBlock, catalog};

    /// A project exercising a declaration, a hook targeting it, and a command.
    fn demo() -> ModProject {
        let mut project = ModProject::new("wonder_blocks");
        project.graph.add_strand(
            0,
            0,
            vec![Instruction::new(McBlock::RegisterCreativeTab {
                tab_id: "main".into(),
                display_name: "Wonder Blocks".into(),
                icon: String::new(),
            })],
        );
        project.graph.add_strand(
            0,
            0,
            vec![Instruction::new(McBlock::RegisterBlock {
                block_id: "chime_block".into(),
                display_name: "Chime Block".into(),
                material: MaterialPreset::Metal,
                hardness: Value::number(2.0),
                resistance: Value::number(4.0),
                light: Value::number(7.0),
                requires_tool: true,
                drops_self: true,
                give_item: true,
                creative_tab: "main".into(),
            })],
        );
        project.graph.add_strand(
            0,
            0,
            vec![
                Instruction::new(McBlock::OnBlockUsed {
                    block_id: "chime_block".into(),
                }),
                Instruction::new(McBlock::Message {
                    target: Target::EventPlayer,
                    kind: MessageKind::ActionBar,
                    text: Value::text("ding"),
                }),
                Instruction::new(catalog::prefab("PlaySoundAt").unwrap()),
            ],
        );
        project
    }

    fn file<'a>(bundle: &'a Bundle, suffix: &str) -> &'a str {
        bundle
            .files
            .iter()
            .find(|(path, _)| path.ends_with(suffix))
            .map(|(_, contents)| contents.as_str())
            .unwrap_or_else(|| panic!("no generated file ending in {suffix}"))
    }

    #[test]
    fn an_empty_project_still_exports_a_buildable_shell() {
        let bundle = export(&ModProject::new("demo")).expect("valid");
        assert!(bundle.files.contains_key("settings.gradle.kts"));
        assert!(bundle.files.contains_key("README.md"));
        assert!(
            bundle
                .files
                .contains_key("common/src/main/java/com/example/demo/Demo.java")
        );
        assert!(
            bundle
                .warnings
                .iter()
                .any(|warning| warning.contains("Nothing on the canvas runs")),
            "{:?}",
            bundle.warnings
        );
    }

    #[test]
    fn a_document_with_errors_exports_nothing() {
        let mut project = ModProject::new("demo");
        project.loaders.fabric = false;
        project.loaders.neoforge = false;
        let report = export(&project).expect_err("no loader selected");
        assert!(report.has_errors());
    }

    #[test]
    fn only_the_chosen_loaders_get_a_subproject() {
        let mut project = ModProject::new("demo");
        project.loaders.neoforge = false;
        let bundle = export(&project).expect("valid");
        assert!(bundle.files.contains_key("fabric/build.gradle.kts"));
        assert!(!bundle.files.contains_key("neoforge/build.gradle.kts"));
        assert!(
            !bundle
                .files
                .keys()
                .any(|path| path.starts_with("neoforge/"))
        );
    }

    #[test]
    fn a_declaration_reaches_every_file_it_needs_to() {
        let bundle = export(&demo()).expect("valid");

        let blocks = file(&bundle, "registry/ModBlocks.java");
        assert!(blocks.contains("public static final DeferredBlock<ScriptedBlock> CHIME_BLOCK"));
        assert!(blocks.contains("MapColor.METAL"));
        assert!(blocks.contains(".strength(2.0F, 4.0F)"));
        assert!(blocks.contains(".lightLevel(state -> 7)"));
        assert!(
            !blocks.contains(".randomTicks()"),
            "no random-tick hook targets this block"
        );

        let items = file(&bundle, "registry/ModItems.java");
        assert!(items.contains("registerBlockItem"));

        let lang = file(&bundle, "lang/en_us.json");
        assert!(lang.contains("\"block.wonder_blocks.chime_block\": \"Chime Block\""));
        assert!(lang.contains("\"itemGroup.wonder_blocks.main\""));

        assert!(bundle.files.contains_key(
            "common/src/main/resources/data/wonder_blocks/loot_table/blocks/chime_block.json"
        ));
        assert!(bundle.files.contains_key(
            "common/src/main/resources/assets/wonder_blocks/blockstates/chime_block.json"
        ));
    }

    #[test]
    fn a_hook_becomes_a_handler_and_a_dispatch_entry() {
        let bundle = export(&demo()).expect("valid");
        let handlers = file(&bundle, "script/Handlers.java");
        assert!(
            handlers.contains("public static boolean onBlockUsed_2(Ctx ctx, Object[] args)"),
            "{handlers}"
        );
        assert!(handlers.contains("Rt.message(ctx, Rt.Target.EVENT_PLAYER, \"ding\", true);"));
        assert!(
            handlers.contains("Map.entry(\"wonder_blocks:chime_block\", chain(Handlers::onBlockUsed_2))")
                || handlers.contains("Map.entry(\"wonder_blocks:chime_block\", Handlers::onBlockUsed_2)"),
            "the dispatch table must key on the qualified id: {handlers}"
        );
        // Falling off the end of a handler means nothing cancelled.
        assert!(handlers.contains("return true;"));
    }

    #[test]
    fn a_tab_lists_what_named_it() {
        let bundle = export(&demo()).expect("valid");
        let tabs = file(&bundle, "registry/ModTabs.java");
        assert!(tabs.contains("output.accept(ModItems.CHIME_BLOCK_ITEM.get());"), "{tabs}");
    }

    #[test]
    fn a_random_tick_hook_turns_on_random_ticking() {
        let mut project = demo();
        project.graph.add_strand(
            0,
            0,
            vec![Instruction::new(McBlock::OnBlockTick {
                block_id: "chime_block".into(),
            })],
        );
        let bundle = export(&project).expect("valid");
        assert!(file(&bundle, "registry/ModBlocks.java").contains(".randomTicks()"));
    }

    #[test]
    fn an_expression_in_a_declaration_is_reported_and_defaulted() {
        let mut project = demo();
        for strand in &mut project.graph.strands {
            if let Some(McBlock::RegisterBlock { hardness, .. }) =
                strand.instructions.first_mut().map(|i| &mut i.kind)
            {
                *hardness = Value::Var {
                    name: "toughness".into(),
                };
            }
        }
        let bundle = export(&project).expect("valid");
        assert!(
            bundle
                .warnings
                .iter()
                .any(|warning| warning.contains("hardness")),
            "{:?}",
            bundle.warnings
        );
        assert!(file(&bundle, "registry/ModBlocks.java").contains(".strength(1.5F, 4.0F)"));
    }

    #[test]
    fn commands_are_built_once_and_bridged_per_loader() {
        let mut project = ModProject::new("demo");
        project.graph.add_strand(
            0,
            0,
            vec![
                Instruction::new(McBlock::OnCommand {
                    name: "ping".into(),
                    op_only: true,
                }),
                Instruction::new(McBlock::Broadcast {
                    text: Value::text("pong"),
                }),
            ],
        );
        let bundle = export(&project).expect("valid");
        let commands = file(&bundle, "script/ModCommands.java");
        assert!(commands.contains("Commands.literal(\"ping\")"));
        assert!(commands.contains(".requires(source -> source.hasPermission(2))"));
        assert!(file(&bundle, "fabric/DemoFabric.java").contains("CommandRegistrationCallback"));
        assert!(file(&bundle, "neoforge/DemoNeoForge.java").contains("RegisterCommandsEvent"));
    }

    #[test]
    fn declared_state_is_seeded_from_the_document() {
        let mut project = ModProject::new("demo");
        project.create_variable("score").unwrap();
        project.create_list("scores").unwrap();
        project.create_dict("scores_by_name").unwrap();
        let bundle = export(&project).expect("valid");
        let main = file(&bundle, "Demo.java");
        assert!(main.contains("Vars.declareVariable(\"score\", Double.valueOf(0.0D));"));
        assert!(main.contains("Vars.declareList(\"scores\");"));
        assert!(main.contains("Vars.declareDict(\"scores_by_name\");"));
    }

    #[test]
    fn entities_bring_their_attributes_and_a_renderer() {
        let mut project = ModProject::new("demo");
        project.graph.add_strand(
            0,
            0,
            vec![Instruction::new(
                catalog::prefab("RegisterEntity")
                    .map(|block| match block {
                        McBlock::RegisterEntity { base, category, .. } => McBlock::RegisterEntity {
                            entity_id: "wisp".into(),
                            display_name: "Wisp".into(),
                            base,
                            category,
                            width: Value::number(0.5),
                            height: Value::number(0.5),
                            max_health: Value::number(6.0),
                            movement_speed: Value::number(0.3),
                            attack_damage: Value::number(1.0),
                            spawn_egg: false,
                        },
                        other => other,
                    })
                    .unwrap(),
            )],
        );
        let bundle = export(&project).expect("valid");
        let entities = file(&bundle, "registry/ModEntities.java");
        assert!(entities.contains("DeferredEntityType<PassiveMob> WISP"));
        assert!(entities.contains("DefaultAttributeRegistry.register(WISP.get(), PassiveMob.attributes(6.0D, 0.3D, 1.0D));"));
        assert!(file(&bundle, "content/PassiveMob.java").contains("extends ScriptedMob"));
        assert!(file(&bundle, "content/ModRenderers.java").contains("NoopRenderer::new"));
        assert!(file(&bundle, "Demo.java").contains("ModEntities.registerAttributes();"));
    }

    #[test]
    fn output_is_deterministic() {
        let first = export(&demo()).expect("valid");
        let second = export(&demo()).expect("valid");
        assert_eq!(first.files, second.files);
    }

    #[test]
    fn every_java_file_declares_a_package_and_balances_its_braces() {
        let bundle = export(&demo()).expect("valid");
        for (path, contents) in &bundle.files {
            if !path.ends_with(".java") {
                continue;
            }
            assert!(
                contents.starts_with("package "),
                "{path} does not start with its package declaration"
            );
            let opens = contents.matches('{').count();
            let closes = contents.matches('}').count();
            assert_eq!(opens, closes, "{path} has unbalanced braces");
        }
    }

    #[test]
    fn every_json_file_parses() {
        let bundle = export(&demo()).expect("valid");
        for (path, contents) in &bundle.files {
            if !path.ends_with(".json") {
                continue;
            }
            serde_json::from_str::<serde_json::Value>(contents)
                .unwrap_or_else(|error| panic!("{path} is not valid JSON: {error}"));
        }
    }
}
