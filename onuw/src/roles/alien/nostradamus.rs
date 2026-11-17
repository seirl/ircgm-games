use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardCards, BoardKey, BoardMut, Roster, SeatSet};
use crate::marks::Mark;
use crate::role::{
    EffectiveAlignment, GameSet, GroupEffect, GroupPromptCtx, PromptCtx, ResolveCtx, Role,
    RoleInfo, SeatEffect, Team, WakeStep, WinnerFilterCtx, WinnerMap,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct NostradamusPredictions(pub Vec<(&'static str, &'static dyn Role)>);

impl NostradamusPredictions {
    fn get_for_card_face(&self, face_id: &str) -> Option<&'static dyn Role> {
        self.0
            .iter()
            .rev()
            .find(|&&(id, _)| id == face_id)
            .or_else(|| self.0.last())
            .map(|&(_, r)| r)
    }
}

impl BoardKey for NostradamusPredictions {
    const ACCESS: u16 = BoardAccess::CUSTOM_0;

    fn init() -> Self {
        Self(Vec::new())
    }
}

pub struct Nostradamus;
pub static NOSTRADAMUS: Nostradamus = Nostradamus;

const NOSTRADAMUS_PROPHECY_STEP: WakeStep = WakeStep::pair(
    "nostradamus:prophecy",
    "Nostradamus Prophecy",
    Team::Independent,
    None,
    198,
    |seats, ctx| Box::pin(Nostradamus::announce_prophecy(seats, ctx)),
)
.eyes_closed()
.run_when_unheld()
.reads(BoardAccess::CUSTOM_0);

impl Role for Nostradamus {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "nostradamus",
            "Nostradamus",
            GameSet::Alien,
            Team::Independent,
            "Look at up to 2 players' cards (3 at 6+ players): the LAST card you view determines your team (announced publicly at night's end), and you MUST survive to win!",
        )
        .aliases(&["nostra"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&NOSTRADAMUS, 156, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                    .copy_on_turn()
                    .reads(BoardAccess::PLAYER_FACES | BoardAccess::CUSTOM_0)
                    .writes(BoardAccess::CUSTOM_0),
                NOSTRADAMUS_PROPHECY_STEP,
            ]
        }
    }

    fn adjust_alignment(
        &self,
        seat: usize,
        board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        let face_id = board.player_card(seat).face_role().id();
        if let Some(predicted) = board
            .get_key::<NostradamusPredictions>()
            .get_for_card_face(face_id)
        {
            if predicted.team() != Team::Independent {
                return EffectiveAlignment {
                    team: predicted.team(),
                    threat_team: None,
                };
            }
        }
        cur
    }

    fn override_win(
        &self,
        holder: usize,
        ctx: &WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        if ctx.dead_seats.contains(holder) || ctx.board.effective_team(holder) != Team::Independent {
            return;
        }
        let face_id = ctx.board.player_card(holder).face_role().id();
        let Some(predicted) = ctx
            .board
            .get_key::<NostradamusPredictions>()
            .get_for_card_face(face_id)
        else {
            return;
        };
        if predicted.team() == Team::Independent {
            let co_won = ctx
                .board
                .active_holders(predicted)
                .into_iter()
                .any(|s| winners.contains_key(&s));
            if co_won {
                winners.insert(holder, "Nostradamus");
            }
        }
    }

    fn apply_forfeits(
        &self,
        holder: usize,
        ctx: &mut WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        if ctx.dead_seats.contains(holder) && ctx.board.mark(holder) != Mark::Vampire {
            winners.remove(&holder);
        }
    }
}

impl Nostradamus {
    fn format_adopted_banner(role: &'static dyn Role) -> Text {
        if role.team() == Team::Independent {
            role.styled_name()
        } else {
            role.team().style_text(role.team().win_label())
        }
    }

    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let max_peeks = if ctx.roster.len() >= 6 { 3 } else { 2 };
        let mut peeked: Vec<usize> = Vec::with_capacity(max_peeks);
        let mut already = SeatSet::EMPTY;
        let mut last_role: Option<&'static dyn Role> = None;

