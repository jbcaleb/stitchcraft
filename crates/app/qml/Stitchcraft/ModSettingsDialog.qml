import QtQuick
import QtQuick.Controls
import com.blockworked.Blockstitch

// The mod's identity: what goes in `fabric.mod.json`, `neoforge.mods.toml` and
// the namespace every registered id gets.
//
// The mod id is the one field with a shape the rest of the export depends on, so
// it is sanitized on the way through rather than validated after the fact.
BwDialog {
    id: root

    required property var backend

    title: "Mod settings"
    standardButtons: Dialog.Ok | Dialog.Cancel
    closePolicy: Popup.CloseOnEscape

    function openForEdit() {
        idField.text = root.backend.modId;
        nameField.text = root.backend.modName;
        versionField.text = root.backend.modVersion;
        packageField.text = root.backend.modPackage;
        fabricSwitch.checked = root.backend.fabric;
        neoforgeSwitch.checked = root.backend.neoforge;
        open();
        idField.forceActiveFocus();
    }

    onAccepted: backend.setMetadata(
        idField.text,
        nameField.text,
        versionField.text,
        packageField.text,
        fabricSwitch.checked,
        neoforgeSwitch.checked
    )

    Column {
        width: 400
        spacing: 10

        Text { text: "Mod id"; color: Theme.text; font.pixelSize: 12 }
        BwTextField {
            id: idField
            width: parent.width
            placeholderText: "wonder_blocks"
        }
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "Lowercase letters, digits and underscores. Everything the canvas registers is namespaced with this."
            color: Theme.textDim
            font.pixelSize: 11
        }

        Text { text: "Display name"; color: Theme.text; font.pixelSize: 12 }
        BwTextField { id: nameField; width: parent.width; placeholderText: "Wonder Blocks" }

        Text { text: "Version"; color: Theme.text; font.pixelSize: 12 }
        BwTextField { id: versionField; width: parent.width; placeholderText: "1.0.0" }

        Text { text: "Java package"; color: Theme.text; font.pixelSize: 12 }
        BwTextField { id: packageField; width: parent.width; placeholderText: "com.example.wonder_blocks" }

        Text { text: "Export for"; color: Theme.text; font.pixelSize: 12 }
        Row {
            spacing: 16
            Row {
                spacing: 7
                BwSwitch { id: fabricSwitch }
                Text {
                    anchors.verticalCenter: fabricSwitch.verticalCenter
                    text: "Fabric"
                    color: Theme.text
                    font.pixelSize: 12
                }
            }
            Row {
                spacing: 7
                BwSwitch { id: neoforgeSwitch }
                Text {
                    anchors.verticalCenter: neoforgeSwitch.verticalCenter
                    text: "NeoForge"
                    color: Theme.text
                    font.pixelSize: 12
                }
            }
        }
    }
}
