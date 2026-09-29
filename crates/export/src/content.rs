//! The content classes: the block, item and mob types the registry registers.
//!
//! FrozenLib has a cross-platform event for most of what a canvas hooks, but not
//! for a block being right-clicked, an item being used, or one of the mod's own
//! mobs ticking - those are not events in the game at all, they are methods on
//! the content itself. So these classes override them and call into
//! `Handlers`'s dispatch tables, which is the other half of `handlers.rs`.
//!
//! Every block is a `ScriptedBlock` and every declared item a `ScriptedItem`,
//! whether or not a hook targets them: the table lookup for something nothing
//! listens to simply misses, and one class per kind reads better than two.

use crate::naming::Names;
use crate::runtime::banner;
use crate::writer::Source;
use stitchcraft_blocks::kinds::EntityBase;
use stitchcraft_blocks::{McBlock, ModProject};

/// Every content file a document needs. Mob classes are only written for the
/// bases the canvas actually declares.
pub fn files(project: &ModProject, names: &Names) -> Vec<(String, String)> {
    let mut out = vec![
        (names.common("content/ScriptedBlock.java"), block(names)),
        (names.common("content/ScriptedItem.java"), item(names)),
    ];
    let bases = declared_bases(project);
    if !bases.is_empty() {
        out.push((names.common("content/ScriptedMob.java"), mob_base(names)));
    }
    for base in bases {
        let class = crate::registry::mob_class(base);
        out.push((
            names.common(&format!("content/{class}.java")),
            mob(names, base),
        ));
    }
    out
}

/// The distinct mob bases the canvas declares, in declaration order.
pub fn declared_bases(project: &ModProject) -> Vec<EntityBase> {
    let mut out = Vec::new();
    for (_, header) in crate::handlers::declarations(project) {
        if let McBlock::RegisterEntity { base, .. } = header
            && !out.contains(base)
        {
            out.push(*base);
        }
    }
    out
}

