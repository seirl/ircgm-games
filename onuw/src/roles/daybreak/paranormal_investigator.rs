use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster, SeatSet};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&PARANORMAL_INVESTIGATOR, 155, |s, ctx| {
                    Box::pin(Self::prompt(s, ctx))
                })
                .reads(BoardAccess::PLAYER_FACES)
                .writes(BoardAccess::PLAYER_BECAME),
            ]
        }
    }
}

impl ParanormalInvestigator {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let mut peeked: Vec<usize> = Vec::with_capacity(2);
        let mut already = SeatSet::EMPTY;
        let mut became: Option<&'static dyn Role> = None;

        for step in 1..=2 {
            let players_str = ctx.format_visible_players();
            let view = ctx.view();
            let prompt = irc_format!(
                "🕵️ Paranormal Investigator (peek {step}/2): Reply {} to inspect, or {}. Players: {players_str}",
                "<nick>".bold(),
                "pass".bold(),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    let t = m.text.trim();
                    if Roster::is_pass(t) || t.eq_ignore_ascii_case("stop") {
                        return AskMatch::accept_say(None, "🕵️ Stopped investigating.");
                    }
                    match view.resolve_nick(t) {
                        Some(idx) if idx == seat => {
                            AskMatch::reject("You cannot inspect your own card!".yellow())
                        }
                        Some(idx) if already.contains(idx) => AskMatch::reject(
                            "You already inspected that player—pick someone else or pass!".yellow(),
                        ),
                        Some(idx) if view.is_shielded(idx) => {
                            AskMatch::reject("🛡️ That card is shielded by the Sentinel!".yellow())
                        }
                        Some(idx) => AskMatch::accept(Some(idx)),
                        None => AskMatch::reject("Unknown player nickname (or pass).".yellow()),
                    }
                })
                .await?;

            let Answer::Ok(Some(target)) = ans else {
                break;
            };
            peeked.push(target);
            already.insert(target);

            let seen_card = ctx.peek_player_card(target);
            let visible_role = seen_card.visible_role();
            let target_nick = ctx.nick(target);

            if visible_role.triggers_pi_and_revealer() {
                ctx.say(irc_format!(
                    "😱 {} is {}! You STOP investigating and become {}!",
                    target_nick.bold(),
                    seen_card.display_for_peek(),
                    visible_role.styled_name(),
                ));
                became = Some(visible_role);
                break;
            }
            if visible_role.is_minion() {
                ctx.say(irc_format!(
                    "🕵️ {} is {} (minion — does not transform you).",
                    target_nick.bold(),
                    seen_card.display_for_peek(),
                ));
            } else {
                ctx.say(irc_format!(
                    "🕵️ {} is {} (Village).",
                    target_nick.bold(),
                    seen_card.display_for_peek(),
                ));
            }
        }

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, &peeked, became, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        peeked: &[usize],
        became: Option<&'static dyn Role>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if !peeked.is_empty() {
            board.record_prompt_peek(seat);
        }
        let inspected = ctx.roster.format_seats(peeked.iter().copied());
        if let Some(new_role) = became {
            board.transform_in_place(seat, new_role);
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
