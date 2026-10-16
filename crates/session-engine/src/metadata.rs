use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct GameInfo {
    pub title: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub region: String,
    #[serde(default = "two")]
    pub players: usize,
    #[serde(default)]
    pub play_mode: String,
    #[serde(default)]
    pub sha256: String,
}
fn two() -> usize {
    2
}
pub fn identify(hash: &str, file_name: &str) -> GameInfo {
    let games: Vec<GameInfo> = serde_json::from_str(include_str!("../../../catalog/games.json"))
        .expect("validated game catalog");
    if let Some(game) = games.into_iter().find(|g| g.sha256 == hash) {
        return game;
    }
    let extras = [
        (game_integrations::snow::ROM_SHA256, "Snow Bros."),
        (game_integrations::ice::ROM_SHA256, "Ice Climber"),
        (
            game_integrations::worldcup::ROM_SHA256,
            "Nintendo World Cup",
        ),
    ];
    GameInfo {
        title: extras
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, n)| *n)
            .unwrap_or(file_name.trim_end_matches(".nes"))
            .into(),
        revision: String::new(),
        region: String::new(),
        players: 2,
        play_mode: String::new(),
        sha256: hash.into(),
    }
}
pub fn actions(hash: &str) -> [&'static str; 2] {
    game_integrations::registry::all()
        .iter()
        .find(|task| task.rom_sha256 == hash)
        .map_or(["A button", "B button"], |task| task.actions)
}
pub fn scope(schema: &str) -> &'static str {
    game_integrations::registry::schema(schema)
        .map_or("Limited game task · see model notes", |d| d.scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_labels_follow_every_integration_contract() {
        for task in game_integrations::registry::all() {
            assert_eq!(actions(task.rom_sha256), task.actions);
        }
        assert_eq!(actions("unknown"), ["A button", "B button"]);
    }
    #[test]
    fn both_catalogs_identify_exact_files_and_preserve_player_modes() {
        for raw in [
            include_str!("../../../fixtures/compatibility/top100-targets.json"),
            include_str!("../../../fixtures/compatibility/ign100-targets.json"),
        ] {
            let catalog: Vec<GameInfo> = serde_json::from_str(raw).unwrap();
            for game in catalog.into_iter().filter(|g| !g.sha256.is_empty()) {
                let found = identify(&game.sha256, "unrelated.nes");
                assert_eq!(found.players, game.players, "{}", game.title);
                assert_eq!(found.play_mode, game.play_mode, "{}", game.title);
                assert_ne!(found.title, "unrelated");
            }
        }
        assert_eq!(identify("unknown", "Unknown.nes").title, "Unknown");
    }
}
