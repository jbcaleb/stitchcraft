//! Checking a document before it is exported.
//!
//! Errors are things the exporter cannot produce valid Java from; warnings are
//! things that will compile and then not do what the canvas suggests. Export
//! refuses on any error and passes warnings through to the caller.
//!
//! Every diagnostic carries the strand it came from, so the app can walk the
//! user to it rather than printing a wall of text.

use crate::block::McBlock;
use crate::context::{HookContext, context_of, reporter_needs};
use crate::document::ModProject;
use crate::kinds::Target;
use blockstitch_core::graph::{BlockKind, Instruction, Strand};
use blockstitch_core::value::{Op, Value};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    /// The strand the problem is in, where there is one.
    pub strand_id: Option<String>,
}

impl Diagnostic {
    fn error(strand_id: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            strand_id: strand_id.map(str::to_string),
        }
    }

    fn warning(strand_id: Option<&str>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            strand_id: strand_id.map(str::to_string),
        }
    }
}

/// Everything found in one pass over a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
    }

    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }
}

/// What the document declares, gathered once so every later check is a lookup.
#[derive(Debug, Default, Clone)]
pub struct Declarations {
    pub blocks: Vec<String>,
    pub items: Vec<String>,
    pub entities: Vec<String>,
    pub sounds: Vec<String>,
    pub tabs: Vec<String>,
}

impl Declarations {
    /// Collects every `Register*` header in the document, in strand order.
    pub fn gather(project: &ModProject) -> Self {
        let mut declarations = Self::default();
        for strand in &project.strands {
            let Some(first) = strand.instructions.first() else {
                continue;
            };
            let Some(id) = first.kind.declared_id() else {
                continue;
            };
            let id = id.trim().to_string();
            if id.is_empty() {
                continue;
            }
            let bucket = match &first.kind {
                McBlock::RegisterBlock { .. } => &mut declarations.blocks,
                McBlock::RegisterItem { .. } => &mut declarations.items,
                McBlock::RegisterEntity { .. } => &mut declarations.entities,
                McBlock::RegisterSound { .. } => &mut declarations.sounds,
                McBlock::RegisterCreativeTab { .. } => &mut declarations.tabs,
                _ => continue,
            };
            if !bucket.contains(&id) {
                bucket.push(id);
            }
        }
        declarations
    }
}

/// True for the characters a registry path may hold: `[a-z0-9_.-]`.
fn is_legal_path(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-'))
}

/// True if `name` is a legal Java package: dot-separated identifiers, none of
/// them a keyword we are likely to hit.
fn is_legal_package(name: &str) -> bool {
    const RESERVED: &[&str] = &[
        "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class",
        "const", "continue", "default", "do", "double", "else", "enum", "extends", "final",
        "finally", "float", "for", "goto", "if", "implements", "import", "instanceof", "int",
        "interface", "long", "native", "new", "package", "private", "protected", "public",
        "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this",
        "throw", "throws", "transient", "try", "void", "volatile", "while",
    ];
    !name.is_empty()
        && name.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with(|c: char| c.is_ascii_digit())
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                && !RESERVED.contains(&part)
        })
}

/// Checks `project`, returning every error and warning found.
pub fn check(project: &ModProject) -> Report {
    let mut report = Report::default();
    let declarations = Declarations::gather(project);
    check_metadata(project, &mut report);
    check_declarations(project, &mut report);
    check_strands(project, &declarations, &mut report);
    report
}

fn check_metadata(project: &ModProject, report: &mut Report) {
    if !project.loaders.fabric && !project.loaders.neoforge {
        report.diagnostics.push(Diagnostic::error(
            None,
            "Pick at least one loader to export for.",
        ));
    }
    if !is_legal_path(&project.mod_id) {
        report.diagnostics.push(Diagnostic::error(
            None,
            format!(
                "\"{}\" is not a usable mod id - use lowercase letters, digits and underscores.",
                project.mod_id
            ),
        ));
    }
    if !is_legal_package(&project.package) {
        report.diagnostics.push(Diagnostic::error(
            None,
            format!("\"{}\" is not a valid Java package.", project.package),
        ));
    }
    if project.version.trim().is_empty() {
        report
            .diagnostics
            .push(Diagnostic::error(None, "Give the mod a version."));
    }
}