fn block(names: &Names) -> String {
    let mut s = Source::new();
    s.line(format!("package {};", names.content_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "Every block the canvas declares.\n\nThe overrides here are the hooks \
         FrozenLib has no event for - being right-clicked, being placed, and the \
         random tick - dispatched by registry id through `Handlers`.",
    );
    s.lines([
        "import net.minecraft.core.BlockPos;",
        "import net.minecraft.core.registries.BuiltInRegistries;",
        "import net.minecraft.server.level.ServerLevel;",
        "import net.minecraft.server.level.ServerPlayer;",
        "import net.minecraft.util.RandomSource;",
        "import net.minecraft.world.InteractionResult;",
        "import net.minecraft.world.entity.player.Player;",
        "import net.minecraft.world.level.Level;",
        "import net.minecraft.world.level.block.Block;",
        "import net.minecraft.world.level.block.state.BlockState;",
        "import net.minecraft.world.phys.BlockHitResult;",
        format!("import {}.Ctx;", names.script_package()).as_str(),
        format!("import {}.Handlers;", names.script_package()).as_str(),
    ]);
    s.blank();
    s.braced("public class ScriptedBlock extends Block", |s| {
        s.braced("public ScriptedBlock(Properties properties)", |s| {
            s.line("super(properties);");
        });
        s.blank();
        s.doc(
            "This block's registry id, which is the key its hooks are stored \
             under. Read rather than stored, because the block is built before it \
             is registered.",
        );
        s.braced("protected String registryId()", |s| {
            s.line("return BuiltInRegistries.BLOCK.getKey(this).toString();");
        });
        s.blank();
        s.line("@Override");
        s.braced(
            "protected InteractionResult useWithoutItem(BlockState state, Level level, BlockPos pos, Player player, BlockHitResult hit)",
            |s| {
                s.comment(
                    "The client half of a right-click has nothing to run: the \
                     handlers are server-side, and SUCCESS keeps the swing \
                     animation.",
                );
                s.line("if (!(level instanceof ServerLevel serverLevel)) return InteractionResult.SUCCESS;");
                s.line("final Ctx ctx = Ctx.of(serverLevel, pos, state).withPlayer(player instanceof ServerPlayer server ? server : null);");
                s.open("if (!Handlers.dispatch(Handlers.BLOCK_USED, this.registryId(), ctx)) {");
                s.line("return InteractionResult.FAIL;");
                s.close("}");
                s.line("return InteractionResult.SUCCESS;");
            },
        );
        s.blank();
        s.line("@Override");
        s.braced(
            "protected void onPlace(BlockState state, Level level, BlockPos pos, BlockState replaced, boolean movedByPiston)",
            |s| {
                s.line("super.onPlace(state, level, pos, replaced, movedByPiston);");
                s.comment(
                    "`onPlace` has no player to hand on, which is why the \
                     \"when placed\" hook offers no player context.",
                );
                s.open("if (level instanceof ServerLevel serverLevel) {");
                s.line("Handlers.dispatch(Handlers.BLOCK_PLACED, this.registryId(), Ctx.of(serverLevel, pos, state));");
                s.close("}");
            },
        );
        s.blank();
        s.line("@Override");
        s.braced(
            "protected void randomTick(BlockState state, ServerLevel level, BlockPos pos, RandomSource random)",
            |s| {
                s.line("super.randomTick(state, level, pos, random);");
                s.comment(
                    "Only reached when the declaration turned on random ticking, \
                     which the exporter does when a tick hook targets the block.",
                );
                s.line("Handlers.dispatch(Handlers.BLOCK_TICKED, this.registryId(), Ctx.of(level, pos, state));");
            },
        );
    });
    s.finish()
}

fn item(names: &Names) -> String {
    let mut s = Source::new();
    s.line(format!("package {};", names.content_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "Every item the canvas declares. `use` is the \"when used\" hook; \
         cancelling it stops the vanilla behaviour the item would otherwise have.",
    );
    s.lines([
        "import net.minecraft.core.registries.BuiltInRegistries;",
        "import net.minecraft.server.level.ServerLevel;",
        "import net.minecraft.server.level.ServerPlayer;",
        "import net.minecraft.world.InteractionHand;",
        "import net.minecraft.world.InteractionResult;",
        "import net.minecraft.world.entity.player.Player;",
        "import net.minecraft.world.item.Item;",
        "import net.minecraft.world.level.Level;",
        format!("import {}.Ctx;", names.script_package()).as_str(),
        format!("import {}.Handlers;", names.script_package()).as_str(),
    ]);
    s.blank();
    s.braced("public class ScriptedItem extends Item", |s| {
        s.braced("public ScriptedItem(Properties properties)", |s| {
            s.line("super(properties);");
        });
        s.blank();
        s.line("@Override");
        s.braced(
            "public InteractionResult use(Level level, Player player, InteractionHand hand)",
            |s| {
                s.open("if (level instanceof ServerLevel serverLevel) {");
                s.line("final Ctx ctx = Ctx.of(serverLevel, player.blockPosition()).withPlayer(player instanceof ServerPlayer server ? server : null);");
                s.line("final String id = BuiltInRegistries.ITEM.getKey(this).toString();");
                s.open("if (!Handlers.dispatch(Handlers.ITEM_USED, id, ctx)) {");
                s.line("return InteractionResult.FAIL;");
                s.close("}");
                s.close("}");
                s.line("return super.use(level, player, hand);");
            },
        );
    });
    s.finish()
}

fn mob_base(names: &Names) -> String {
    let mut s = Source::new();
    s.line(format!("package {};", names.content_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "What every declared entity shares: the attribute builder its \
         declaration's numbers go into, and the per-tick dispatch behind the \
         \"every tick of\" hook.\n\nThis is a behaviour shell, not a model. A \
         declared entity has no renderer of its own, so it is invisible in game \
         until you write one - see `ModRenderers` on the client side.",
    );
    s.lines([
        "import net.minecraft.core.registries.BuiltInRegistries;",
        "import net.minecraft.server.level.ServerLevel;",
        "import net.minecraft.world.entity.EntityType;",
        "import net.minecraft.world.entity.Mob;",
        "import net.minecraft.world.entity.PathfinderMob;",
        "import net.minecraft.world.entity.ai.attributes.AttributeSupplier;",
        "import net.minecraft.world.entity.ai.attributes.Attributes;",
        "import net.minecraft.world.level.Level;",
        format!("import {}.Ctx;", names.script_package()).as_str(),
        format!("import {}.Handlers;", names.script_package()).as_str(),
    ]);
    s.blank();
    s.braced("public abstract class ScriptedMob extends PathfinderMob", |s| {
        s.braced(
            "protected ScriptedMob(EntityType<? extends ScriptedMob> type, Level level)",
            |s| {
                s.line("super(type, level);");
            },
        );
        s.blank();
        s.doc(
            "The attributes a declaration's health, speed and attack fields turn \
             into. `ModEntities.registerAttributes` calls this once per type.",
        );
        s.braced(
            "public static AttributeSupplier.Builder attributes(double health, double speed, double attack)",
            |s| {
                s.open("return Mob.createMobAttributes()");
                s.lines([
                    ".add(Attributes.MAX_HEALTH, health)",
                    ".add(Attributes.MOVEMENT_SPEED, speed)",
                    ".add(Attributes.ATTACK_DAMAGE, attack)",
                    ".add(Attributes.FOLLOW_RANGE, 16.0D);",
                ]);
                s.close("");
            },
        );
        s.blank();
        s.line("@Override");
        s.braced("public void tick()", |s| {
            s.line("super.tick();");
            s.open("if (this.level() instanceof ServerLevel serverLevel) {");
            s.line("final String id = BuiltInRegistries.ENTITY_TYPE.getKey(this.getType()).toString();");
            s.line("Handlers.dispatch(Handlers.ENTITY_TICKED, id, Ctx.of(serverLevel, this.blockPosition()).withEntity(this));");
            s.close("}");
        });
    });
    s.finish()
}

fn mob(names: &Names, base: EntityBase) -> String {
    let class = crate::registry::mob_class(base);
    let mut s = Source::new();
    s.line(format!("package {};", names.content_package()));
    s.blank();
    banner(
        &mut s,
        names,
        match base {
            EntityBase::Passive => {
                "A passive entity: it wanders, watches players, and panics when hurt."
            }
            EntityBase::Hostile => {
                "A hostile entity: it wanders, and targets and melees the nearest player."
            }
            EntityBase::Flying => {
                "A flying entity: it drifts through the air rather than walking."
            }
        },
    );
    let mut imports: Vec<&str> = vec![
        "import net.minecraft.world.entity.EntityType;",
        "import net.minecraft.world.entity.ai.goal.FloatGoal;",
        "import net.minecraft.world.entity.ai.goal.LookAtPlayerGoal;",
        "import net.minecraft.world.entity.ai.goal.RandomLookAroundGoal;",
        "import net.minecraft.world.entity.player.Player;",
        "import net.minecraft.world.level.Level;",
    ];
    match base {
        EntityBase::Passive => imports.extend([
            "import net.minecraft.world.entity.ai.goal.PanicGoal;",
            "import net.minecraft.world.entity.ai.goal.WaterAvoidingRandomStrollGoal;",
        ]),
        EntityBase::Hostile => imports.extend([
            "import net.minecraft.world.entity.ai.goal.MeleeAttackGoal;",
            "import net.minecraft.world.entity.ai.goal.WaterAvoidingRandomStrollGoal;",
            "import net.minecraft.world.entity.ai.goal.target.NearestAttackableTargetGoal;",
        ]),
        EntityBase::Flying => imports.extend([
            "import net.minecraft.world.entity.ai.control.FlyingMoveControl;",
            "import net.minecraft.world.entity.ai.goal.WaterAvoidingRandomFlyingGoal;",
            "import net.minecraft.world.entity.ai.navigation.FlyingPathNavigation;",
            "import net.minecraft.world.entity.ai.navigation.PathNavigation;",
        ]),
    }
    imports.sort_unstable();
    s.lines(&imports);
    s.blank();
    s.braced(format!("public class {class} extends ScriptedMob"), |s| {
        s.braced(
            format!("public {class}(EntityType<? extends {class}> type, Level level)"),
            |s| {
                s.line("super(type, level);");
                if base == EntityBase::Flying {
                    s.comment("Flies rather than walks, so it needs both a move control and a navigator.");
                    s.line("this.moveControl = new FlyingMoveControl(this, 20, true);");
                }
            },
        );
        if base == EntityBase::Flying {
            s.blank();
            s.line("@Override");
            s.braced("protected PathNavigation createNavigation(Level level)", |s| {
                s.line("final FlyingPathNavigation navigation = new FlyingPathNavigation(this, level);");
                s.line("navigation.setCanOpenDoors(false);");
                s.line("navigation.setCanFloat(true);");
                s.line("return navigation;");
            });
            s.blank();
            s.line("@Override");
            s.braced("public boolean isFlying()", |s| {
                s.line("return !this.onGround();");
            });
        }
        s.blank();
        s.line("@Override");
        s.braced("protected void registerGoals()", |s| {
            s.line("this.goalSelector.addGoal(0, new FloatGoal(this));");
            match base {
                EntityBase::Passive => {
                    s.line("this.goalSelector.addGoal(1, new PanicGoal(this, 1.25D));");
                    s.line("this.goalSelector.addGoal(2, new WaterAvoidingRandomStrollGoal(this, 1.0D));");
                }
                EntityBase::Hostile => {
                    s.line("this.goalSelector.addGoal(1, new MeleeAttackGoal(this, 1.0D, false));");
                    s.line("this.goalSelector.addGoal(2, new WaterAvoidingRandomStrollGoal(this, 1.0D));");
                }
                EntityBase::Flying => {
                    s.line("this.goalSelector.addGoal(2, new WaterAvoidingRandomFlyingGoal(this, 1.0D));");
                }
            }
            s.line("this.goalSelector.addGoal(3, new LookAtPlayerGoal(this, Player.class, 8.0F));");
            s.line("this.goalSelector.addGoal(4, new RandomLookAroundGoal(this));");
            if base == EntityBase::Hostile {
                s.line("this.targetSelector.addGoal(1, new NearestAttackableTargetGoal<>(this, Player.class, true));");
            }
        });
    });
    s.finish()
}

/// `ModRenderers.java` - the client-side registration a declared entity needs
/// or the game crashes the first time one comes into view.
pub fn renderers_java(project: &ModProject, names: &Names) -> String {
    let declared = crate::registry::Declared::gather(project);
    let mut s = Source::new();
    s.line(format!("package {};", names.content_package()));
    s.blank();
    banner(
        &mut s,
        names,
        "Client-side renderers for the declared entities.\n\nStitchcraft \
         describes behaviour, not models, so each entity gets a `NoopRenderer` - \
         enough for the client not to crash, and invisible in game. Replace a \
         `NoopRenderer` here with your own `EntityRenderer` when you want to see \
         the mob; nothing else in the generated code depends on this file.",
    );
    s.lines([
        "import net.frozenblock.lib.renderer.entity.EntityRendererRegistry;",
        "import net.minecraft.client.renderer.entity.NoopRenderer;",
        format!("import {}.registry.ModEntities;", names.package).as_str(),
    ]);
    s.blank();
    s.braced("public final class ModRenderers", |s| {
        s.line("private ModRenderers() {}");
        s.blank();
        s.braced("public static void register()", |s| {
            if declared.entities.is_empty() {
                s.comment("No entities declared.");
                return;
            }
            for header in &declared.entities {
                let McBlock::RegisterEntity { entity_id, .. } = header else {
                    continue;
                };
                s.line(format!(
                    "EntityRendererRegistry.register(ModEntities.{}, NoopRenderer::new);",
                    crate::naming::constant_name(entity_id.trim())
                ));
            }
        });
    });
    s.finish()
}
