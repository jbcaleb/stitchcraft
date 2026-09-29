//! Stitchcraft's block vocabulary: what a canvas can say about a Minecraft mod.
//!
//! blockstitch-core supplies the document model, the value system and every
//! editing operation; this crate supplies the one thing it asks a host for - an
//! instruction enum ([`McBlock`]) implementing
//! [`blockstitch_core::graph::BlockKind`] - plus the three things built on it:
//!
//! * [`document::ModProject`], the saved document: a mod's identity and build
//!   targets around a [`blockstitch_core::graph::BlockGraph`];
//! * [`catalog`], how each block draws and what a fresh one holds, which is
//!   also what the QML canvas is handed at startup;
//! * [`reporters`], the Minecraft-specific reporter blocks, and [`validate`],
//!   which says what will not survive being exported.
//!
//! Nothing here runs a document. `stitchcraft-export` compiles one to Java for
//! Fabric and NeoForge, and [`context`] is the table both crates read to know
//! what a hook can tell its body.
//!
//! ```
//! use stitchcraft_blocks::{McBlock, ModProject, catalog, reporters, validate};
//! use blockstitch_core::graph::Instruction;
//!
//! reporters::register();
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
//! assert!(!validate::check(&project).has_errors());
//! ```

pub mod block;
pub mod catalog;
pub mod context;
pub mod document;
pub mod kinds;
pub mod reporters;
pub mod validate;

pub use block::{Body, McBlock};
pub use context::{ContextField, HookContext};
pub use document::{BuildTargets, Loaders, ModProject};
pub use kinds::{EntityBase, MaterialPreset, MessageKind, MobCategoryKind, Rarity, Target, Weather};
pub use validate::{Declarations, Diagnostic, Report, Severity};

/// The QML module the app's own components are registered under. The canvas
/// components come from blockstitch's `com.blockworked.Blockstitch` module.
pub const QML_URI: &str = "com.blockworked.Stitchcraft";
