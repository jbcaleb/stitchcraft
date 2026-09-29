//! `stitchcraft-export` - compiles a saved canvas into a mod source tree.
//!
//! The app does this in-process; this is the same thing from a shell, for a
//! build script or a quick check without opening the editor.
//!
//! ```text
//! stitchcraft-export <project.stitch> <output-directory>
//! ```

use std::process::ExitCode;
use stitchcraft_blocks::{ModProject, reporters};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(document), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: stitchcraft-export <project.stitch> <output-directory>");
        return ExitCode::from(2);
    };

    // The editor's reporters have to be registered before a saved document is
    // read, or an operator's arity is not known and its arguments look wrong.
    reporters::register();

    let text = match std::fs::read_to_string(&document) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("could not read {document}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut project: ModProject = match serde_json::from_str(&text) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("{document} is not a Stitchcraft project: {error}");
            return ExitCode::FAILURE;
        }
    };
    project.migrate_after_load();

    let bundle = match stitchcraft_export::export(&project) {
        Ok(bundle) => bundle,
        Err(report) => {
            eprintln!("{} cannot be exported yet:", project.name);
            for diagnostic in report.errors() {
                eprintln!("  error: {}", diagnostic.message);
            }
            return ExitCode::FAILURE;
        }
    };

    for warning in &bundle.warnings {
        eprintln!("  warning: {warning}");
    }
    // Files left over from a previous export are pointed out rather than
    // deleted: removing things under a directory the user named is their call.
    for path in bundle.stale_files(&output) {
        eprintln!("  stale: {path} is left over from an earlier export");
    }
    if let Err(error) = bundle.write_to(&output) {
        eprintln!("could not write to {output}: {error}");
        return ExitCode::FAILURE;
    }
    println!(
        "{} file(s), {} bytes written to {output}",
        bundle.files.len(),
        bundle.byte_count()
    );
    ExitCode::SUCCESS
}
