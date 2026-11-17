use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team,
    WakeStep,
};

/// Per-game randomized Mother Ship mind-read mode for `Psychic` (`#5G`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PsychicMode {
    /// View either `left` or `right` neighbor's card.
    ChooseNeighbor,
    /// View BOTH immediate neighbors' cards.
    BothNeighbors,
    /// View 1 odd-numbered seat (`#1`, `#3`, `#5`, ...).
    OddSeat,
    /// View 1 even-numbered seat (`#2`, `#4`, `#6`, ...).
    EvenSeat,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedPsychicMode(pub Option<PsychicMode>);

impl BoardKey for ForcedPsychicMode {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

pub struct Psychic;
pub static PSYCHIC: Psychic = Psychic;

impl Role for Psychic {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "psychic",
            "Psychic",
            GameSet::Alien,
            Team::Village,
            "Wake in Night to read 1 or 2 players' minds as instructed by the Mother Ship (1 neighbor, both neighbors, or an odd/even seat)!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::group(&PSYCHIC, 159, |seats, ctx| Box::pin(Self::wake(seats, ctx)))
                .run_when_unheld()
                .writes(BoardAccess::PUBLIC_ANNOUNCE)]
        }
    }
}

impl Psychic {
    fn roll_mode() -> PsychicMode {
        match fastrand::u8(..100) {
            0..=34 => PsychicMode::ChooseNeighbor,
            35..=59 => PsychicMode::BothNeighbors,
            60..=79 => PsychicMode::OddSeat,
            _ => PsychicMode::EvenSeat,
        }
    }

    async fn wake(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let mode = ctx
            .get_key::<ForcedPsychicMode>()
            .0
            .unwrap_or_else(Self::roll_mode);
        let effects = ctx
            .prompt_seats(&PSYCHIC, active_seats, |seat, pctx| {
                Box::pin(Self::prompt_seat(seat, mode, pctx))
            })
            .await?;

        Ok(GroupEffect::new(move |board, rctx| {
            rctx.apply_seat_effects(effects, board);
            let desc = match mode {
                PsychicMode::ChooseNeighbor => "view 1 adjacent neighbor's card",
                PsychicMode::BothNeighbors => "view BOTH adjacent neighbors' cards",
                PsychicMode::OddSeat => "view 1 ODD-numbered player's card",
                PsychicMode::EvenSeat => "view 1 EVEN-numbered player's card",
            };
            rctx.announce(irc_format!(
                "🧠 {} {} was instructed to {desc}!",
                "Mother Ship (Psychic):".bold().cyan(),
                PSYCHIC.info().styled_name(),
            ));
        }))
    }

    async fn prompt_seat(
        seat: usize,
        mode: PsychicMode,
        mut ctx: PromptCtx<'_>,
    ) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let view = ctx.view();
        let (left, right) = roster.neighbors(seat);

