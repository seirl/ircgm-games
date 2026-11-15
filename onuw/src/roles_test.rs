use crate::role::GameSet;
use crate::roles::{find_role, roles_in_game, ALL_ROLES};

#[test]
fn all_role_ids_and_names_are_unique_and_resolvable() {
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for &role in ALL_ROLES {
        assert!(ids.insert(role.id()), "duplicate role id `{}`", role.id());
        assert!(
            names.insert(role.name()),
            "duplicate role name `{}`",
            role.name()
        );
        assert_eq!(find_role(role.id()).unwrap().id(), role.id());
        assert_eq!(find_role(role.name()).unwrap().id(), role.id());
        for &alias in role.aliases() {
            assert_eq!(
                find_role(alias).unwrap().id(),
                role.id(),
                "alias `{alias}` did not resolve to `{}`",
                role.id()
            );
        }
    }
    for gs in GameSet::ALL {
        assert!(!roles_in_game(gs).is_empty(), "empty game set {gs:?}");
    }
}
