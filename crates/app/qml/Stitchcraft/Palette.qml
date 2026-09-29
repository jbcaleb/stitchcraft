import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch

// The sidebar: every block a canvas can hold, in the sections the Rust catalogue
// declares.
//
// This is Stitchcraft's own palette rather than blockstitch's `PalettePanel`,
// because that one's operator list is a `readonly property` holding Blockwork's
// reporters - there is no way to put `event player name` or `block id at` in it
// from outside. The draggable pieces themselves (`PaletteBlock`, `PaletteValue`,
// `CollectionPanel`) are blockstitch's, so a palette block looks and drags
// exactly like a canvas one.
Rectangle {
    id: root

    required property var backend
    // The document, already parsed by the window.
    required property var document

    // Pixels per wheel notch. The stock ScrollView moves ~11px a notch, which
    // crawls through a palette this long; a browser moves ~100.
    readonly property real wheelStep: 0.9

    signal dragStarted(var spec, real sceneX, real sceneY, real offsetX, real offsetY)
    signal dragMoved(real sceneX, real sceneY)
    signal dragEnded(real sceneX, real sceneY)
    signal dragCanceled()
    signal makeVariableRequested()
    signal makeListRequested()
    signal makeDictRequested()
    signal makeBlockRequested()
    signal renameListRequested(string name)
    signal deleteListRequested(string name)
    signal renameDictRequested(string name)
    signal deleteDictRequested(string name)

    implicitWidth: 330
    color: Theme.panel
    border.color: Theme.borderSoft

    readonly property var sections: JSON.parse(backend.sectionsJson || "[]")
    readonly property var reporters: JSON.parse(backend.reportersJson || "[]")
    readonly property var listReporters: JSON.parse(backend.listReportersJson || "[]")
    readonly property var dictReporters: JSON.parse(backend.dictReportersJson || "[]")
    readonly property var listTypes: JSON.parse(backend.listTypesJson || "[]")
    readonly property var dictTypes: JSON.parse(backend.dictTypesJson || "[]")

    readonly property var variableNames: (document.variables || []).map(v => v.name)
    readonly property var lists: document.lists || []
    readonly property var dicts: document.dicts || []
    readonly property var blockDefinitions: document.block_defs || []

    function firstName(collection) {
        return collection && collection.length ? collection[0].name : "";
    }

    // A fresh instruction of `type`, with the collection and variable names the
    // document actually has filled in - a bare `set [ ] to 0` is no use.
    function prefab(type) {
        const instruction = BlockRegistry.prefab(type);
        if (!instruction) return { id: "palette-" + type, type: type };
        // Only the types whose `name` really is a variable, list or dict. An
        // "any empty name" rule is tempting but wrong: `OnCommand.name` is the
        // text after the slash, and it would come out pre-filled with a variable.
        if (instruction.name === "") {
            if (root.listTypes.indexOf(type) >= 0) instruction.name = root.firstName(root.lists);
            else if (root.dictTypes.indexOf(type) >= 0) instruction.name = root.firstName(root.dicts);
            else if ((type === "SetVariable" || type === "ChangeVariable") && root.variableNames.length)
                instruction.name = root.variableNames[0];
        }
        return instruction;
    }

    // A fresh reporter of `kind`, from the arity the Rust table declared.
    function reporterValue(entry) {
        const kind = entry[0];
        const arity = entry[2];
        const args = [];
        for (let i = 0; i < arity; ++i) args.push({ kind: "Number", value: 0 });
        return { kind: "Op", op: kind, args: args, saved: { kind: "Number", value: 0 } };
    }

    // The collection reporters take their collection's name as one of their
    // arguments; which one is the wire contract, so it comes from the blanks the
    // registry built rather than being guessed here.
    function collectionValue(entry, names) {
        const value = root.reporterValue(entry);
        const spec = BlockRegistry.operator(entry[0]);
        const index = spec && spec.enumArg ? spec.enumArg.index : 0;
        for (let i = 0; i < value.args.length; ++i) {
            value.args[i] = i === index
                ? { kind: "Text", value: root.firstName(names) }
                : { kind: "Text", value: "" };
        }
        return value;
    }

    // The panel sits over the canvas now (see Main.qml), so anything the palette
    // does not handle would fall through to the canvas beneath: a wheel over the
    // gap between two blocks would pan the canvas, and a drag would pull it. This
    // sits under everything else and simply swallows what nothing above wants.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onWheel: wheel => wheel.accepted = true
    }

    Flickable {
        id: scroll
        // The whole panel, so the scroll bar lives at its real right edge. The
        // blocks are padded in from that by hand below; padding the *Flickable*
        // instead pulled the bar in with them, which is what made it look
        // misaligned - floating a few pixels off the edge, beside the blocks.
        anchors.fill: parent
        anchors.margins: root.border.width
        clip: true

        readonly property real pad: 10
        // Width reserved on the right for the bar, so it never sits on a block.
        readonly property real gutter: 14

        contentWidth: width
        contentHeight: column.implicitHeight + 2 * pad
        boundsBehavior: Flickable.StopAtBounds
        // While a wheel gesture is running the Flickable must not also run its
        // own wheel logic, or every notch is handled twice - the canvas hit the
        // same thing, and its comment on `interactive` explains it.
        interactive: !wheelScroll.active

        ScrollBar.vertical: SlimScrollBar {
            // Kept off the rounded corners, and always on when there is something
            // to scroll: a wheel does not wake the stock "show while moving"
            // behaviour, so without this there is no sign the panel scrolls.
            topPadding: 8
            bottomPadding: 8
            policy: ScrollBar.AlwaysOn
        }

        WheelHandler {
            id: wheelScroll
            target: null
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            orientation: Qt.Vertical
            onWheel: event => {
                // A touchpad reports real pixels; a mouse reports notches of 120.
                let dy = event.pixelDelta.y;
                if (dy === 0) dy = event.angleDelta.y * root.wheelStep;
                if (event.inverted) dy = -dy;
                scroll.contentY = Math.max(0, Math.min(scroll.contentHeight - scroll.height, scroll.contentY - dy));
                event.accepted = true;
            }
        }

        ColumnLayout {
            id: column
            x: scroll.pad
            y: scroll.pad
            width: scroll.width - scroll.pad - scroll.gutter
            spacing: 14

            // ── Instruction sections ──────────────────────────────────────
            Repeater {
                model: root.sections
                delegate: ColumnLayout {
                    id: section
                    required property var modelData
                    Layout.fillWidth: true
                    spacing: 6

                    Text {
                        text: section.modelData.title
                        color: Theme.textDim
                        font.pixelSize: 11
                        font.bold: true
                    }

                    // A declaration block is a whole sentence wide - `register
                    // block [id] named [name] as [material] hardness ( ) ...` -
                    // so clipping one at the sidebar edge would hide the fields
                    // it is asking about. Each section scrolls sideways to its
                    // own widest block instead.
                    //
                    // A plain Column rather than a layout: its implicit width is
                    // the widest child's, and nothing stretches children back to
                    // the panel, so there is no binding loop between the two.
                    Flickable {
                        id: rail
                        Layout.fillWidth: true
                        // Does the widest block here not fit the panel?
                        readonly property bool overflows: contentWidth > width + 1
                        // The bar gets a strip of its own beneath the blocks. An
                        // attached bar is drawn along the Flickable's bottom edge,
                        // so with no spare height it was painted over the bottom of
                        // the last block instead of under it.
                        readonly property real barStrip: 14
                        implicitHeight: blocks.implicitHeight + (overflows ? barStrip : 0)
                        contentWidth: blocks.implicitWidth
                        contentHeight: blocks.implicitHeight
                        flickableDirection: Flickable.HorizontalFlick
                        boundsBehavior: Flickable.StopAtBounds
                        clip: true
                        // Not draggable: a horizontal flick would compete with the
                        // drag that lifts a wide block out of the palette, and
                        // would swallow the wheel before the palette sees it. The
                        // bar below still scrolls it.
                        interactive: false

                        ScrollBar.horizontal: SlimScrollBar {
                            policy: rail.overflows ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
                        }

                        // A sideways wheel or touchpad swipe scrolls it as well, so
                        // the bar is not the only way in. Vertical wheel events are
                        // not this handler's - they go on to the panel's own.
                        WheelHandler {
                            target: null
                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                            orientation: Qt.Horizontal
                            enabled: rail.overflows
                            onWheel: event => {
                                let dx = event.pixelDelta.x;
                                if (dx === 0) dx = event.angleDelta.x * root.wheelStep;
                                rail.contentX = Math.max(0, Math.min(rail.contentWidth - rail.width, rail.contentX - dx));
                                event.accepted = true;
                            }
                        }

                        Column {
                            id: blocks
                            spacing: 4
                            Repeater {
                                model: section.modelData.types
                                delegate: PaletteBlock {
                                    required property string modelData
                                    instruction: root.prefab(modelData)
                                    spec: ({ kind: "instruction", instruction: instruction })
                                    variables: root.variableNames
                                    lists: root.lists
                                    blockDefinitions: root.blockDefinitions
                                    onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                                    onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                                    onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                                    onDragCanceled: root.dragCanceled()
                                }
                            }
                        }
                    }
                }
            }

            // ── Reporters ─────────────────────────────────────────────────
            Text {
                text: "Reporters"
                color: Theme.textDim
                font.pixelSize: 11
                font.bold: true
            }
            Flow {
                Layout.fillWidth: true
                spacing: 5
                Repeater {
                    model: root.reporters
                    delegate: PaletteValue {
                        required property var modelData
                        valueData: root.reporterValue(modelData)
                        forceBoolean: modelData[1] === "bool"
                        spec: ({ kind: "value", value: valueData })
                        blockDefinitions: root.blockDefinitions
                        onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                        onDragCanceled: root.dragCanceled()
                    }
                }
            }

            // ── Variables ─────────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                Text { text: "Variables"; color: Theme.textDim; font.pixelSize: 11; font.bold: true }
                Item { Layout.fillWidth: true }
                BwButton { text: "Make a variable"; onClicked: root.makeVariableRequested() }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 5
                Repeater {
                    model: root.variableNames
                    delegate: PaletteValue {
                        required property string modelData
                        valueData: ({ kind: "Var", name: modelData })
                        spec: ({ kind: "value", value: valueData })
                        editable: false
                        onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                        onDragCanceled: root.dragCanceled()
                    }
                }
            }

            // ── Lists ─────────────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                Text { text: "Lists"; color: Theme.textDim; font.pixelSize: 11; font.bold: true }
                Item { Layout.fillWidth: true }
                BwButton { text: "Make a list"; onClicked: root.makeListRequested() }
            }
            CollectionPanel {
                Layout.fillWidth: true
                collections: root.lists
                noun: "list"
                emptyText: "No lists yet."
                rowWidth: root.width - 24
                onEditorStateRequested: (name, visible, x, y) => root.backend.setListEditorState(name, visible, x, y)
                onRenameRequested: name => root.renameListRequested(name)
                onDeleteRequested: name => root.deleteListRequested(name)
            }
            Flow {
                Layout.fillWidth: true
                spacing: 5
                visible: root.lists.length > 0
                Repeater {
                    model: root.lists.length ? root.listTypes : []
                    delegate: PaletteBlock {
                        required property string modelData
                        instruction: root.prefab(modelData)
                        spec: ({ kind: "instruction", instruction: instruction })
                        variables: root.variableNames
                        lists: root.lists
                        onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                        onDragCanceled: root.dragCanceled()
                    }
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 5
                visible: root.lists.length > 0
                Repeater {
                    model: root.lists.length ? root.listReporters : []
                    delegate: PaletteValue {
                        required property var modelData
                        valueData: root.collectionValue(modelData, root.lists)
                        forceBoolean: modelData[1] === "bool"
                        spec: ({ kind: "value", value: valueData })
                        onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                        onDragCanceled: root.dragCanceled()
                    }
                }
            }

            // ── Dicts ─────────────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                Text { text: "Dicts"; color: Theme.textDim; font.pixelSize: 11; font.bold: true }
                Item { Layout.fillWidth: true }
                BwButton { text: "Make a dict"; onClicked: root.makeDictRequested() }
            }
            CollectionPanel {
                Layout.fillWidth: true
                collections: root.dicts
                noun: "dict"
                emptyText: "No dicts yet."
                rowWidth: root.width - 24
                onEditorStateRequested: (name, visible, x, y) => root.backend.setDictEditorState(name, visible, x, y)
                onRenameRequested: name => root.renameDictRequested(name)
                onDeleteRequested: name => root.deleteDictRequested(name)
            }
            Flow {
                Layout.fillWidth: true
                spacing: 5
                visible: root.dicts.length > 0
                Repeater {
                    model: root.dicts.length ? root.dictTypes : []
                    delegate: PaletteBlock {
                        required property string modelData
                        instruction: root.prefab(modelData)
                        spec: ({ kind: "instruction", instruction: instruction })
                        variables: root.variableNames
                        lists: root.lists
                        onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                        onDragCanceled: root.dragCanceled()
                    }
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 5
                visible: root.dicts.length > 0
                Repeater {
                    model: root.dicts.length ? root.dictReporters : []
                    delegate: PaletteValue {
                        required property var modelData
                        valueData: root.collectionValue(modelData, root.dicts)
                        forceBoolean: modelData[1] === "bool"
                        spec: ({ kind: "value", value: valueData })
                        onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                        onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                        onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                        onDragCanceled: root.dragCanceled()
                    }
                }
            }

            // ── Custom blocks ─────────────────────────────────────────────
            RowLayout {
                Layout.fillWidth: true
                Text { text: "My Blocks"; color: Theme.textDim; font.pixelSize: 11; font.bold: true }
                Item { Layout.fillWidth: true }
                BwButton { text: "Make a block"; onClicked: root.makeBlockRequested() }
            }
            Repeater {
                model: root.blockDefinitions
                delegate: PaletteBlock {
                    required property var modelData
                    // A reporter-shaped definition is dragged as a value, not an
                    // instruction, so only command-shaped ones appear here.
                    visible: modelData.shape !== "ReturnsValue" && modelData.shape !== "ReturnsBool"
                    height: visible ? implicitHeight : 0
                    instruction: ({
                        id: "palette-call-" + modelData.id,
                        type: "CallBlock",
                        block_id: modelData.id,
                        args: (modelData.pieces || [])
                            .filter(p => p.kind === "Input")
                            .map(p => p.value_type === "Bool"
                                ? { kind: "Bool" }
                                : { kind: "Number", value: 0 })
                    })
                    spec: ({ kind: "instruction", instruction: instruction })
                    blockDefinitions: root.blockDefinitions
                    blockColor: modelData.color
                    onDragStarted: (spec, sx, sy, ox, oy) => root.dragStarted(spec, sx, sy, ox, oy)
                    onDragMoved: (sx, sy) => root.dragMoved(sx, sy)
                    onDragEnded: (sx, sy) => root.dragEnded(sx, sy)
                    onDragCanceled: root.dragCanceled()
                }
            }

            Item { Layout.fillWidth: true; implicitHeight: 24 }
        }
    }

}