        let targets: SeatSet = match mode {
            PsychicMode::BothNeighbors => {
                ctx.say(
                    "🧠 Psychic: Cosmic signals are AMPLIFIED — reading BOTH neighbors' minds! Vision arrives when the phase ends.",
                );
                SeatSet::from([left, right])
            }
            PsychicMode::ChooseNeighbor => {
                let unshielded = SeatSet::from([left, right]) - view.shielded_seats();
                if unshielded.is_empty() {
                    ctx.say(
                        "🛡️ Psychic: Both neighboring cards are shielded — you cannot read either mind tonight!"
                            .yellow(),
                    );
                    SeatSet::EMPTY
                } else {
                    let left_nick = roster.seats[left].user.nick();
                    let right_nick = roster.seats[right].user.nick();
                    let prompt = irc_format!(
                        "🧠 Psychic: Reply {} ({}), {} ({}), or {} to read a neighbor's mind!",
                        "left".bold(),
                        left_nick,
                        "right".bold(),
                        right_nick,
                        "pass".bold(),
                    );
                    let ans = ctx
                        .ask(prompt, move |m| {
                            let raw = m.text.trim();
                            if Roster::is_pass(raw) {
                                return AskMatch::accept_say(
                                    None,
                                    "🧠 Passed — keeping your mind closed.",
                                );
                            }
                            let lower = raw.to_ascii_lowercase();
                            let picked = match lower.as_str() {
                                "left" | "l" => Some(left),
                                "right" | "r" => Some(right),
                                _ => roster
                                    .resolve_nick(raw)
                                    .filter(|&i| i == left || i == right),
                            };
                            match picked {
                                Some(t) if view.is_shielded(t) => {
                                    AskMatch::reject("🛡️ That neighbor's mind is shielded!".yellow())
                                }
                                Some(t) => AskMatch::accept_say(
                                    Some(t),
                                    irc_format!(
                                        "🧠 Locked in reading {}'s mind — vision arrives when the phase ends!",
                                        roster.seats[t].user.nick().bold(),
                                    ),
                                ),
                                None => AskMatch::reject("Reply left, right, or pass.".yellow()),
                            }
                        })
                        .await?;
                    match ans {
                        Answer::Ok(Some(t)) => SeatSet::single(t),
                        Answer::Ok(None) => SeatSet::EMPTY,
                        _ => SeatSet::single(unshielded.first().expect("unshielded non-empty")),
                    }
                }
            }
            PsychicMode::OddSeat | PsychicMode::EvenSeat => {
                let want_even = mode == PsychicMode::EvenSeat;
                let parity_label = if want_even { "EVEN" } else { "ODD" };
                let eligible_parity: SeatSet = roster
                    .all_seats()
                    .without(seat)
                    .into_iter()
                    .filter(|&i| ((i + 1) % 2 == 0) == want_even)
                    .collect();
                let unshielded = eligible_parity - view.shielded_seats();
                if unshielded.is_empty() {
                    ctx.say(
                        format!(
                            "🛡️ Psychic: No unshielded {parity_label}-numbered player seats are available tonight!"
                        )
                        .yellow(),
                    );
                    SeatSet::EMPTY
                } else {
                    let choices_str: String = eligible_parity
                        .into_iter()
                        .map(|i| {
                            let mut s = format!("#{} {}", i + 1, roster.seats[i].user.nick());
                            if view.is_shielded(i) {
                                s.push('🛡');
                            }
                            s
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let prompt = irc_format!(
                        "🧠 Psychic: Reply {} (or {}) to read one {parity_label}-numbered player's mind! Candidates: {choices_str}",
                        "<nick>".bold(),
                        "pass".bold(),
                    );
                    let ans = ctx
                        .ask(prompt, move |m| {
                            let raw = m.text.trim();
                            if Roster::is_pass(raw) {
                                return AskMatch::accept_say(
                                    None,
                                    "🧠 Passed — keeping your mind closed.",
                                );
                            }
                            let by_num = raw
                                .strip_prefix('#')
                                .unwrap_or(raw)
                                .parse::<usize>()
                                .ok()
                                .and_then(|one_based| one_based.checked_sub(1))
                                .filter(|&i| i < roster.len());
                            let picked = by_num.or_else(|| roster.resolve_nick(raw));
                            match picked {
                                Some(t) if !eligible_parity.contains(t) => AskMatch::reject(
                                    format!("Pick an {parity_label}-numbered other player!").yellow(),
                                ),
                                Some(t) if view.is_shielded(t) => {
                                    AskMatch::reject("🛡️ That player's mind is shielded!".yellow())
                                }
                                Some(t) => AskMatch::accept_say(
                                    Some(t),
                                    irc_format!(
                                        "🧠 Locked in reading {}'s mind — vision arrives when the phase ends!",
                                        roster.seats[t].user.nick().bold(),
                                    ),
                                ),
                                None => AskMatch::reject(
                                    format!("Reply an {parity_label}-seat nickname or pass.").yellow(),
                                ),
                            }
                        })
                        .await?;
                    match ans {
                        Answer::Ok(Some(t)) => SeatSet::single(t),
                        Answer::Ok(None) => SeatSet::EMPTY,
                        _ => SeatSet::single(unshielded.choose_random().expect("unshielded non-empty")),
                    }
                }
            }
        };

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, targets, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        targets: SeatSet,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if targets.is_empty() {
            ctx.record(irc_format!(
                "🧠 {actor_label} ({}) read no minds tonight.",
                actor_nick.bold(),
            ));
            return;
        }

        let mut seen_parts = Vec::new();
        for target in targets {
            if let Some(seen) = board.peek_player(seat, target) {
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
        }

        if seen_parts.is_empty() {
            ctx.dm(seat, "🧠 Psychic: Target minds were shielded tonight!");
            ctx.record(irc_format!(
                "🧠 {actor_label} ({}) found target cards shielded.",
                actor_nick.bold(),
            ));
        } else {
            ctx.record(irc_format!(
                "🧠 {actor_label} ({}) psychically read {}.",
                actor_nick.bold(),
                Text::join(seen_parts, ", "),
            ));
        }
    }
}

#[cfg(test)]
#[path = "psychic_test.rs"]
mod test;
