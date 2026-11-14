use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use fuzzy_matcher::FuzzyMatcher;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::prelude::INVALID_FILE_CHARS;

use crate::{
    resource::{config::Root, manifest::Manifest},
    scan::launchers::LauncherGame,
};

fn make_fuzzy_matcher() -> fuzzy_matcher::skim::SkimMatcherV2 {
    fuzzy_matcher::skim::SkimMatcherV2::default()
        .ignore_case()
        .score_config(fuzzy_matcher::skim::SkimScoreConfig {
            penalty_case_mismatch: 0,
            ..Default::default()
        })
}

fn fuzzy_match(
    matcher: &fuzzy_matcher::skim::SkimMatcherV2,
    reference: &str,
    candidate: &str,
    ideal: Option<i64>,
) -> Option<i64> {
    if reference == candidate {
        return Some(i64::MAX);
    }

    // A space-consolidating regex would be better, but is too much of a performance hit.
    // Also, this is used for files/folders, so we can always ignore illegal characters.
    let candidate = candidate
        .replace(['_', '-'], " ")
        .replace(INVALID_FILE_CHARS, " ")
        .replace("    ", " ")
        .replace("   ", " ")
        .replace("  ", " ");

    let actual = matcher.fuzzy_match(reference, &candidate);
    if let (Some(ideal), Some(actual)) = (ideal, actual) {
        if actual == ideal {
            return Some(i64::MAX);
        } else if actual > (ideal / 4 * 3) {
            return Some(actual);
        }
    }
    None
}

/// Directories that can never contain an emulator config; skipped during full-tree search.
const FIND_SKIP_DIRS: [&str; 5] = ["node_modules", ".git", ".svn", ".hg", "__MACOSX"];

/// Safety cap for the number of directory entries visited in a full-tree search.
const FIND_MAX_FILES: usize = 200_000;

fn is_skipped_dir(name: &str) -> bool {
    FIND_SKIP_DIRS.iter().any(|s| s.eq_ignore_ascii_case(name))
}

/// Full-tree search by file name, with no depth limit.
/// Mirrors `findFiles` in `detect-steam-game.mjs`: skip known-irrelevant directories,
/// stop after `FIND_MAX_FILES` entries, and never follow symlinked directories.
fn find_files<F>(root: &Path, matches: F) -> Vec<PathBuf>
where
    F: Fn(&str) -> bool,
{
    let mut out = Vec::new();
    let mut scanned = 0usize;

    let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
        if e.file_type().is_dir() {
            if let Some(name) = e.file_name().to_str() {
                return !is_skipped_dir(name);
            }
        }
        true
    });

    for entry in walker.filter_map(|e| e.ok()) {
        scanned += 1;
        if scanned > FIND_MAX_FILES {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            if matches(name) {
                out.push(entry.into_path());
            }
        }
    }

    out
}

fn is_name(name: &str, expected: &str) -> bool {
    name.eq_ignore_ascii_case(expected)
}

/// Read a file that contains a bare numeric AppID (e.g. `steam_appid.txt`).
fn read_appid_number(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    trimmed.parse::<u32>().ok()
}

/// Read `AppId=<number>` from ini text (RUNE style).
fn read_appid_line(text: &str) -> Option<u32> {
    let re = regex::Regex::new(r"(?im)^\s*AppId\s*=\s*(\d{3,8})\s*$").ok()?;
    let caps = re.captures(text)?;
    caps.get(1)?.as_str().parse::<u32>().ok()
}

/// Read an AppID from a config/ini file, accepting `AppID=123`, `AppId = 123` or a bare number line.
fn read_appid_config(text: &str) -> Option<u32> {
    let re = regex::Regex::new(
        r"(?im)(?:^\s*(?:AppID|AppId|appid)\s*=\s*|^\s*)(\d{3,8})\s*$",
    )
    .ok()?;
    let caps = re.captures(text)?;
    caps.get(1)?.as_str().parse::<u32>().ok()
}