fn check_declarations(project: &ModProject, report: &mut Report) {
    // Registry ids only have to be unique within their own registry, so each
    // family is counted separately.
    let mut seen: HashMap<(&'static str, String), usize> = HashMap::new();
    for strand in &project.strands {
        let Some(first) = strand.instructions.first() else {
            continue;
        };
        let Some(raw) = first.kind.declared_id() else {
            continue;
        };
        let family = match &first.kind {
            McBlock::RegisterBlock { .. } => "block",
            McBlock::RegisterItem { .. } => "item",
            McBlock::RegisterEntity { .. } => "entity",
            McBlock::RegisterSound { .. } => "sound",
            McBlock::RegisterCreativeTab { .. } => "creative tab",
            _ => continue,
        };
        let id = raw.trim();
        if id.is_empty() {
            report.diagnostics.push(Diagnostic::error(
                Some(&strand.id),
                format!("This {family} declaration needs an id."),
            ));
            continue;
        }
        if !is_legal_path(id) {
            report.diagnostics.push(Diagnostic::error(
                Some(&strand.id),
                format!(
                    "\"{id}\" is not a usable {family} id - use lowercase letters, digits and underscores."
                ),
            ));
        }
        *seen.entry((family, id.to_string())).or_default() += 1;
        if seen[&(family, id.to_string())] == 2 {
            report.diagnostics.push(Diagnostic::error(
                Some(&strand.id),
                format!("Two {family} declarations both use the id \"{id}\"."),
            ));
        }
        if strand.instructions.len() > 1 {
            report.diagnostics.push(Diagnostic::warning(
                Some(&strand.id),
                format!(
                    "A {family} declaration has no body - the {} block(s) stacked under this one are ignored. Move them into an event hook.",
                    strand.instructions.len() - 1
                ),
            ));
        }
    }
}

/// Per-strand state a walk needs to answer "is this block where it belongs?".
struct Scope<'a> {
    strand_id: &'a str,
    context: HookContext,
    /// The hook's own type name, for readable messages.
    hook: &'static str,
    /// True inside the body of a reporter-shaped custom block.
    reports_value: bool,
    /// How many loops enclose the instruction being looked at.
    loop_depth: usize,
    /// True inside an "after N ticks" body, which runs once the event is over.
    delayed: bool,
}

fn hook_label(kind: &McBlock) -> &'static str {
    match kind {
        McBlock::OnModInit => "when the mod loads",
        McBlock::OnClientInit => "when the client loads",
        McBlock::OnServerStarted => "when the server starts",
        McBlock::OnServerStopping => "when the server stops",
        McBlock::OnServerTick => "every server tick",
        McBlock::OnLevelTick => "every level tick",
        McBlock::OnPlayerJoin => "when a player joins",
        McBlock::OnPlayerLeave => "when a player leaves",
        McBlock::OnBlockPlaced { .. } => "when a block is placed",
        McBlock::OnBlockBroken { .. } => "when a block is broken",
        McBlock::OnBlockUsed { .. } => "when a block is right-clicked",
        McBlock::OnBlockTick { .. } => "on a block's random tick",
        McBlock::OnItemUsed { .. } => "when an item is used",
        McBlock::OnEntityTick { .. } => "every tick of an entity",
        McBlock::OnEntityHurt => "when an entity is hurt",
        McBlock::OnEntityDeath => "when an entity dies",
        McBlock::OnCommand { .. } => "on a command",
        McBlock::BlockHeader { .. } => "a custom block",
        _ => "this hook",
    }
}

