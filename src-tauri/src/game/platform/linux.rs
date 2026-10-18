use super::super::model::{DiscoveryLocations, GameSource};
use std::{
    env,
    path::{Path, PathBuf},
};

pub fn locations() -> DiscoveryLocations {
    let home = env::var_os("HOME").map(PathBuf::from);
    let data = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    from_paths(home.as_deref(), data.as_deref())
}

fn from_paths(home: Option<&Path>, data: Option<&Path>) -> DiscoveryLocations {
    let mut result = DiscoveryLocations::default();
    if let Some(data) = data {
        result.steam_roots.push(data.join("Steam"));
    }
    if let Some(home) = home {
        for relative in [
            ".steam/steam",
            ".steam/root",
            ".steam/debian-installation",
            ".local/share/Steam",
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            ".var/app/com.valvesoftware.Steam/data/Steam",
            "snap/steam/common/.local/share/Steam",
            "snap/steam/common/.steam/steam",
        ] {
            result.steam_roots.push(home.join(relative));
        }
        for relative in [
            "GOG Games/Stardew Valley/game",
            "GOG Games/StardewValley/game",
            "GOGGames/StardewValley/game",
            "GOGGames/Stardew Valley/game",
        ] {
            result
                .candidates
                .push((home.join(relative), GameSource::Gog));
        }
        for relative in ["Games/heroic/Stardew Valley", "Games/Heroic/Stardew Valley"] {
            result
                .candidates
                .push((home.join(relative), GameSource::Common));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_xdg_flatpak_snap_and_gog_without_real_environment() {
        let paths = from_paths(Some(Path::new("/home/玩家")), Some(Path::new("/data/apps")));
        assert!(paths
            .steam_roots
            .contains(&PathBuf::from("/data/apps/Steam")));
        assert!(paths.steam_roots.contains(&PathBuf::from(
            "/home/玩家/.var/app/com.valvesoftware.Steam/.local/share/Steam"
        )));
        assert!(paths.steam_roots.contains(&PathBuf::from(
            "/home/玩家/snap/steam/common/.local/share/Steam"
        )));
        assert!(paths
            .candidates
            .iter()
            .any(|(p, _)| p == Path::new("/home/玩家/GOGGames/StardewValley/game")));
        assert!(from_paths(None, None).steam_roots.is_empty());
    }
}
