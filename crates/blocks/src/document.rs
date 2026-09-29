//! The saved document: the mod's identity and build targets, plus the canvas
//! itself.
//!
//! [`ModProject`] is the host document blockstitch-core asks for - a type that
//! owns a [`BlockGraph`] rather than re-declaring its collections.
//! `#[serde(flatten)]` keeps the save file flat, and [`Deref`] keeps
//! `project.strands` reading the way it would if the graph were inlined.

use crate::block::McBlock;
use blockstitch_core::graph::BlockGraph;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

/// The dependency versions the generated Gradle build pins. The defaults track
/// FrozenLib 3.0-mc26.3, which is what `stitchcraft-export` was written
/// against; a document saved against older ones keeps its own.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildTargets {
    pub minecraft_version: String,
    pub java_version: u32,
    pub frozenlib_version: String,
    pub fabric_loader_version: String,
    pub fabric_api_version: String,
    pub neoforge_version: String,
}

impl Default for BuildTargets {
    fn default() -> Self {
        Self {
            minecraft_version: "26.3".to_string(),
            java_version: 25,
            frozenlib_version: "3.0-mc26.3".to_string(),
            fabric_loader_version: "0.19.5".to_string(),
            fabric_api_version: "0.161.0+26.3".to_string(),
            neoforge_version: "26.3.0.7-beta".to_string(),
        }
    }
}

/// Which loaders the export emits a subproject for. At least one has to be on;
/// [`crate::validate`] rejects a project with neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Loaders {
    pub fabric: bool,
    pub neoforge: bool,
}

impl Default for Loaders {
    fn default() -> Self {
        Self {
            fabric: true,
            neoforge: true,
        }
    }
}

/// One Stitchcraft document: a mod's identity, its build targets, and the
/// canvas that becomes its code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModProject {
    /// The mod id, and the namespace every registered `Identifier` gets.
    pub mod_id: String,
    /// Human-readable name, for `fabric.mod.json` and the creative tab.
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default = "default_license")]
    pub license: String,
    /// Root Java package for the generated sources.
    pub package: String,
    #[serde(default)]
    pub targets: BuildTargets,
    #[serde(default)]
    pub loaders: Loaders,
    #[serde(flatten)]
    pub graph: BlockGraph<McBlock>,
}

fn default_license() -> String {
    "MIT".to_string()
}

impl Default for ModProject {
    fn default() -> Self {
        Self::new("example_mod")
    }
}

impl ModProject {
    /// A fresh, empty project for `mod_id`, with a package and display name
    /// derived from it.
    pub fn new(mod_id: &str) -> Self {
        let mod_id = sanitize_mod_id(mod_id);
        Self {
            name: title_case(&mod_id),
            package: format!("com.example.{}", mod_id.replace('-', "_")),
            mod_id,
            version: "1.0.0".to_string(),
            description: String::new(),
            authors: Vec::new(),
            license: default_license(),
            targets: BuildTargets::default(),
            loaders: Loaders::default(),
            graph: BlockGraph::new(),
        }
    }

    /// `Mod` class name - the mod id in upper camel case, which is what the
    /// generated entrypoint classes are named after.
    pub fn class_prefix(&self) -> String {
        title_case(&self.mod_id).replace(' ', "")
    }

    /// One-time repairs after loading a save: legacy boolean slots and any
    /// hand-edited block color.
    pub fn migrate_after_load(&mut self) {
        self.graph.migrate_bool_slots();
        self.graph.normalize_block_colors();
    }
}

/// Keeps `project.strands`, `project.variables` and friends reading as though
/// the graph's collections were the project's own fields.
impl Deref for ModProject {
    type Target = BlockGraph<McBlock>;

    fn deref(&self) -> &Self::Target {
        &self.graph
    }
}

impl DerefMut for ModProject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.graph
    }
}

/// Reduces `input` to the characters a mod id may hold: lowercase letters,
/// digits and underscores, never leading with a digit.
pub fn sanitize_mod_id(input: &str) -> String {
    let mut out: String = input
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return "example_mod".to_string();
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'm');
    }
    out
}

/// `wonder_blocks` -> `Wonder Blocks`, for display names the user has not set.
pub fn title_case(id: &str) -> String {
    id.split(['_', '-', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::McBlock;
    use blockstitch_core::graph::Instruction;

    #[test]
    fn mod_ids_are_reduced_to_legal_characters() {
        assert_eq!(sanitize_mod_id("Wonder Blocks!"), "wonder_blocks");
        assert_eq!(sanitize_mod_id("  "), "example_mod");
        assert_eq!(sanitize_mod_id("3d_stuff"), "m3d_stuff");
    }

    #[test]
    fn a_new_project_derives_its_name_and_package() {
        let project = ModProject::new("Wonder Blocks");
        assert_eq!(project.mod_id, "wonder_blocks");
        assert_eq!(project.name, "Wonder Blocks");
        assert_eq!(project.package, "com.example.wonder_blocks");
        assert_eq!(project.class_prefix(), "WonderBlocks");
    }

    #[test]
    fn the_graph_is_flattened_into_the_save_file() {
        let mut project = ModProject::new("demo");
        project
            .graph
            .add_strand(0, 0, vec![Instruction::new(McBlock::OnServerTick)]);
        let json = serde_json::to_value(&project).unwrap();
        assert_eq!(json["mod_id"], "demo");
        assert_eq!(
            json["strands"][0]["instructions"][0]["kind"]["type"],
            "OnServerTick",
            "strands sit beside the metadata, not under a `graph` key"
        );

        let round_tripped: ModProject = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, project);
    }

    #[test]
    fn deref_reaches_the_graphs_collections() {
        let mut project = ModProject::new("demo");
        project.create_variable("score").unwrap();
        assert_eq!(project.variables.len(), 1);
    }
}
