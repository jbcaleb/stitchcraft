//! The `Backend` QObject: the one thing QML talks to.
//!
//! # Why everything crosses as JSON
//!
//! blockstitch's QML canvas already speaks JSON-shaped JavaScript objects - an
//! instruction is `{type, ...fields}`, a value is `{kind, ...}`, a path is
//! `[{index, slot}]` - and those are exactly the shapes serde produces from
//! [`McBlock`], [`Value`] and [`PathStep`]. Passing them as `QString` of JSON
//! means the two halves share one format with no hand-written marshalling in
//! between, and no `QVariantMap` conversions to keep in step with the enums.
//!
//! So: QML stringifies a gesture's arguments, Rust parses them, the edit runs
//! through blockstitch-core, and the whole document goes back as one property
//! change. A canvas is small enough that redrawing all of it is cheaper than
//! working out what changed.
//!
//! # Where the editing actually happens
//!
//! Nowhere in this file. Every gesture resolves to one blockstitch-core editor
//! operation, which validates before it mutates; this is the layer that
//! snapshots for undo, marks the document dirty, and pushes the result back.

use blockstitch_core::editor::{
    EditSession, History, InstrPath, PathStep, ValueBuffers, ValueLocation, drop_strand_buffers,
    prune_value_buffers,
};
use blockstitch_core::graph::{
    BlockGraph, BlockPiece, BlockShape, DictEntry, Instruction, ListItem, normalize_block_color,
};
use blockstitch_core::value::Value;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde::de::DeserializeOwned;
use std::pin::Pin;
use stitchcraft_blocks::{McBlock, ModProject, catalog, reporters, validate};

