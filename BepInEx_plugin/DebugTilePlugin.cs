using BepInEx;
using HarmonyLib;
using System;
using System.IO;
using System.Text;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using Dorfromantik;

namespace DebugTileGen
{
    [BepInPlugin("com.dorfromantik.debugtile", "Debug Tile Generator", "1.0.0")]
    public class DebugTilePlugin : BaseUnityPlugin
    {
        public static string LogPath;
        public static int StepCounter = 0;
        public static bool ConfigDumped = false;

        void Awake()
        {
            LogPath = Path.Combine(Paths.GameRootPath, "debug_tile.log");
            File.WriteAllText(LogPath, "=== DEBUG TILE GENERATOR ===\n");
            var harmony = new Harmony("com.dorfromantik.debugtile");
            harmony.PatchAll();
            AppendLog("Plugin loaded");
        }

        public static void AppendLog(string line)
        {
            File.AppendAllText(LogPath, line + "\n");
        }

        public static void DumpTileGenConfig(TileGenConfiguration config)
        {
            if (config == null)
            {
                AppendLog("!!! TileGenConfiguration is NULL");
                return;
            }

            var sb = new StringBuilder();
            sb.AppendLine("");
            sb.AppendLine("========== TILE GEN CONFIGURATION DUMP ==========");
            sb.AppendLine($"Config name: {config.name}");

            // ── 1. globalGroupTypeProbabilities ──
            sb.AppendLine("");
            sb.AppendLine("─── globalGroupTypeProbabilities ───");
            if (config.globalGroupTypeProbabilities != null)
            {
                sb.AppendLine($"  Count: {config.globalGroupTypeProbabilities.Count}");
                for (int i = 0; i < config.globalGroupTypeProbabilities.Count; i++)
                {
                    var gtc = config.globalGroupTypeProbabilities[i];
                    string gtId = gtc.groupType?.id.ToString() ?? "NULL";
                    sb.AppendLine($"  [{i}] groupType={gtId} rawProb={gtc.rawProbability} probInPercent={gtc.probabilityInPercent} displayProb={gtc._displayProbability}");
                }
            }
            else
            {
                sb.AppendLine("  NULL");
            }

            // ── 2. segmentPresetCollections ──
            sb.AppendLine("");
            sb.AppendLine("─── segmentPresetCollections ───");
            if (config.segmentPresetCollections != null)
            {
                sb.AppendLine($"  Count: {config.segmentPresetCollections.Count}");
                for (int i = 0; i < config.segmentPresetCollections.Count; i++)
                {
                    var spc = config.segmentPresetCollections[i];
                    sb.AppendLine($"  [{i}] collectionName=\"{spc.collectionName}\"");
                    // Sub: groupTypeProbabilities per collection
                    if (spc.groupTypeProbabilities != null)
                    {
                        sb.AppendLine($"       groupTypeProbabilities ({spc.groupTypeProbabilities.Count}):");
                        foreach (var gtp in spc.groupTypeProbabilities)
                        {
                            string subGtId = gtp.groupType?.id.ToString() ?? "NULL";
                            sb.AppendLine($"         - {subGtId}: rawProb={gtp.rawProbability} probInPercent={gtp.probabilityInPercent}");
                        }
                    }
                    // Sub: segmentPresets per collection
                    if (spc.segmentPresets != null)
                    {
                        sb.AppendLine($"       segmentPresets ({spc.segmentPresets.Count}):");
                        foreach (var spi in spc.segmentPresets)
                        {
                            sb.AppendLine($"         - segmentType={spi.segmentType} possibleTypes={spi.possibleTypes?.Count ?? 0}");
                        }
                    }
                }
            }
            else
            {
                sb.AppendLine("  NULL");
            }

            // ── 3. tilePresetCollections ──
            sb.AppendLine("");
            sb.AppendLine("─── tilePresetCollections ───");
            if (config.tilePresetCollections != null)
            {
                sb.AppendLine($"  Count: {config.tilePresetCollections.Count}");
                for (int i = 0; i < config.tilePresetCollections.Count; i++)
                {
                    var tpc = config.tilePresetCollections[i];
                    sb.AppendLine($"  [{i}] name=\"{tpc.name}\" rawProb={tpc.collectionRawProbability} collectionProb={tpc.collectionProbability}");

                    int tileCount = tpc.tilePresets?.Count ?? 0;
                    int subCount = tpc.subCollections?.Count ?? 0;
                    sb.AppendLine($"       tilePresets={tileCount} subCollections={subCount}");

                    if (tpc.tilePresets != null && tpc.tilePresets.Count > 0)
                    {
                        sb.AppendLine($"       tilePresets:");
                        foreach (var tp in tpc.tilePresets)
                        {
                            sb.AppendLine($"         - \"{tp.name}\" (rawProb={tp.rawProbability} finalProb={tp.tilePresetProbability} occupiedEdges={tp.occupiedEdges})");
                            if (tp.segmentProbabilities != null)
                            {
                                foreach (var spi in tp.segmentProbabilities)
                                {
                                    sb.AppendLine($"             segType={spi.segmentType} possibleTypes={spi.possibleTypes?.Count ?? 0}");
                                }
                            }
                        }
                    }

                    if (tpc.subCollections != null && tpc.subCollections.Count > 0)
                    {
                        for (int j = 0; j < tpc.subCollections.Count; j++)
                        {
                            var sub = tpc.subCollections[j];
                            sb.AppendLine($"       subCollection[{j}]: name=\"{sub.name}\" rawProb={sub.subCollectionRawProbability} subProb={sub.subCollectionProbability} tiles={sub.tilePresets?.Count ?? 0}");
                            if (sub.tilePresets != null)
                            {
                                foreach (var tp in sub.tilePresets)
                                {
                                    sb.AppendLine($"         - \"{tp.name}\" (rawProb={tp.rawProbability} finalProb={tp.tilePresetProbability} occupiedEdges={tp.occupiedEdges})");
                                }
                            }
                        }
                    }
                }
            }
            else
            {
                sb.AppendLine("  NULL");
            }

            // ── 4. allTilePresets ──
            sb.AppendLine("");
            sb.AppendLine("─── allTilePresets (computed) ───");
            if (config.allTilePresets != null)
            {
                sb.AppendLine($"  Count: {config.allTilePresets.Count}");
                for (int i = 0; i < config.allTilePresets.Count; i++)
                {
                    var tp = config.allTilePresets[i];
                    sb.AppendLine($"  [{i}] \"{tp.name}\" prob={tp.tilePresetProbability} occupiedEdges={tp.occupiedEdges}");
                }
            }
            else
            {
                sb.AppendLine("  NULL");
            }

            // ── 5. allSegmentPresets ──
            sb.AppendLine("");
            sb.AppendLine("─── allSegmentPresets (computed) ───");
            if (config.allSegmentPresets != null)
            {
                sb.AppendLine($"  Count: {config.allSegmentPresets.Count}");
                for (int i = 0; i < config.allSegmentPresets.Count; i++)
                {
                    var spi = config.allSegmentPresets[i];
                    if (spi != null)
                    {
                        sb.AppendLine($"  [{i}] segmentType={spi.segmentType} possibleTypes={spi.possibleTypes?.Count ?? 0}");
                        if (spi.possibleTypes != null)
                        {
                            foreach (var pt in spi.possibleTypes)
                            {
                                string gtId = pt.groupType?.id.ToString() ?? "NULL";
                                sb.AppendLine($"       - {gtId}: rawProb={pt.rawProbability} probInPercent={pt.probabilityInPercent}");
                            }
                        }
                    }
                }
            }
            else
            {
                sb.AppendLine("  NULL");
            }

            sb.AppendLine("========== END CONFIGURATION DUMP ==========");
            sb.AppendLine("");

            AppendLog(sb.ToString());
        }
    }

