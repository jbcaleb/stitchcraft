import QtQuick
import com.blockworked.Blockstitch

// Hands the Minecraft block vocabulary to blockstitch's BlockRegistry.
//
// The tables themselves are built in Rust (`stitchcraft_blocks::catalog`) so the
// block enum and its drawing cannot drift apart, and arrive here as JSON. Two
// things JSON cannot carry have to be put back on this side:
//
//   * a dropdown whose choices depend on the document - written as
//     `options: {source: "declaredBlocks"}`, and turned into the function
//     BlockRegistry asks for;
//   * a boolean field - written as `options: "yesno"`, since the piece kinds are
//     label/value/dropdown/text with no checkbox among them. It becomes a
//     two-entry dropdown with an encode/decode pair, because the underlying
//     field is a real JSON boolean rather than the string a dropdown hands back.
QtObject {
    id: root

    // The Backend, for the row tables and the current choice sets.
    required property var backend

    readonly property var choices: JSON.parse(backend.choicesJson || "{}")

    // Re-read whenever the document changes: a newly declared block has to show
    // up in every "when [block] is right-clicked" dropdown immediately.
    onChoicesChanged: BlockRegistry.revision++

    readonly property var yesNoOptions: [
        { value: "true", label: "yes" },
        { value: "false", label: "no" }
    ]

    function sourceChoices(name) {
        const names = root.choices[name] || [];
        if (!names.length) return [{ value: "", label: "none yet" }];
        return names.map(n => ({ value: n, label: n }));
    }

    // Replaces the data stand-ins in one piece with what BlockRegistry wants.
    function resolvePiece(piece) {
        if (piece.kind !== "dropdown") return piece;
        const out = Object.assign({}, piece);
        if (out.options === "yesno") {
            out.options = root.yesNoOptions;
            // The field is a JSON boolean; the dropdown trades in strings.
            out.encode = chosen => chosen === "true";
            out.decode = value => value ? "true" : "false";
        } else if (out.options && out.options.source !== undefined) {
            const name = out.options.source;
            out.options = () => root.sourceChoices(name);
        }
        return out;
    }

    function resolveRow(row) {
        const out = Object.assign({}, row);
        out.head = (row.head || []).map(resolvePiece);
        return out;
    }

    function register() {
        const rows = JSON.parse(backend.rowsJson || "{}");
        const resolved = {};
        for (const type in rows) resolved[type] = resolveRow(rows[type]);
        BlockRegistry.registerRows(resolved);

        BlockRegistry.registerOperators(JSON.parse(backend.operatorRowsJson || "{}"));

        // Prefabs arrive without their `type`, which BlockRegistry.prefab adds
        // from the key it was asked for.
        BlockRegistry.registerPrefabs(JSON.parse(backend.prefabsJson || "{}"));

        // Blockwork's own right-click extras do not apply to a mod project.
        BlockRegistry.recordingTargets = false;
    }

    Component.onCompleted: register()
}