/// How many undo steps a session keeps.
const UNDO_LIMIT: usize = 200;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        /// The editor's whole state, as QML sees it.
        ///
        /// The `*_json` properties are what the canvas and palette bind to; the
        /// rest is chrome - a window title, an enabled state for a menu item.
        #[qobject]
        #[qml_element]
        #[qproperty(QString, document_json, cxx_name = "documentJson")]
        #[qproperty(QString, rows_json, cxx_name = "rowsJson")]
        #[qproperty(QString, prefabs_json, cxx_name = "prefabsJson")]
        #[qproperty(QString, operator_rows_json, cxx_name = "operatorRowsJson")]
        #[qproperty(QString, sections_json, cxx_name = "sectionsJson")]
        #[qproperty(QString, reporters_json, cxx_name = "reportersJson")]
        #[qproperty(QString, list_reporters_json, cxx_name = "listReportersJson")]
        #[qproperty(QString, dict_reporters_json, cxx_name = "dictReportersJson")]
        #[qproperty(QString, list_types_json, cxx_name = "listTypesJson")]
        #[qproperty(QString, dict_types_json, cxx_name = "dictTypesJson")]
        #[qproperty(QString, choices_json, cxx_name = "choicesJson")]
        #[qproperty(QString, diagnostics_json, cxx_name = "diagnosticsJson")]
        #[qproperty(QString, mod_id, cxx_name = "modId")]
        #[qproperty(QString, mod_name, cxx_name = "modName")]
        #[qproperty(QString, mod_version, cxx_name = "modVersion")]
        #[qproperty(QString, mod_package, cxx_name = "modPackage")]
        #[qproperty(bool, fabric)]
        #[qproperty(bool, neoforge)]
        #[qproperty(QString, document_path, cxx_name = "documentPath")]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, can_undo, cxx_name = "canUndo")]
        #[qproperty(bool, can_redo, cxx_name = "canRedo")]
        #[qproperty(QString, status, cxx_name = "status")]
        type Backend = super::BackendRust;

        // ── Document ──────────────────────────────────────────────────────
        #[qinvokable]
        #[cxx_name = "newProject"]
        fn new_project(self: Pin<&mut Backend>);

        #[qinvokable]
        fn open(self: Pin<&mut Backend>, path: &QString) -> bool;

        #[qinvokable]
        fn save(self: Pin<&mut Backend>, path: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "exportMod"]
        fn export_mod(self: Pin<&mut Backend>, directory: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "setMetadata"]
        fn set_metadata(
            self: Pin<&mut Backend>,
            mod_id: &QString,
            name: &QString,
            version: &QString,
            package: &QString,
            fabric: bool,
            neoforge: bool,
        );

        #[qinvokable]
        fn undo(self: Pin<&mut Backend>);

        #[qinvokable]
        fn redo(self: Pin<&mut Backend>);

        // ── Strands and instructions ──────────────────────────────────────
        #[qinvokable]
        #[cxx_name = "moveStrand"]
        fn move_strand(self: Pin<&mut Backend>, strand_id: &QString, x: i32, y: i32);

        #[qinvokable]
        #[cxx_name = "splitInstruction"]
        fn split_instruction(
            self: Pin<&mut Backend>,
            strand_id: &QString,
            path: &QString,
            x: i32,
            y: i32,
        );

        #[qinvokable]
        #[cxx_name = "mergeStrands"]
        fn merge_strands(
            self: Pin<&mut Backend>,
            dragged_id: &QString,
            target_id: &QString,
            path: &QString,
        );

        #[qinvokable]
        #[cxx_name = "mergeTail"]
        fn merge_tail(
            self: Pin<&mut Backend>,
            strand_id: &QString,
            path: &QString,
            target_id: &QString,
            target_path: &QString,
        );

        #[qinvokable]
        #[cxx_name = "removeInstruction"]
        fn remove_instruction(self: Pin<&mut Backend>, strand_id: &QString, path: &QString);

        #[qinvokable]
        #[cxx_name = "deleteTail"]
        fn delete_tail(self: Pin<&mut Backend>, strand_id: &QString, path: &QString);

        #[qinvokable]
        #[cxx_name = "duplicateInstruction"]
        fn duplicate_instruction(
            self: Pin<&mut Backend>,
            strand_id: &QString,
            path: &QString,
            instruction: &QString,
        );

        #[qinvokable]
        #[cxx_name = "editInstruction"]
        fn edit_instruction(
            self: Pin<&mut Backend>,
            strand_id: &QString,
            path: &QString,
            instruction: &QString,
        );

        #[qinvokable]
        #[cxx_name = "dropPalette"]
        fn drop_palette(
            self: Pin<&mut Backend>,
            instruction: &QString,
            target_id: &QString,
            path: &QString,
            x: i32,
            y: i32,
        );

        // ── Values ────────────────────────────────────────────────────────
        #[qinvokable]
        #[cxx_name = "editValue"]
        fn edit_value(self: Pin<&mut Backend>, location: &QString, text: &QString);

        #[qinvokable]
        #[cxx_name = "takeValue"]
        fn take_value(self: Pin<&mut Backend>, location: &QString);

        #[qinvokable]
        #[cxx_name = "deleteValue"]
        fn delete_value(self: Pin<&mut Backend>, location: &QString);

        #[qinvokable]
        #[cxx_name = "putValue"]
        fn put_value(self: Pin<&mut Backend>, location: &QString, value: &QString);

        #[qinvokable]
        #[cxx_name = "createValue"]
        fn create_value(self: Pin<&mut Backend>, x: i32, y: i32, value: &QString);

        #[qinvokable]
        #[cxx_name = "moveFloatingValue"]
        fn move_floating_value(self: Pin<&mut Backend>, floating_id: &QString, x: i32, y: i32);

        #[qinvokable]
        #[cxx_name = "removeFloatingValue"]
        fn remove_floating_value(self: Pin<&mut Backend>, floating_id: &QString);

        // ── Notes ─────────────────────────────────────────────────────────
        #[qinvokable]
        #[cxx_name = "addComment"]
        fn add_comment(self: Pin<&mut Backend>, x: i32, y: i32, attached_to: &QString);

        #[qinvokable]
        #[cxx_name = "moveComment"]
        fn move_comment(self: Pin<&mut Backend>, comment_id: &QString, x: i32, y: i32);

        #[qinvokable]
        #[cxx_name = "editComment"]
        fn edit_comment(self: Pin<&mut Backend>, comment_id: &QString, text: &QString);

        #[qinvokable]
        #[cxx_name = "setCommentCollapsed"]
        fn set_comment_collapsed(
            self: Pin<&mut Backend>,
            comment_id: &QString,
            collapsed: bool,
        );

        #[qinvokable]
        #[cxx_name = "removeComment"]
        fn remove_comment(self: Pin<&mut Backend>, comment_id: &QString);

        // ── Variables, lists, dicts ───────────────────────────────────────
        #[qinvokable]
        #[cxx_name = "createVariable"]
        fn create_variable(self: Pin<&mut Backend>, name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "renameVariable"]
        fn rename_variable(self: Pin<&mut Backend>, old: &QString, new_name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "deleteVariable"]
        fn delete_variable(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "createList"]
        fn create_list(self: Pin<&mut Backend>, name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "renameList"]
        fn rename_list(self: Pin<&mut Backend>, old: &QString, new_name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "deleteList"]
        fn delete_list(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "setListItems"]
        fn set_list_items(self: Pin<&mut Backend>, name: &QString, items: &QString);

        #[qinvokable]
        #[cxx_name = "setListEditorState"]
        fn set_list_editor_state(
            self: Pin<&mut Backend>,
            name: &QString,
            visible: bool,
            x: i32,
            y: i32,
        );

        #[qinvokable]
        #[cxx_name = "createDict"]
        fn create_dict(self: Pin<&mut Backend>, name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "renameDict"]
        fn rename_dict(self: Pin<&mut Backend>, old: &QString, new_name: &QString) -> bool;

        #[qinvokable]
        #[cxx_name = "deleteDict"]
        fn delete_dict(self: Pin<&mut Backend>, name: &QString);

        #[qinvokable]
        #[cxx_name = "setDictEntries"]
        fn set_dict_entries(self: Pin<&mut Backend>, name: &QString, entries: &QString);

        #[qinvokable]
        #[cxx_name = "setDictEditorState"]
        fn set_dict_editor_state(
            self: Pin<&mut Backend>,
            name: &QString,
            visible: bool,
            x: i32,
            y: i32,
        );

        // ── Custom blocks ─────────────────────────────────────────────────
        #[qinvokable]
        #[cxx_name = "makeBlock"]
        fn make_block(
            self: Pin<&mut Backend>,
            pieces: &QString,
            shape: &QString,
            color: &QString,
            x: i32,
            y: i32,
        ) -> bool;

        #[qinvokable]
        #[cxx_name = "updateBlock"]
        fn update_block(
            self: Pin<&mut Backend>,
            block_id: &QString,
            pieces: &QString,
            shape: &QString,
            color: &QString,
        ) -> bool;

        #[qinvokable]
        #[cxx_name = "removeBlock"]
        fn remove_block(self: Pin<&mut Backend>, block_id: &QString);
    }
}

