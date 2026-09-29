//! Document edits the bridge performs that are more than one call into
//! blockstitch-core, kept out of the Qt layer so they can be tested without one.

use blockstitch_core::editor::PathStep;
use blockstitch_core::graph::BlockKind;
use stitchcraft_blocks::ModProject;

/// Deletes the block at `path` **and everything below it**.
///
/// That is what the canvas carries when a block is picked up - the whole tail
/// rides along - so deleting only the block that was grabbed would leave the rest
/// of the stack behind, having visibly just been dropped on the trash.
///
/// Three cases:
///
/// * the head of a stack: the whole stack goes;
/// * anywhere lower: the stack keeps what is above the grabbed block;
/// * a custom block's header: the block *is* its header, so the definition and
///   every call of it go too, rather than being left dangling.
pub fn delete_tail(
    project: &mut ModProject,
    strand_id: &str,
    path: &[PathStep],
) -> Result<(), String> {
    let block_id = project
        .instruction_at(strand_id, path)
        .and_then(|instruction| instruction.kind.block_header_id().map(str::to_string));
    if let Some(block_id) = block_id {
        project.remove_block(&block_id);
        return Ok(());
    }

    // Split the tail off into a strand of its own, then discard that strand.
    let tail = project.split_strand(strand_id, path, 0, 0)?;
    project.remove_strand(&tail);
    // Splitting at the head leaves the original with nothing in it, and an empty
    // strand is not something to keep.
    let emptied = project
        .strand(strand_id)
        .is_some_and(|strand| strand.instructions.is_empty());
    if emptied {
        project.remove_strand(strand_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockstitch_core::graph::{BlockPiece, BlockShape, Instruction};
    use blockstitch_core::value::Value;
    use stitchcraft_blocks::McBlock;

    fn stack(project: &mut ModProject, blocks: Vec<McBlock>) -> String {
        project
            .graph
            .add_strand(0, 0, blocks.into_iter().map(Instruction::new).collect())
    }

    fn say(text: &str) -> McBlock {
        McBlock::Broadcast {
            text: Value::text(text),
        }
    }

    fn texts(project: &ModProject, strand_id: &str) -> Vec<String> {
        project
            .strand(strand_id)
            .map(|strand| {
                strand
                    .instructions
                    .iter()
                    .map(|i| match &i.kind {
                        McBlock::Broadcast { text } => match text {
                            Value::Text { value } => value.clone(),
                            _ => "?".to_string(),
                        },
                        other => format!("{other:?}"),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn grabbing_the_head_of_a_stack_deletes_the_whole_stack() {
        let mut project = ModProject::new("demo");
        let id = stack(&mut project, vec![McBlock::OnServerTick, say("a"), say("b")]);
        delete_tail(&mut project, &id, &[PathStep::at(0)]).unwrap();
        assert!(project.strands.is_empty(), "no stack, and no empty husk of one");
    }

    #[test]
    fn grabbing_a_middle_block_takes_everything_below_it_and_only_that() {
        let mut project = ModProject::new("demo");
        let id = stack(
            &mut project,
            vec![McBlock::OnServerTick, say("keep"), say("gone"), say("also gone")],
        );
        delete_tail(&mut project, &id, &[PathStep::at(2)]).unwrap();
        assert_eq!(project.strands.len(), 1, "no stray strand is left holding the tail");
        assert_eq!(texts(&project, &id).len(), 2);
        assert_eq!(texts(&project, &id)[1], "keep");
    }

    #[test]
    fn grabbing_the_last_block_takes_only_that_block() {
        let mut project = ModProject::new("demo");
        let id = stack(&mut project, vec![McBlock::OnServerTick, say("a"), say("b")]);
        delete_tail(&mut project, &id, &[PathStep::at(2)]).unwrap();
        assert_eq!(texts(&project, &id).len(), 2);
    }

    #[test]
    fn a_tail_inside_a_nested_body_leaves_the_rest_of_the_outer_stack_alone() {
        let mut project = ModProject::new("demo");
        let id = stack(
            &mut project,
            vec![
                McBlock::OnServerTick,
                McBlock::Repeat {
                    count: Value::number(2.0),
                    body: vec![
                        Instruction::new(say("inner keep")),
                        Instruction::new(say("inner gone")),
                    ],
                },
                say("outer after"),
            ],
        );
        // Grab the second block inside the repeat's body.
        let path = [PathStep::into_body(1, 0), PathStep::at(1)];
        delete_tail(&mut project, &id, &path).unwrap();

        let strand = project.strand(&id).unwrap();
        assert_eq!(strand.instructions.len(), 3, "the outer stack is intact");
        let body = strand.instructions[1].kind.body(0).unwrap();
        assert_eq!(body.len(), 1, "the repeat lost only its second block");
        assert_eq!(project.strands.len(), 1, "and no stray strand remains");
    }

    #[test]
    fn deleting_a_custom_blocks_header_takes_the_definition_and_its_calls() {
        let mut project = ModProject::new("demo");
        let block_id = project.graph.create_block(
            vec![BlockPiece::Label {
                id: "l".into(),
                text: "shout".into(),
            }],
            BlockShape::Normal,
            "#4C97FF".into(),
            0,
            0,
            |id| McBlock::BlockHeader {
                block_id: id.to_string(),
            },
        );
        let caller = stack(
            &mut project,
            vec![
                McBlock::OnServerTick,
                McBlock::CallBlock {
                    block_id: block_id.clone(),
                    args: vec![],
                },
            ],
        );
        let header_strand = project
            .strands
            .iter()
            .find(|s| s.block_header_id() == Some(block_id.as_str()))
            .map(|s| s.id.clone())
            .unwrap();

        delete_tail(&mut project, &header_strand, &[PathStep::at(0)]).unwrap();

        assert!(project.block_def(&block_id).is_none(), "the definition is gone");
        assert!(
            project.strand(&header_strand).is_none(),
            "and so is its body"
        );
        assert_eq!(
            texts(&project, &caller).len(),
            1,
            "the call to it is removed rather than left dangling"
        );
    }

    #[test]
    fn a_path_that_no_longer_exists_is_an_error_and_changes_nothing() {
        let mut project = ModProject::new("demo");
        let id = stack(&mut project, vec![McBlock::OnServerTick, say("a")]);
        let before = project.clone();
        assert!(delete_tail(&mut project, &id, &[PathStep::at(9)]).is_err());
        assert_eq!(project, before);
    }
}
