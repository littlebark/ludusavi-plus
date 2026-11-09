use std::collections::{HashMap, HashSet};

use crate::{
    path::CommonPath,
    prelude::StrictPath,
    resource::{config::root, manifest::Os},
    scan::{LauncherGame, TitleFinder},
};

mod library {
    use super::*;

    pub const PATH: &str = "games.json";

    #[derive(Debug, serde::Deserialize)]
    pub struct Data(pub Vec<Game>);

    #[derive(Debug, serde::Deserialize)]
    pub struct Game {
        /// ID of the game itself.
        #[serde(rename = "gameid")]
        pub id: Option<String>,
        /// Human-readable.
        pub title: Option<String>,
        /// Path to executable.
        #[serde(rename = "path")]
        pub exe: Option<StrictPath>,
        /// Path to prefix.
        pub prefix: Option<StrictPath>,
    }
}

pub fn scan(root: &root::Faugus, title_finder: &TitleFinder) -> HashMap<String, HashSet<LauncherGame>> {
    log::trace!("Faugus: Scanning root for games: {root:?}");

    let spec_path = root.path.joined(library::PATH);
    let spec = read_spec(&spec_path);
    let games = spec.map(|spec| scan_spec(spec, title_finder)).unwrap_or_default();

    if Os::HOST == Os::Linux
        && let Some(normal_home) = CommonPath::Home.get().map(StrictPath::new)
        && let Some(flatpak_home) = root.flatpak_home()
    {
        log::debug!("Faugus: For Flatpak root {root:?}, translating home {normal_home:?} to {flatpak_home:?}");
        return games
            .into_iter()
            .map(|(title, mut info)| {
                let flatpak: HashSet<_> = info
                    .iter()
                    .map(|info| info.replace_in_paths(&normal_home, &flatpak_home))
                    .collect();

                info.extend(flatpak);

                (title, info)
            })
            .collect();
    }

    log::trace!("Faugus: Finished scanning root for games: {root:?} => {games:?}");

    games
}

fn read_spec(file: &StrictPath) -> Option<library::Data> {
    log::debug!("Faugus: Reading library: {file:?}");

    let content = match file.try_read() {
        Ok(x) => x,
        Err(e) => {
            log::warn!("Faugus: Unable to read library: {file:?} | {e:?}");
            return None;
        }
    };

    log::trace!("Faugus: Library content: {content}");

    match serde_json::from_str::<library::Data>(&content) {
        Ok(x) => Some(x),
        Err(e) => {
            log::warn!("Faugus: Unable to parse library: {file:?} | {e:?}");
            None
        }
    }
}

fn scan_spec(spec: library::Data, title_finder: &TitleFinder) -> HashMap<String, HashSet<LauncherGame>> {
    log::debug!("Faugus: Inspecting library content");

    let mut out = HashMap::<String, HashSet<LauncherGame>>::new();

    for game in spec.0 {
        let library::Game { id, title, exe, prefix } = game;

        let title = match title {
            Some(title) => {
                let Some(title) = title_finder.find_one_by_normalized_name(&title) else {
                    log::info!("Faugus: Ignoring unrecognized title '{title}' for {id:?}");
                    continue;
                };
                title
            }
            None => {
                log::debug!("Faugus: Skipping game {id:?} without title");
                continue;
            }
        };

        let exe = match exe {
            Some(exe) => {
                if exe.is_absolute() {
                    Some(exe)
                } else if let Some(prefix) = prefix.as_ref() {
                    Some(prefix.joined(exe.raw()))
                } else {
                    log::info!("Faugus: {id:?} has relative exe and no prefix");
                    None
                }
            }
            None => {
                log::info!("Faugus: {id:?} has no exe path");
                None
            }
        };

        let install_dir = exe.and_then(|exe| exe.parent_raw());

        let platform = Some(if prefix.is_some() { Os::Windows } else { Os::HOST });

        let entry = LauncherGame {
            install_dir,
            prefix,
            platform,
        };

        out.entry(title)
            .and_modify(|xs| {
                xs.insert(entry.clone());
            })
            .or_insert(HashSet::from_iter([entry]));
    }

    out
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use velcro::{hash_map, hash_set};

    use super::*;
    use crate::{
        resource::{ResourceFile, manifest::Manifest},
        testing::repo,
    };

    fn manifest() -> Manifest {
        Manifest::load_from_string(
            r#"
            Exact Match:
              files:
                <base>/file1.txt: {}
            Fuzzy Match:
              files:
                <base>/file1.txt: {}
            "#,
        )
        .unwrap()
    }

    fn title_finder() -> TitleFinder {
        TitleFinder::new(&Default::default(), &manifest(), Default::default())
    }

    #[test]
    fn scan_finds_nothing_when_folder_does_not_exist() {
        let root = root::Faugus {
            path: format!("{}/tests/nonexistent", repo()).into(),
        };
        let games = scan(&root, &title_finder());
        assert_eq!(HashMap::new(), games);
    }

    #[test]
    fn scan_finds_all_games() {
        let root = root::Faugus {
            path: format!("{}/tests/launchers/faugus", repo()).into(),
        };
        let games = scan(&root, &title_finder());
        assert_eq!(
            hash_map! {
                "Exact Match".to_string(): hash_set![LauncherGame {
                    install_dir: Some(StrictPath::new("/path/to/game".to_string())),
                    prefix: Some(StrictPath::new("/home/user/Faugus/game-title".to_string())),
                    platform: Some(Os::Windows),
                }],
                "Fuzzy Match".to_string(): hash_set![LauncherGame {
                    install_dir: None,
                    prefix: None,
                    platform: Some(Os::HOST),
                }],
            },
            games,
        );
    }

    #[test]
    fn can_scan_spec_with_absolute_exe() {
        let spec = library::Data(vec![library::Game {
            id: Some("exact-match".to_string()),
            title: Some("Exact Match".to_string()),
            exe: Some(StrictPath::new("/install/launcher.exe")),
            prefix: None,
        }]);
        assert_eq!(
            &LauncherGame {
                install_dir: Some(StrictPath::new("/install")),
                prefix: None,
                platform: Some(Os::HOST),
            },
            scan_spec(spec, &title_finder())["Exact Match"].iter().next().unwrap(),
        );
    }

    #[test]
    fn can_scan_spec_with_relative_exe_but_prefix() {
        let spec = library::Data(vec![library::Game {
            id: Some("exact-match".to_string()),
            title: Some("Exact Match".to_string()),
            exe: Some(StrictPath::new("install/launcher.exe")),
            prefix: Some(StrictPath::new("/prefix")),
        }]);
        assert_eq!(
            &LauncherGame {
                install_dir: Some(StrictPath::new("/prefix/install")),
                prefix: Some(StrictPath::new("/prefix")),
                platform: Some(Os::Windows),
            },
            scan_spec(spec, &title_finder())["Exact Match"].iter().next().unwrap(),
        );
    }
}
