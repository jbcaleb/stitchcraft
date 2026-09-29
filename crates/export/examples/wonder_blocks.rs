//! Builds the sample project and exports it, as a worked example of the whole
//! pipeline without the editor in the way.
//!
//! ```text
//! cargo run -p stitchcraft-export --example wonder_blocks -- out
//! ```
//!
//! It writes `wonder_blocks.stitch` next to the output, which is a real saved
//! document - open it in the app to see the same canvas the code below builds.

use blockstitch_core::graph::Instruction;
use blockstitch_core::value::{Op, Value};
use stitchcraft_blocks::kinds::{
    EntityBase, MaterialPreset, MessageKind, MobCategoryKind, Rarity, Target,
};
use stitchcraft_blocks::{McBlock, ModProject, reporters};

fn main() {
    reporters::register();

    let output = std::env::args().nth(1).unwrap_or_else(|| "out".to_string());
    let project = sample();

    let document = serde_json::to_string_pretty(&project).expect("a project serializes");
    let document_path = format!("{output}/wonder_blocks.stitch");
    std::fs::create_dir_all(&output).expect("output directory");
    std::fs::write(&document_path, document).expect("saving the document");

    match stitchcraft_export::export(&project) {
        Ok(bundle) => {
            for warning in &bundle.warnings {
                eprintln!("warning: {warning}");
            }
            let tree = format!("{output}/wonder_blocks");
            bundle.write_to(&tree).expect("writing the mod");
            println!(
                "{} file(s) in {tree}, and the canvas itself in {document_path}",
                bundle.files.len()
            );
        }
        Err(report) => {
            for diagnostic in report.errors() {
                eprintln!("error: {}", diagnostic.message);
            }
            std::process::exit(1);
        }
    }
}

fn number(n: f64) -> Value {
    Value::number(n)
}

/// A small mod: a block that chimes when you click it, an item, a wisp that
/// drifts about, a counter, and a command to read it back.
fn sample() -> ModProject {
    let mut project = ModProject::new("wonder_blocks");
    project.name = "Wonder Blocks".to_string();
    project.description = "A block that chimes, and a wisp that watches.".to_string();
    project.authors = vec!["you".to_string()];
    project.package = "com.example.wonderblocks".to_string();
    project
        .create_variable("chimes rung")
        .expect("a fresh variable");

    let mut strand = |x: i32, y: i32, instructions: Vec<McBlock>| {
        project.graph.add_strand(
            x,
            y,
            instructions.into_iter().map(Instruction::new).collect(),
        );
    };

    // ── Declarations ──────────────────────────────────────────────────────
    strand(
        0,
        0,
        vec![McBlock::RegisterCreativeTab {
            tab_id: "main".into(),
            display_name: "Wonder Blocks".into(),
            icon: String::new(),
        }],
    );
    strand(
        0,
        120,
        vec![McBlock::RegisterBlock {
            block_id: "chime_block".into(),
            display_name: "Chime Block".into(),
            material: MaterialPreset::Metal,
            hardness: number(2.0),
            resistance: number(4.0),
            light: number(7.0),
            requires_tool: true,
            drops_self: true,
            give_item: true,
            creative_tab: "main".into(),
        }],
    );
    strand(
        0,
        260,
        vec![McBlock::RegisterItem {
            item_id: "wisp_lantern".into(),
            display_name: "Wisp Lantern".into(),
            max_stack: number(1.0),
            rarity: Rarity::Rare,
            creative_tab: "main".into(),
        }],
    );
    strand(
        0,
        380,
        vec![McBlock::RegisterSound {
            sound_id: "chime".into(),
            subtitle: "A chime rings".into(),
        }],
    );
    strand(
        0,
        480,
        vec![McBlock::RegisterEntity {
            entity_id: "wisp".into(),
            display_name: "Wisp".into(),
            base: EntityBase::Flying,
            category: MobCategoryKind::Ambient,
            width: number(0.5),
            height: number(0.5),
            max_health: number(6.0),
            movement_speed: number(0.3),
            attack_damage: number(0.0),
            spawn_egg: true,
        }],
    );

    // ── Clicking the chime block ──────────────────────────────────────────
    strand(
        520,
        0,
        vec![
            McBlock::OnBlockUsed {
                block_id: "chime_block".into(),
            },
            McBlock::ChangeVariable {
                name: "chimes rung".into(),
                value: number(1.0),
            },
            McBlock::PlaySoundAt {
                sound: "chime".into(),
                x: Value::op(Op::Ext("EventX".into()), vec![]),
                y: Value::op(Op::Ext("EventY".into()), vec![]),
                z: Value::op(Op::Ext("EventZ".into()), vec![]),
                volume: number(1.0),
                pitch: number(1.2),
            },
            McBlock::Message {
                target: Target::EventPlayer,
                kind: MessageKind::ActionBar,
                text: Value::op(
                    Op::Join,
                    vec![
                        Value::text("chime "),
                        Value::Var {
                            name: "chimes rung".into(),
                        },
                    ],
                ),
            },
            // Sparkle a moment later, so the two do not land on the same tick.
            McBlock::ScheduleAfter {
                ticks: number(10.0),
                body: vec![Instruction::new(McBlock::SpawnParticleAt {
                    particle: "minecraft:end_rod".into(),
                    x: Value::op(Op::Ext("EventX".into()), vec![]),
                    y: Value::op(
                        Op::Add,
                        vec![Value::op(Op::Ext("EventY".into()), vec![]), number(1.0)],
                    ),
                    z: Value::op(Op::Ext("EventZ".into()), vec![]),
                    count: number(12.0),
                })],
            },
        ],
    );

    // ── Every tenth chime, a wisp ─────────────────────────────────────────
    strand(
        520,
        420,
        vec![
            McBlock::OnBlockTick {
                block_id: "chime_block".into(),
            },
            McBlock::If {
                condition: Value::op(Op::Ext("RandomChance".into()), vec![number(10.0)]),
                body: vec![Instruction::new(McBlock::SpawnEntityAt {
                    entity: "wisp".into(),
                    x: Value::op(Op::Ext("EventX".into()), vec![]),
                    y: Value::op(
                        Op::Add,
                        vec![Value::op(Op::Ext("EventY".into()), vec![]), number(2.0)],
                    ),
                    z: Value::op(Op::Ext("EventZ".into()), vec![]),
                })],
            },
        ],
    );

    // ── A greeting, and a command to read the counter ──────────────────────
    strand(
        1040,
        0,
        vec![
            McBlock::OnPlayerJoin,
            McBlock::Message {
                target: Target::EventPlayer,
                kind: MessageKind::Chat,
                text: Value::op(
                    Op::Join,
                    vec![
                        Value::text("Welcome. Chimes so far: "),
                        Value::Var {
                            name: "chimes rung".into(),
                        },
                    ],
                ),
            },
        ],
    );
    strand(
        1040,
        200,
        vec![
            McBlock::OnCommand {
                name: "chimes".into(),
                op_only: false,
            },
            McBlock::Message {
                target: Target::EventPlayer,
                kind: MessageKind::Chat,
                text: Value::op(
                    Op::Join,
                    vec![
                        Value::Var {
                            name: "chimes rung".into(),
                        },
                        Value::text(" chime(s) rung"),
                    ],
                ),
            },
        ],
    );

    project
}
