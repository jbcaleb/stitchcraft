import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.blockworked.Blockstitch

// What the canvas will and will not survive being exported, plus the status line.
//
// Errors stop an export; warnings are things that will compile and then not do
// what the blocks suggest. Both come from `stitchcraft_blocks::validate`, so the
// editor and the exporter never disagree about what is wrong.
Rectangle {
    id: root

    required property var backend
    // Clicking a diagnostic asks the window to bring its stack into view.
    signal strandRequested(string strandId)

    readonly property var diagnostics: JSON.parse(backend.diagnosticsJson || "[]")
    readonly property var errors: diagnostics.filter(d => d.severity === "error")
    readonly property var warnings: diagnostics.filter(d => d.severity === "warning")
    readonly property bool expanded: listView.visible

    implicitHeight: summary.implicitHeight + 16 + (listView.visible ? listView.height + 8 : 0)
    color: Theme.panel
    border.color: Theme.borderSoft

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 8
        spacing: 8

        RowLayout {
            id: summary
            Layout.fillWidth: true
            spacing: 10

            LucideIcon {
                name: root.errors.length ? "circle-alert" : root.warnings.length ? "triangle-alert" : "circle-check"
                color: root.errors.length ? Theme.danger : root.warnings.length ? Theme.warning : Theme.textDim
                implicitWidth: 15
                implicitHeight: 15
            }
            Text {
                text: root.errors.length
                    ? root.errors.length + (root.errors.length === 1 ? " error" : " errors")
                    : root.warnings.length
                        ? root.warnings.length + (root.warnings.length === 1 ? " warning" : " warnings")
                        : "Ready to export"
                color: Theme.text
                font.pixelSize: 12
            }
            Text {
                Layout.fillWidth: true
                text: root.backend.status
                color: Theme.textDim
                font.pixelSize: 12
                elide: Text.ElideRight
            }
            BwButton {
                visible: root.diagnostics.length > 0
                text: listView.visible ? "Hide details" : "Show details"
                onClicked: listView.visible = !listView.visible
            }
        }

        ListView {
            id: listView
            visible: false
            Layout.fillWidth: true
            // Tall enough for a handful without taking the canvas over.
            height: Math.min(contentHeight, 150)
            clip: true
            model: root.diagnostics
            spacing: 2
            ScrollBar.vertical: ScrollBar {}

            delegate: Rectangle {
                required property var modelData
                width: listView.width
                height: label.implicitHeight + 8
                radius: Theme.radius
                color: hover.hovered && modelData.strandId ? Theme.panelRaised : "transparent"

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 6
                    anchors.rightMargin: 6
                    spacing: 7

                    LucideIcon {
                        name: parent.parent.modelData.severity === "error" ? "circle-alert" : "triangle-alert"
                        color: parent.parent.modelData.severity === "error" ? Theme.danger : Theme.warning
                        implicitWidth: 13
                        implicitHeight: 13
                    }
                    Text {
                        id: label
                        Layout.fillWidth: true
                        text: parent.parent.modelData.message
                        color: Theme.text
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                    }
                }
                HoverHandler { id: hover }
                TapHandler {
                    enabled: !!parent.modelData
                    onTapped: {
                        const strandId = listView.model[index] ? listView.model[index].strandId : "";
                        if (strandId) root.strandRequested(strandId);
                    }
                }
            }
        }
    }
}