fn check_strands(project: &ModProject, declarations: &Declarations, report: &mut Report) {
    let variables: HashSet<&str> = project.variables.iter().map(|v| v.name.as_str()).collect();
    let lists: HashSet<&str> = project.lists.iter().map(|l| l.name.as_str()).collect();
    let dicts: HashSet<&str> = project.dicts.iter().map(|d| d.name.as_str()).collect();
    let mut command_names: HashMap<String, usize> = HashMap::new();

    for strand in &project.strands {
        let Some(first) = strand.instructions.first() else {
            continue;
        };
        if !first.kind.is_header() {
            report.diagnostics.push(Diagnostic::warning(
                Some(&strand.id),
                "This stack has no event block on top, so none of it runs. Give it a header.",
            ));
            continue;
        }
        if first.kind.is_declaration() {
            check_declaration_fields(project, strand, declarations, report);
            continue;
        }

        check_hook_target(strand, &first.kind, declarations, report);
        if let McBlock::OnCommand { name, .. } = &first.kind {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                report.diagnostics.push(Diagnostic::error(
                    Some(&strand.id),
                    "This command hook needs a name.",
                ));
            } else {
                *command_names.entry(trimmed.to_string()).or_default() += 1;
                if command_names[trimmed] == 2 {
                    report.diagnostics.push(Diagnostic::error(
                        Some(&strand.id),
                        format!("Two hooks both register the command /{trimmed}."),
                    ));
                }
            }
        }

        let reports_value = strand
            .block_header_id()
            .and_then(|id| project.block_def(id))
            .is_some_and(|def| def.shape.returns_value());
        let scope = Scope {
            strand_id: &strand.id,
            context: context_of(&first.kind).unwrap_or(HookContext::NONE),
            hook: hook_label(&first.kind),
            reports_value,
            loop_depth: 0,
            delayed: false,
        };
        if let McBlock::BlockHeader { block_id } = &first.kind
            && project.block_def(block_id).is_none()
        {
            report.diagnostics.push(Diagnostic::error(
                Some(&strand.id),
                "This custom block's definition is missing from the document.",
            ));
        }
        for instruction in &strand.instructions[1..] {
            check_instruction(
                instruction,
                &scope,
                project,
                declarations,
                (&variables, &lists, &dicts),
                report,
            );
        }
    }
}

fn check_declaration_fields(
    project: &ModProject,
    strand: &Strand<McBlock>,
    declarations: &Declarations,
    report: &mut Report,
) {
    let tab = match &strand.instructions[0].kind {
        McBlock::RegisterBlock { creative_tab, .. } | McBlock::RegisterItem { creative_tab, .. } => {
            creative_tab.trim()
        }
        _ => return,
    };
    if tab.is_empty() {
        return;
    }
    if !declarations.tabs.iter().any(|declared| declared == tab) {
        report.diagnostics.push(Diagnostic::warning(
            Some(&strand.id),
            format!(
                "Creative tab \"{tab}\" is not declared anywhere, so this will not show up in the creative menu."
            ),
        ));
    }
    let _ = project;
}

fn check_hook_target(
    strand: &Strand<McBlock>,
    hook: &McBlock,
    declarations: &Declarations,
    report: &mut Report,
) {
    let (id, declared, family) = match hook {
        McBlock::OnBlockUsed { block_id } | McBlock::OnBlockTick { block_id } => {
            (block_id, &declarations.blocks, "block")
        }
        McBlock::OnItemUsed { item_id } => (item_id, &declarations.items, "item"),
        McBlock::OnEntityTick { entity_id } => (entity_id, &declarations.entities, "entity"),
        _ => return,
    };
    let id = id.trim();
    if id.is_empty() {
        report.diagnostics.push(Diagnostic::error(
            Some(&strand.id),
            format!("Choose which {family} this hook is for."),
        ));
    } else if !declared.iter().any(|candidate| candidate == id) {
        report.diagnostics.push(Diagnostic::error(
            Some(&strand.id),
            format!(
                "This hook is for the {family} \"{id}\", which the document does not register. Only the mod's own {family}s can be hooked this way."
            ),
        ));
    }
}

