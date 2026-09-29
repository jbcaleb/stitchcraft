//! The Gradle build for the exported mod.
//!
//! The layout is the plainest multiloader arrangement that needs no third-party
//! plugin: `common/` is a source directory rather than a subproject, and each
//! loader adds it to its own source sets. That means the shared code is compiled
//! twice, once against Fabric's Minecraft and once against NeoForge's, which is
//! exactly what makes it work without something like Architectury standing in
//! the middle and without having to compile `common` against a stripped vanilla
//! jar.
//!
//! Plugin and dependency versions live in `gradle.properties` so they can be
//! bumped without touching a build script. The generated `README.md` says which
//! ones to confirm before the first build, because Minecraft's ecosystem moves
//! faster than any generator can track.

use crate::naming::Names;
use stitchcraft_blocks::ModProject;

/// Every build file. Only the loaders the project asked for get a subproject.
pub fn files(project: &ModProject, names: &Names) -> Vec<(String, String)> {
    let mut out = vec![
        ("settings.gradle.kts".to_string(), settings(project)),
        ("build.gradle.kts".to_string(), root_build(project)),
        (
            "gradle.properties".to_string(),
            properties(project, names),
        ),
        (".gitignore".to_string(), gitignore()),
    ];
    if project.loaders.fabric {
        out.push(("fabric/build.gradle.kts".to_string(), fabric_build()));
    }
    if project.loaders.neoforge {
        out.push(("neoforge/build.gradle.kts".to_string(), neoforge_build()));
    }
    out
}

fn settings(project: &ModProject) -> String {
    let mut out = String::from(
        "pluginManagement {\n    \
         repositories {\n        \
         maven(\"https://maven.fabricmc.net/\") { name = \"Fabric\" }\n        \
         maven(\"https://maven.neoforged.net/releases\") { name = \"NeoForged\" }\n        \
         gradlePluginPortal()\n    \
         }\n\
         }\n\n",
    );
    out.push_str("plugins {\n    id(\"org.gradle.toolchains.foojay-resolver-convention\") version \"1.0.0\"\n}\n\n");
    out.push_str(&format!(
        "rootProject.name = \"{}\"\n\n",
        project.mod_id
    ));
    out.push_str(
        "// `common/` is a source directory shared by both loaders rather than a\n\
         // subproject of its own - see each loader's build script.\n",
    );
    if project.loaders.fabric {
        out.push_str("include(\"fabric\")\n");
    }
    if project.loaders.neoforge {
        out.push_str("include(\"neoforge\")\n");
    }
    out
}

fn root_build(project: &ModProject) -> String {
    format!(
        "// Settings every subproject shares. The loader-specific plugins are\n\
         // applied in the subprojects themselves, because Loom and ModDevGradle\n\
         // cannot both be on one project.\n\
         plugins {{\n    \
         id(\"java\")\n\
         }}\n\n\
         val modVersion: String by project\n\n\
         allprojects {{\n    \
         apply(plugin = \"java\")\n\n    \
         group = \"{}\"\n    \
         version = modVersion\n\n    \
         repositories {{\n        \
         mavenCentral()\n        \
         maven(\"https://maven.frozenblock.net/release\") {{ name = \"FrozenBlock\" }}\n        \
         maven(\"https://maven.frozenblock.net/snapshot\") {{ name = \"FrozenBlock Snapshot\" }}\n        \
         // FrozenLib is published to Modrinth's maven as well, which is the\n        \
         // coordinate the loader builds below use by default.\n        \
         exclusiveContent {{\n            \
         forRepository {{ maven(\"https://api.modrinth.com/maven\") {{ name = \"Modrinth\" }} }}\n            \
         filter {{ includeGroup(\"maven.modrinth\") }}\n        \
         }}\n    \
         }}\n\n    \
         java {{\n        \
         val javaVersion = JavaVersion.toVersion((project.property(\"javaVersion\") as String).toInt())\n        \
         toolchain {{ languageVersion.set(JavaLanguageVersion.of(javaVersion.majorVersion)) }}\n        \
         sourceCompatibility = javaVersion\n        \
         targetCompatibility = javaVersion\n        \
         withSourcesJar()\n    \
         }}\n\n    \
         tasks.withType<JavaCompile>().configureEach {{\n        \
         options.encoding = \"UTF-8\"\n        \
         options.release.set((project.property(\"javaVersion\") as String).toInt())\n    \
         }}\n\n    \
         tasks.withType<ProcessResources>().configureEach {{\n        \
         // Both loaders' metadata files are written out complete, so nothing\n        \
         // needs expanding here - this only keeps the task reproducible.\n        \
         duplicatesStrategy = DuplicatesStrategy.INCLUDE\n    \
         }}\n\
         }}\n",
        project.package
    )
}

