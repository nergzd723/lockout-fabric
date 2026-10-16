package me.marin.lockout;

import net.minecraft.server.permissions.Permissions;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import me.marin.lockout.lockout.DefaultGoalRegister;
import me.marin.lockout.network.CustomBoardPayload;
import me.marin.lockout.network.Networking;
import me.marin.lockout.server.LockoutServer;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.command.v2.CommandRegistrationCallback;
import net.fabricmc.fabric.api.loot.v3.LootTableEvents;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.Version;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.arguments.GameProfileArgument;
import net.minecraft.core.registries.Registries;
import net.minecraft.world.item.Items;
import net.minecraft.world.item.alchemy.Potions;
import net.minecraft.world.item.enchantment.Enchantments;
import net.minecraft.world.level.storage.loot.BuiltInLootTables;
import net.minecraft.world.level.storage.loot.LootPool;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.level.storage.loot.entries.LootItem;
import net.minecraft.world.level.storage.loot.functions.EnchantRandomlyFunction;
import net.minecraft.world.level.storage.loot.functions.SetItemCountFunction;
import net.minecraft.world.level.storage.loot.functions.SetPotionFunction;
import net.minecraft.core.Holder;
import net.minecraft.world.level.storage.loot.providers.number.ints.ContextIntProvider;
import net.minecraft.world.level.storage.loot.providers.number.ints.ContextIntProviders;
import java.util.Objects;
import java.util.Optional;
import java.util.function.Predicate;

import static me.marin.lockout.Constants.MAX_BOARD_SIZE;
import static me.marin.lockout.Constants.NAMESPACE;

public class LockoutInitializer implements ModInitializer {

    // getServer() is null when the game checks requirements against its placeholder source while building the command tree packet.
    private static final Predicate<CommandSourceStack> PERMISSIONS = (ssc) -> ssc.permissions().hasPermission(Permissions.COMMANDS_GAMEMASTER) || (ssc.getServer() != null && ssc.getServer().isSingleplayer());

    public static Version MOD_VERSION;