fn check_instruction(
    instruction: &Instruction<McBlock>,
    scope: &Scope<'_>,
    project: &ModProject,
    declarations: &Declarations,
    names: (&HashSet<&str>, &HashSet<&str>, &HashSet<&str>),
    report: &mut Report,
) {
    let (variables, lists, dicts) = names;
    let strand = Some(scope.strand_id);
    match &instruction.kind {
        McBlock::CancelEvent if scope.delayed => {
            report.diagnostics.push(Diagnostic::warning(
                strand,
                "A delayed block runs after the event is over, so there is nothing left to cancel.",
            ));
        }
        McBlock::CancelEvent if !scope.context.cancellable => {
            report.diagnostics.push(Diagnostic::warning(
                strand,
                format!("\"{}\" cannot be cancelled, so this does nothing.", scope.hook),
            ));
        }
        McBlock::EscapeLoop | McBlock::ContinueLoop if scope.loop_depth == 0 => {
            report.diagnostics.push(Diagnostic::error(
                strand,
                "This block only works inside a repeat, while or for-each loop.",
            ));
        }
        McBlock::Return { .. } if !scope.reports_value => {
            report.diagnostics.push(Diagnostic::warning(
                strand,
                "Only a custom block shaped as a reporter can report a value; this stops the handler instead.",
            ));
        }
        McBlock::SetVariable { name, .. } | McBlock::ChangeVariable { name, .. } => {
            if !variables.contains(name.as_str()) {
                report.diagnostics.push(Diagnostic::error(
                    strand,
                    format!("There is no variable named \"{name}\"."),
                ));
            }
        }
        McBlock::CallBlock { block_id, args } => match project.block_def(block_id) {
            None => report.diagnostics.push(Diagnostic::error(
                strand,
                "This calls a custom block that is no longer defined.",
            )),
            Some(def) => {
                let expected = def.input_names().count();
                if expected != args.len() {
                    report.diagnostics.push(Diagnostic::error(
                        strand,
                        format!(
                            "This call passes {} argument(s) but the block takes {expected}.",
                            args.len()
                        ),
                    ));
                }
            }
        },
        _ => {}
    }

    if let Some(list) = list_target(&instruction.kind)
        && !lists.contains(list)
    {
        report.diagnostics.push(Diagnostic::error(
            strand,
            format!("There is no list named \"{list}\"."),
        ));
    }
    if let Some(dict) = dict_target(&instruction.kind)
        && !dicts.contains(dict)
    {
        report.diagnostics.push(Diagnostic::error(
            strand,
            format!("There is no dict named \"{dict}\"."),
        ));
    }
    if let Some(target) = command_target(&instruction.kind)
        && !scope.context.supports(target)
    {
        report.diagnostics.push(Diagnostic::warning(
            strand,
            format!(
                "\"{}\" does not give you {}, so this block has nothing to act on.",
                scope.hook,
                match target {
                    Target::EventPlayer => "an event player",
                    Target::EventEntity => "an event entity",
                    Target::NearestPlayer => "a position to measure from",
                    Target::AllPlayers => "a server",
                }
            ),
        ));
    }
    check_referenced_ids(&instruction.kind, scope, declarations, report);

    // Values first, then nested bodies, so a diagnostic reads in source order.
    let mut reported = HashSet::new();
    let mut kind = instruction.kind.clone();
    kind.visit_values_mut(&mut |value, _| {
        check_value(value, scope, variables, &mut reported, report);
    });

    let deeper = Scope {
        // A delayed body is compiled as a method of its own, so an enclosing
        // loop is not there to break out of any more.
        loop_depth: if matches!(instruction.kind, McBlock::ScheduleAfter { .. }) {
            0
        } else {
            scope.loop_depth + usize::from(opens_a_loop(&instruction.kind))
        },
        delayed: scope.delayed || matches!(instruction.kind, McBlock::ScheduleAfter { .. }),
        ..*scope
    };
    for slot in 0..McBlock::BODY_SLOTS {
        let Some(body) = instruction.kind.body(slot) else {
            continue;
        };
        for child in body {
            check_instruction(child, &deeper, project, declarations, names, report);
        }
    }
}

impl Clone for Scope<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Scope<'_> {}

fn opens_a_loop(kind: &McBlock) -> bool {
    matches!(
        kind,
        McBlock::Repeat { .. } | McBlock::While { .. } | McBlock::ForEachPlayer { .. }
    )
}

fn list_target(kind: &McBlock) -> Option<&str> {
    match kind {
        McBlock::AddToList { name, .. }
        | McBlock::DeleteOfList { name, .. }
        | McBlock::DeleteAllOfList { name }
        | McBlock::InsertIntoList { name, .. }
        | McBlock::ReplaceItemOfList { name, .. } => Some(name),
        _ => None,
    }
}

fn dict_target(kind: &McBlock) -> Option<&str> {
    match kind {
        McBlock::DictSet { name, .. }
        | McBlock::DictDelete { name, .. }
        | McBlock::DictClear { name } => Some(name),
        _ => None,
    }
}

fn command_target(kind: &McBlock) -> Option<Target> {
    match kind {
        McBlock::Message { target, .. }
        | McBlock::GiveItem { target, .. }
        | McBlock::DamageTarget { target, .. }
        | McBlock::HealTarget { target, .. }
        | McBlock::AddEffect { target, .. }
        | McBlock::TeleportTarget { target, .. }
        | McBlock::PushTarget { target, .. } => Some(*target),
        _ => None,
    }
}

