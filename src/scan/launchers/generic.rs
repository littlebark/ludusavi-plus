use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

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

/// Read a steam_appid.txt file and parse its contents as a u32 AppID.
fn parse_appid_file(path: &Path) -> Option<u32> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    trimmed.parse::<u32>().ok()
}

/// Try to extract a Steam AppID from an ini/config file in the given directory.
/// Checks: SteamConfig.ini, SmartSteamEmu.ini, steam_appid.txt,
/// steam_settings/steam_appid.txt, flt.ini.
/// Matches patterns like `AppID=123456` or `AppId = 123456` or bare number lines.
fn read_appid_from_ini_at(dir: &Path) -> Option<u32> {
    let patterns = [
        "SteamConfig.ini",
        "SmartSteamEmu.ini",
        "steam_appid.txt",
        "steam_settings/steam_appid.txt",
        "flt.ini",
    ];
    let re = regex::Regex::new(r"(?im)^\s*(?:AppID|AppId|appid)\s*=\s*(\d{3,8})\s*$").ok()?;
    for rel in patterns {
        let full = dir.join(rel);
        if let Ok(text) = fs::read_to_string(&full) {
            // steam_appid.txt is just a number, not ini format
            if rel.ends_with("steam_appid.txt") {
                if let Ok(id) = text.trim().parse::<u32>() {
                    return Some(id);
                }
                continue;
            }
            if let Some(caps) = re.captures(&text) {
                if let Some(m) = caps.get(1) {
                    if let Ok(id) = m.as_str().parse::<u32>() {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

/// Search upward up to 5 levels for ini-based AppID, then downward up to 6 levels.
fn appid_from_config(game_dir: &Path) -> Option<u32> {
    // Search upward
    let mut cur = game_dir.to_path_buf();
    for _ in 0..5 {
        if let Some(id) = read_appid_from_ini_at(&cur) {
            return Some(id);
        }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None => break,
        }
    }
    // Search downward recursively up to 6 levels
    for entry in WalkDir::new(game_dir)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip hidden and $-prefixed directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name.starts_with('$') {
                continue;
            }
        }
        if let Some(id) = read_appid_from_ini_at(path) {
            return Some(id);
        }
    }
    None
}

/// Extract a trailing numeric segment from the path (e.g., .../2288630).
fn appid_from_path(game_dir: &Path) -> Option<u32> {
    let last = game_dir.file_name()?.to_str()?;
    let re = regex::Regex::new(r"(\d+)$").ok()?;
    let caps = re.captures(last)?;
    caps.get(1)?.as_str().parse::<u32>().ok()
}

/// Detect a Steam AppID from a game install directory.
///
/// Mirrors the logic from `detect-steam-game.mjs`:
/// 1. Check `steam_appid.txt` directly in the directory.
/// 2. Check `steam_settings/steam_appid.txt`.
/// 3. Recurse up to 8 levels deep for `steam_appid.txt` (common with emulated releases).
/// 4. Check ini files (SmartSteamEmu/SSE/flt) for AppID — up 5, then down 6.
/// 5. Fallback: trailing number in folder name.
fn detect_steam_appid(game_dir: &Path) -> Option<u32> {
    // 1. Direct file
    let direct = game_dir.join("steam_appid.txt");
    if let Some(id) = parse_appid_file(&direct) {
        return Some(id);
    }

    // 2. steam_settings subfolder
    let settings = game_dir.join("steam_settings").join("steam_appid.txt");
    if let Some(id) = parse_appid_file(&settings) {
        return Some(id);
    }

    // 3. Recurse up to depth 8 (skip hidden dirs and $-prefixed dirs)
    for entry in WalkDir::new(game_dir)
        .max_depth(8)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        // Skip hidden and $-prefixed directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name.starts_with('$') {
                continue;
            }
        }
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_str()?;
        if name.eq_ignore_ascii_case("steam_appid.txt") {
            if let Some(id) = parse_appid_file(path) {
                return Some(id);
            }
        }
    }

    // 4. Ini-based detection (SmartSteamEmu/SSE/flt)
    if let Some(id) = appid_from_config(game_dir) {
        return Some(id);
    }

    // 5. Trailing number in path
    appid_from_path(game_dir)
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
