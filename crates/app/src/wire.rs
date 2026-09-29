//! Translating between the shape blockstitch-core saves and the shape its QML
//! canvas reads.
//!
//! `Instruction<K>` is a wrapper - `{"id": ..., "kind": {...}}` - so that an
//! instruction keeps a stable identity across drags and reorders while its
//! contents are just the host's enum. The canvas, though, reads an instruction
//! flat: `ins.type` for which block it is, `ins[key]` for each field, `ins.id`
//! for identity. The two disagree, and the disagreement is invisible until a
//! block is on the canvas, because the palette builds its blocks from
//! `BlockRegistry.prefab()` - already flat - rather than from the document.
//!
//! So the document is flattened on the way out and re-nested on the way back.
//! Both walks are recursive, because a mouth's contents (`body`, `then_body`,
//! `else_body`) are instruction lists nested inside the kind.

use serde_json::{Map, Value};

/// True for the `{"id": ..., "kind": {...}}` envelope and nothing else.
///
/// The discriminator has to be exact, because a canvas document is full of
/// other two-key objects: a `Value` leaf is `{"kind": "Number", "value": 3}`,
/// which has a `kind` but no `id` and whose `kind` is a string rather than an
/// object.
fn is_envelope(map: &Map<String, Value>) -> bool {
    map.len() == 2 && map.contains_key("id") && map.get("kind").is_some_and(Value::is_object)
}

/// Rewrites every instruction envelope in `value` as a flat object, in place.
///
/// `{"id": "a", "kind": {"type": "Repeat", "body": [...]}}`
/// becomes `{"type": "Repeat", "body": [...], "id": "a"}`.
pub fn flatten(value: &mut Value) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(flatten),
        Value::Object(map) => {
            // Depth first, so a mouth's instructions are already flat by the
            // time their parent is unwrapped.
            map.values_mut().for_each(flatten);
            if is_envelope(map) {
                let id = map.remove("id").unwrap_or(Value::Null);
                let Some(Value::Object(mut kind)) = map.remove("kind") else {
                    return;
                };
                // `id` last: a kind can never have a field of that name, and
                // putting it at the end keeps the type tag first when the JSON
                // is read by a person.
                kind.insert("id".to_string(), id);
                *map = kind;
            }
        }
        _ => {}
    }
}

/// The inverse of [`flatten`]: rewrites flat instructions back into envelopes.
///
/// A flat instruction is recognised by its `type` tag, which is `McBlock`'s
/// serde discriminator. Values use `kind` rather than `type`, so they are left
/// alone.
///
/// With `fresh_ids`, the `id` is dropped instead of carried over, and
/// `Instruction`'s deserializer mints a new one. That is what a palette drop
/// and a duplicate want: reusing the id of the block they came from would give
/// two instructions the same identity, which is what comments attach to and
/// what a drag addresses.
pub fn nest(value: &mut Value, fresh_ids: bool) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| nest(item, fresh_ids)),
        Value::Object(map) => {
            map.values_mut().for_each(|child| nest(child, fresh_ids));
            if !map.contains_key("type") {
                return;
            }
            let id = map.remove("id").filter(|_| !fresh_ids);
            let kind = std::mem::take(map);
            match id {
                Some(id) => {
                    map.insert("id".to_string(), id);
                    map.insert("kind".to_string(), Value::Object(kind));
                }
                // No id: the bare-kind form, which `Instruction`'s deserializer
                // accepts and gives a fresh id.
                None => *map = kind,
            }
        }
        _ => {}
    }
}

/// A whole document, as the canvas wants to read it.
pub fn document_json<T: serde::Serialize>(project: &T) -> String {
    match serde_json::to_value(project) {
        Ok(mut value) => {
            flatten(&mut value);
            value.to_string()
        }
        Err(_) => "{}".to_string(),
    }
}

