use ircgm::prelude::*;

use super::oracle::ORACLE;
use crate::board::{BoardCards, OracleOmen, Roster};
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, Team, Wake};

pub struct Psychic;
pub static PSYCHIC: Psychic = Psychic;

impl Role for Psychic {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "psychic",
            "Psychic",
            GameSet::Alien,
            Team::Village,
            "View 1 neighbor's card (or BOTH neighbors' cards if the Oracle amplified signals tonight!).",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has(&ORACLE) {
            self.info().summary.to_owned()
        } else {
            "Wake in Night to view your left neighbor's card!".to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::passive(159, Self::inspect).immediate_on_copy())
    }
}

impl Psychic {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        let (left, right) = ctx.roster.neighbors(seat);
        let mut candidates: Vec<usize> = match board.oracle_omen {
            OracleOmen::AmplifiedSignals => vec![left, right],
            OracleOmen::Normal => vec![left],
            OracleOmen::QuantumStatic => vec![right],
        };
        candidates.dedup();
        candidates.retain(|&s| !board.is_shielded(s));

        if candidates.is_empty() {
            ctx.dm(seat, "🧠 Psychic: Neighboring minds were shielded tonight!");
            ctx.record(irc_format!(
                "🧠 {actor_label} ({}) found neighboring cards shielded.",
                actor_nick.bold(),
            ));
            return;
        }

        board.record_card_interaction(seat);
        let mut seen_parts = Vec::new();
        for &target in &candidates {
            let seen = board.players[target];
            let target_nick = ctx.nick(target);
            ctx.dm(
                seat,
                irc_format!(
                    "🧠 Psychic Vision: {} holds {}!",
                    target_nick.as_str().bold(),
                    seen.display_for_peek(),
                ),
            );
            seen_parts.push(irc_format!(
                "{} ({})",
                target_nick.bold(),
                seen.display_for_peek()
            ));
        }
        ctx.record(irc_format!(
            "🧠 {actor_label} ({}) psychically read {}.",
            actor_nick.bold(),
            Text::join(seen_parts, ", "),
        ));
    }
}
