//! The Stitchcraft editor.
//!
//! A Scratch-style canvas whose blocks describe a Minecraft mod, and which
//! exports that mod as Java for Fabric and NeoForge.
//!
//! Three pieces, in three crates:
//!
//! * `blockstitch-core` and `blockstitch-qml` (from GitHub) are the block editor
//!   itself - the document model, the editing operations, and the Qt canvas;
//! * `stitchcraft-blocks` is the vocabulary those operate on: what a block can
//!   say about a mod;
//! * `stitchcraft-export` compiles a canvas into a mod source tree.
//!
//! This binary is the shell around them: it starts Qt, registers both QML
//! modules, and hands the canvas a [`bridge::qobject::Backend`].

// A release build on Windows is a GUI program: launched from a shortcut it must not
// bring a console window up behind the editor. Debug builds keep the console, so a
// `cargo run` still shows Qt's warnings and panics.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod bridge;
mod edit;
mod wire;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if !app.is_null() {
        app.pin_mut()
            .set_application_name(&QString::from("Stitchcraft"));
        app.pin_mut()
            .set_application_display_name(&QString::from("Stitchcraft"));
        app.pin_mut()
            .set_organization_name(&QString::from("Blockworked"));
        app.pin_mut()
            .set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    }

    // Both QML modules are statically linked into this binary, so their
    // resources have to be initialized before anything imports them.
    blockstitch_qml::init();
    cxx_qt::init_qml_module!("com.blockworked.Stitchcraft");

    if !engine.is_null() {
        // The resource alias keeps the path the file has in the crate, so the
        // module prefix is followed by `qml/Stitchcraft/` and not just the file
        // name. Everything else resolves by type name through the module's
        // `qmldir`; only this one entry point is addressed by URL.
        engine.pin_mut().load(&QUrl::from(
            "qrc:/qt/qml/com/blockworked/Stitchcraft/qml/Stitchcraft/Main.qml",
        ));
    }

    if !app.is_null() {
        app.pin_mut().exec();
    }
}
