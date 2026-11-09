// detect-steam-game.mjs
// 零依赖：从任意游戏目录识别 Steam AppID（仅此而已，不涉及存档/成就路径）。
// 逻辑搬运自 AW2（emugen.js / discover.js 的 appid 识别部分），
// 但不 import 项目内任何模块，可被任何外部程序调用。
//
// 作为模块使用：
//   import { detectSteamGame } from "./detect-steam-game.mjs";
//   const appid = await detectSteamGame("F:/Games/SomeGame");
//   // → "2288630" 或 null
//
// 作为 CLI 使用：
//   node detect-steam-game.mjs <游戏目录>
//   → 仅打印 appid（无则打印空行）

import path from "node:path";
import fsp from "node:fs/promises";
import { pathToFileURL } from "node:url";

// ============================================================
// AppID 识别（三路 fallback，与 AW2 行为一致）
// ============================================================

// 直接读本目录或 steam_settings 下的 steam_appid.txt
async function readAppidIn(dir) {
  for (const rel of ["steam_appid.txt", path.join("steam_settings", "steam_appid.txt")]) {
    try {
      const txt = (await fsp.readFile(path.join(dir, rel), "utf8")).trim();
      if (/^\d+$/.test(txt)) return txt;
    } catch { /* next */ }
  }
  return null;
}

// 向下最多 4 层递归找 steam_appid.txt（emu 运行的必需品）
async function detectGameAppid(gameDir) {
  const direct = await readAppidIn(gameDir);
  if (direct) return direct;

  async function walk(dir, depth) {
    let entries;
    try { entries = await fsp.readdir(dir, { withFileTypes: true }); } catch { return null; }
    for (const e of entries) {
      if (e.isDirectory()) {
        if (e.name.startsWith(".") || /^\$/.test(e.name) || depth <= 0) continue;
        const hit = await walk(path.join(dir, e.name), depth - 1);
        if (hit) return hit;
        continue;
      }
      if (/^steam_appid\.txt$/i.test(e.name)) {
        try {
          const txt = (await fsp.readFile(path.join(dir, e.name), "utf8")).trim();
          if (/^\d+$/.test(txt)) return txt;
        } catch { /* ignore */ }
      }
    }
    return null;
  }
  return walk(gameDir, 4);
}

// 向上最多 5 层找 ini（SmartSteamEmu / SSE / flt）里的 AppID
async function appidFromConfig(dir) {
  let cur = dir;
  for (let i = 0; i < 5; i++) {
    for (const f of ["SteamConfig.ini", "SmartSteamEmu.ini", "steam_appid.txt", "steam_settings/steam_appid.txt", "flt.ini"]) {
      try {
        const txt = await fsp.readFile(path.join(cur, f), "utf8");
        const m = txt.match(/(?:^\s*(?:AppID|AppId|appid)\s*=\s*|^\s*)(\d{3,8})\s*$/im);
        if (m) return m[1];
      } catch { /* next */ }
    }
    const parent = path.dirname(cur);
    if (parent === cur) break;
    cur = parent;
  }
  return null;
}

// 路径末尾数字段（如 .../2288630）
function appidFromPath(dirPath) {
  const m = dirPath.match(/([0-9]+)$/);
  return m ? m[0] : null;
}

// ============================================================
// 主入口：输入文件夹 → 返回 appid（string | null）
// ============================================================
export async function detectSteamGame(dir) {
  const gameDir = path.resolve(dir);
  let st;
  try { st = await fsp.stat(gameDir); } catch { st = null; }
  if (!st?.isDirectory()) throw new Error(`目录不存在：${gameDir}`);

  const appid = (await detectGameAppid(gameDir))
    || (await appidFromConfig(gameDir))
    || appidFromPath(gameDir)
    || null;
  return appid;
}

// ============================================================
// CLI
// ============================================================
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const target = process.argv[2];
  if (!target) {
    console.error("用法: node detect-steam-game.mjs <游戏目录>");
    process.exit(1);
  }
  try {
    const appid = await detectSteamGame(target);
    if (appid) console.log(appid);
    else { console.error("未能识别到 Steam AppID"); process.exit(2); }
  } catch (e) {
    console.error("错误:", e.message);
    process.exit(1);
  }
}