/// Warns about a command naming an id the mod does not register, where the
/// field is meant to be one of the mod's own.
fn check_referenced_ids(
    kind: &McBlock,
    scope: &Scope<'_>,
    declarations: &Declarations,
    report: &mut Report,
) {
    let strand = Some(scope.strand_id);
    // A namespaced id names someone else's content, which is fine and cannot
    // be checked from here. A bare path is meant to be one of ours.
    let unqualified_and_unknown = |id: &str, declared: &[String]| {
        let id = id.trim();
        !id.is_empty() && !id.contains(':') && !declared.iter().any(|d| d == id)
    };
    let (id, declared, family) = match kind {
        McBlock::SetBlockAt { block, .. } => (block.as_str(), &declarations.blocks, "block"),
        McBlock::GiveItem { item, .. } => (item.as_str(), &declarations.items, "item"),
        McBlock::SpawnEntityAt { entity, .. } => {
            (entity.as_str(), &declarations.entities, "entity")
        }
        McBlock::PlaySoundAt { sound, .. } => (sound.as_str(), &declarations.sounds, "sound"),
        _ => return,
    };
    if unqualified_and_unknown(id, declared) {
        report.diagnostics.push(Diagnostic::warning(
            strand,
            format!(
                "\"{id}\" is not a {family} this mod registers. Write it as `minecraft:{id}` if you meant a vanilla one."
            ),
        ));
    }
}

