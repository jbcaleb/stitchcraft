//! The resources a declaration needs to show up in game: translations, models,
//! blockstates, loot tables and the sound index.
//!
//! These are the files that make a registered block look like a block rather
//! than a purple-and-black cube with an untranslated name. What is *not*
//! generated is the artwork - a `.png` is not something a canvas can describe -
//! so every model points at a texture path the user fills in, and the export
//! summary says which ones are missing.

use crate::naming::{self, Names};
use serde_json::{Value as Json, json};
use stitchcraft_blocks::{McBlock, ModProject};

/// Pretty-prints with a trailing newline, so the output is diffable and behaves
/// in a text editor.
fn write(value: &Json) -> String {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

/// Every resource file for `project`.
pub fn files(project: &ModProject, names: &Names) -> Vec<(String, String)> {
    let declared = crate::registry::Declared::gather(project);
    let mut out = vec![
        (names.common_resource("pack.mcmeta"), pack_mcmeta(project)),
        (names.asset("lang/en_us.json"), lang(project, &declared)),
    ];

    for header in &declared.blocks {
        let McBlock::RegisterBlock {
            block_id,
            give_item,
            drops_self,
            ..
        } = header
        else {
            continue;
        };
        let id = block_id.trim();
        if id.is_empty() {
            continue;
        }
        let model = format!("{}:block/{id}", project.mod_id);
        out.push((
            names.asset(&format!("blockstates/{id}.json")),
            write(&json!({ "variants": { "": { "model": model } } })),
        ));
        out.push((
            names.asset(&format!("models/block/{id}.json")),
            write(&json!({
                "parent": "minecraft:block/cube_all",
                "textures": { "all": format!("{}:block/{id}", project.mod_id) },
            })),
        ));
        if *give_item {
            out.push((
                names.asset(&format!("models/item/{id}.json")),
                write(&json!({ "parent": model })),
            ));
            out.push((
                names.asset(&format!("items/{id}.json")),
                item_definition(&format!("{}:item/{id}", project.mod_id), &model),
            ));
        }
        if *drops_self {
            out.push((
                names.data(&format!("loot_table/blocks/{id}.json")),
                loot_table(project, id),
            ));
        }
    }

    for header in &declared.items {
        let McBlock::RegisterItem { item_id, .. } = header else {
            continue;
        };
        let id = item_id.trim();
        if id.is_empty() {
            continue;
        }
        let texture = format!("{}:item/{id}", project.mod_id);
        let model = format!("{}:item/{id}", project.mod_id);
        out.push((
            names.asset(&format!("models/item/{id}.json")),
            write(&json!({
                "parent": "minecraft:item/generated",
                "textures": { "layer0": texture },
            })),
        ));
        out.push((
            names.asset(&format!("items/{id}.json")),
            item_definition(&texture, &model),
        ));
    }

    if !declared.sounds.is_empty() {
        out.push((names.asset("sounds.json"), sounds(project, &declared)));
    }
    out
}

/// The item model definition modern versions look up before the model itself.
fn item_definition(_texture: &str, model: &str) -> String {
    write(&json!({
        "model": { "type": "minecraft:model", "model": model },
    }))
}

/// `pack.mcmeta`.
///
/// `supported_formats` is deliberately wide rather than pinned: the pack format
/// number changes with almost every Minecraft release, and a wrong one stops the
/// resources loading at all. A range that always matches is the safer default
/// for generated output - narrow it if you ship the mod.
fn pack_mcmeta(project: &ModProject) -> String {
    write(&json!({
        "pack": {
            "description": format!("{} resources", project.name),
            "pack_format": 64,
            "supported_formats": { "min_inclusive": 1, "max_inclusive": 999 },
        }
    }))
}

/// `en_us.json` - one entry per declaration, falling back to a title-cased id
/// where the canvas left the display name blank.
fn lang(project: &ModProject, declared: &crate::registry::Declared<'_>) -> String {
    let mut entries = serde_json::Map::new();
    let mut insert = |key: String, value: &str, id: &str| {
        let text = if value.trim().is_empty() {
            naming::display_fallback(id)
        } else {
            value.trim().to_string()
        };
        entries.insert(key, json!(text));
    };
    let mod_id = &project.mod_id;

    for header in &declared.blocks {
        if let McBlock::RegisterBlock {
            block_id,
            display_name,
            ..
        } = header
        {
            let id = block_id.trim();
            insert(format!("block.{mod_id}.{id}"), display_name, id);
        }
    }
    for header in &declared.items {
        if let McBlock::RegisterItem {
            item_id,
            display_name,
            ..
        } = header
        {
            let id = item_id.trim();
            insert(format!("item.{mod_id}.{id}"), display_name, id);
        }
    }
    for header in &declared.entities {
        if let McBlock::RegisterEntity {
            entity_id,
            display_name,
            ..
        } = header
        {
            let id = entity_id.trim();
            insert(format!("entity.{mod_id}.{id}"), display_name, id);
        }
    }
    for header in &declared.tabs {
        if let McBlock::RegisterCreativeTab {
            tab_id,
            display_name,
            ..
        } = header
        {
            let id = tab_id.trim();
            insert(format!("itemGroup.{mod_id}.{id}"), display_name, id);
        }
    }
    for header in &declared.sounds {
        if let McBlock::RegisterSound {
            sound_id, subtitle, ..
        } = header
        {
            let id = sound_id.trim();
            insert(format!("subtitles.{mod_id}.{id}"), subtitle, id);
        }
    }
    write(&Json::Object(entries))
}

/// The "drops itself" loot table.
fn loot_table(project: &ModProject, block_id: &str) -> String {
    write(&json!({
        "type": "minecraft:block",
        "pools": [{
            "rolls": 1,
            "bonus_rolls": 0,
            "entries": [{
                "type": "minecraft:item",
                "name": format!("{}:{block_id}", project.mod_id),
            }],
            "conditions": [{ "condition": "minecraft:survives_explosion" }],
        }],
    }))
}

/// `sounds.json`, pointing each declared sound at an ogg named after its id.
fn sounds(project: &ModProject, declared: &crate::registry::Declared<'_>) -> String {
    let mut entries = serde_json::Map::new();
    for header in &declared.sounds {
        let McBlock::RegisterSound { sound_id, .. } = header else {
            continue;
        };
        let id = sound_id.trim();
        if id.is_empty() {
            continue;
        }
        entries.insert(
            id.to_string(),
            json!({
                "subtitle": format!("subtitles.{}.{id}", project.mod_id),
                "sounds": [format!("{}:{id}", project.mod_id)],
            }),
        );
    }
    write(&Json::Object(entries))
}

/// The artwork and audio a declaration needs but a canvas cannot produce, as
/// paths relative to the export root. The export reports these so the user knows
/// what is still missing rather than finding out in game.
pub fn missing_art(project: &ModProject, names: &Names) -> Vec<String> {
    let declared = crate::registry::Declared::gather(project);
    let mut out = Vec::new();
    for header in &declared.blocks {
        if let McBlock::RegisterBlock { block_id, .. } = header {
            let id = block_id.trim();
            if !id.is_empty() {
                out.push(names.asset(&format!("textures/block/{id}.png")));
            }
        }
    }
    for header in &declared.items {
        if let McBlock::RegisterItem { item_id, .. } = header {
            let id = item_id.trim();
            if !id.is_empty() {
                out.push(names.asset(&format!("textures/item/{id}.png")));
            }
        }
    }
    for header in &declared.sounds {
        if let McBlock::RegisterSound { sound_id, .. } = header {
            let id = sound_id.trim();
            if !id.is_empty() {
                out.push(names.asset(&format!("sounds/{id}.ogg")));
            }
        }
    }
    out
}
