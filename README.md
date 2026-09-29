# Stitchcraft

A Scratch-style block editor whose canvas is a Minecraft mod. Drag blocks
together, press Export, and get a buildable Fabric **and** NeoForge mod source
tree — one set of Java, both loaders, on
[FrozenLib](https://github.com/FrozenBlock/FrozenLib)'s cross-platform registry
and event APIs.

Built on [Blockworked/blockstitch](https://github.com/Blockworked/blockstitch)
(`qml` branch): `blockstitch-core` is the document model and every editing
operation, and the Qt 6/QML components are the canvas itself.

## The shape of it

```
crates/blocks    the vocabulary — what a block can say about a mod
crates/export    the compiler   — canvas in, Java + Gradle + resources out
crates/app       the editor     — Qt 6/QML, on blockstitch's canvas
```

### Blocks

blockstitch-core asks a host for exactly one thing: an instruction enum
implementing `BlockKind`. That is `McBlock`, and it has three families:

- **declarations** — `register block`, `register item`, `register entity`,
  `register sound`, `register creative tab`. Header-shaped, one per stack.
- **event hooks** — `when a player joins`, `when [block] is right-clicked`,
  `on command /[name]`, `every server tick`, and so on. Also header-shaped; the
  handler is the rest of the stack, the way `when green flag clicked` works.
- **commands and control flow** — everything else, including variables, lists
  and dicts, which blockstitch-core already knows how to rename and reconcile.

How each block *draws* is declared in Rust too (`catalog.rs`) rather than in
QML, so a field renamed on the enum stops the build instead of quietly drawing
an empty slot. The tests check every row against the variant it builds.

### Export

One canvas stack becomes one Java method. Every command becomes a call into a
generated `Rt.java`, which is **the only generated file that touches
Minecraft's own API** — so when a game update renames a method, the fix is a few
lines there rather than every call site the canvas produced.

FrozenLib is what makes one set of sources serve both loaders:

| Job | What it uses |
| --- | --- |
| Registering blocks, items, entities, sounds, tabs | `DeferredRegister` — immediate on Fabric, deferred to NeoForge's own on NeoForge |
| Lifecycle, ticks, joins, block breaks, damage, death | its cross-platform `Event<T>` |
| Entity attributes | `DefaultAttributeRegistry` |
| Client-side renderers | `EntityRendererRegistry` |

What FrozenLib has no answer for is generated per loader, and there is very
little of it: an entrypoint class each, and a four-line command bridge
(`CommandRegistrationCallback` on Fabric, `RegisterCommandsEvent` on NeoForge).

The hooks that are not events in the game at all — a block being right-clicked,
an item being used, one of the mod's own mobs ticking — are dispatched from
generated `ScriptedBlock` / `ScriptedItem` / `ScriptedMob` classes through a
table in `Handlers`.

The exported project is a plain multiloader Gradle build: `common/` is a shared
source directory that each loader adds to its own source sets, so it compiles
twice, once against each loader's Minecraft. No Architectury, no vanilla-only
jar to compile against.

## Building

Needs Rust 1.85+, Qt 6.10+ and a C++ toolchain (cxx-qt 0.10 requires Qt 6.10).

Cargo downloads blockstitch (the `qml` branch of
[Blockworked/blockstitch](https://github.com/Blockworked/blockstitch)) by itself on
the first build, and `Cargo.lock` pins the exact commit. Nothing to fetch or copy.
`cargo update -p blockstitch-core -p blockstitch-qml` moves to a newer one.

One-time setup: tell Cargo where Qt is. Copy `.cargo/config.toml.example` to
`.cargo/config.toml` and edit the `QMAKE` path. That file is git-ignored because
the path is specific to your machine. After that, running the editor is just:

```bash
cargo run
```

The config sets `QMAKE` for the build and adds a runner (`scripts/run-with-qt.cmd`)
that puts Qt's DLLs on `PATH` for the run, so nothing has to be set by hand. A bare
`cargo run` and `cargo build` cover just the editor; use `cargo test --workspace`
to test everything.

The logic crates need no Qt at all:

```bash
cargo test -p stitchcraft-blocks -p stitchcraft-export
```

### Exporting without the editor

```bash
cargo run -p stitchcraft-export --bin stitchcraft-export -- project.stitch out/
```

And a worked example that builds a small mod in code, saves the canvas, and
exports it:

```bash
cargo run -p stitchcraft-export --example wonder_blocks -- out
```

## Licensing

Stitchcraft is GPL-3.0-or-later (see `LICENSE`). It links `blockstitch-qml`, which
is GPL-3.0-or-later; `blockstitch-core` is MIT. The mods it exports depend on
[FrozenLib](https://github.com/FrozenBlock/FrozenLib), which is GPL-3.0, so check
its terms before distributing an exported mod.

## What is verified, and what is not

Verified here: 104 tests pass; the whole workspace builds warning-free; the
editor starts, loads its QML and renders with no runtime errors; the exporter
produces a complete, deterministic tree whose Java files all balance their
braces and whose JSON all parses.

**Not verified: the generated Java has never been compiled against Minecraft.**
There is no JDK 25 or Gradle on the machine this was built on, and FrozenLib
3.0-mc26.3 targets a very new Minecraft where `ResourceLocation` is `Identifier`
and `hurt` is `hurtServer`. The generated code is written against the API
FrozenLib itself is built on — every signature was read out of the FrozenLib
source rather than recalled — but *read* is not *compiled*. Expect to fix some
signatures on the first `gradle build`, and expect them to be in `Rt.java`,
which is exactly why everything funnels through it.

Two things in the generated Gradle build are pinned on judgement rather than
knowledge, and the exported `README.md` says so: the `fabric-loom` and
`net.neoforged.moddev` plugin versions, and the coordinate FrozenLib is pulled
from (`maven.modrinth:frozenlib`, with the FrozenBlock maven also configured).

## Known limits

- Dragging a block or value onto the sidebar deletes it, along with everything below
  it in the stack; the sidebar turns into a "Drag here to delete" zone while you
  carry one. The block itself is still clipped at the canvas edge on the way in,
  because blockstitch draws it inside the canvas.
- Stitchcraft describes behaviour, not art. Models point at texture paths you
  fill in, and declared entities get a `NoopRenderer` — enough not to crash,
  invisible in game. Every export lists what is missing.
- `while` loops are bounded (`Rt.LOOP_LIMIT`). A handler runs inside the server
  tick, so an unterminated loop would hang the world rather than just spin.
- Variables, lists and dicts are saved server-wide under
  `<game dir>/stitchcraft/<mod id>.json`, not per world — a canvas has no notion
  of which save it belongs to.
- Custom blocks support inputs but not branch callbacks; blockstitch-core's
  `BlockPiece::Branch` is unused.
