import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import com.blockworked.Blockstitch
import com.blockworked.Stitchcraft

// The editor window: a palette, a canvas, and the diagnostics that say whether
// what is on the canvas can be exported.
//
// Every gesture on the canvas ends up as one call into the Rust backend, with
// its arguments stringified - see `bridge.rs` for why JSON is the boundary. The
// backend answers by replacing `documentJson`, and the canvas redraws from it.
ApplicationWindow {
    id: window

    width: 1500
    height: 940
    visible: true
    color: Theme.window
    title: (backend.dirty ? "• " : "")
        + (backend.modName.length ? backend.modName : "Untitled")
        + " — Stitchcraft"

    Backend { id: backend }

    // Hands the Minecraft vocabulary to blockstitch's registry.
    BlockRows { id: blockRows; backend: backend }

    // The document, parsed once per change rather than per binding.
    readonly property var document: JSON.parse(backend.documentJson || "{}")

    function str(value) { return JSON.stringify(value); }

    // True when a window-space point is over the palette. The canvas reports drop
    // positions in these coordinates (it maps them with `mapFromItem(null, ...)`),
    // so this is what tells a delete from a drop somewhere harmless.
    function overPalette(sceneX, sceneY) {
        return palette.contains(palette.mapFromItem(null, sceneX, sceneY));
    }

    // ── Deleting by dropping on the menu ──────────────────────────────────
    // The canvas spans the whole window with the menu floating over its left
    // edge - that is what lets a block being carried stay visible over the menu
    // instead of being clipped at the canvas boundary. The cost is that a drop
    // over the menu is, to the canvas, an ordinary drop inside itself: it reports
    // a move or a split, not "dropped outside". So each drop handler below asks
    // `droppedOnPalette()` first and deletes instead.
    //
    // The pointer position is tracked rather than read at drop time because the
    // canvas clears its drag state *before* it emits the drop signals; the last
    // position seen while the drag was still live is the drop position.
    property real dragX: -1
    property real dragY: -1
    Connections {
        target: canvas
        function onDragSceneXChanged() { if (canvas.dragging) window.dragX = canvas.dragSceneX; }
        function onDragSceneYChanged() { if (canvas.dragging) window.dragY = canvas.dragSceneY; }
    }
    function droppedOnPalette() { return overPalette(dragX, dragY); }

    // A block pulled out of the menu that has since left it - i.e. one that is
    // now being slid *back*. Not true at the moment of picking it up.
    readonly property bool returning: paletteDrag.active && paletteDrag.leftPalette
    // Wherever the pointer is, whichever kind of drag is running.
    readonly property real pointerX: canvas.dragging ? canvas.dragSceneX : paletteDrag.sceneX
    readonly property real pointerY: canvas.dragging ? canvas.dragSceneY : paletteDrag.sceneY

    // A stack's path to its first block, for deleting a whole stack by its head.
    readonly property string wholeStrand: JSON.stringify([{ index: 0 }])

    // ── Palette drags ─────────────────────────────────────────────────────
    // A palette drag is host-driven: the sidebar reports pointer movement, this
    // window draws the ghost and asks the canvas where it would land, and on
    // release the drop becomes either an insert into a stack or a new strand.
    QtObject {
        id: paletteDrag
        property bool active: false
        // "instruction" or "value"
        property string kind: ""
        property var payload: null
        property real sceneX: 0
        property real sceneY: 0
        property real offsetX: 0
        property real offsetY: 0
        property real ghostWidth: 200
        property real ghostHeight: 48
        // True once the pointer has been anywhere but over the palette. A drag
        // *starts* over the palette, so "is it over the palette" alone cannot
        // tell picking something up from sliding it back; this can, and it is
        // what decides when the delete zone appears.
        property bool leftPalette: false
    }

    function beginPaletteDrag(spec, sceneX, sceneY, offsetX, offsetY) {
        paletteDrag.leftPalette = false;
        paletteDrag.kind = spec.kind || "instruction";
        paletteDrag.payload = spec.kind === "value" ? spec.value : spec.instruction;
        paletteDrag.offsetX = offsetX;
        paletteDrag.offsetY = offsetY;
        paletteDrag.sceneX = sceneX;
        paletteDrag.sceneY = sceneY;
        paletteDrag.active = true;
        movePaletteDrag(sceneX, sceneY);
    }

    function movePaletteDrag(sceneX, sceneY) {
        if (!paletteDrag.active) return;
        paletteDrag.sceneX = sceneX;
        paletteDrag.sceneY = sceneY;
        const overMenu = overPalette(sceneX, sceneY);
        if (!overMenu) paletteDrag.leftPalette = true;
        // The canvas extends underneath the palette, so it thinks a pointer over
        // the menu is over the canvas and would go looking for a slot or a stack
        // to snap to among things nobody can see. Nothing can land here: this is
        // the way back to the menu.
        if (overMenu) {
            canvas.clearSnap();
            canvas.clearPaletteHighlight();
            return;
        }
        if (paletteDrag.kind === "value") {
            canvas.updatePaletteValueTarget(sceneX, sceneY);
            return;
        }
        // The canvas wants the ghost's top-left in workspace coordinates.
        // `workspacePoint` answers null while the pointer is anywhere but over
        // the canvas - which is where every palette drag starts, since it starts
        // over the palette. Nothing can snap out there, so clear and wait.
        const topLeft = canvas.workspacePoint(
            sceneX - paletteDrag.offsetX,
            sceneY - paletteDrag.offsetY
        );
        if (!topLeft) {
            canvas.clearSnap();
            return;
        }
        canvas.updatePaletteSnap(
            topLeft.x,
            topLeft.y,
            paletteDrag.payload,
            paletteDrag.ghostWidth,
            paletteDrag.ghostHeight
        );
    }

    function endPaletteDrag(sceneX, sceneY) {
        if (!paletteDrag.active) return;
        movePaletteDrag(sceneX, sceneY);
        // Released back over the menu: the block was never placed, so deleting it
        // is just not creating it.
        if (overPalette(sceneX, sceneY)) {
            cancelPaletteDrag();
            return;
        }
        const dropped = paletteDrag.payload;
        const isValue = paletteDrag.kind === "value";
        const target = isValue ? canvas.paletteValueTarget : null;
        const snapValid = canvas.paletteSnapValid;
        const snapTarget = canvas.paletteSnapTargetId;
        const snapPath = canvas.paletteSnapPath;
        const at = canvas.workspacePoint(
            sceneX - paletteDrag.offsetX,
            sceneY - paletteDrag.offsetY
        );
        cancelPaletteDrag();

        // Released anywhere but over the canvas - back on the palette, say - is
        // a change of mind rather than a drop, so nothing is created. (A value
        // dropped onto a slot has a target and needs no position, but that
        // target is only ever set while the pointer is over the canvas.)
        if (!at && !target) return;

        if (isValue) {
            if (target) backend.putValue(window.str(target), window.str(dropped));
            else backend.createValue(Math.round(at.x), Math.round(at.y), window.str(dropped));
            return;
        }
        if (snapValid) {
            backend.dropPalette(window.str(dropped), snapTarget, window.str(snapPath), 0, 0);
        } else {
            backend.dropPalette(window.str(dropped), "", "", Math.round(at.x), Math.round(at.y));
        }
    }

    function cancelPaletteDrag() {
        paletteDrag.active = false;
        paletteDrag.payload = null;
        canvas.clearSnap();
        canvas.clearPaletteHighlight();
    }

    // ── Menus and shortcuts ───────────────────────────────────────────────
    menuBar: MenuBar {
        Menu {
            title: "&Project"
            MenuItem { text: "New"; onTriggered: backend.newProject() }
            MenuItem { text: "Open..."; onTriggered: openDialog.open() }
            MenuItem {
                text: "Save"
                onTriggered: backend.documentPath.length ? backend.save("") : saveDialog.open()
            }
            MenuItem { text: "Save as..."; onTriggered: saveDialog.open() }
            MenuSeparator {}
            MenuItem { text: "Mod settings..."; onTriggered: modSettings.openForEdit() }
            MenuItem { text: "Export mod..."; onTriggered: exportDialog.open() }
        }
        Menu {
            title: "&Edit"
            MenuItem { text: "Undo"; enabled: backend.canUndo; onTriggered: backend.undo() }
            MenuItem { text: "Redo"; enabled: backend.canRedo; onTriggered: backend.redo() }
            MenuSeparator {}
            MenuItem { text: "Reset view"; onTriggered: canvas.resetView() }
        }
    }

    Shortcut { sequences: [StandardKey.Undo]; onActivated: backend.undo() }
    Shortcut { sequences: [StandardKey.Redo]; onActivated: backend.redo() }
    Shortcut {
        sequences: [StandardKey.Save]
        onActivated: backend.documentPath.length ? backend.save("") : saveDialog.open()
    }
    Shortcut { sequences: [StandardKey.Open]; onActivated: openDialog.open() }
    Shortcut { sequences: [StandardKey.New]; onActivated: backend.newProject() }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            spacing: 8

            BwButton {
                iconName: "settings"
                text: backend.modId.length ? backend.modId : "set a mod id"
                onClicked: modSettings.openForEdit()
            }
            BwButton { iconName: "undo"; enabled: backend.canUndo; onClicked: backend.undo() }
            BwButton { iconName: "redo"; enabled: backend.canRedo; onClicked: backend.redo() }
            Item { Layout.fillWidth: true }
            Text {
                text: (backend.fabric ? "Fabric" : "") + (backend.fabric && backend.neoforge ? " + " : "")
                    + (backend.neoforge ? "NeoForge" : "")
                color: Theme.textDim
                font.pixelSize: 12
            }
            BwButton { iconName: "zoom-out"; onClicked: canvas.setZoom(Math.max(0.4, canvas.zoom - 0.1)) }
            BwButton { iconName: "zoom-in"; onClicked: canvas.setZoom(Math.min(1.8, canvas.zoom + 0.1)) }
            BwButton {
                primary: true
                iconName: "download"
                text: "Export"
                onClicked: exportDialog.open()
            }
        }
    }

    // ── Layout ────────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The canvas fills the stage edge to edge; the menu and the delete zone are
        // laid over its left side. See `droppedOnPalette` for why.
        Item {
            id: stage
            Layout.fillWidth: true
            Layout.fillHeight: true

            Palette {
                id: palette
                z: 1
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: implicitWidth
                backend: backend
                document: window.document
                // Fades out while a canvas block is carried, so the block stays
                // visible where the menu is: the canvas underneath draws it natively,
                // and a menu that was opaque would hide it. Only dimmed for a block
                // coming back from the canvas - there the ghost is a separate item
                // above everything, so nothing needs to show through.
                opacity: canvas.dragging ? 0.0 : window.returning ? 0.35 : 1.0
                Behavior on opacity { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                onDragStarted: (spec, sx, sy, ox, oy) => window.beginPaletteDrag(spec, sx, sy, ox, oy)
                onDragMoved: (sx, sy) => window.movePaletteDrag(sx, sy)
                onDragEnded: (sx, sy) => window.endPaletteDrag(sx, sy)
                onDragCanceled: window.cancelPaletteDrag()
                onMakeVariableRequested: variableDialog.openForCreate()
                onMakeListRequested: listDialog.openForCreate()
                onMakeDictRequested: dictDialog.openForCreate()
                onMakeBlockRequested: backend.makeBlock(
                    // A placeholder prototype; the canvas header can be renamed
                    // from its own context menu afterwards.
                    JSON.stringify([{ kind: "Label", id: "l0", text: "my block" }]),
                    "Normal",
                    "#4C97FF",
                    40,
                    40
                )
                onRenameListRequested: name => listDialog.openForRename(name)
                onDeleteListRequested: name => backend.deleteList(name)
                onRenameDictRequested: name => dictDialog.openForRename(name)
                onDeleteDictRequested: name => backend.deleteDict(name)
            }

            // Stacking is by `z`, not declaration order: canvas at the bottom, the
            // menu above it, the delete zone above that.
            BlockCanvas {
                id: canvas
                anchors.fill: parent
                z: 0

                strands: window.document.strands || []
                comments: window.document.comments || []
                floatingValues: window.document.floating_values || []
                variables: (window.document.variables || []).map(v => v.name)
                lists: window.document.lists || []
                dicts: window.document.dicts || []
                blockDefinitions: window.document.block_defs || []

                // ── Stacks ────────────────────────────────────────────────
                // Every way a stack can be dropped, each asking first whether it was
                // dropped on the menu - in which case it is deleted instead. A whole
                // stack is deleted by its head; a tail, from where it was cut.
                onStrandMoved: (strandId, x, y) => window.droppedOnPalette()
                    ? backend.deleteTail(strandId, window.wholeStrand)
                    : backend.moveStrand(strandId, x, y)
                onInstructionSplit: (strandId, path, x, y) => window.droppedOnPalette()
                    ? backend.deleteTail(strandId, window.str(path))
                    : backend.splitInstruction(strandId, window.str(path), x, y)
                onStrandsMerged: (draggedId, targetId, path) => window.droppedOnPalette()
                    ? backend.deleteTail(draggedId, window.wholeStrand)
                    : backend.mergeStrands(draggedId, targetId, window.str(path))
                onTailMerged: (strandId, path, targetId, targetPath) => window.droppedOnPalette()
                    ? backend.deleteTail(strandId, window.str(path))
                    : backend.mergeTail(strandId, window.str(path), targetId, window.str(targetPath))
                onInstructionRemoved: (strandId, path) =>
                    backend.removeInstruction(strandId, window.str(path))
                onInstructionDuplicated: (strandId, path, instruction) =>
                    backend.duplicateInstruction(strandId, window.str(path), window.str(instruction))
                onInstructionEdited: (strandId, path, instruction) =>
                    backend.editInstruction(strandId, window.str(path), window.str(instruction))
                // Dropped *outside* the canvas altogether (over the toolbar, say) is
                // deliberately not handled: the document is untouched, so the stack
                // stays where it was. The menu no longer counts as outside - it sits
                // over the canvas - so deleting is handled by the drop handlers above.

                // ── Values ────────────────────────────────────────────────
                onValueEdited: (location, text) => backend.editValue(window.str(location), text)
                // A value moved is "take it, then put it down", two signals in a row.
                // Dropped on the menu, the take becomes a delete and the put is
                // skipped, so nothing is put anywhere.
                onValueTakeRequested: location => window.droppedOnPalette()
                    ? backend.deleteValue(window.str(location))
                    : backend.takeValue(window.str(location))
                onValuePutRequested: (location, value) => {
                    if (!window.droppedOnPalette())
                        backend.putValue(window.str(location), window.str(value));
                }
                onValueCreateRequested: (x, y, value) => {
                    if (!window.droppedOnPalette())
                        backend.createValue(x, y, window.str(value));
                }
                onFloatingValueMoved: (floatingId, x, y) => window.droppedOnPalette()
                    ? backend.removeFloatingValue(floatingId)
                    : backend.moveFloatingValue(floatingId, x, y)
                onFloatingValueRemoved: floatingId => backend.removeFloatingValue(floatingId)

                // ── Notes ─────────────────────────────────────────────────
                onCanvasNoteRequested: (x, y) => backend.addComment(x, y, "")
                onCommentForInstructionRequested: instruction =>
                    backend.addComment(24, 0, instruction.id)
                onCommentMoved: (commentId, x, y) => backend.moveComment(commentId, x, y)
                onCommentEdited: (commentId, text) => backend.editComment(commentId, text)
                onCommentCollapseChanged: (commentId, collapsed) =>
                    backend.setCommentCollapsed(commentId, collapsed)
                onCommentRemoved: commentId => backend.removeComment(commentId)

                // ── Collections ───────────────────────────────────────────
                onListItemsEdited: (name, items) => backend.setListItems(name, window.str(items))
                onListEditorStateChanged: (name, visible, x, y) =>
                    backend.setListEditorState(name, visible, x, y)
                onDictEntriesEdited: (name, entries) =>
                    backend.setDictEntries(name, window.str(entries))
                onDictEditorStateChanged: (name, visible, x, y) =>
                    backend.setDictEditorState(name, visible, x, y)

                onClearRequested: clearConfirm.open()
            }

            // The delete target, laid over the menu. It appears for a block being
            // carried around the canvas, and for one pulled out of the menu that
            // is now being slid back - that one was never placed, so "delete" just
            // means not placing it, but it is the same gesture and gets the same
            // cue.
            DeleteZone {
                id: deleteZone
                z: 2
                x: palette.x
                y: palette.y
                width: palette.width
                height: palette.height
                shown: canvas.dragging || window.returning
                hot: shown && window.overPalette(window.pointerX, window.pointerY)
                // Only a stack drags a tail along with it; a lone value does not,
                // and a block still on its way out of the menu has none.
                hint: canvas.dragging && !canvas.valueDragState.active
                    ? "Blocks below the one you are holding go too."
                    : ""
            }
        }

        DiagnosticsBar {
            id: diagnostics
            Layout.fillWidth: true
            backend: backend
            onStrandRequested: strandId => canvas.centerOnOrigin()
        }
    }

    // The palette ghost, drawn above everything while a palette drag is running.
    //
    // The pointer arrives in *window* coordinates, but a plain child of an
    // ApplicationWindow lives in its content area, which starts below the menu bar
    // and toolbar - so placing it at the raw position drew it ~80px too low and
    // it jumped up to the pointer on release. It is parented to the overlay (which
    // spans the whole window and sits above the palette) and positioned by mapping
    // the window point into whatever it is actually parented to, so it cannot be
    // wrong by the size of any chrome.
    Loader {
        id: ghost
        active: paletteDrag.active
        parent: Overlay.overlay
        readonly property point at: parent
            ? parent.mapFromItem(null, paletteDrag.sceneX - paletteDrag.offsetX,
                                       paletteDrag.sceneY - paletteDrag.offsetY)
            : Qt.point(0, 0)
        x: at.x
        y: at.y
        z: 1000
        opacity: 0.85
        sourceComponent: paletteDrag.kind === "value" ? valueGhost : instructionGhost
    }
    Component {
        id: instructionGhost
        InstructionBlock {
            instruction: paletteDrag.payload
            blockDefinitions: window.document.block_defs || []
            paletteMode: true
            locked: true
            onImplicitWidthChanged: paletteDrag.ghostWidth = implicitWidth
            onImplicitHeightChanged: paletteDrag.ghostHeight = implicitHeight
        }
    }
    Component {
        id: valueGhost
        ValueChip {
            valueData: paletteDrag.payload
            boxed: true
            paletteMode: true
            editable: false
        }
    }

    // ── Dialogs ───────────────────────────────────────────────────────────
    ModSettingsDialog {
        id: modSettings
        backend: backend
        anchors.centerIn: Overlay.overlay
    }

    NameDialog {
        id: variableDialog
        noun: "variable"
        anchors.centerIn: Overlay.overlay
        onSubmitted: (name, renameTarget) => {
            const ok = renameTarget.length
                ? backend.renameVariable(renameTarget, name)
                : backend.createVariable(name);
            if (ok) close();
            else fail(backend.status);
        }
    }
    NameDialog {
        id: listDialog
        noun: "list"
        anchors.centerIn: Overlay.overlay
        onSubmitted: (name, renameTarget) => {
            const ok = renameTarget.length
                ? backend.renameList(renameTarget, name)
                : backend.createList(name);
            if (ok) close();
            else fail(backend.status);
        }
    }
    NameDialog {
        id: dictDialog
        noun: "dict"
        anchors.centerIn: Overlay.overlay
        onSubmitted: (name, renameTarget) => {
            const ok = renameTarget.length
                ? backend.renameDict(renameTarget, name)
                : backend.createDict(name);
            if (ok) close();
            else fail(backend.status);
        }
    }

    BwDialog {
        id: clearConfirm
        title: "Clear the canvas?"
        anchors.centerIn: Overlay.overlay
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: backend.newProject()
        Text {
            width: 340
            wrapMode: Text.WordWrap
            text: "Every stack is removed. This can be undone."
            color: Theme.text
        }
    }

    FileDialog {
        id: openDialog
        title: "Open a Stitchcraft project"
        nameFilters: ["Stitchcraft project (*.stitch)", "All files (*)"]
        onAccepted: backend.open(selectedFile.toString().replace("file:///", ""))
    }
    FileDialog {
        id: saveDialog
        title: "Save the project"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "stitch"
        nameFilters: ["Stitchcraft project (*.stitch)"]
        onAccepted: backend.save(selectedFile.toString().replace("file:///", ""))
    }
    FolderDialog {
        id: exportDialog
        title: "Export the mod into"
        onAccepted: backend.exportMod(selectedFolder.toString().replace("file:///", ""))
    }

    Component.onCompleted: {
        // A brand new project has no mod id, and nothing else makes sense until
        // it does - the namespace of every registered block depends on it.
        if (!backend.modId.length) modSettings.openForEdit();
    }
}
