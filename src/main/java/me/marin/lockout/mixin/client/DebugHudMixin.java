package me.marin.lockout.mixin.client;

import com.google.common.collect.Maps;
import me.marin.lockout.LockoutConfig;
import net.minecraft.ChatFormatting;
import net.minecraft.SharedConstants;
import net.minecraft.client.ClientBrandRetriever;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.DebugScreenOverlay;
import net.minecraft.client.gui.components.debug.DebugScreenDisplayer;
import net.minecraft.client.gui.components.debug.DebugScreenEntries;
import net.minecraft.client.gui.components.debug.DebugScreenEntry;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.ServerChunkCache;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.HitResult;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

import java.util.*;

@Mixin(DebugScreenOverlay.class)
public abstract class DebugHudMixin {

    @Unique
    private static final Map<Heightmap.Types, String> HEIGHTMAP_NAMES = Maps.newEnumMap(Map.of(
            Heightmap.Types.WORLD_SURFACE_WG, "SW",
            Heightmap.Types.WORLD_SURFACE, "S",
            Heightmap.Types.OCEAN_FLOOR_WG, "OW",
            Heightmap.Types.OCEAN_FLOOR, "O",
            Heightmap.Types.MOTION_BLOCKING, "M",
            Heightmap.Types.MOTION_BLOCKING_NO_LEAVES, "ML"
    ));

    @Shadow @Final
    private Minecraft minecraft;

    @Shadow
    private void extractLines(GuiGraphicsExtractor graphics, List<String> lines, boolean alignLeft, int scaledScreenWidth) {}

    @Shadow
    private Level getLevel() { throw new AbstractMethodError("Shadow"); }

    @Shadow
    private LevelChunk getClientChunk() { throw new AbstractMethodError("Shadow"); }

    @Shadow
    private ServerLevel getServerLevel() { throw new AbstractMethodError("Shadow"); }

    @Shadow
    private LevelChunk getServerChunk() { throw new AbstractMethodError("Shadow"); }

