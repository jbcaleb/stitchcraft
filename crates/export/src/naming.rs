//! Turning a document's names into Java names, and the file layout they land in.
//!
//! Everything that decides "what is this called in the output" lives here, so
//! the emitters never build a path or an identifier by hand.

use stitchcraft_blocks::ModProject;

/// The names and paths one export uses throughout.
#[derive(Debug, Clone)]
pub struct Names {
    pub mod_id: String,
    /// Root Java package, e.g. `com.example.wonder_blocks`.
    pub package: String,
    /// The common mod class, e.g. `WonderBlocks`.
    pub main_class: String,
}

impl Names {
    pub fn of(project: &ModProject) -> Self {
        Self {
            mod_id: project.mod_id.clone(),
            package: project.package.clone(),
            main_class: project.class_prefix(),
        }
    }

    /// The package's directory form, `com/example/wonder_blocks`.
    pub fn package_dir(&self) -> String {
        self.package.replace('.', "/")
    }

    /// A source path in the shared `common` subproject. `relative` is the path
    /// below the root package, e.g. `registry/ModBlocks.java`.
    pub fn common(&self, relative: &str) -> String {
        format!(
            "common/src/main/java/{}/{relative}",
            self.package_dir()
        )
    }

    /// A source path in the Fabric subproject, under the `fabric` subpackage.
    pub fn fabric(&self, relative: &str) -> String {
        format!(
            "fabric/src/main/java/{}/fabric/{relative}",
            self.package_dir()
        )
    }

    /// A source path in the NeoForge subproject.
    pub fn neoforge(&self, relative: &str) -> String {
        format!(
            "neoforge/src/main/java/{}/neoforge/{relative}",
            self.package_dir()
        )
    }

    /// A resource path under `common/src/main/resources`.
    pub fn common_resource(&self, relative: &str) -> String {
        format!("common/src/main/resources/{relative}")
    }

    /// An asset path in the mod's own namespace.
    pub fn asset(&self, relative: &str) -> String {
        self.common_resource(&format!("assets/{}/{relative}", self.mod_id))
    }

    /// A data path in the mod's own namespace.
    pub fn data(&self, relative: &str) -> String {
        self.common_resource(&format!("data/{}/{relative}", self.mod_id))
    }

    pub fn registry_package(&self) -> String {
        format!("{}.registry", self.package)
    }

    pub fn content_package(&self) -> String {
        format!("{}.content", self.package)
    }

    pub fn script_package(&self) -> String {
        format!("{}.script", self.package)
    }
}

/// A legal Java identifier from arbitrary text, for the parts of a generated
/// name that come from the document. Never empty, never starting with a digit.
pub fn identifier(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut fresh_word = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(if fresh_word {
                c.to_ascii_uppercase()
            } else {
                c
            });
            fresh_word = false;
        } else {
            // A run of punctuation collapses into one camel-case boundary.
            fresh_word = !out.is_empty();
        }
    }
    if out.is_empty() {
        return "unnamed".to_string();
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// `wonder_thing` -> `WonderThing`, for a generated class name.
pub fn class_name(raw: &str) -> String {
    let name = identifier(raw);
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => name,
    }
}

/// `wonder thing` -> `WONDER_THING`, for a generated constant.
pub fn constant_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 4);
    let mut previous_was_lower = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            // A lower-to-upper transition is a word boundary in camel case,
            // which is how a display name reaches this function.
            if previous_was_lower && c.is_ascii_uppercase() {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
            previous_was_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
            previous_was_lower = false;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        return "UNNAMED".to_string();
    }
    if trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{trimmed}")
    } else {
        trimmed
    }
}

/// `wonder_thing` -> `Wonder Thing`, for a translation entry the user has not
/// filled in.
pub fn display_fallback(id: &str) -> String {
    stitchcraft_blocks::document::title_case(id)
}

/// Namespaces a bare id. Anything already carrying a `:` is left alone, so a
/// field can hold `minecraft:stone` as readily as one of the mod's own.
pub fn qualify(mod_id: &str, id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_string()
    } else {
        format!("{mod_id}:{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_survive_arbitrary_text() {
        assert_eq!(identifier("chime block"), "chimeBlock");
        assert_eq!(identifier("chime--block!"), "chimeBlock");
        assert_eq!(identifier("2fast"), "_2fast");
        assert_eq!(identifier("!!!"), "unnamed");
    }

    #[test]
    fn class_and_constant_names_follow_java_convention() {
        assert_eq!(class_name("chime_block"), "ChimeBlock");
        assert_eq!(constant_name("chime_block"), "CHIME_BLOCK");
        assert_eq!(constant_name("chimeBlock"), "CHIME_BLOCK");
        assert_eq!(constant_name("chime block!"), "CHIME_BLOCK");
        assert_eq!(constant_name("9lives"), "_9LIVES");
        assert_eq!(constant_name(""), "UNNAMED");
    }

    #[test]
    fn bare_ids_get_the_mods_namespace_and_qualified_ones_do_not() {
        assert_eq!(qualify("demo", "chime"), "demo:chime");
        assert_eq!(qualify("demo", "minecraft:stone"), "minecraft:stone");
    }

    #[test]
    fn paths_land_under_the_package() {
        let names = Names {
            mod_id: "demo".into(),
            package: "com.example.demo".into(),
            main_class: "Demo".into(),
        };
        assert_eq!(
            names.common("registry/ModBlocks.java"),
            "common/src/main/java/com/example/demo/registry/ModBlocks.java"
        );
        assert_eq!(
            names.asset("lang/en_us.json"),
            "common/src/main/resources/assets/demo/lang/en_us.json"
        );
        assert_eq!(
            names.neoforge("DemoNeoForge.java"),
            "neoforge/src/main/java/com/example/demo/neoforge/DemoNeoForge.java"
        );
    }
}