        for step in 1..=max_peeks {
            let players_str = ctx.roster.format_players();
            let view = ctx.view();
            let prompt = irc_format!(
                "🔭 Nostradamus (peek {step}/{max_peeks}): Reply {} to view a card (your LAST viewed card sets your team!), or {}. Players: {players_str}",
                "<nick>".bold(),
                "pass".bold(),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    let t = m.text.trim();
                    if Roster::is_pass(t) || t.eq_ignore_ascii_case("stop") {
                        return AskMatch::accept_say(None, "🔭 Stopped gazing into the future.");
                    }
                    match view.resolve_nick(t) {
                        Some(idx) if idx == seat => {
                            AskMatch::reject("You cannot view your own card!".yellow())
                        }
                        Some(idx) if already.contains(idx) => AskMatch::reject(
                            "You already viewed that player — pick someone else or pass!".yellow(),
                        ),
                        Some(idx) if view.is_shielded(idx) => {
                            AskMatch::reject("🛡️ That player's card is shielded!".yellow())
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

            let card = ctx.peek_player_card(target);
            let role = card.visible_role();
            last_role = Some(role);

            if role.team() != Team::Village {
                ctx.say(irc_format!(
                    "🔭 {} holds {} (non-Village — you MUST stop looking! Joined {})!",
                    ctx.nick(target).bold(),
                    card.display_for_peek(),
                    Self::format_adopted_banner(role),
                ));
                break;
            }

            ctx.say(irc_format!(
                "🔭 {} holds {} (current destiny: {})!",
                ctx.nick(target).bold(),
                card.display_for_peek(),
                Self::format_adopted_banner(role),
            ));
        }

        if let Some(role) = last_role {
            ctx.say(irc_format!(
                "🔭 Destiny sealed: Nostradamus joins {} (and MUST survive to win)!",
                Self::format_adopted_banner(role),
            ));
        }

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, &peeked, last_role, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        peeked: &[usize],
        last_role: Option<&'static dyn Role>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if !peeked.is_empty() {
            board.record_prompt_peek(seat);
        }
        if let Some(adopted) = last_role {
            let source_face_id = ctx.roster.seats[seat].initial_role.id();
            let mut preds = board.get_key::<NostradamusPredictions>();
            preds.0.push((source_face_id, adopted));
            board.set_key(preds);

            let names = ctx.roster.format_seats(peeked.iter().copied());
            ctx.record(irc_format!(
                "🔭 {actor_label} ({}) viewed {} — joined {}!",
                actor_nick.bold(),
                names.bold(),
                Self::format_adopted_banner(adopted),
            ));
        } else {
            ctx.record(irc_format!(
                "🔭 {actor_label} ({}) viewed no cards.",
                actor_nick.bold(),
            ));
        }
    }

    async fn announce_prophecy(
        _active_seats: SeatSet,
        _ctx: GroupPromptCtx<'_>,
    ) -> Result<GroupEffect> {
        Ok(GroupEffect::new(move |board, rctx| {
            let preds = board.get_key::<NostradamusPredictions>();
            if preds.0.is_empty() {
                let candidates: Vec<&'static dyn Role> = rctx
                    .roster
                    .deck
                    .iter()
                    .copied()
                    .filter(|r| r.id() != "nostradamus")
                    .collect();
                let picked = if candidates.is_empty() {
                    &NOSTRADAMUS as &'static dyn Role
                } else {
                    candidates[fastrand::usize(..candidates.len())]
                };
                let banner = Self::format_adopted_banner(picked);
                rctx.announce(irc_format!(
                    "🔭 {} {} has joined {}!",
                    "Nostradamus Prophecy:".bold().orange(),
                    NOSTRADAMUS.info().styled_name(),
                    banner.clone(),
                ));
                rctx.record(irc_format!(
                    "🔭 Nostradamus Prophecy bluffed {} (no vision recorded).",
                    banner,
                ));
            } else {
                let banners: Vec<Text> = preds
                    .0
                    .iter()
                    .map(|&(_, r)| Self::format_adopted_banner(r))
                    .collect();
                let joined = Text::join(&banners, " & ".grey());
                rctx.announce(irc_format!(
                    "🔭 {} {} has joined {}!",
                    "Nostradamus Prophecy:".bold().orange(),
                    NOSTRADAMUS.info().styled_name(),
                    joined,
                ));
            }
        }))
    }
}

#[cfg(test)]
#[path = "nostradamus_test.rs"]
mod test;