    // Replace the left side with Lockout's debug text, and don't draw the right side at all.
    // Lines from entries pinned outside the F3 overlay are hidden as well.
    @Redirect(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/DebugScreenOverlay;extractLines(Lnet/minecraft/client/gui/GuiGraphicsExtractor;Ljava/util/List;ZI)V"))
    private void drawLines(DebugScreenOverlay instance, GuiGraphicsExtractor context, List<String> lines, boolean alignLeft, int scaledScreenWidth) {
        if (alignLeft && this.minecraft.debugEntries.isOverlayVisible()) {
            this.extractLines(context, lockout$getLeftText(), true, scaledScreenWidth);
        }
    }

    @Unique
    private List<String> lockout$getLeftText() {
        List<String> text = new ArrayList<>();

        Entity entity = this.minecraft.getCameraEntity();
        if (entity == null || this.minecraft.level == null) {
            return text;
        }
        BlockPos blockPos = entity.blockPosition();
        Direction direction = entity.getDirection();

        String directionString = switch (direction) {
            case NORTH -> "Towards negative Z";
            case SOUTH -> "Towards positive Z";
            case WEST -> "Towards negative X";
            case EAST -> "Towards positive X";
            default -> "Invalid";
        };

        ChunkPos chunkPos = ChunkPos.containing(blockPos);

        text.add("Minecraft " + SharedConstants.getCurrentVersion().name() + " (" + this.minecraft.getLaunchedVersion() + "/" + ClientBrandRetriever.getClientModName() + ")");
        lockout$addEntryLines(text, DebugScreenEntries.FPS);
        lockout$addEntryLines(text, DebugScreenEntries.CHUNK_RENDER_STATS); // C value
        lockout$addEntryLines(text, DebugScreenEntries.ENTITY_RENDER_STATS); // E value
        text.add("");
        text.add(String.format(Locale.ROOT, "XYZ: %.3f / %.5f / %.3f", entity.getX(), entity.getY(), entity.getZ()));
        text.add(String.format(Locale.ROOT, "Block: %d %d %d", blockPos.getX(), blockPos.getY(), blockPos.getZ()));
        text.add(String.format(Locale.ROOT, "Chunk: %d %d %d in %d %d %d", blockPos.getX() & 15, blockPos.getY() & 15, blockPos.getZ() & 15, chunkPos.x(), SectionPos.blockToSectionCoord(blockPos.getY()), chunkPos.z()));
        text.add(String.format(Locale.ROOT, "Facing: %s (%s) (%.1f / %.1f)", direction, directionString, Mth.wrapDegrees(entity.getYRot()), Mth.wrapDegrees(entity.getXRot())));

        LevelChunk worldChunk = this.getClientChunk();
        if (worldChunk == null || worldChunk.isEmpty()) {
            text.add("Waiting for chunk...");
        } else {
            LevelChunk wc = this.getServerChunk();
            StringBuilder sb = new StringBuilder("SH");
            for (Heightmap.Types typex : Heightmap.Types.values()) {
                if (typex.keepAfterWorldgen()) {
                    sb.append(" ").append(HEIGHTMAP_NAMES.get(typex)).append(": ");
                    if (wc != null) {
                        sb.append(wc.getHeight(typex, blockPos.getX(), blockPos.getZ()));
                    } else {
                        sb.append("??");
                    }
                }
            }
            text.add(sb.toString());

            Holder<Biome> biome = this.minecraft.level.getBiome(blockPos);
            text.add("Biome: " + biome.unwrap().map(key -> key.identifier().toString(), l -> "[unregistered " + l + "]"));
        }

        if (LockoutConfig.getInstance().showNoiseRouterLine) {
            ServerLevel serverWorld = this.getServerLevel();
            if (serverWorld != null) {
                ServerChunkCache serverChunkManager = serverWorld.getChunkSource();
                ChunkGenerator chunkGenerator = serverChunkManager.getGenerator();
                RandomState noiseConfig = serverChunkManager.randomState();
                chunkGenerator.addDebugScreenInfo(text, noiseConfig, blockPos, SamplerContext.builder().enableCaches().build());
            }
        }

        HitResult blockHit = entity.pick(20.0, 0.0F, false);
        if (blockHit.getType() == HitResult.Type.BLOCK) {
            blockPos = ((BlockHitResult) blockHit).getBlockPos();
            BlockState blockState = this.minecraft.level.getBlockState(blockPos);
            text.add("");
            text.add(ChatFormatting.UNDERLINE + "Targeted Block: " + blockPos.getX() + ", " + blockPos.getY() + ", " + blockPos.getZ());
            text.add(String.valueOf(BuiltInRegistries.BLOCK.getKey(blockState.getBlock())));

            blockState.getValues().forEach(value -> text.add(lockout$getPropertyValueString(value)));
        }

        HitResult fluidHit = entity.pick(20.0, 0.0F, true);
        if (fluidHit.getType() == HitResult.Type.BLOCK) {
            blockPos = ((BlockHitResult) fluidHit).getBlockPos();
            FluidState fluidState = this.minecraft.level.getFluidState(blockPos);
            text.add("");
            text.add(ChatFormatting.UNDERLINE + "Targeted Fluid: " + blockPos.getX() + ", " + blockPos.getY() + ", " + blockPos.getZ());
            text.add(String.valueOf(BuiltInRegistries.FLUID.getKey(fluidState.getType())));

            fluidState.getValues().forEach(value -> text.add(lockout$getPropertyValueString(value)));
        }

        Entity targetedEntity = this.minecraft.crosshairPickEntity;
        if (targetedEntity != null) {
            text.add("");
            text.add(ChatFormatting.UNDERLINE + "Targeted Entity");
            text.add(String.valueOf(BuiltInRegistries.ENTITY_TYPE.getKey(targetedEntity.getType())));
        }

        return text;
    }

    // Adds all lines of a vanilla debug entry, regardless of whether it's enabled.
    @Unique
    private void lockout$addEntryLines(List<String> text, Identifier id) {
        DebugScreenEntry entry = DebugScreenEntries.getEntry(id);
        if (entry == null) return;

        entry.display(new DebugScreenDisplayer() {
            @Override
            public void addPriorityLine(String line) {
                text.add(line);
            }

            @Override
            public void addLine(String line) {
                text.add(line);
            }

            @Override
            public void addToGroup(Identifier group, Collection<String> lines) {
                text.addAll(lines);
            }

            @Override
            public void addToGroup(Identifier group, String line) {
                text.add(line);
            }
        }, this.getLevel(), this.getClientChunk(), this.getServerChunk());
    }

    @Unique
    private static String lockout$getPropertyValueString(Property.Value<?> entry) {
        String valueString = entry.valueName();
        if (Boolean.TRUE.equals(entry.value())) {
            valueString = ChatFormatting.GREEN + valueString;
        } else if (Boolean.FALSE.equals(entry.value())) {
            valueString = ChatFormatting.RED + valueString;
        }
        return entry.property().getName() + ": " + valueString;
    }

}