fn properties(project: &ModProject, names: &Names) -> String {
    let targets = &project.targets;
    format!(
        "org.gradle.jvmargs=-Xmx3G\n\
         org.gradle.parallel=true\n\n\
         # Identity\n\
         modId={}\n\
         modName={}\n\
         modVersion={}\n\
         modPackage={}\n\
         javaVersion={}\n\n\
         # Game and loaders\n\
         minecraftVersion={}\n\
         fabricLoaderVersion={}\n\
         fabricApiVersion={}\n\
         neoforgeVersion={}\n\n\
         # FrozenLib, which the generated registry and event code is built on.\n\
         frozenlibVersion={}\n\n\
         # Build plugins. These track the game version rather than the mod, and\n\
         # are the first thing to check if the build fails before compiling.\n\
         loomVersion=1.12-SNAPSHOT\n\
         modDevVersion=2.0.+\n",
        project.mod_id,
        project.name,
        project.version,
        names.package,
        targets.java_version,
        targets.minecraft_version,
        targets.fabric_loader_version,
        targets.fabric_api_version,
        targets.neoforge_version,
        targets.frozenlib_version,
    )
}

fn fabric_build() -> String {
    String::from(
        "// The Fabric build. `common/` is pulled in as extra source rather than a\n\
         // project dependency, so it compiles against Fabric's Minecraft directly.\n\
         plugins {\n    \
         id(\"fabric-loom\") version providers.gradleProperty(\"loomVersion\").get()\n\
         }\n\n\
         val minecraftVersion: String by project\n\
         val fabricLoaderVersion: String by project\n\
         val fabricApiVersion: String by project\n\
         val frozenlibVersion: String by project\n\
         val modId: String by project\n\n\
         base { archivesName.set(\"$modId-fabric\") }\n\n\
         sourceSets {\n    \
         main {\n        \
         java.srcDir(rootProject.file(\"common/src/main/java\"))\n        \
         resources.srcDir(rootProject.file(\"common/src/main/resources\"))\n    \
         }\n\
         }\n\n\
         dependencies {\n    \
         minecraft(\"com.mojang:minecraft:$minecraftVersion\")\n    \
         mappings(loom.officialMojangMappings())\n    \
         modImplementation(\"net.fabricmc:fabric-loader:$fabricLoaderVersion\")\n    \
         modImplementation(\"net.fabricmc.fabric-api:fabric-api:$fabricApiVersion\")\n\n    \
         // FrozenLib supplies the deferred registries and the cross-platform\n    \
         // events the generated code is written against.\n    \
         modImplementation(\"maven.modrinth:frozenlib:$frozenlibVersion\")\n\
         }\n",
    )
}

fn neoforge_build() -> String {
    String::from(
        "// The NeoForge build, using NeoForge's own ModDevGradle. As on the Fabric\n\
         // side, `common/` is extra source rather than a project dependency.\n\
         plugins {\n    \
         id(\"net.neoforged.moddev\") version providers.gradleProperty(\"modDevVersion\").get()\n\
         }\n\n\
         val neoforgeVersion: String by project\n\
         val frozenlibVersion: String by project\n\
         val modId: String by project\n\n\
         base { archivesName.set(\"$modId-neoforge\") }\n\n\
         neoForge {\n    \
         version = neoforgeVersion\n\n    \
         runs {\n        \
         create(\"client\") { client() }\n        \
         create(\"server\") { server() }\n    \
         }\n\n    \
         mods {\n        \
         create(modId) { sourceSet(sourceSets.main.get()) }\n    \
         }\n\
         }\n\n\
         sourceSets {\n    \
         main {\n        \
         java.srcDir(rootProject.file(\"common/src/main/java\"))\n        \
         resources.srcDir(rootProject.file(\"common/src/main/resources\"))\n    \
         }\n\
         }\n\n\
         dependencies {\n    \
         implementation(\"maven.modrinth:frozenlib:$frozenlibVersion\")\n\
         }\n",
    )
}