    [HarmonyPatch(typeof(TileGenerator), "GenerateTile")]
    public static class Patch_GenTile
    {
        [HarmonyPrefix]
        public static void Prefix(TileGenerator __instance)
        {
            // Dump config on first call
            if (!DebugTilePlugin.ConfigDumped)
            {
                DebugTilePlugin.ConfigDumped = true;
                var tileConfig = __instance.Configuration;
                DebugTilePlugin.DumpTileGenConfig(tileConfig);
            }

            var t = Traverse.Create(__instance);
            int genCount = t.Field<int>("generatedTileCount").Value;
            int questCount = t.Property<int>("GeneratedQuestCount").Value;
            int seed = t.Property<int>("TileGenerationSeed").Value;
            int step = t.Field<int>("tileSeedIncrementStep").Value;
            DebugTilePlugin.StepCounter++;

            // QuestManager để lấy ActiveQuestCount
            QuestManager qm = t.Field<QuestManager>("questManager").Value;

            var sb = new StringBuilder();
            sb.AppendLine($"=== STEP {DebugTilePlugin.StepCounter} (PREFIX) ===");
            sb.AppendLine($"generatedTileCount={genCount} GeneratedQuestCount={questCount}");
            sb.AppendLine($"TileGenerationSeed={seed} step={step}");

            int num = seed + (genCount - questCount) * step;
            int questSeed = seed + (genCount + 1) * step;
            sb.AppendLine($"num(baseSeed)={num} questCheckSeed={questSeed}");

            // Active quest count
            int activeQuests = (qm != null) ? qm.ActiveQuestCount : -1;
            sb.AppendLine($"ActiveQuestCount={activeQuests}");

            // Quest check
            UnityEngine.Random.InitState(questSeed);
            float qroll = UnityEngine.Random.value;
            sb.AppendLine($"--- QUEST CHECK ---");
            sb.AppendLine($"InitState({questSeed})");
            sb.AppendLine($"UnityEngine.Random.value = {qroll:F10}");

            // Quest probability từ config
            var config = t.Field<TileGenConfiguration>("configuration").Value;
            if (config != null)
            {
                int atLeastTwo = t.Field<int>("atLeastTwoEmptyEdgesForXTurns").Value;
                // Công thức filter: generatedTileCount (sau increment) <= atLeastTwo
                var filter = ((genCount + 1) <= atLeastTwo)
                    ? TileGenFilter.AtLeastTwoEmptyEdges : TileGenFilter.None;
                var presets = config.GetFilteredTilePresets(filter);

                // Preset simulation
                UnityEngine.Random.InitState(num);
                float pv = UnityEngine.Random.value;
                float totalP = presets.Sum(p => p.tilePresetProbability);
                float rp = pv * totalP;
                float cum = 0f;
                string chosen = presets[0].name;
                int chosenIdx = 0;
                for (int i = 0; i < presets.Count; i++)
                {
                    cum += presets[i].tilePresetProbability;
                    if (rp <= cum) { chosen = presets[i].name; chosenIdx = i; break; }
                }
                sb.AppendLine($"--- PRESET CHECK (simulate) ---");
                sb.AppendLine($"InitState({num}) => UnityEngine.Random.value = {pv:F10}");
                sb.AppendLine($"filter={filter} presetCount={presets.Count} totalP={totalP:F6}");
                sb.AppendLine($"roll*P={rp:F6} => \"{chosen}\" (idx {chosenIdx})");

                // Preset list for debugging
                sb.AppendLine($"  PRESET LIST:");
                for (int i = 0; i < presets.Count; i++)
                {
                    string marker = (i == chosenIdx) ? " ***" : "";
                    sb.AppendLine($"    [{i,2}] {presets[i].tilePresetProbability,10:F6} \"{presets[i].name}\"{marker}");
                }
            }

            DebugTilePlugin.AppendLog(sb.ToString());
        }

