use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct ParanormalInvestigator;
pub static PARANORMAL_INVESTIGATOR: ParanormalInvestigator = ParanormalInvestigator;

impl Role for ParanormalInvestigator {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "paranormal_investigator",
            "Paranormal Investigator",
            GameSet::Daybreak,
            Team::Village,
            "Look at up to 2 players' cards. If you see an Evil threat or Independent (e.g. Tanner), you STOP immediately and become that role!",
        )
        .aliases(&["pi", "investigator"])
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        format!(
            "Look at up to 2 players' cards. If you see {}, you STOP immediately and become that role!",
            roster.describe_non_village_roles()
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(155, Self::prompt).reads_board().mutates())
    }
}

impl ParanormalInvestigator {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let mut peeked: Vec<usize> = Vec::new();
            let mut became: Option<&'static dyn Role> = None;

            for step in 1..=2 {
                let already = peeked.clone();
                let (roster, shielded) = (ctx.roster, ctx.shielded);
                let prompt = format!(
                    "🕵️ Paranormal Investigator (peek {step}/2): Reply `<nick>` to inspect, or `pass`. Players: {}",
                    roster.format_players_marking_shields(shielded)
                );
                let ans = ctx
                    .ask(prompt, move |m| {
                        let t = m.text.trim();
                        if Roster::is_pass(t) || t.eq_ignore_ascii_case("stop") {
                            return AskMatch::accept(None);
                        }
                        match roster.resolve_nick(t) {
                            Some(idx) if idx == seat => {
                                AskMatch::reject("You cannot inspect your own card!".yellow())
                            }
                            Some(idx) if already.contains(&idx) => AskMatch::reject(
                                "You already inspected that player—pick someone else or `pass`!"
                                    .yellow(),
                            ),
                            Some(idx) if shielded.contains(&idx) => AskMatch::reject(
                                "🛡️ That card is shielded by the Sentinel!".yellow(),
                            ),
                            Some(idx) => AskMatch::accept(Some(idx)),
                            None => AskMatch::reject("Unknown player nickname (or `pass`).".yellow()),
                        }
                    })
                    .await?;

                let Answer::Ok(Some(target)) = ans else {
                    break;
                };
                peeked.push(target);

                let seen_card = ctx.board().players[target];
                let face_role = seen_card.face_role();
                let target_nick = ctx.nick(target);

                if face_role.team() != Team::Village {
                    ctx.say(irc_format!(
                        "😱 {} is {}! You STOP investigating and become {}!",
                        target_nick.bold(),
                        seen_card.display_for_peek(),
                        face_role.styled_name(),
                    ));
                    became = Some(face_role);
                    break;
                }
                ctx.say(irc_format!(
                    "🕵️ {} is {} (Village).",
                    target_nick.bold(),
                    seen_card.display_for_peek(),
                ));
            }

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, &peeked, became, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        peeked: &[usize],
        became: Option<&'static dyn Role>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if !peeked.is_empty() {
            board.record_card_interaction(seat);
        }
        let inspected = ctx.roster.format_indices(peeked);
        if let Some(new_role) = became {
            board.players[seat].transform(new_role);
            ctx.record(irc_format!(
                "🕵️ {actor_label} ({}) inspected {} and transformed into {}!",
                actor_nick.bold(),
                inspected.bold(),
                new_role.styled_name(),
            ));
        } else if peeked.is_empty() {
            ctx.record(irc_format!(
                "🕵️ {actor_label} ({}) did not inspect anyone.",
                actor_nick.bold(),
            ));
        } else {
            ctx.record(irc_format!(
                "🕵️ {actor_label} ({}) inspected {} (all Village).",
                actor_nick.bold(),
                inspected.bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "paranormal_investigator_test.rs"]
mod test;