fn gitignore() -> String {
    String::from(
        "# Gradle\n\
         .gradle/\n\
         build/\n\
         */build/\n\n\
         # Development runs\n\
         run/\n\
         */run/\n\n\
         # IDEs\n\
         .idea/\n\
         *.iml\n\
         .vscode/\n\
         .eclipse/\n\
         bin/\n",
    )
}

/// The `README.md` the export drops next to the build, saying what has to be
/// confirmed or filled in before the mod will build and run.
pub fn readme(project: &ModProject, names: &Names, missing_art: &[String]) -> String {
    let mut out = format!(
        "# {}\n\n\
         Generated by Stitchcraft from a block canvas. Everything under \
         `common/`, `fabric/` and `neoforge/` is written fresh on each export, so \
         edit the canvas rather than the code unless you are ready to stop \
         exporting over it.\n\n\
         ## Layout\n\n\
         | Path | What's in it |\n\
         | --- | --- |\n\
         | `common/src/main/java/{pkg}/registry/` | The `DeferredRegister` classes - blocks, items, entities, sounds, creative tabs |\n\
         | `common/src/main/java/{pkg}/content/` | The block, item and mob classes those registries register |\n\
         | `common/src/main/java/{pkg}/script/` | The canvas itself: `Handlers` (one method per stack), `Hooks` (event wiring), `Rt`, `Vars`, `Scheduler` |\n\
         | `fabric/`, `neoforge/` | Entrypoints, metadata, and the per-loader command bridge |\n\n\
         `common/` is a shared source directory rather than a Gradle subproject: \
         each loader adds it to its own source sets, so it is compiled twice, \
         once against each loader's Minecraft. That is what lets the build work \
         without Architectury or a vanilla-only Minecraft jar.\n\n\
         `Rt.java` is the only generated file that calls Minecraft's own API. If \
         a game update renames something, that is the file to fix - not the \
         hundreds of call sites the canvas produced.\n\n\
         ## Before the first build\n\n\
         Stitchcraft pins what it can and guesses what it cannot. Check these in \
         `gradle.properties`:\n\n\
         - `loomVersion` and `modDevVersion` - build plugins, which track the \
         game version rather than the mod. A build that fails before compiling \
         anything is almost always one of these.\n\
         - `frozenlibVersion` - the generated code depends on FrozenLib through \
         Modrinth's maven (`maven.modrinth:frozenlib`). The version there is the \
         *file* version and differs per loader; the FrozenBlock maven is also \
         configured if you would rather use `net.frozenblock` coordinates.\n\
         - `minecraftVersion`, `fabricApiVersion`, `neoforgeVersion` - set from \
         the canvas's build targets ({mc}).\n\n\
         There is no Gradle wrapper in the export. Run `gradle wrapper` once, or \
         build with a Gradle you already have.\n\n",
        project.name,
        pkg = names.package_dir(),
        mc = project.targets.minecraft_version,
    );

    if !missing_art.is_empty() {
        out.push_str(
            "## Art and audio\n\n\
             A canvas describes behaviour, not artwork, so the models and sound \
             index point at files that are not there yet. Add these and the \
             declarations will look and sound like themselves:\n\n",
        );
        for path in missing_art {
            out.push_str(&format!("- `{path}`\n"));
        }
        out.push('\n');
    }

    if !crate::content::declared_bases(project).is_empty() {
        out.push_str(
            "Declared entities are registered with a `NoopRenderer`, which keeps \
             the client from crashing but leaves them invisible. Replace those in \
             `content/ModRenderers.java` with a real `EntityRenderer` when you \
             want to see them.\n\n",
        );
    }

    out.push_str(&format!(
        "## Saved state\n\n\
         The canvas's variables, lists and dicts are declared at mod init and \
         saved to `<game dir>/stitchcraft/{}.json` when the server stops. That is \
         server-wide rather than per world, because a canvas has no notion of \
         which save it belongs to.\n",
        project.mod_id
    ));
    out
}
