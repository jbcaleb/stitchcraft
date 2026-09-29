//! The registry classes, built on FrozenLib's `DeferredRegister`.
//!
//! `DeferredRegister` is why one set of generated sources serves both loaders:
//! on Fabric its `register()` registers everything immediately, and on NeoForge
//! it defers to NeoForge's own deferred register and hooks the mod event bus.
//! The generated code never has to know which it is running on.
//!
//! A declaration's numeric fields have to be constants, because registration
//! happens while classes are loading - long before there is a world to read a
//! variable out of. Anything that is not a plain number is reported as a warning
//! and falls back to the field's default.

use crate::naming::{self, Names};
use crate::runtime::banner;
use crate::writer::{Source, double, float, int, quote};
use blockstitch_core::value::Value;
use stitchcraft_blocks::kinds::{EntityBase, MaterialPreset, MobCategoryKind, Rarity};
use stitchcraft_blocks::{McBlock, ModProject};

/// A declaration's numeric field, which has to be a literal. Returns the
/// fallback and a complaint when it is not.
fn constant(value: &Value, field: &str, id: &str, warnings: &mut Vec<String>, fallback: f64) -> f64 {
    match value {
        Value::Number { value } => *value,
        _ => {
            warnings.push(format!(
                "\"{id}\" has an expression in its {field}, but a registry entry is built before \
                 there is a world to read one in. Using {fallback} instead - put the calculation in \
                 an event hook if it needs to change."
            ));
            fallback
        }
    }
}

/// The map colour and sound type a material preset stands for.
fn material_properties(material: MaterialPreset) -> (&'static str, &'static str) {
    match material {
        MaterialPreset::Stone => ("MapColor.STONE", "SoundType.STONE"),
        MaterialPreset::Wood => ("MapColor.WOOD", "SoundType.WOOD"),
        MaterialPreset::Metal => ("MapColor.METAL", "SoundType.METAL"),
        MaterialPreset::Glass => ("MapColor.NONE", "SoundType.GLASS"),
        MaterialPreset::Wool => ("MapColor.WOOL", "SoundType.WOOL"),
        MaterialPreset::Dirt => ("MapColor.DIRT", "SoundType.GRAVEL"),
        MaterialPreset::Sand => ("MapColor.SAND", "SoundType.SAND"),
        MaterialPreset::Plant => ("MapColor.PLANT", "SoundType.GRASS"),
    }
}

fn rarity_constant(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "Rarity.COMMON",
        Rarity::Uncommon => "Rarity.UNCOMMON",
        Rarity::Rare => "Rarity.RARE",
        Rarity::Epic => "Rarity.EPIC",
    }
}

fn category_constant(category: MobCategoryKind) -> &'static str {
    match category {
        MobCategoryKind::Creature => "MobCategory.CREATURE",
        MobCategoryKind::Monster => "MobCategory.MONSTER",
        MobCategoryKind::Ambient => "MobCategory.AMBIENT",
        MobCategoryKind::WaterCreature => "MobCategory.WATER_CREATURE",
        MobCategoryKind::Misc => "MobCategory.MISC",
    }
}

/// The generated mob class an entity base is built on - see `content.rs`.
pub fn mob_class(base: EntityBase) -> &'static str {
    match base {
        EntityBase::Passive => "PassiveMob",
        EntityBase::Hostile => "HostileMob",
        EntityBase::Flying => "FlyingMob",
    }
}

/// What one declaration family contributes, gathered so the emitters and the
/// asset writer see the same list.
pub struct Declared<'a> {
    pub blocks: Vec<&'a McBlock>,
    pub items: Vec<&'a McBlock>,
    pub entities: Vec<&'a McBlock>,
    pub sounds: Vec<&'a McBlock>,
    pub tabs: Vec<&'a McBlock>,
}

