//! Registers the app's own QML module and runs the cxx-qt bridge through the
//! macro.
//!
//! blockstitch's `com.blockworked.Blockstitch` module comes from the
//! `blockstitch-qml` dependency, which declares it in its own build script -
//! cxx-qt-build picks a dependency's QML module up automatically, so it only has
//! to be named here as a dependency of ours for the import to resolve.

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("com.blockworked.Stitchcraft")
            .version(1, 0)
            .depend("com.blockworked.Blockstitch")
            .qml_files([
                // Order does not matter to the module, but it reads as the
                // dependency order: theme, then rows, then the shell.
                "qml/Stitchcraft/BlockRows.qml",
                "qml/Stitchcraft/Palette.qml",
                "qml/Stitchcraft/DeleteZone.qml",
                "qml/Stitchcraft/SlimScrollBar.qml",
                "qml/Stitchcraft/DiagnosticsBar.qml",
                "qml/Stitchcraft/ModSettingsDialog.qml",
                "qml/Stitchcraft/Main.qml",
            ]),
    )
    .qrc("qml/controls.qrc")
    .file("src/bridge.rs")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .build();
}
