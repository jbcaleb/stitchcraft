import QtQuick
import QtQuick.Shapes
import com.blockworked.Blockstitch

// The delete target laid over the left menu while something is being carried.
//
// It is deliberately see-through. The menu itself fades out while a canvas block
// is dragged, so the block - which the canvas draws natively - is visible right
// through this zone rather than sliding under a panel. The tint and outline say
// "drop here"; the card in the middle keeps the words legible over whatever
// happens to be underneath.
//
// It takes no pointer input: the drag belongs to whoever started it, and the
// window works out from the pointer position whether it ended over this.
Item {
    id: root

    // Fade in while there is something to delete, out when there is not.
    property bool shown: false
    // The pointer is over it, so letting go here deletes.
    property bool hot: false
    property string label: "Drag here to delete"
    property string hotLabel: "Release to delete"
    // Small print under the prompt; empty hides it.
    property string hint: ""

    // Opacity rather than an on/off switch, so it eases in and out. `visible` is
    // dropped once fully faded so a hidden zone costs nothing to render.
    opacity: shown ? 1 : 0
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }

    // The tint: faint while merely offered, red once the pointer is over it.
    Rectangle {
        anchors.fill: parent
        radius: Theme.radius
        color: root.hot ? Qt.rgba(1.0, 0.28, 0.28, 0.20) : Qt.rgba(1.0, 1.0, 1.0, 0.045)
        Behavior on color { ColorAnimation { duration: 130 } }
    }

    // A dashed outline reads as "a place to drop" where a solid one reads as a
    // panel. Drawn as a Shape because Rectangle borders cannot be dashed.
    Shape {
        anchors.fill: parent
        antialiasing: true
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            strokeWidth: 2
            strokeColor: root.hot ? Theme.danger : Theme.border
            fillColor: "transparent"
            strokeStyle: ShapePath.DashLine
            dashPattern: [4, 3]
            capStyle: ShapePath.RoundCap
            Behavior on strokeColor { ColorAnimation { duration: 130 } }

            PathRectangle {
                x: 6
                y: 6
                width: root.width - 12
                height: root.height - 12
                radius: Theme.radius + 2
            }
        }
    }

    Rectangle {
        id: card
        anchors.centerIn: parent
        width: Math.min(root.width - 40, 240)
        height: content.implicitHeight + 36
        radius: 10
        color: Qt.rgba(0.11, 0.115, 0.125, 0.88)
        border.width: 1
        border.color: root.hot ? Theme.danger : Theme.borderSoft
        // A small swell when the pointer arrives, so the state change is felt as
        // well as seen.
        scale: root.hot ? 1.06 : 1.0
        Behavior on scale { NumberAnimation { duration: 140; easing.type: Easing.OutBack } }
        Behavior on border.color { ColorAnimation { duration: 130 } }

        Column {
            id: content
            anchors.centerIn: parent
            width: parent.width - 28
            spacing: 10

            LucideIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                name: "trash"
                color: root.hot ? Theme.danger : Theme.textDim
                implicitWidth: 34
                implicitHeight: 34
                Behavior on color { ColorAnimation { duration: 130 } }
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: root.hot ? root.hotLabel : root.label
                color: root.hot ? Theme.danger : Theme.text
                font.pixelSize: 14
                font.bold: true
                Behavior on color { ColorAnimation { duration: 130 } }
            }
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.WordWrap
                visible: root.hint.length > 0 && !root.hot
                text: root.hint
                color: Theme.textDim
                font.pixelSize: 11
            }
        }
    }
}