/// The editor's state. Only the `*_json` fields are QML properties; the rest is
/// the document itself and the bookkeeping around editing it.
pub struct BackendRust {
    // ── Properties ────────────────────────────────────────────────────────
    document_json: QString,
    rows_json: QString,
    prefabs_json: QString,
    operator_rows_json: QString,
    sections_json: QString,
    reporters_json: QString,
    list_reporters_json: QString,
    dict_reporters_json: QString,
    list_types_json: QString,
    dict_types_json: QString,
    choices_json: QString,
    diagnostics_json: QString,
    mod_id: QString,
    mod_name: QString,
    mod_version: QString,
    mod_package: QString,
    fabric: bool,
    neoforge: bool,
    document_path: QString,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
    status: QString,

    // ── State ─────────────────────────────────────────────────────────────
    project: ModProject,
    history: History<BlockGraph<McBlock>>,
    /// Half-typed numbers, so a field does not snap back mid-edit.
    buffers: ValueBuffers,
    /// A value was just lifted out of a slot and is on its way to another, so the
    /// next checkpoint is part of the same gesture rather than a new undo step.
    lifted: bool,
}

impl Default for BackendRust {
    fn default() -> Self {
        // Registering the reporters before any document is read is what gives
        // the palette their arity; doing it here means no startup ordering to
        // get wrong.
        reporters::register();

        // `stitchcraft <project.stitch>` opens that project instead of starting
        // empty. A path that will not load leaves a blank project and says so in
        // the status line, rather than refusing to start.
        let (project, path, status) = match std::env::args().nth(1) {
            Some(path) => match load(&path) {
                Ok(project) => {
                    let message = format!("Opened {path}.");
                    (project, path, message)
                }
                Err(error) => (ModProject::default(), String::new(), error),
            },
            None => (ModProject::default(), String::new(), String::new()),
        };

        Self {
            // Derived here as well as in `refresh`, so an opened project is on
            // the canvas before the first edit rather than after it.
            document_json: QString::from(&crate::wire::document_json(&project)),
            rows_json: QString::from(&catalog::rows_json().to_string()),
            prefabs_json: QString::from(&catalog::prefabs_json().to_string()),
            operator_rows_json: QString::from(&reporters::operator_rows_json().to_string()),
            sections_json: QString::from(&catalog::sections_json().to_string()),
            reporters_json: QString::from(&reporters::palette_entries_json().to_string()),
            list_reporters_json: QString::from(
                &reporters::list_palette_entries_json().to_string(),
            ),
            dict_reporters_json: QString::from(
                &reporters::dict_palette_entries_json().to_string(),
            ),
            list_types_json: QString::from(&serde_json::json!(catalog::LIST_TYPES).to_string()),
            dict_types_json: QString::from(&serde_json::json!(catalog::DICT_TYPES).to_string()),
            choices_json: QString::from(&BackendRust::choices_of(&project)),
            diagnostics_json: QString::from(&BackendRust::diagnostics_of(&project)),
            mod_id: QString::from(&project.mod_id),
            mod_name: QString::from(&project.name),
            mod_version: QString::from(&project.version),
            mod_package: QString::from(&project.package),
            fabric: project.loaders.fabric,
            neoforge: project.loaders.neoforge,
            document_path: QString::from(&path),
            dirty: false,
            can_undo: false,
            can_redo: false,
            status: QString::from(&status),
            project,
            history: History::new(UNDO_LIMIT),
            buffers: ValueBuffers::new(),
            lifted: false,
        }
    }
}

/// Reads a saved project, with the one-time repairs a loaded document needs.
fn load(path: &str) -> Result<ModProject, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|error| format!("Could not open {path}: {error}"))?;
    let mut project: ModProject = serde_json::from_str(&contents)
        .map_err(|error| format!("{path} is not a Stitchcraft project: {error}"))?;
    project.migrate_after_load();
    Ok(project)
}

/// Parses JSON that came from QML. A malformed payload means the QML side sent
/// something this build does not understand, which is a bug rather than bad
/// input - so it is reported in the status line and the edit is dropped, never
/// applied halfway.
fn parse<T: DeserializeOwned>(json: &QString) -> Option<T> {
    serde_json::from_str(&json.to_string()).ok()
}

fn path_of(json: &QString) -> Option<InstrPath> {
    parse::<Vec<PathStep>>(json)
}

/// One instruction as the canvas sends it - flat - back into the envelope the
/// document stores.
///
/// `fresh_ids` is false for an in-place edit, which must keep the identity of
/// the block it edited: comments attach to an instruction id, and so does every
/// path the canvas addresses. It is true for a palette drop and a duplicate,
/// where carrying the source's id over would give two blocks the same identity.
fn instruction_of(json: &QString, fresh_ids: bool) -> Option<Instruction<McBlock>> {
    let nested = crate::wire::instruction_json(&json.to_string(), fresh_ids)?;
    serde_json::from_value(nested).ok()
}

fn text(value: &QString) -> String {
    value.to_string()
}

impl qobject::Backend {
    // ── Pushing state out ─────────────────────────────────────────────────

