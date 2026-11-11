# ![Logo](assets/icon.svg) Ludusavi Plus

> **This is a fork** of [mtkennerly/ludusavi](https://github.com/mtkennerly/ludusavi).
> All original credit goes to Matthew T. Kennerly and contributors.
> This fork adds Steam AppID-based game detection for custom ("other") root directories.

## 本 fork 改动说明

### 自定义根目录的 Steam AppID 自动检测

原版 Ludusavi 对自定义根目录（类型"其他"）下的游戏，是通过文件夹名与 manifest 的 `installDir` 做模糊匹配来识别的。当文件夹名与官方命名差异较大时（如 `Fate Samurai Remnant` vs manifest 里的 `FateSamuraiRemnant`），或者是 Goldberg 模拟器打包的中文/破解版文件夹名，就会匹配失败。

**本 fork 新增 `detect_steam_appid()`**，采用 5 层 fallback 检测：

1. **直接文件**：`<游戏目录>/steam_appid.txt`
2. **steam_settings 子目录**：`<游戏目录>/steam_settings/steam_appid.txt`
3. **递归搜索**：向下最多 8 层查找 `steam_appid.txt`，跳过隐藏目录（`.`开头）和 `$` 开头的目录
4. **ini 配置文件**：向上搜索 5 层目录、向下递归搜索 6 层，识别 `SteamConfig.ini`、`SmartSteamEmu.ini`、`flt.ini` 中的 `AppID=123456` 格式，同时检查每层的 `steam_appid.txt` 和 `steam_settings/steam_appid.txt`
5. **路径末尾数字**：如果文件夹名以数字结尾（如 `game/2288630`），取该数字作为 AppID

检测到 AppID 后，通过 manifest 的 `steam.id` 字段反查官方游戏条目，正确解析 `<base>`/`<game>` 路径占位符。

修改文件：`src/scan/launchers/generic.rs`

### 游戏名归一化

改进 `find_one_by_name`，自动剥除非 ASCII 字符的括号副标题（如 `"STAR OCEAN THE SECOND STORY R (星之海洋2 第二个故事R)"` → `"STAR OCEAN THE SECOND STORY R"`），并默认启用归一化匹配。

修改文件：`src/scan/title.rs`

### 命令行新增 `--steam-id` 参数

给 `backup` 和 `restore` 子命令添加 `--steam-id <ID>` 参数。当名字匹配失败时，通过 AppID 反查 manifest 兜底。

修改文件：`src/cli/parse.rs`、`src/cli.rs`

---

---

Ludusavi is a tool for backing up your PC video game save data,
written in [Rust](https://www.rust-lang.org).
It is cross-platform and supports multiple game stores.

## Features
* Ability to back up data from more than 19,000 games plus your own custom entries.
* Backup and restore for Steam, GOG, Epic, Heroic, Lutris, and other game libraries.
* Both a graphical interface and command line interface for scripting.
  Tab completion is available for Bash, Fish, Zsh, PowerShell, and Elvish.
* Support for:
  * Saves that are stored as files and in the Windows registry.
  * Proton saves with Steam.
  * Steam screenshots.
* Available as a [Playnite](https://playnite.link) extension:
  https://github.com/mtkennerly/ludusavi-playnite
* Works on the Steam Deck.

This tool uses the [Ludusavi Manifest](https://github.com/mtkennerly/ludusavi-manifest)
for info on what to back up for each game.
The data is primarily sourced from [PCGamingWiki](https://www.pcgamingwiki.com/wiki/Home),
so please contribute any new or fixed data back to the wiki itself,
and your improvements will be incorporated into Ludusavi's data as well.

If you'd like to help translate Ludusavi into other languages,
[check out the Crowdin project](https://crowdin.com/project/ludusavi).

## Demo
<!-- These anchors are kept for compatibility with old section headers. -->
<a name="gui"></a>

> ![GUI demo of previewing a backup](docs/demo-gui.gif)

## Installation
<!-- These anchors are kept for compatibility with old section headers. -->
<a name="requirements"></a>
<a name="methods"></a>

Download the executable for Windows, Linux, or Mac from the
[releases page](https://github.com/mtkennerly/ludusavi/releases).
It's portable, so you can simply download it and put it anywhere on your system.
<!--
On Mac, extract the archive and open `Ludusavi.app` like any other application.
The included `ludusavi` link can still be used from the terminal.
-->

If you prefer, Ludusavi is also available via
[Winget, Scoop, Flatpak, and Cargo](docs/help/installation.md).

Note:

* Windows users may see a popup that says
  "Windows protected your PC",
  because Windows does not recognize the program's publisher.
  Click "more info" and then "run anyway" to start the program.
* Mac users may see a popup that says
  "Ludusavi can't be opened because it is from an unidentified developer".
  To allow Ludusavi to run, please refer to [this article](https://support.apple.com/en-us/102445),
  specifically the section on `If you want to open an app [...] from an unidentified developer`.

## Usage
<!-- These anchors are kept for compatibility with old section headers. -->
<a name="backup-exclusions"></a>
<a name="backup-retention"></a>
<a name="backup-structure"></a>
<a name="backup-validation"></a>
<a name="cli-api"></a>
<a name="cloud-backup"></a>
<a name="command-line"></a>
<a name="configuration"></a>
<a name="configuration-file"></a>
<a name="custom-games"></a>
<a name="duplicates"></a>
<a name="environment-variables"></a>
<a name="filter"></a>
<a name="game-launch-wrapping"></a>
<a name="logging"></a>
<a name="redirects"></a>
<a name="roots"></a>
<a name="selective-scanning"></a>
<a name="troubleshooting"></a>

Detailed help documentation is available for several topics.

### General
* [Backup automation](/docs/help/backup-automation.md)
* [Backup exclusions](/docs/help/backup-exclusions.md)
* [Backup retention](/docs/help/backup-retention.md)
* [Backup validation](/docs/help/backup-validation.md)
* [Cloud backup](/docs/help/cloud-backup.md)
* [Custom games](/docs/help/custom-games.md)
* [Duplicates](/docs/help/duplicates.md)
* [Filter](/docs/help/filter.md)
* [Game launch wrapping](/docs/help/game-launch-wrapping.md)
* [Redirects](/docs/help/redirects.md)
* [Roots](/docs/help/roots.md)
* [Selective scanning](/docs/help/selective-scanning.md)
* [Transfer between operating systems](/docs/help/transfer-between-operating-systems.md)

### Interfaces
* [Application folder](/docs/help/application-folder.md)
* [Backup structure](/docs/help/backup-structure.md)
* [Command line](/docs/help/command-line.md)
* [Configuration file](/docs/help/configuration-file.md)
* [Environment variables](/docs/help/environment-variables.md)
* [Logging](/docs/help/logging.md)

### Other
* [Troubleshooting](/docs/help/troubleshooting.md)
* [What if my saves aren't found?](/docs/help/missing-saves.md)

## Community

The community has created some additional resources you may find useful.
Please note that this is not an exhaustive list
and that these projects are not officially affiliated with Ludusavi itself:

* Secondary manifests:
  * https://github.com/BloodShed-Oni/ludusavi-extra-manifests
  * https://github.com/hblamo/ludusavi-emudeck-manifest
  * https://github.com/hvmzx/ludusavi-manifests
    * This has an example of using a scheduled GitHub workflow
      to generate a manifest that adds more paths to the primary manifest's entries.
* Plugins for other applications:
  * Decky Loader on Steam Deck:
    * https://github.com/GedasFX/decky-ludusavi
    * https://github.com/beallio/SDH-Ludusavi
  * LaunchBox: https://github.com/johagan94/ludusavi-launchbox
  * VS Code: https://marketplace.visualstudio.com/items?itemName=claui.ludusavi
* Tools:
  * https://github.com/jose-l-martins/GSM-to-Ludusavi-converter

## Comparison with other tools
There are other excellent backup tools available, but not a singular
cross-platform and cross-store solution:

* [GameSave Manager](https://www.gamesave-manager.com) (as of v3.1.512.0):
  * Only supports Windows.
  * Much slower than Ludusavi. On the same hardware and with default settings,
    an initial scan of the whole system takes 2 minutes in GSM versus 10 seconds in Ludusavi.
    Performing a backup immediately after that scan takes 4 minutes 16 seconds in GSM versus 4.5 seconds in Ludusavi.
    In this test, GSM found 257 games with 2.84 GB, and Ludusavi found 297 games with 2.95 GiB.
  * Closed source, so the community cannot contribute improvements.
  * Interface can be slow or unresponsive.
    For example, when clicking "select all / de-select all", each checkbox has to individually toggle itself.
    With 257 games, this means you end up having to wait around 42 seconds.
  * Minimal command line interface.
  * Can create symlinks for games and game data.
    Ludusavi does not support this.
* [Game Backup Monitor](https://mikemaximus.github.io/gbm-web) (as of v1.2.2):
  * Does not support Mac.
  * Database only covers 577 games (as of 2022-11-16), although it can also import
    the Ludusavi manifest starting in 1.3.1.
  * No command line interface.
  * Can automatically back up saves for a game after you play it.
    Ludusavi can only do that in conjunction with a launcher like Playnite.
* [Gaming Backup Multitool for Linux](https://supremesonicbrazil.gitlab.io/gbml-web) (as of v1.4.0.0):
  * Only supports Linux and Steam.
  * Database is not actively updated. As of 2022-11-16, the last update was 2018-06-05.
  * No command line interface.

## Development
Please refer to [CONTRIBUTING.md](./CONTRIBUTING.md).