/// Shallow shortcut: `steam_appid.txt` in the game directory or its `steam_settings` subfolder.
fn read_appid_in(dir: &Path) -> Option<u32> {
    if let Some(id) = read_appid_number(&dir.join("steam_appid.txt")) {
        return Some(id);
    }
    read_appid_number(&dir.join("steam_settings").join("steam_appid.txt"))
}

/// 1) `steam_appid.txt` anywhere in the tree, preferring the shallowest one.
fn detect_game_appid(game_dir: &Path) -> Option<u32> {
    if let Some(id) = read_appid_in(game_dir) {
        return Some(id);
    }

    let files = find_files(game_dir, |n| is_name(n, "steam_appid.txt"));
    if files.is_empty() {
        return None;
    }

    let mut files = files;
    files.sort_by_key(|p| p.components().count());

    for file in files {
        if let Some(id) = read_appid_number(&file) {
            return Some(id);
        }
    }

    None
}

/// 2) RUNE and similar emulators don't create `steam_appid.txt`;
/// the AppID lives in `steam_emu.ini` / `rune.ini`, possibly buried deep
/// (e.g. UE's `Engine/Binaries/.../Win64/`). Prefer the copy that sits next
/// to `steam_api.dll` / `steam_api64.dll`.
fn read_appid_from_emu_ini(game_dir: &Path) -> Option<u32> {
    let ini_files = find_files(game_dir, |n| is_name(n, "steam_emu.ini") || is_name(n, "rune.ini"));
    if ini_files.is_empty() {
        return None;
    }

    let dll_dirs: HashSet<PathBuf> = find_files(game_dir, |n| {
        is_name(n, "steam_api.dll") || is_name(n, "steam_api64.dll")
    })
    .into_iter()
    .filter_map(|p| p.parent().map(|d| d.to_path_buf()))
    .collect();

    for file in &ini_files {
        if let Some(dir) = file.parent() {
            if dll_dirs.contains(dir) {
                if let Some(id) = fs::read_to_string(file).ok().and_then(|t| read_appid_line(&t)) {
                    return Some(id);
                }
            }
        }
    }

    for file in &ini_files {
        if let Some(id) = fs::read_to_string(file).ok().and_then(|t| read_appid_line(&t)) {
            return Some(id);
        }
    }

    None
}

/// 3) Upstream config files (SmartSteamEmu / SSE / flt), anywhere in the tree.
fn appid_from_config(game_dir: &Path) -> Option<u32> {
    let files = find_files(game_dir, |n| {
        is_name(n, "SteamConfig.ini")
            || is_name(n, "SmartSteamEmu.ini")
            || is_name(n, "steam_appid.txt")
            || is_name(n, "flt.ini")
    });

    for file in files {
        if let Some(id) = fs::read_to_string(&file)
            .ok()
            .and_then(|t| read_appid_config(&t))
        {
            return Some(id);
        }
    }

    None
}

/// 4) Trailing numeric segment of the path (e.g. `.../2288630`),
/// ignoring the SteamEmu stats folders that some emulators append.
fn appid_from_path(game_dir: &Path) -> Option<u32> {
    let mut s = game_dir.to_string_lossy().to_string();
    while s.ends_with('\\') || s.ends_with('/') {
        s.pop();
    }

    for suffix in ["\\stats", "/stats", "\\SteamEmu\\UserStats", "/SteamEmu/UserStats", "\\SteamEmu", "/SteamEmu"] {
        let len = suffix.len();
        if s.len() >= len && s[s.len() - len..].eq_ignore_ascii_case(suffix) {
            s.truncate(s.len() - len);
            break;
        }
    }

    let re = regex::Regex::new(r"(\d+)$").ok()?;
    let caps = re.captures(&s)?;
    caps.get(1)?.as_str().parse::<u32>().ok()
}