    /// Rewrites every derived property from the document. Called at the end of
    /// anything that changed it.
    fn refresh(mut self: Pin<&mut Self>) {
        // Flattened, because the canvas reads an instruction as `ins.type` and
        // `ins[key]` rather than through the `{id, kind}` envelope the document
        // is saved as - see `wire`.
        let document = crate::wire::document_json(&self.rust().project);
        let choices = self.rust().choices();
        let diagnostics = self.rust().diagnostics();
        let project_mod_id = self.rust().project.mod_id.clone();
        let project_name = self.rust().project.name.clone();
        let project_version = self.rust().project.version.clone();
        let project_package = self.rust().project.package.clone();
        let (fabric, neoforge) = {
            let loaders = self.rust().project.loaders;
            (loaders.fabric, loaders.neoforge)
        };
        let (can_undo, can_redo) = {
            let history = &self.rust().history;
            (history.can_undo(), history.can_redo())
        };

        self.as_mut().set_document_json(QString::from(&document));
        self.as_mut().set_choices_json(QString::from(&choices));
        self.as_mut()
            .set_diagnostics_json(QString::from(&diagnostics));
        self.as_mut().set_mod_id(QString::from(&project_mod_id));
        self.as_mut().set_mod_name(QString::from(&project_name));
        self.as_mut()
            .set_mod_version(QString::from(&project_version));
        self.as_mut()
            .set_mod_package(QString::from(&project_package));
        self.as_mut().set_fabric(fabric);
        self.as_mut().set_neoforge(neoforge);
        self.as_mut().set_can_undo(can_undo);
        self.as_mut().set_can_redo(can_redo);
    }

    fn say(mut self: Pin<&mut Self>, message: impl AsRef<str>) {
        self.as_mut().set_status(QString::from(message.as_ref()));
    }

    /// Checkpoints the document for undo. Every structural edit calls this
    /// before it mutates.
    fn checkpoint(mut self: Pin<&mut Self>) {
        // Moving a value between slots is two calls - lift it, then put it down -
        // and one gesture. The lift already checkpointed, so the put must not: an
        // undo that stopped halfway, with the value gone from both places, would be
        // worse than no undo.
        if std::mem::take(&mut self.as_mut().rust_mut().lifted) {
            return;
        }
        let snapshot = self.rust().project.graph.clone();
        self.as_mut().rust_mut().history.push(snapshot);
    }

    /// Checkpoints only if this is a different edit from the one in progress, so
    /// a run of keystrokes in one field is one undo step.
    fn checkpoint_session(mut self: Pin<&mut Self>, session: EditSession) {
        let snapshot = self.rust().project.graph.clone();
        self.as_mut()
            .rust_mut()
            .history
            .push_for_session(snapshot, session);
    }

    /// Marks the document changed and pushes it out.
    fn changed(mut self: Pin<&mut Self>) {
        self.as_mut().set_dirty(true);
        self.refresh();
    }

    /// Reports a rejected edit without touching the document. blockstitch-core's
    /// operations validate before they mutate, so there is nothing to roll back.
    fn rejected(self: Pin<&mut Self>, reason: impl AsRef<str>) {
        self.say(reason);
    }

    // ── Document ──────────────────────────────────────────────────────────