        [HarmonyPostfix]
        public static void Postfix(TileGenerator __instance, Tile __result)
        {
            var sb = new StringBuilder();
            sb.AppendLine($"--- POSTFIX (Step {DebugTilePlugin.StepCounter}) ---");
            if (__result == null) { sb.AppendLine("result=NULL"); DebugTilePlugin.AppendLog(sb.ToString()); return; }

            bool isQuest = __result is QuestTile;
            sb.AppendLine($"name=\"{__result.name}\" isQuest={isQuest}");

            // Actual segments
            string[] world = new string[6];
            for (int i = 0; i < 6; i++) world[i] = "_";

            if (__result.AllElementGroupSegments != null)
            {
                foreach (var seg in __result.AllElementGroupSegments)
                {
                    if (seg?.GroupType == null) continue;
                    string gt = seg.GroupType.id.ToString();
                    string code = gt.Substring(0, 1);
                    string edges = seg.Edges != null ? string.Join(",", seg.Edges) : "NONE";
                    sb.AppendLine($"seg: [{gt}] code={code} rot={seg.RotationIndex} edges=[{edges}]");
                    foreach (int e in seg.Edges)
                        if (e >= 0 && e < 6) world[e] = code;
                }
            }

            var t = Traverse.Create(__instance);
            int genCount = t.Field<int>("generatedTileCount").Value;
            int questCount = t.Property<int>("GeneratedQuestCount").Value;
            int seed = t.Property<int>("TileGenerationSeed").Value;
            int step = t.Field<int>("tileSeedIncrementStep").Value;
            int num = seed + (genCount - 1 - questCount) * step;

            // QuestManager
            QuestManager qm = t.Field<QuestManager>("questManager").Value;
            int activeQuests = (qm != null) ? qm.ActiveQuestCount : -1;

            sb.AppendLine($"AFTER: genCount={genCount} questCount={questCount} num={num} activeQuests={activeQuests}");
            sb.AppendLine($"World Edges: [{string.Join(" ", world)}]");

            if (!isQuest)
            {
                var config = t.Field<TileGenConfiguration>("configuration").Value;
                if (config != null)
                {
                    int atLeastTwo = t.Field<int>("atLeastTwoEmptyEdgesForXTurns").Value;
                    var filter = (genCount <= atLeastTwo)
                        ? TileGenFilter.AtLeastTwoEmptyEdges : TileGenFilter.None;
                    var presets = config.GetFilteredTilePresets(filter);

                    UnityEngine.Random.InitState(num);
                    float rv = UnityEngine.Random.value;
                    float totalP = presets.Sum(p => p.tilePresetProbability);
                    float rp = rv * totalP;
                    float cum = 0f;
                    string chosenName = presets[0].name;
                    int chosenIdx = 0;
                    for (int i = 0; i < presets.Count; i++)
                    {
                        cum += presets[i].tilePresetProbability;
                        if (rp <= cum) { chosenName = presets[i].name; chosenIdx = i; break; }
                    }
                    sb.AppendLine($"REPLAY: InitState({num}) => value={rv:F10} filter={filter}");
                    sb.AppendLine($"totalP={totalP:F6} roll*P={rp:F6} => \"{chosenName}\" (idx {chosenIdx})");
                    sb.AppendLine($"  PRESET LIST:");
                    for (int i = 0; i < presets.Count; i++)
                    {
                        string marker = (i == chosenIdx) ? " ***" : "";
                        sb.AppendLine($"    [{i,2}] {presets[i].tilePresetProbability,10:F6} \"{presets[i].name}\"{marker}");
                    }
                }
            }

            // Nếu là quest tile, dump object counts
            // End quest details
            DebugTilePlugin.AppendLog(sb.ToString());
        }
    }
}