    @Override
    public void onInitialize() {
        MOD_VERSION = FabricLoader.getInstance().getModContainer(NAMESPACE).get().getMetadata().getVersion();

        LockoutConfig.load();
        Networking.registerPayloads();
        DefaultGoalRegister.registerGoals();

        CommandRegistrationCallback.EVENT.register((dispatcher, registryAccess, environment) -> {
            {
                {
                    // Lockout command
                    var commandNode = Commands.literal("lockout").requires(PERMISSIONS).build();
                    var teamsNode = Commands.literal("teams").build();
                    var playersNode = Commands.literal("players").build();
                    //TODO make custom argument types
                    var teamListNode = Commands.argument("team names", StringArgumentType.greedyString()).executes(LockoutServer::lockoutCommandLogic).build();
                    var playerListNode = Commands.argument("player names", StringArgumentType.greedyString()).executes(LockoutServer::lockoutCommandLogic).build();

                    dispatcher.getRoot().addChild(commandNode);
                    commandNode.addChild(teamsNode);
                    commandNode.addChild(playersNode);
                    teamsNode.addChild(teamListNode);
                    playersNode.addChild(playerListNode);

                    // Draft: teams pick the board's goals in turns, then the match starts.
                    var draftNode = Commands.literal("draft").build();
                    var draftTeamsNode = Commands.literal("teams").build();
                    var draftPlayersNode = Commands.literal("players").build();
                    var draftTeamListNode = Commands.argument("team names", StringArgumentType.greedyString()).executes(LockoutServer::draftCommandLogic).build();
                    var draftPlayerListNode = Commands.argument("player names", StringArgumentType.greedyString()).executes(LockoutServer::draftCommandLogic).build();
                    var draftCancelNode = Commands.literal("cancel").executes(LockoutServer::cancelDraftCommand).build();

                    commandNode.addChild(draftNode);
                    draftNode.addChild(draftTeamsNode);
                    draftNode.addChild(draftPlayersNode);
                    draftNode.addChild(draftCancelNode);
                    draftTeamsNode.addChild(draftTeamListNode);
                    draftPlayersNode.addChild(draftPlayerListNode);
                }


                {
                    // Blackout command
                    var commandNode = Commands.literal("blackout").requires(PERMISSIONS).build();
                    var teamNode = Commands.literal("team").build();
                    var playersNode = Commands.literal("players").build();
                    //TODO make custom argument types
                    var teamNameNode = Commands.argument("team name", StringArgumentType.greedyString()).executes(LockoutServer::blackoutCommandLogic).build();
                    var playerListNode = Commands.argument("player names", StringArgumentType.greedyString()).executes(LockoutServer::blackoutCommandLogic).build();

                    dispatcher.getRoot().addChild(commandNode);
                    commandNode.addChild(teamNode);
                    commandNode.addChild(playersNode);
                    teamNode.addChild(teamNameNode);
                    playersNode.addChild(playerListNode);
                }
            }


            {
                // Chat command
                var chatCommandNode = Commands.literal("chat").build();
                var chatTeamNode = Commands.literal("team").executes(context -> LockoutServer.setChat(context, ChatManager.Type.TEAM)).build();
                var chatLocalNode = Commands.literal("local").executes(context -> LockoutServer.setChat(context, ChatManager.Type.LOCAL)).build();

                dispatcher.getRoot().addChild(chatCommandNode);
                chatCommandNode.addChild(chatTeamNode);
                chatCommandNode.addChild(chatLocalNode);
            }


            {
                // GiveGoal command
                var giveGoalRoot = Commands.literal("GiveGoal").requires(PERMISSIONS).build();
                var playerName = Commands.argument("player name", GameProfileArgument.gameProfile()).build();
                var goalIndex = Commands.argument("goal number", IntegerArgumentType.integer(1, MAX_BOARD_SIZE * MAX_BOARD_SIZE)).executes(LockoutServer::giveGoal).build();

                dispatcher.getRoot().addChild(giveGoalRoot);
                giveGoalRoot.addChild(playerName);
                playerName.addChild(goalIndex);
            }

            {
                // SetStartTime command
                var setStartTimeRoot = Commands.literal("SetStartTime").requires(PERMISSIONS).build();
                var seconds = Commands.argument("seconds", IntegerArgumentType.integer(5, 300)).executes(LockoutServer::setStartTime).build();

                dispatcher.getRoot().addChild(setStartTimeRoot);
                setStartTimeRoot.addChild(seconds);
            }

            {
                // RemoveCustomBoard command (SetCustomBoard is registered in LockoutClient, and server listens for a packet)

                dispatcher.getRoot().addChild(Commands.literal("RemoveCustomBoard").requires(PERMISSIONS).executes((context) -> {
                    ClientPlayNetworking.send(new CustomBoardPayload(Optional.empty()));
                    return 1;
                }).build());
            }

            {
                // SetDraftSettings command

                var setDraftSettingsRoot = Commands.literal("SetDraftSettings").requires(PERMISSIONS).build();
                var offerSize = Commands.argument("offer size", IntegerArgumentType.integer(1, 5)).build();
                var pickSeconds = Commands.argument("pick seconds", IntegerArgumentType.integer(3, 120)).build();
                var rerolls = Commands.argument("rerolls", IntegerArgumentType.integer(0, 10)).executes(LockoutServer::setDraftSettings).build();

                dispatcher.getRoot().addChild(setDraftSettingsRoot);
                setDraftSettingsRoot.addChild(offerSize);
                offerSize.addChild(pickSeconds);
                pickSeconds.addChild(rerolls);
            }

            {
                // SetBoardSize command

                var setBoardTimeRoot = Commands.literal("SetBoardSize").requires(PERMISSIONS).build();
                var size = Commands.argument("board size", IntegerArgumentType.integer(3, 7)).executes(LockoutServer::setBoardSize).build();

                dispatcher.getRoot().addChild(setBoardTimeRoot);
                setBoardTimeRoot.addChild(size);
            }

        });

        LootTableEvents.REPLACE.register(((key, original, source, registries) -> {
            if (Objects.equals(key, BuiltInLootTables.PIGLIN_BARTERING)) {
                Holder<ContextIntProvider> ironNuggetsCount = ContextIntProviders.between(9, 36);
                Holder<ContextIntProvider> quartzCount = ContextIntProviders.between(8, 16);
                Holder<ContextIntProvider> glowstoneDustCount = ContextIntProviders.between(5, 12);
                Holder<ContextIntProvider> magmaCreamCount = ContextIntProviders.between(2, 6);
                Holder<ContextIntProvider> enderPearlCount = ContextIntProviders.between(4, 8);
                Holder<ContextIntProvider> stringCount = ContextIntProviders.between(8, 24);
                Holder<ContextIntProvider> fireChargeCount = ContextIntProviders.between(1, 5);
                Holder<ContextIntProvider> gravelCount = ContextIntProviders.between(8, 16);
                Holder<ContextIntProvider> leatherCount = ContextIntProviders.between(4, 10);
                Holder<ContextIntProvider> netherBrickCount = ContextIntProviders.between(4, 16);
                Holder<ContextIntProvider> cryingObsidianCount = ContextIntProviders.between(1, 3);
                Holder<ContextIntProvider> soulSandCount = ContextIntProviders.between(4, 16);

                LootPool pool = LootPool.lootPool()
                        .add(LootItem.lootTableItem(Items.BOOK).apply(EnchantRandomlyFunction.randomEnchantment().withEnchantment(registries.lookupOrThrow(Registries.ENCHANTMENT).getOrThrow(Enchantments.SOUL_SPEED))).setWeight(5))
                        .add(LootItem.lootTableItem(Items.IRON_BOOTS).apply(EnchantRandomlyFunction.randomEnchantment().withEnchantment(registries.lookupOrThrow(Registries.ENCHANTMENT).getOrThrow(Enchantments.SOUL_SPEED))).setWeight(8))
                        .add(LootItem.lootTableItem(Items.POTION).apply(SetPotionFunction.setPotion(Potions.FIRE_RESISTANCE)).setWeight(10))
                        .add(LootItem.lootTableItem(Items.SPLASH_POTION).apply(SetPotionFunction.setPotion(Potions.FIRE_RESISTANCE)).setWeight(10))
                        .add(LootItem.lootTableItem(Items.IRON_NUGGET).apply(SetItemCountFunction.setCount(ironNuggetsCount)).setWeight(10))
                        .add(LootItem.lootTableItem(Items.QUARTZ).apply(SetItemCountFunction.setCount(quartzCount)).setWeight(20))
                        .add(LootItem.lootTableItem(Items.GLOWSTONE_DUST).apply(SetItemCountFunction.setCount(glowstoneDustCount)).setWeight(20))
                        .add(LootItem.lootTableItem(Items.MAGMA_CREAM).apply(SetItemCountFunction.setCount(magmaCreamCount)).setWeight(20))
                        .add(LootItem.lootTableItem(Items.ENDER_PEARL).apply(SetItemCountFunction.setCount(enderPearlCount)).setWeight(20))
                        .add(LootItem.lootTableItem(Items.STRING).apply(SetItemCountFunction.setCount(stringCount)).setWeight(20))
                        .add(LootItem.lootTableItem(Items.FIRE_CHARGE).apply(SetItemCountFunction.setCount(fireChargeCount)).setWeight(40))
                        .add(LootItem.lootTableItem(Items.GRAVEL).apply(SetItemCountFunction.setCount(gravelCount)).setWeight(40))
                        .add(LootItem.lootTableItem(Items.LEATHER).apply(SetItemCountFunction.setCount(leatherCount)).setWeight(40))
                        .add(LootItem.lootTableItem(Items.NETHER_BRICK).apply(SetItemCountFunction.setCount(netherBrickCount)).setWeight(40))
                        .add(LootItem.lootTableItem(Items.OBSIDIAN).setWeight(40))
                        .add(LootItem.lootTableItem(Items.CRYING_OBSIDIAN).apply(SetItemCountFunction.setCount(cryingObsidianCount)).setWeight(40))
                        .add(LootItem.lootTableItem(Items.SOUL_SAND).apply(SetItemCountFunction.setCount(soulSandCount)).setWeight(40))
                        .build();
                return LootTable.lootTable().pool(pool).build();
            }
            return null;
        }));

    }

}