/// Walks one value tree, warning about reporters the enclosing hook cannot
/// answer and variables that are not declared. `reported` keeps one message per
/// reporter per instruction rather than one per occurrence.
fn check_value(
    value: &Value,
    scope: &Scope<'_>,
    variables: &HashSet<&str>,
    reported: &mut HashSet<String>,
    report: &mut Report,
) {
    match value {
        Value::Op { op, args, .. } => {
            if let Op::Ext(kind) = op
                && let Some(field) = reporter_needs(kind)
                && !scope.context.has(field)
                && reported.insert(kind.to_string())
            {
                report.diagnostics.push(Diagnostic::warning(
                    Some(scope.strand_id),
                    format!(
                        "\"{}\" does not give you {}, so this reporter reads as empty.",
                        scope.hook,
                        field.label()
                    ),
                ));
            }
            for arg in args {
                check_value(arg, scope, variables, reported, report);
            }
        }
        Value::Var { name } => {
            if !variables.contains(name.as_str()) && reported.insert(format!("var:{name}")) {
                report.diagnostics.push(Diagnostic::warning(
                    Some(scope.strand_id),
                    format!("Variable \"{name}\" is not declared; it reads as 0."),
                ));
            }
        }
        Value::Call { args, .. } => {
            for arg in args {
                check_value(arg, scope, variables, reported, report);
            }
        }
        Value::Number { .. } | Value::Text { .. } | Value::Bool | Value::Param { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{MaterialPreset, MessageKind};

    fn project_with(strands: Vec<Vec<McBlock>>) -> ModProject {
        let mut project = ModProject::new("demo");
        for blocks in strands {
            project.graph.add_strand(
                0,
                0,
                blocks.into_iter().map(Instruction::new).collect::<Vec<_>>(),
            );
        }
        project
    }

    fn messages(report: &Report) -> Vec<&str> {
        report
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect()
    }

    #[test]
    fn an_empty_project_is_valid() {
        let report = check(&ModProject::new("demo"));
        assert!(!report.has_errors(), "{:?}", messages(&report));
    }

    #[test]
    fn a_bad_package_is_an_error() {
        let mut project = ModProject::new("demo");
        project.package = "com.new.thing".to_string();
        let report = check(&project);
        assert!(report.errors().any(|d| d.message.contains("Java package")));
    }

    #[test]
    fn a_hook_on_an_unregistered_block_is_an_error() {
        let project = project_with(vec![vec![McBlock::OnBlockUsed {
            block_id: "chime".into(),
        }]]);
        let report = check(&project);
        assert!(
            report
                .errors()
                .any(|d| d.message.contains("does not register")),
            "{:?}",
            messages(&report)
        );
    }

    #[test]
    fn a_hook_on_a_registered_block_is_fine() {
        let project = project_with(vec![
            vec![McBlock::RegisterBlock {
                block_id: "chime".into(),
                display_name: "Chime".into(),
                material: MaterialPreset::Stone,
                hardness: Value::number(1.0),
                resistance: Value::number(1.0),
                light: Value::number(0.0),
                requires_tool: false,
                drops_self: true,
                give_item: true,
                creative_tab: String::new(),
            }],
            vec![McBlock::OnBlockUsed {
                block_id: "chime".into(),
            }],
        ]);
        let report = check(&project);
        assert!(!report.has_errors(), "{:?}", messages(&report));
    }

    #[test]
    fn a_break_outside_a_loop_is_an_error_and_inside_one_is_not() {
        let outside = project_with(vec![vec![McBlock::OnServerTick, McBlock::EscapeLoop]]);
        assert!(check(&outside).has_errors());

        let inside = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::Repeat {
                count: Value::number(3.0),
                body: vec![Instruction::new(McBlock::EscapeLoop)],
            },
        ]]);
        let report = check(&inside);
        assert!(!report.has_errors(), "{:?}", messages(&report));
    }

    #[test]
    fn a_reporter_the_hook_cannot_answer_is_a_warning() {
        let project = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::Broadcast {
                text: Value::op(Op::Ext("EventPlayerName".into()), vec![]),
            },
        ]]);
        let report = check(&project);
        assert!(!report.has_errors());
        assert!(
            report.warnings().any(|d| d.message.contains("a player")),
            "{:?}",
            messages(&report)
        );
    }

    #[test]
    fn the_same_reporter_is_only_reported_once_per_instruction() {
        let name = || Value::op(Op::Ext("EventPlayerName".into()), vec![]);
        let project = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::Broadcast {
                text: Value::op(Op::Join, vec![name(), name()]),
            },
        ]]);
        assert_eq!(check(&project).warnings().count(), 1);
    }

    #[test]
    fn targeting_the_event_player_from_a_tick_hook_warns() {
        let project = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::Message {
                target: Target::EventPlayer,
                kind: MessageKind::Chat,
                text: Value::text("hi"),
            },
        ]]);
        let report = check(&project);
        assert!(
            report
                .warnings()
                .any(|d| d.message.contains("an event player")),
            "{:?}",
            messages(&report)
        );
    }

    #[test]
    fn a_declarations_tail_is_flagged_as_dead() {
        let project = project_with(vec![vec![
            McBlock::RegisterSound {
                sound_id: "chime".into(),
                subtitle: String::new(),
            },
            McBlock::Broadcast {
                text: Value::text("hi"),
            },
        ]]);
        let report = check(&project);
        assert!(
            report.warnings().any(|d| d.message.contains("are ignored")),
            "{:?}",
            messages(&report)
        );
    }

    #[test]
    fn a_headerless_stack_is_flagged_as_inert() {
        let project = project_with(vec![vec![McBlock::Broadcast {
            text: Value::text("hi"),
        }]]);
        let report = check(&project);
        assert!(report.warnings().any(|d| d.message.contains("none of it runs")));
    }

    #[test]
    fn two_hooks_cannot_claim_one_command_name() {
        let project = project_with(vec![
            vec![McBlock::OnCommand {
                name: "ping".into(),
                op_only: false,
            }],
            vec![McBlock::OnCommand {
                name: "ping".into(),
                op_only: true,
            }],
        ]);
        let report = check(&project);
        assert!(report.errors().any(|d| d.message.contains("/ping")));
    }

    #[test]
    fn undeclared_collections_are_errors() {
        let project = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::AddToList {
                name: "scores".into(),
                value: Value::number(1.0),
            },
        ]]);
        let report = check(&project);
        assert!(report.errors().any(|d| d.message.contains("no list named")));
    }

    #[test]
    fn a_bare_id_the_mod_does_not_register_is_a_warning() {
        let project = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::SetBlockAt {
                x: Value::number(0.0),
                y: Value::number(0.0),
                z: Value::number(0.0),
                block: "chime".into(),
            },
        ]]);
        let report = check(&project);
        assert!(
            report.warnings().any(|d| d.message.contains("minecraft:chime")),
            "{:?}",
            messages(&report)
        );

        let vanilla = project_with(vec![vec![
            McBlock::OnServerTick,
            McBlock::SetBlockAt {
                x: Value::number(0.0),
                y: Value::number(0.0),
                z: Value::number(0.0),
                block: "minecraft:stone".into(),
            },
        ]]);
        assert_eq!(check(&vanilla).warnings().count(), 0);
    }
}