/// One instruction from the canvas, as blockstitch-core wants to read it.
pub fn instruction_json(flat: &str, fresh_ids: bool) -> Option<Value> {
    let mut value: Value = serde_json::from_str(flat).ok()?;
    nest(&mut value, fresh_ids);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockstitch_core::graph::{BlockKind, Instruction};
    use blockstitch_core::value::Value as BlockValue;
    use serde_json::json;
    use stitchcraft_blocks::{McBlock, ModProject};

    #[test]
    fn a_document_reaches_the_canvas_flat() {
        let mut project = ModProject::new("demo");
        project.graph.add_strand(
            0,
            0,
            vec![
                Instruction::new(McBlock::OnServerTick),
                Instruction::new(McBlock::Broadcast {
                    text: BlockValue::text("hi"),
                }),
            ],
        );
        let document: Value = serde_json::from_str(&document_json(&project)).unwrap();
        let first = &document["strands"][0]["instructions"][0];
        assert_eq!(
            first["type"], "OnServerTick",
            "the canvas reads `ins.type`, so the tag has to be at the top level"
        );
        assert!(first["id"].is_string(), "identity survives flattening");
        assert!(first.get("kind").is_none(), "no envelope is left behind");
        assert_eq!(document["strands"][0]["instructions"][1]["text"]["value"], "hi");
    }

    #[test]
    fn nested_bodies_are_flattened_too() {
        let mut project = ModProject::new("demo");
        project.graph.add_strand(
            0,
            0,
            vec![
                Instruction::new(McBlock::OnServerTick),
                Instruction::new(McBlock::IfElse {
                    condition: BlockValue::Bool,
                    then_body: vec![Instruction::new(McBlock::CancelEvent)],
                    else_body: vec![Instruction::new(McBlock::EscapeLoop)],
                }),
            ],
        );
        let document: Value = serde_json::from_str(&document_json(&project)).unwrap();
        let branch = &document["strands"][0]["instructions"][1];
        assert_eq!(branch["type"], "IfElse");
        assert_eq!(branch["then_body"][0]["type"], "CancelEvent");
        assert_eq!(branch["else_body"][0]["type"], "EscapeLoop");
    }

    #[test]
    fn value_leaves_are_left_alone() {
        // `{"kind": "Number", "value": 3}` is two keys with a `kind` - close
        // enough to an envelope to be worth a test.
        let mut value = json!({ "kind": "Number", "value": 3.0 });
        flatten(&mut value);
        assert_eq!(value, json!({ "kind": "Number", "value": 3.0 }));

        let mut op = json!({
            "kind": "Op",
            "op": "Add",
            "args": [{ "kind": "Number", "value": 1.0 }],
            "saved": { "kind": "Number", "value": 0.0 },
        });
        let before = op.clone();
        flatten(&mut op);
        assert_eq!(op, before);
    }

    #[test]
    fn a_floating_value_is_not_an_envelope() {
        // It has an `id` and a `value`, but no `kind` of its own.
        let mut floating = json!({
            "id": "f1", "x": 0, "y": 0,
            "value": { "kind": "Text", "value": "hi" },
            "origin_block_id": null,
        });
        let before = floating.clone();
        flatten(&mut floating);
        assert_eq!(floating, before);
    }

    #[test]
    fn an_edited_block_round_trips_with_its_identity() {
        let original = Instruction::new(McBlock::Broadcast {
            text: BlockValue::text("hi"),
        });
        let id = original.id.clone();

        let mut flat = serde_json::to_value(&original).unwrap();
        flatten(&mut flat);
        // What the canvas would hand back after an in-place edit.
        flat["text"]["value"] = json!("bye");

        let nested = instruction_json(&flat.to_string(), false).unwrap();
        let parsed: Instruction<McBlock> = serde_json::from_value(nested).unwrap();
        assert_eq!(parsed.id, id, "an edit keeps the block it edited");
        assert_eq!(
            parsed.kind,
            McBlock::Broadcast {
                text: BlockValue::text("bye")
            }
        );
    }

    #[test]
    fn a_dropped_or_duplicated_block_gets_a_new_identity() {
        let original = Instruction::new(McBlock::Repeat {
            count: BlockValue::number(3.0),
            body: vec![Instruction::new(McBlock::CancelEvent)],
        });
        let mut flat = serde_json::to_value(&original).unwrap();
        flatten(&mut flat);

        let nested = instruction_json(&flat.to_string(), true).unwrap();
        let copy: Instruction<McBlock> = serde_json::from_value(nested).unwrap();
        assert_ne!(copy.id, original.id, "a copy is its own block");
        assert_eq!(copy.kind, original.kind, "and holds the same thing");

        // ...including the blocks nested inside it.
        let original_child = original.kind.body(0).unwrap()[0].id.clone();
        let copy_child = copy.kind.body(0).unwrap()[0].id.clone();
        assert_ne!(copy_child, original_child);
    }

    #[test]
    fn a_palette_prefab_nests_without_an_id() {
        // `BlockRegistry.prefab` stamps a placeholder id like "palette-Repeat";
        // it must not become the dropped block's identity.
        let flat = json!({ "id": "palette-Repeat", "type": "Repeat",
                           "count": { "kind": "Number", "value": 10.0 }, "body": [] });
        let nested = instruction_json(&flat.to_string(), true).unwrap();
        let dropped: Instruction<McBlock> = serde_json::from_value(nested).unwrap();
        assert_ne!(dropped.id, "palette-Repeat");
        assert!(!dropped.id.is_empty());
    }

    #[test]
    fn flatten_and_nest_are_inverses_over_a_whole_document() {
        let mut project = ModProject::new("demo");
        project.graph.add_strand(
            0,
            0,
            vec![
                Instruction::new(McBlock::OnPlayerJoin),
                Instruction::new(McBlock::Repeat {
                    count: BlockValue::number(2.0),
                    body: vec![Instruction::new(McBlock::Broadcast {
                        text: BlockValue::text("hi"),
                    })],
                }),
            ],
        );
        let saved = serde_json::to_value(&project).unwrap();
        let mut round_tripped = saved.clone();
        flatten(&mut round_tripped);
        nest(&mut round_tripped, false);
        assert_eq!(round_tripped, saved);
    }
}