impl<'a> Declared<'a> {
    pub fn gather(project: &'a ModProject) -> Self {
        let mut declared = Self {
            blocks: Vec::new(),
            items: Vec::new(),
            entities: Vec::new(),
            sounds: Vec::new(),
            tabs: Vec::new(),
        };
        for (_, header) in crate::handlers::declarations(project) {
            match header {
                McBlock::RegisterBlock { .. } => declared.blocks.push(header),
                McBlock::RegisterItem { .. } => declared.items.push(header),
                McBlock::RegisterEntity { .. } => declared.entities.push(header),
                McBlock::RegisterSound { .. } => declared.sounds.push(header),
                McBlock::RegisterCreativeTab { .. } => declared.tabs.push(header),
                _ => {}
            }
        }
        declared
    }

    /// The ids and display names of everything that needs a translation entry.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
            && self.items.is_empty()
            && self.entities.is_empty()
            && self.sounds.is_empty()
            && self.tabs.is_empty()
    }
}

/// Opens a registry class, writing its package, banner, imports and the
/// `DeferredRegister` field.
fn open_registry(names: &Names, note: &str, imports: &[&str]) -> Source {
    let mut s = Source::new();
    s.line(format!("package {};", names.registry_package()));
    s.blank();
    banner(&mut s, names, note);
    let mut all: Vec<String> = imports.iter().map(|i| i.to_string()).collect();
    all.push(format!(
        "import {}.{};",
        names.package, names.main_class
    ));
    all.sort();
    s.lines(&all);
    s.blank();
    s
}

/// `ModBlocks.java`.
pub fn blocks_java(project: &ModProject, names: &Names, warnings: &mut Vec<String>) -> String {
    let declared = Declared::gather(project);
    let ticking = ticking_blocks(project);
    let main = &names.main_class;
    let mut s = open_registry(
        names,
        "Every block the canvas declares.\n\nAll of them are `ScriptedBlock`s, \
         whether or not a hook targets them: the class dispatches through a table \
         that is simply empty for a block nothing listens to, and one block class \
         is easier to read than two.",
        &[
            "import net.frozenblock.lib.platform.api.registry.DeferredBlock;",
            "import net.frozenblock.lib.platform.api.registry.DeferredRegister;",
            "import net.minecraft.core.registries.Registries;",
            "import net.minecraft.resources.ResourceKey;",
            "import net.minecraft.world.level.block.Block;",
            "import net.minecraft.world.level.block.SoundType;",
            "import net.minecraft.world.level.block.state.BlockBehaviour;",
            "import net.minecraft.world.level.material.MapColor;",
        ],
    );
    s.line(format!("import {}.ScriptedBlock;", names.content_package()));
    s.blank();
    s.braced("public final class ModBlocks", |s| {
        s.line(format!(
            "public static final DeferredRegister.Blocks REGISTRY = DeferredRegister.createBlocks({main}.MOD_ID);"
        ));
        s.line("");
        for header in &declared.blocks {
            let McBlock::RegisterBlock {
                block_id,
                material,
                hardness,
                resistance,
                light,
                requires_tool,
                ..
            } = header
            else {
                continue;
            };
            let id = block_id.trim();
            let (map_color, sound) = material_properties(*material);
            let hardness = constant(hardness, "hardness", id, warnings, 1.5);
            let resistance = constant(resistance, "blast resistance", id, warnings, 6.0);
            let light = constant(light, "light level", id, warnings, 0.0).clamp(0.0, 15.0);
            s.doc(&format!("`{}:{id}`.", project.mod_id));
            s.open(format!(
                "public static final DeferredBlock<ScriptedBlock> {} = REGISTRY.registerBlock(",
                naming::constant_name(id)
            ));
            s.line(format!(
                "ResourceKey.create(Registries.BLOCK, {main}.id({})),",
                quote(id)
            ));
            s.line("ScriptedBlock::new,");
            s.open("() -> BlockBehaviour.Properties.of()");
            s.line(format!(".mapColor({map_color})"));
            s.line(format!(".sound({sound})"));
            s.line(format!(
                ".strength({}, {})",
                float(hardness),
                float(resistance)
            ));
            if *requires_tool {
                s.line(".requiresCorrectToolForDrops()");
            }
            if light > 0.0 {
                s.line(format!(".lightLevel(state -> {})", int(light)));
            }
            if ticking.iter().any(|candidate| candidate == id) {
                s.comment("A random-tick hook targets this block.");
                s.line(".randomTicks()");
            }
            s.close("");
            s.close(");");
            s.line("");
        }
        s.line("private ModBlocks() {}");
    });
    s.finish()
}

