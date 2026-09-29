import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch

// A thin scroll bar that stays out of the way: a rounded thumb, no track, dim
// until the pointer is on it.
//
// The stock Basic bar is 12px of opaque track that draws over whatever it is
// scrolling. Both bars in the palette needed the same look, and a shared file
// keeps them from drifting apart.
ScrollBar {
    id: control

    // Breathing room around the thumb, so it never touches the panel edge.
    padding: 3
    minimumSize: 0.1

    // Nothing to scroll, nothing to draw.
    readonly property bool needed: size < 1.0

    contentItem: Rectangle {
        implicitWidth: 6
        implicitHeight: 6
        radius: 3
        color: control.pressed ? Theme.accent : Theme.textDim
        opacity: !control.needed ? 0.0 : control.pressed ? 0.95 : control.hovered ? 0.75 : 0.4
        Behavior on opacity { NumberAnimation { duration: 140 } }
        Behavior on color { ColorAnimation { duration: 120 } }
    }
    background: Item {}
}