/// Detect a Steam AppID from a game install directory.
///
/// Mirrors `detectSteamGame()` from `detect-steam-game.mjs`, which has four fallbacks:
/// 1. `steam_appid.txt` (shallow first, then anywhere in the tree, shallowest wins).
/// 2. `steam_emu.ini` / `rune.ini` (RUNE-style emulators), preferring the one next to `steam_api*.dll`.
/// 3. Other emulator configs (`SteamConfig.ini`, `SmartSteamEmu.ini`, `flt.ini`, `steam_appid.txt`).
/// 4. Trailing number in the path.
fn detect_steam_appid(game_dir: &Path) -> Option<u32> {
    detect_game_appid(game_dir)
        .or_else(|| read_appid_from_emu_ini(game_dir))
        .or_else(|| appid_from_config(game_dir))
        .or_else(|| appid_from_path(game_dir))
}

pub fn scan(root: &Root, manifest: &Manifest, subjects: &[String]) -> HashMap<String, HashSet<LauncherGame>> {
    log::debug!("ranking installations for root: {:?}", root);

    let install_parent = root.games_path();
    let matcher = make_fuzzy_matcher();

    let actual_dirs: Vec<_> = install_parent
        .read_dir()
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter_map(|entry| match entry.file_type() {
                    Ok(ft) if ft.is_dir() => Some(entry.file_name().to_string_lossy().to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    log::debug!("actual install folders: {}", actual_dirs.join(" | "));

    // ---- Phase 1: existing fuzzy name matching ----
    let scores: Vec<_> = subjects
        .into_par_iter()
        .filter_map(|name| {
            let expected_install_dirs = manifest.0[name].install_dir.keys().chain(std::iter::once(name));

            let mut best: Option<(i64, &String)> = None;
            'dirs: for expected_dir in expected_install_dirs {
                log::trace!("[{name}] looking for install dir: {expected_dir}");

                if expected_dir.contains(['/', '\\']) {
                    if root.path().joined(expected_dir).is_dir() {
                        log::trace!("[{name}] using exact nested install dir");
                        best = Some((i64::MAX, expected_dir));
                        break;
                    } else {
                        continue;
                    }
                }

                let ideal = matcher.fuzzy_match(expected_dir, expected_dir);
                for actual_dir in &actual_dirs {
                    let score = fuzzy_match(&matcher, expected_dir, actual_dir, ideal);
                    if let Some(score) = score {
                        if let Some((previous, _)) = best {
                            if score > previous {
                                log::trace!("[{name}] score {score} beats previous {previous}: {actual_dir}");
                                best = Some((score, actual_dir));
                            }
                        } else {
                            log::trace!("[{name}] new score {score}: {actual_dir}");
                            best = Some((score, actual_dir));
                        }
                    } else {
                        // irrelevant
                    }
                    if score == Some(i64::MAX) {
                        break 'dirs;
                    }
                }
            }
            best.map(|(score, subdir)| {
                log::debug!("[{name}] selecting subdir with score {score}: {subdir}");
                (score, name, subdir)
            })
        })
        .collect();

    let mut by_title = HashMap::<String, (i64, String)>::new();
    for (score, name, subdir) in &scores {
        by_title
            .entry(name.to_string())
            .and_modify(|(stored_score, stored_subdir)| {
                if score > stored_score {
                    *stored_score = *score;
                    *stored_subdir = subdir.to_string();
                }
            })
            .or_insert((*score, subdir.to_string()));
    }

    let mut by_subdir = HashMap::<String, Vec<String>>::new();
    for (_score, name, subdir) in &scores {
        by_subdir
            .entry(subdir.to_string())
            .and_modify(|names| names.push(name.to_string()))
            .or_insert_with(|| vec![name.to_string()]);
    }

    let mut result: HashMap<String, HashSet<LauncherGame>> = subjects
        .iter()
        .filter_map(|name| {
            let (score, subdir) = by_title.get(name)?;

            if *score < i64::MAX
                && let Some(competitors) = by_subdir.get(subdir)
            {
                for competitor in competitors {
                    if let Some((competitor_score, _)) = by_title.get(competitor)
                        && competitor_score > score
                    {
                        log::debug!("[{name}] outranked by '{competitor}' for subdir '{subdir}'");
                        return None;
                    }
                }
            }

            Some((
                name.clone(),
                HashSet::from_iter([LauncherGame {
                    install_dir: Some(install_parent.joined(subdir)),
                    prefix: None,
                    platform: None,
                }]),
            ))
        })
        .collect();

    // ---- Phase 2: Steam AppID fallback ----
    // For folders that were NOT claimed by fuzzy matching, try to read steam_appid.txt
    // and reverse-lookup the game from the manifest's steam.id field.
    let claimed_subdirs: HashSet<&String> = by_title.values().map(|(_, s)| s).collect();
    let unclaimed_dirs: Vec<&String> = actual_dirs.iter().filter(|d| !claimed_subdirs.contains(d)).collect();

    if !unclaimed_dirs.is_empty() {
        // Build reverse index: steam AppID -> game name
        let steam_index: HashMap<u32, &String> = manifest
            .0
            .iter()
            .filter_map(|(name, game)| game.steam.id.map(|id| (id, name)))
            .collect();

        if !steam_index.is_empty() {
            log::debug!(
                "AppID fallback: scanning {} unclaimed folders against {} steam-indexed games",
                unclaimed_dirs.len(),
                steam_index.len()
            );

            for actual_dir in unclaimed_dirs {
                let full_path = install_parent.joined(actual_dir);
                let native_path = Path::new(full_path.raw());

                match detect_steam_appid(native_path) {
                    Some(appid) => {
                        log::debug!(
                            "AppID fallback: folder '{}' has AppID {}",
                            actual_dir,
                            appid
                        );
                        if let Some(game_name) = steam_index.get(&appid) {
                            // Only add if the game wasn't already matched by folder name.
                            // This prevents false positives from shared redistributable folders
                            // (e.g. F:\Games\Tools containing a steam_appid.txt for a game
                            // whose main folder was already correctly identified).
                            if result.contains_key(*game_name) {
                                log::trace!(
                                    "AppID fallback: '{}' already matched for '{}', skipping folder '{}'",
                                    game_name,
                                    game_name,
                                    actual_dir
                                );
                                continue;
                            }
                            log::info!(
                                "AppID fallback: matched '{}' -> '{}' (AppID {})",
                                actual_dir,
                                game_name,
                                appid
                            );
                            result
                                .entry((*game_name).clone())
                                .or_default()
                                .insert(LauncherGame {
                                    install_dir: Some(full_path),
                                    prefix: None,
                                    platform: None,
                                });
                        } else {
                            log::debug!(
                                "AppID fallback: folder '{}' AppID {} not found in manifest",
                                actual_dir,
                                appid
                            );
                        }
                    }
                    None => {
                        log::trace!("AppID fallback: no AppID found in '{}'", actual_dir);
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn fuzzy_matching() {
        let matcher = make_fuzzy_matcher();

        for (reference, candidate, output) in vec![
            ("a", "a", Some(i64::MAX)),
            ("a", "b", None),
            ("Something", "Something", Some(i64::MAX)),
            // Too short:
            ("ab", "a", None),
            ("ab", "b", None),
            ("abc", "ab", None),
            // Long enough:
            ("abcd", "abc", Some(71)),
            ("A Fun Game", "a fun game", Some(i64::MAX)),
            ("A Fun Game", "a  fun  game", Some(i64::MAX)),
            ("AFunGame", "A Fun Game", Some(171)),
            ("A_Fun_Game", "A _ Fun _ Game", Some(i64::MAX)),
            ("a-fun-game", "a - fun - game", Some(i64::MAX)),
            ("A FUN GAME", "A FUN GAME", Some(i64::MAX)),
            ("A Fun Game!", "A Fun Game", Some(219)),
            ("A Funner Game", "A Fun Game", Some(209)),
            ("A Fun Game 2", "A Fun Game", Some(219)),
        ] {
            assert_eq!(
                output,
                fuzzy_match(
                    &matcher,
                    reference,
                    candidate,
                    matcher.fuzzy_match(reference, reference)
                )
            );
        }
    }
}