/// The block ids a random-tick hook targets, which is what turns on
/// `Properties.randomTicks()`.
fn ticking_blocks(project: &ModProject) -> Vec<String> {
    project
        .strands
        .iter()
        .filter_map(|strand| match &strand.instructions.first()?.kind {
            McBlock::OnBlockTick { block_id } => Some(block_id.trim().to_string()),
            _ => None,
        })
        .filter(|id| !id.is_empty())
        .collect()
}

/// `ModItems.java` - declared items, plus a `BlockItem` for every block that
/// asked for one.
pub fn items_java(project: &ModProject, names: &Names, warnings: &mut Vec<String>) -> String {
    let declared = Declared::gather(project);
    let main = &names.main_class;
    let mut s = open_registry(
        names,
        "Every item the canvas declares, and the block items that go with the \
         blocks that asked for one.",
        &[
            "import net.frozenblock.lib.platform.api.registry.DeferredItem;",
            "import net.frozenblock.lib.platform.api.registry.DeferredRegister;",
            "import net.minecraft.world.item.BlockItem;",
            "import net.minecraft.world.item.Rarity;",
        ],
    );
    s.line(format!("import {}.ScriptedItem;", names.content_package()));
    s.blank();
    s.braced("public final class ModItems", |s| {
        s.line(format!(
            "public static final DeferredRegister.Items REGISTRY = DeferredRegister.createItems({main}.MOD_ID);"
        ));
        s.line("");
        for header in &declared.items {
            let McBlock::RegisterItem {
                item_id,
                max_stack,
                rarity,
                ..
            } = header
            else {
                continue;
            };
            let id = item_id.trim();
            let stack = constant(max_stack, "stack size", id, warnings, 64.0).clamp(1.0, 99.0);
            s.doc(&format!("`{}:{id}`.", project.mod_id));
            s.open(format!(
                "public static final DeferredItem<ScriptedItem> {} = REGISTRY.registerItem(",
                naming::constant_name(id)
            ));
            s.line(format!("{},", quote(id)));
            s.line("ScriptedItem::new,");
            s.line(format!(
                "properties -> properties.stacksTo({}).rarity({})",
                int(stack),
                rarity_constant(*rarity)
            ));
            s.close(");");
            s.line("");
        }
        for header in &declared.blocks {
            let McBlock::RegisterBlock {
                block_id,
                give_item,
                ..
            } = header
            else {
                continue;
            };
            if !give_item {
                continue;
            }
            let id = block_id.trim();
            s.doc(&format!("The item form of `{}:{id}`.", project.mod_id));
            s.open(format!(
                "public static final DeferredItem<BlockItem> {} = REGISTRY.registerBlockItem(",
                naming::constant_name(&format!("{id}_item"))
            ));
            s.line(format!("{},", quote(id)));
            s.line("BlockItem::new,");
            s.line(format!("ModBlocks.{}", naming::constant_name(id)));
            s.close(");");
            s.line("");
        }
        s.line("private ModItems() {}");
    });
    s.finish()
}

