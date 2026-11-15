use super::{find_entry, EnabledGames};

#[test]
fn enabled_games_parses_all_aliases_and_deduplicates() {
    let all = EnabledGames::from_specs::<&str>(&[]).unwrap();
    assert_eq!(all.slugs(), &["onuw"]);

    let by_alias = EnabledGames::from_specs(&["werewolf, onuw"]).unwrap();
    assert_eq!(by_alias.slugs(), &["onuw"]);

    let explicit_all = EnabledGames::from_specs(&["all"]).unwrap();
    assert_eq!(explicit_all, EnabledGames::all());
}

#[test]
fn enabled_games_rejects_empty_or_unknown_specs() {
    let empty_err = EnabledGames::from_specs(&[" , "]).unwrap_err();
    assert!(empty_err.contains("must enable at least one game"));

    let unknown_err = EnabledGames::from_specs(&["onuw,chess"]).unwrap_err();
    assert!(unknown_err.contains("unknown game `chess`"));
}

#[test]
fn find_entry_matches_slug_and_alias_case_insensitively() {
    assert_eq!(find_entry("ONUW").unwrap().slug, "onuw");
    assert_eq!(find_entry("!werewolf").unwrap().slug, "onuw");
    assert!(find_entry("nonexistent").is_none());
}