    fn new_project(mut self: Pin<&mut Self>) {
        let fresh = ModProject::default();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.project = fresh;
            rust.history = History::new(UNDO_LIMIT);
            rust.buffers.clear();
        }
        self.as_mut().set_document_path(QString::from(""));
        self.as_mut().set_dirty(false);
        self.as_mut().say("New project.");
        self.refresh();
    }

    fn open(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let path = text(path);
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                self.rejected(format!("Could not open {path}: {error}"));
                return false;
            }
        };
        let mut project: ModProject = match serde_json::from_str(&contents) {
            Ok(project) => project,
            Err(error) => {
                self.rejected(format!("{path} is not a Stitchcraft project: {error}"));
                return false;
            }
        };
        project.migrate_after_load();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.project = project;
            rust.history = History::new(UNDO_LIMIT);
            rust.buffers.clear();
        }
        self.as_mut().set_document_path(QString::from(&path));
        self.as_mut().set_dirty(false);
        self.as_mut().say(format!("Opened {path}."));
        self.refresh();
        true
    }

    fn save(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let path = text(path);
        let path = if path.is_empty() {
            text(&self.rust().document_path)
        } else {
            path
        };
        if path.is_empty() {
            self.rejected("This project has no file yet - choose where to save it.");
            return false;
        }
        let contents = match serde_json::to_string_pretty(&self.rust().project) {
            Ok(contents) => contents,
            Err(error) => {
                self.rejected(format!("Could not serialize the project: {error}"));
                return false;
            }
        };
        if let Err(error) = std::fs::write(&path, contents) {
            self.rejected(format!("Could not write {path}: {error}"));
            return false;
        }
        self.as_mut().set_document_path(QString::from(&path));
        self.as_mut().set_dirty(false);
        self.say(format!("Saved to {path}."));
        true
    }

    fn export_mod(self: Pin<&mut Self>, directory: &QString) -> bool {
        let directory = text(directory);
        if directory.is_empty() {
            self.rejected("Choose a directory to export into.");
            return false;
        }
        let bundle = match stitchcraft_export::export(&self.rust().project) {
            Ok(bundle) => bundle,
            Err(report) => {
                let first = report
                    .errors()
                    .next()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| "the canvas has errors".to_string());
                let count = report.errors().count();
                self.rejected(if count > 1 {
                    format!("{first} (and {} more)", count - 1)
                } else {
                    first
                });
                return false;
            }
        };
        if let Err(error) = bundle.write_to(&directory) {
            self.rejected(format!("Could not write to {directory}: {error}"));
            return false;
        }
        let mut message = format!("Exported {} files to {directory}.", bundle.files.len());
        if let Some(warning) = bundle.warnings.first() {
            message.push(' ');
            message.push_str(warning);
        }
        self.say(message);
        true
    }

    fn set_metadata(
        mut self: Pin<&mut Self>,
        mod_id: &QString,
        name: &QString,
        version: &QString,
        package: &QString,
        fabric: bool,
        neoforge: bool,
    ) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.project.mod_id = stitchcraft_blocks::document::sanitize_mod_id(&text(mod_id));
            rust.project.name = text(name);
            rust.project.version = text(version);
            rust.project.package = text(package);
            rust.project.loaders.fabric = fabric;
            rust.project.loaders.neoforge = neoforge;
        }
        self.changed();
    }

    fn undo(mut self: Pin<&mut Self>) {
        let current = self.rust().project.graph.clone();
        let restored = self.as_mut().rust_mut().history.undo(current);
        match restored {
            Some(graph) => {
                self.as_mut().rust_mut().project.graph = graph;
                self.as_mut().say("Undone.");
                self.changed();
            }
            None => self.say("Nothing to undo."),
        }
    }

    fn redo(mut self: Pin<&mut Self>) {
        let current = self.rust().project.graph.clone();
        let restored = self.as_mut().rust_mut().history.redo(current);
        match restored {
            Some(graph) => {
                self.as_mut().rust_mut().project.graph = graph;
                self.as_mut().say("Redone.");
                self.changed();
            }
            None => self.say("Nothing to redo."),
        }
    }

    // ── Strands and instructions ──────────────────────────────────────────

    fn move_strand(mut self: Pin<&mut Self>, strand_id: &QString, x: i32, y: i32) {
        let strand_id = text(strand_id);
        // A drag is one undo step, and the canvas only reports the end of one.
        self.as_mut().checkpoint();
        if self
            .as_mut()
            .rust_mut()
            .project
            .move_strand(&strand_id, x, y)
        {
            self.changed();
        }
    }

    fn split_instruction(
        mut self: Pin<&mut Self>,
        strand_id: &QString,
        path: &QString,
        x: i32,
        y: i32,
    ) {
        let (strand_id, Some(path)) = (text(strand_id), path_of(path)) else {
            self.rejected("The canvas sent a path this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        match self
            .as_mut()
            .rust_mut()
            .project
            .split_strand(&strand_id, &path, x, y)
        {
            Ok(_) => self.changed(),
            Err(error) => self.rejected(error),
        }
    }

    fn merge_strands(
        mut self: Pin<&mut Self>,
        dragged_id: &QString,
        target_id: &QString,
        path: &QString,
    ) {
        let (dragged_id, target_id, Some(path)) =
            (text(dragged_id), text(target_id), path_of(path))
        else {
            self.rejected("The canvas sent a path this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        let result = self
            .as_mut()
            .rust_mut()
            .project
            .merge_strand(&dragged_id, &target_id, &path);
        match result {
            Ok(()) => {
                self.as_mut().rust_mut().prune_strand(&dragged_id);
                self.changed();
            }
            Err(error) => self.rejected(error),
        }
    }

    fn merge_tail(
        mut self: Pin<&mut Self>,
        strand_id: &QString,
        path: &QString,
        target_id: &QString,
        target_path: &QString,
    ) {
        let (strand_id, target_id, Some(path), Some(target_path)) = (
            text(strand_id),
            text(target_id),
            path_of(path),
            path_of(target_path),
        ) else {
            self.rejected("The canvas sent a path this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        let result = self
            .as_mut()
            .rust_mut()
            .project
            .merge_tail(&strand_id, &path, &target_id, &target_path);
        match result {
            Ok(()) => self.changed(),
            Err(error) => self.rejected(error),
        }
    }

    fn remove_instruction(mut self: Pin<&mut Self>, strand_id: &QString, path: &QString) {
        let (strand_id, Some(path)) = (text(strand_id), path_of(path)) else {
            self.rejected("The canvas sent a path this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        // `remove_instruction` leaves everything below where it is, which is what
        // deleting one block from a stack means - `delete_instruction` would
        // detach the tail into a strand of its own instead.
        let removed = self
            .as_mut()
            .rust_mut()
            .project
            .remove_instruction(&strand_id, &path);
        if removed {
            self.changed();
        }
    }

    /// Deletes a dragged block *and everything below it*. That is what the
    /// canvas carries when you pick a block up - the whole tail rides along - so
    /// deleting only the block you grabbed would leave the rest of the stack
    /// behind, having visibly just been dropped on the trash.
    fn delete_tail(mut self: Pin<&mut Self>, strand_id: &QString, path: &QString) {
        let (strand_id, Some(path)) = (text(strand_id), path_of(path)) else {
            self.rejected("The canvas sent a path this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        let result = crate::edit::delete_tail(&mut self.as_mut().rust_mut().project, &strand_id, &path);
        match result {
            Ok(()) => self.changed(),
            Err(error) => self.rejected(error),
        }
    }

    fn duplicate_instruction(
        mut self: Pin<&mut Self>,
        strand_id: &QString,
        path: &QString,
        instruction: &QString,
    ) {
        let (strand_id, Some(path), Some(copy)) = (
            text(strand_id),
            path_of(path),
            instruction_of(instruction, true),
        ) else {
            self.rejected("The canvas sent a block this build does not understand.");
            return;
        };
        // One past the block it came from, which is where a duplicate belongs.
        let mut below = path.clone();
        if let Some(last) = below.last_mut() {
            last.index += 1;
        }
        self.as_mut().checkpoint();
        let result = self.as_mut().rust_mut().project.insert_instruction(
            &strand_id,
            &below,
            copy,
        );
        match result {
            Ok(_) => self.changed(),
            Err(error) => self.rejected(error),
        }
    }

    fn edit_instruction(
        mut self: Pin<&mut Self>,
        strand_id: &QString,
        path: &QString,
        instruction: &QString,
    ) {
        let (strand_id, Some(path), Some(edited)) = (
            text(strand_id),
            path_of(path),
            instruction_of(instruction, false),
        ) else {
            self.rejected("The canvas sent a block this build does not understand.");
            return;
        };
        // A run of edits to one block coalesces: typing in a field should not
        // leave one undo step per character.
        self.as_mut().checkpoint_session(EditSession::Instruction {
            strand_id: strand_id.clone(),
            index: path.clone(),
        });
        let replaced = self
            .as_mut()
            .rust_mut()
            .project
            .replace_instruction(&strand_id, &path, edited);
        if replaced {
            self.changed();
        } else {
            self.rejected("That block is not there any more.");
        }
    }

    fn drop_palette(
        mut self: Pin<&mut Self>,
        instruction: &QString,
        target_id: &QString,
        path: &QString,
        x: i32,
        y: i32,
    ) {
        let Some(dropped) = instruction_of(instruction, true) else {
            self.rejected("The palette sent a block this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        let target_id = text(target_id);
        let landed = path_of(path).filter(|_| !target_id.is_empty());
        let result = match landed {
            // Dropped onto an existing stack.
            Some(path) => self.as_mut().rust_mut().project.insert_instruction(
                &target_id,
                &path,
                dropped,
            ),
            // Dropped on open canvas: a strand of its own.
            None => {
                self.as_mut()
                    .rust_mut()
                    .project
                    .add_strand(x, y, vec![dropped]);
                Ok(true)
            }
        };
        match result {
            Ok(_) => self.changed(),
            // The one rejection here is a header dropped onto an existing
            // stack, which `check_header_placement` explains.
            Err(error) => self.rejected(error),
        }
    }

    // ── Values ────────────────────────────────────────────────────────────

    fn edit_value(mut self: Pin<&mut Self>, location: &QString, text_value: &QString) {
        let Some(location) = parse::<ValueLocation>(location) else {
            self.rejected("The canvas sent a value slot this build does not understand.");
            return;
        };
        let typed = text(text_value);
        self.as_mut()
            .checkpoint_session(EditSession::Value(location.clone()));
        let edit = self
            .as_mut()
            .rust_mut()
            .project
            .edit_value_text(&location, typed.clone());
        {
            let mut rust = self.as_mut().rust_mut();
            match edit {
                // A half-written number like "-" or "1e" is kept as raw text so
                // the field does not snap back while it is being typed.
                blockstitch_core::editor::ValueEdit::Number { parsed: false } => {
                    rust.buffers.insert(location, typed);
                }
                _ => {
                    rust.buffers.remove(&location);
                }
            }
        }
        self.changed();
    }

    /// Lifts a value out of its slot - the first half of moving it. The canvas
    /// follows with `putValue` (into another slot) or `createValue` (onto open
    /// canvas), and that second call is what puts it back down; this one must
    /// not also park a copy somewhere, or every move would leave a duplicate.
    fn take_value(mut self: Pin<&mut Self>, location: &QString) {
        let Some(location) = parse::<ValueLocation>(location) else {
            self.rejected("The canvas sent a value slot this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        let taken = self.as_mut().rust_mut().project.take_value(&location);
        if taken.is_some() {
            let mut rust = self.as_mut().rust_mut();
            prune_value_buffers(&mut rust.buffers, &location);
            // The put that follows belongs to this same gesture; see `checkpoint`.
            rust.lifted = true;
            drop(rust);
            self.changed();
        } else {
            self.rejected("That value is not there any more.");
        }
    }

    /// Removes a value for good: a chip dropped on the delete zone. What it
    /// shadowed (or the slot's blank) takes its place, exactly as when lifting.
    fn delete_value(mut self: Pin<&mut Self>, location: &QString) {
        let Some(location) = parse::<ValueLocation>(location) else {
            self.rejected("The canvas sent a value slot this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        // Core's `take_value` also deletes a parked block addressed at its root,
        // so one call covers a chip in a slot and a block on open canvas.
        let taken = self.as_mut().rust_mut().project.take_value(&location);
        if taken.is_some() {
            prune_value_buffers(&mut self.as_mut().rust_mut().buffers, &location);
            self.changed();
        } else {
            self.rejected("That value is not there any more.");
        }
    }

    fn put_value(mut self: Pin<&mut Self>, location: &QString, value: &QString) {
        let (Some(location), Some(value)) =
            (parse::<ValueLocation>(location), parse::<Value>(value))
        else {
            self.rejected("The canvas sent a value this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        if self.as_mut().rust_mut().project.put_value(&location, value) {
            let mut rust = self.as_mut().rust_mut();
            prune_value_buffers(&mut rust.buffers, &location);
            drop(rust);
            self.changed();
        } else {
            self.rejected("That value cannot go there.");
        }
    }

    fn create_value(mut self: Pin<&mut Self>, x: i32, y: i32, value: &QString) {
        let Some(value) = parse::<Value>(value) else {
            self.rejected("The palette sent a value this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        self.as_mut()
            .rust_mut()
            .project
            .add_floating_value(x, y, value, None);
        self.changed();
    }

    fn move_floating_value(mut self: Pin<&mut Self>, floating_id: &QString, x: i32, y: i32) {
        let floating_id = text(floating_id);
        self.as_mut().checkpoint();
        if self
            .as_mut()
            .rust_mut()
            .project
            .move_floating_value(&floating_id, x, y)
        {
            self.changed();
        }
    }

    fn remove_floating_value(mut self: Pin<&mut Self>, floating_id: &QString) {
        let floating_id = text(floating_id);
        self.as_mut().checkpoint();
        if self
            .as_mut()
            .rust_mut()
            .project
            .remove_floating_value(&floating_id)
        {
            self.changed();
        }
    }

    // ── Notes ─────────────────────────────────────────────────────────────

    fn add_comment(mut self: Pin<&mut Self>, x: i32, y: i32, attached_to: &QString) {
        let attached = text(attached_to);
        let attached = (!attached.is_empty()).then_some(attached);
        self.as_mut().checkpoint();
        self.as_mut()
            .rust_mut()
            .project
            .add_comment(x, y, String::new(), attached);
        self.changed();
    }

    fn move_comment(mut self: Pin<&mut Self>, comment_id: &QString, x: i32, y: i32) {
        let comment_id = text(comment_id);
        self.as_mut().checkpoint();
        if self
            .as_mut()
            .rust_mut()
            .project
            .move_comment(&comment_id, x, y)
        {
            self.changed();
        }
    }

    fn edit_comment(mut self: Pin<&mut Self>, comment_id: &QString, text_value: &QString) {
        let comment_id = text(comment_id);
        let body = text(text_value);
        self.as_mut().checkpoint_session(EditSession::Comment {
            comment_id: comment_id.clone(),
        });
        if self
            .as_mut()
            .rust_mut()
            .project
            .set_comment_text(&comment_id, body)
        {
            self.changed();
        }
    }

    fn set_comment_collapsed(mut self: Pin<&mut Self>, comment_id: &QString, collapsed: bool) {
        let comment_id = text(comment_id);
        self.as_mut().checkpoint();
        if self
            .as_mut()
            .rust_mut()
            .project
            .set_comment_collapsed(&comment_id, collapsed)
        {
            self.changed();
        }
    }

    fn remove_comment(mut self: Pin<&mut Self>, comment_id: &QString) {
        let comment_id = text(comment_id);
        self.as_mut().checkpoint();
        if self
            .as_mut()
            .rust_mut()
            .project
            .remove_comment(&comment_id)
        {
            self.changed();
        }
    }

    // ── Variables, lists, dicts ───────────────────────────────────────────

    fn create_variable(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = text(name);
        self.as_mut().checkpoint();
        match self.as_mut().rust_mut().project.create_variable(&name) {
            Ok(_) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn rename_variable(mut self: Pin<&mut Self>, old: &QString, new_name: &QString) -> bool {
        let (old, new_name) = (text(old), text(new_name));
        self.as_mut().checkpoint();
        match self
            .as_mut()
            .rust_mut()
            .project
            .rename_variable(&old, &new_name)
        {
            Ok(_) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn delete_variable(mut self: Pin<&mut Self>, name: &QString) {
        let name = text(name);
        self.as_mut().checkpoint();
        self.as_mut().rust_mut().project.remove_variable(&name);
        self.changed();
    }

    fn create_list(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = text(name);
        self.as_mut().checkpoint();
        match self.as_mut().rust_mut().project.create_list(&name) {
            Ok(_) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn rename_list(mut self: Pin<&mut Self>, old: &QString, new_name: &QString) -> bool {
        let (old, new_name) = (text(old), text(new_name));
        self.as_mut().checkpoint();
        match self.as_mut().rust_mut().project.rename_list(&old, &new_name) {
            Ok(_) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn delete_list(mut self: Pin<&mut Self>, name: &QString) {
        let name = text(name);
        self.as_mut().checkpoint();
        self.as_mut().rust_mut().project.remove_list(&name);
        self.changed();
    }

    fn set_list_items(mut self: Pin<&mut Self>, name: &QString, items: &QString) {
        let (name, Some(items)) = (text(name), parse::<Vec<ListItem>>(items)) else {
            self.rejected("The list editor sent items this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        match self.as_mut().rust_mut().project.set_list_items(&name, items) {
            Ok(()) => self.changed(),
            Err(error) => self.rejected(error),
        }
    }

    fn set_list_editor_state(
        mut self: Pin<&mut Self>,
        name: &QString,
        visible: bool,
        x: i32,
        y: i32,
    ) {
        let name = text(name);
        // Opening a monitor is not an edit worth undoing, but it is worth saving.
        if self
            .as_mut()
            .rust_mut()
            .project
            .set_list_editor_state(&name, visible, x, y)
            .is_ok()
        {
            self.changed();
        }
    }

    fn create_dict(mut self: Pin<&mut Self>, name: &QString) -> bool {
        let name = text(name);
        self.as_mut().checkpoint();
        match self.as_mut().rust_mut().project.create_dict(&name) {
            Ok(_) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn rename_dict(mut self: Pin<&mut Self>, old: &QString, new_name: &QString) -> bool {
        let (old, new_name) = (text(old), text(new_name));
        self.as_mut().checkpoint();
        match self.as_mut().rust_mut().project.rename_dict(&old, &new_name) {
            Ok(_) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn delete_dict(mut self: Pin<&mut Self>, name: &QString) {
        let name = text(name);
        self.as_mut().checkpoint();
        self.as_mut().rust_mut().project.remove_dict(&name);
        self.changed();
    }

    fn set_dict_entries(mut self: Pin<&mut Self>, name: &QString, entries: &QString) {
        let (name, Some(entries)) = (text(name), parse::<Vec<DictEntry>>(entries)) else {
            self.rejected("The dict editor sent entries this build does not understand.");
            return;
        };
        self.as_mut().checkpoint();
        match self
            .as_mut()
            .rust_mut()
            .project
            .set_dict_entries(&name, entries)
        {
            Ok(()) => self.changed(),
            Err(error) => self.rejected(error),
        }
    }

    fn set_dict_editor_state(
        mut self: Pin<&mut Self>,
        name: &QString,
        visible: bool,
        x: i32,
        y: i32,
    ) {
        let name = text(name);
        if self
            .as_mut()
            .rust_mut()
            .project
            .set_dict_editor_state(&name, visible, x, y)
            .is_ok()
        {
            self.changed();
        }
    }

    // ── Custom blocks ─────────────────────────────────────────────────────

    fn make_block(
        mut self: Pin<&mut Self>,
        pieces: &QString,
        shape: &QString,
        color: &QString,
        x: i32,
        y: i32,
    ) -> bool {
        let Some(pieces) = parse::<Vec<BlockPiece>>(pieces) else {
            self.rejected("The Make a Block dialog sent something this build does not understand.");
            return false;
        };
        if let Err(error) = blockstitch_core::graph::BlockDef::validate_pieces(&pieces) {
            self.rejected(error);
            return false;
        }
        let shape = shape_of(shape);
        let color = normalize_block_color(&text(color))
            .unwrap_or_else(blockstitch_core::graph::default_block_color);
        self.as_mut().checkpoint();
        self.as_mut()
            .rust_mut()
            .project
            .create_block(pieces, shape, color, x, y, |block_id| McBlock::BlockHeader {
                block_id: block_id.to_string(),
            });
        self.changed();
        true
    }

    fn update_block(
        mut self: Pin<&mut Self>,
        block_id: &QString,
        pieces: &QString,
        shape: &QString,
        color: &QString,
    ) -> bool {
        let (block_id, Some(pieces)) = (text(block_id), parse::<Vec<BlockPiece>>(pieces)) else {
            self.rejected("The Make a Block dialog sent something this build does not understand.");
            return false;
        };
        let shape = shape_of(shape);
        let color = text(color);
        self.as_mut().checkpoint();
        let result = self
            .as_mut()
            .rust_mut()
            .project
            .update_block(&block_id, pieces, shape, &color);
        match result {
            Ok(()) => {
                self.changed();
                true
            }
            Err(error) => {
                self.rejected(error);
                false
            }
        }
    }

    fn remove_block(mut self: Pin<&mut Self>, block_id: &QString) {
        let block_id = text(block_id);
        self.as_mut().checkpoint();
        self.as_mut().rust_mut().project.remove_block(&block_id);
        self.changed();
    }
}

/// The wire name of a [`BlockShape`], as the Make a Block dialog sends it.
fn shape_of(shape: &QString) -> BlockShape {
    match shape.to_string().as_str() {
        "Ending" => BlockShape::Ending,
        "ReturnsValue" => BlockShape::ReturnsValue,
        "ReturnsBool" => BlockShape::ReturnsBool,
        _ => BlockShape::Normal,
    }
}

impl BackendRust {
    /// The dynamic dropdown choices the rows refer to by name - see
    /// [`catalog::DYNAMIC_SOURCES`].
    fn choices(&self) -> String {
        Self::choices_of(&self.project)
    }

    fn choices_of(project: &ModProject) -> String {
        let declarations = validate::Declarations::gather(project);
        serde_json::json!({
            "declaredBlocks": declarations.blocks,
            "declaredItems": declarations.items,
            "declaredEntities": declarations.entities,
            "declaredSounds": declarations.sounds,
            "declaredTabs": declarations.tabs,
            "variables": project
                .variables
                .iter()
                .map(|variable| variable.name.clone())
                .collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// The document's diagnostics, as `[{severity, message, strandId}]`.
    fn diagnostics(&self) -> String {
        Self::diagnostics_of(&self.project)
    }

    /// The document.s diagnostics, as `[{severity, message, strandId}]`.
    fn diagnostics_of(project: &ModProject) -> String {
        let report = validate::check(project);
        serde_json::json!(
            report
                .diagnostics
                .iter()
                .map(|diagnostic| serde_json::json!({
                    "severity": match diagnostic.severity {
                        validate::Severity::Error => "error",
                        validate::Severity::Warning => "warning",
                    },
                    "message": diagnostic.message,
                    "strandId": diagnostic.strand_id,
                }))
                .collect::<Vec<_>>()
        )
        .to_string()
    }

    /// Drops the text buffers belonging to a strand that has gone away.
    fn prune_strand(&mut self, strand_id: &str) {
        drop_strand_buffers(&mut self.buffers, strand_id);
    }
}