/// `ModEntities.java`, plus the cross-platform attribute registration every mob
/// needs or the game throws when one spawns.
pub fn entities_java(project: &ModProject, names: &Names, warnings: &mut Vec<String>) -> String {
    let declared = Declared::gather(project);
    let main = &names.main_class;
    let mut s = open_registry(
        names,
        "Every entity the canvas declares.\n\nAn entity type is only half the \
         job: without default attributes the game throws the first time one \
         spawns, so `registerAttributes` is called from the common mod init \
         through FrozenLib's cross-platform `DefaultAttributeRegistry`.",
        &[
            "import net.frozenblock.lib.entity.api.attribute.DefaultAttributeRegistry;",
            "import net.frozenblock.lib.platform.api.registry.DeferredEntityType;",
            "import net.frozenblock.lib.platform.api.registry.DeferredRegister;",
            "import net.minecraft.core.registries.Registries;",
            "import net.minecraft.resources.ResourceKey;",
            "import net.minecraft.world.entity.EntityType;",
            "import net.minecraft.world.entity.MobCategory;",
        ],
    );
    let bases: Vec<&'static str> = declared
        .entities
        .iter()
        .filter_map(|header| match header {
            McBlock::RegisterEntity { base, .. } => Some(mob_class(*base)),
            _ => None,
        })
        .collect();
    let mut seen = Vec::new();
    for base in bases {
        if !seen.contains(&base) {
            seen.push(base);
            s.line(format!("import {}.{base};", names.content_package()));
        }
    }
    s.blank();
    s.braced("public final class ModEntities", |s| {
        s.line(format!(
            "public static final DeferredRegister.Entities REGISTRY = DeferredRegister.createEntities({main}.MOD_ID);"
        ));
        s.line("");
        for header in &declared.entities {
            let McBlock::RegisterEntity {
                entity_id,
                base,
                category,
                width,
                height,
                ..
            } = header
            else {
                continue;
            };
            let id = entity_id.trim();
            let class = mob_class(*base);
            let width = constant(width, "width", id, warnings, 0.8).clamp(0.1, 16.0);
            let height = constant(height, "height", id, warnings, 1.4).clamp(0.1, 16.0);
            s.doc(&format!("`{}:{id}`.", project.mod_id));
            s.open(format!(
                "public static final DeferredEntityType<{class}> {} = REGISTRY.register(",
                naming::constant_name(id)
            ));
            s.line(format!(
                "ResourceKey.create(Registries.ENTITY_TYPE, {main}.id({})),",
                quote(id)
            ));
            s.line(format!("{class}::new,"));
            s.line(format!("{},", category_constant(*category)));
            s.line(format!(
                "builder -> builder.sized({}, {})",
                float(width),
                float(height)
            ));
            s.close(");");
            s.line("");
        }
        s.doc(
            "Called from the common mod init, once the entity types exist. \
             FrozenLib routes this to Fabric's attribute registry or NeoForge's \
             attribute event as needed.",
        );
        s.braced("public static void registerAttributes()", |s| {
            if declared.entities.is_empty() {
                s.comment("No entities declared.");
                return;
            }
            for header in &declared.entities {
                let McBlock::RegisterEntity {
                    entity_id,
                    base,
                    max_health,
                    movement_speed,
                    attack_damage,
                    ..
                } = header
                else {
                    continue;
                };
                let id = entity_id.trim();
                let class = mob_class(*base);
                let health = constant(max_health, "health", id, warnings, 10.0).max(1.0);
                let speed = constant(movement_speed, "speed", id, warnings, 0.25).max(0.0);
                let damage = constant(attack_damage, "attack damage", id, warnings, 2.0).max(0.0);
                s.line(format!(
                    "DefaultAttributeRegistry.register({}.get(), {class}.attributes({}, {}, {}));",
                    naming::constant_name(id),
                    double(health),
                    double(speed),
                    double(damage)
                ));
            }
        });
        s.line("");
        s.line("private ModEntities() {}");
    });
    s.finish()
}

/// `ModSounds.java`.
pub fn sounds_java(project: &ModProject, names: &Names) -> String {
    let declared = Declared::gather(project);
    let main = &names.main_class;
    let mut s = open_registry(
        names,
        "Every sound event the canvas declares. The ogg files themselves go in \
         `assets/<mod>/sounds/`, named after the id; `sounds.json` already points \
         at them.",
        &[
            "import net.frozenblock.lib.platform.api.registry.DeferredRegister;",
            "import net.frozenblock.lib.platform.api.registry.DeferredSoundEvent;",
        ],
    );
    s.braced("public final class ModSounds", |s| {
        s.line(format!(
            "public static final DeferredRegister.SoundEvents REGISTRY = DeferredRegister.createSoundEvents({main}.MOD_ID);"
        ));
        s.line("");
        for header in &declared.sounds {
            let McBlock::RegisterSound { sound_id, .. } = header else {
                continue;
            };
            let id = sound_id.trim();
            s.line(format!(
                "public static final DeferredSoundEvent {} = REGISTRY.register({});",
                naming::constant_name(id),
                quote(id)
            ));
        }
        s.line("");
        s.line("private ModSounds() {}");
    });
    s.finish()
}

/// `ModTabs.java` - the mod's own creative tabs, each listing the content that
/// named it.
pub fn tabs_java(project: &ModProject, names: &Names) -> String {
    let declared = Declared::gather(project);
    let main = &names.main_class;
    let mut s = open_registry(
        names,
        "The creative tabs the canvas declares.\n\nEach tab lists the blocks and \
         items whose declaration named it, which is why this needs no \
         loader-specific \"add to tab\" event: a tab we own can simply say what \
         is in it.",
        &[
            "import net.frozenblock.lib.platform.api.registry.DeferredHolder;",
            "import net.frozenblock.lib.platform.api.registry.DeferredRegister;",
            "import net.minecraft.core.registries.Registries;",
            "import net.minecraft.network.chat.Component;",
            "import net.minecraft.world.item.CreativeModeTab;",
            "import net.minecraft.world.item.ItemStack;",
        ],
    );
    s.line(format!("import {}.Rt;", names.script_package()));
    s.blank();
    s.braced("public final class ModTabs", |s| {
        s.line(format!(
            "public static final DeferredRegister<CreativeModeTab> REGISTRY = DeferredRegister.create(Registries.CREATIVE_MODE_TAB, {main}.MOD_ID);"
        ));
        s.line("");
        for header in &declared.tabs {
            let McBlock::RegisterCreativeTab { tab_id, icon, .. } = header else {
                continue;
            };
            let id = tab_id.trim();
            let contents = tab_contents(&declared, id);
            let icon_expression = match icon.trim() {
                "" => contents
                    .first()
                    .map(|entry| entry.stack())
                    .unwrap_or_else(|| "new ItemStack(net.minecraft.world.item.Items.CRAFTING_TABLE)".to_string()),
                explicit => format!(
                    "new ItemStack(Rt.item({}))",
                    quote(&naming::qualify(&project.mod_id, explicit))
                ),
            };
            s.doc(&format!("The `{id}` tab."));
            s.open(format!(
                "public static final DeferredHolder<CreativeModeTab, CreativeModeTab> {} = REGISTRY.register(",
                naming::constant_name(id)
            ));
            s.line(format!("{},", quote(id)));
            s.open("() -> CreativeModeTab.builder()");
            s.line(format!(
                ".title(Component.translatable(\"itemGroup.{}.{id}\"))",
                project.mod_id
            ));
            s.line(format!(".icon(() -> {icon_expression})"));
            s.open(".displayItems((parameters, output) -> {");
            if contents.is_empty() {
                s.comment("Nothing declared into this tab yet.");
            }
            for entry in &contents {
                s.line(format!("output.accept({});", entry.accept()));
            }
            s.close("})");
            s.line(".build()");
            s.close("");
            s.close(");");
            s.line("");
        }
        s.line("private ModTabs() {}");
    });
    s.finish()
}

/// One thing that goes in a creative tab.
enum TabEntry {
    /// A block, added through the item form it was given.
    Block(String),
    Item(String),
}

impl TabEntry {
    fn accept(&self) -> String {
        match self {
            TabEntry::Block(constant) => format!("ModItems.{constant}.get()"),
            TabEntry::Item(constant) => format!("ModItems.{constant}.get()"),
        }
    }

    fn stack(&self) -> String {
        format!("new ItemStack({})", self.accept())
    }
}

/// The blocks and items whose declaration named `tab`. A block with no item form
/// cannot go in a tab, so it is skipped.
fn tab_contents(declared: &Declared<'_>, tab: &str) -> Vec<TabEntry> {
    let mut out = Vec::new();
    for header in &declared.blocks {
        if let McBlock::RegisterBlock {
            block_id,
            creative_tab,
            give_item,
            ..
        } = header
            && *give_item
            && creative_tab.trim() == tab
        {
            out.push(TabEntry::Block(naming::constant_name(&format!(
                "{}_item",
                block_id.trim()
            ))));
        }
    }
    for header in &declared.items {
        if let McBlock::RegisterItem {
            item_id,
            creative_tab,
            ..
        } = header
            && creative_tab.trim() == tab
        {
            out.push(TabEntry::Item(naming::constant_name(item_id.trim())));
        }
    }
    out
}
