use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, SpecialVictory, Team, VictoryCtx,
    WakeStep,
};

/// Per-game randomized Mother Ship peek clearance for `Mortician` (`#13A`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MorticianMode {
    /// May peek at either `left` or `right` neighbor's card.
    ChooseNeighbor,
    /// May peek at `left` neighbor's card.
    LeftOnly,
    /// May peek at `right` neighbor's card.
    RightOnly,
    /// May peek at BOTH `left` and `right` neighbors' cards.
    BothNeighbors,
    /// Mother Ship forbids peeking at any neighbor's card tonight ("none of his neighbors").
    None,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedMorticianMode(pub Option<MorticianMode>);

impl BoardKey for ForcedMorticianMode {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveMorticianMode(MorticianMode);

impl BoardKey for ActiveMorticianMode {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(MorticianMode::ChooseNeighbor)
    }
}

pub struct Mortician;
pub static MORTICIAN: Mortician = Mortician;

impl Role for Mortician {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mortician",
            "Mortician",
            GameSet::Alien,
            Team::Independent,
            "Wake at the end of Night to peek at one, both, or none of your neighbors' cards as instructed by the Mother Ship! Win ONLY if at least one adjacent neighbor dies!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&MORTICIAN, |seat, ctx| Box::pin(Self::prompt_seat(seat, ctx)))
                    .copy_on_turn()
                    .with_tier_hooks(Self::init_tier, Self::finish_tier)
                    .writes(BoardAccess::PUBLIC_ANNOUNCE),
            ]
        }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let (left, right) = ctx.roster.neighbors(seat);
        (ctx.is_dead(left) || ctx.is_dead(right)).then_some(SpecialVictory::independent())
    }
}

impl Mortician {
    fn roll_mode() -> MorticianMode {
        match fastrand::u8(..100) {
            0..=34 => MorticianMode::ChooseNeighbor,
            35..=49 => MorticianMode::LeftOnly,
            50..=64 => MorticianMode::RightOnly,
            65..=79 => MorticianMode::BothNeighbors,
            _ => MorticianMode::None,
        }
    }

    fn is_affirmative(raw: &str) -> bool {
        matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "yes" | "y" | "peek" | "view" | "both"
        )
    }

    fn init_tier(board: &mut BoardMut<'_>, _roster: &Roster) {
        let mode = board
            .get_key::<ForcedMorticianMode>()
            .0
            .unwrap_or_else(Self::roll_mode);
        board.set_key(ActiveMorticianMode(mode));
    }

    fn finish_tier(board: &mut BoardMut<'_>, rctx: &mut ResolveCtx<'_>) {
        let mode = board.get_key::<ActiveMorticianMode>().0;
        let desc = match mode {
            MorticianMode::ChooseNeighbor => "peek at either adjacent neighbor's card",
            MorticianMode::LeftOnly => "peek at their LEFT neighbor's card",
            MorticianMode::RightOnly => "peek at their RIGHT neighbor's card",
            MorticianMode::BothNeighbors => "peek at BOTH adjacent neighbors' cards",
            MorticianMode::None => "peek at NONE of their neighbors' cards",
        };
        rctx.announce(irc_format!(
            "⚰️ {} {} was instructed to {desc}!",
            "Mother Ship (Mortician):".bold().orange(),
            MORTICIAN.info().styled_name(),
        ));
    }

    async fn prompt_seat(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let mode = ctx.get_key::<ActiveMorticianMode>().0;
        let roster = ctx.roster;
        let view = ctx.view();
        let (left, right) = roster.neighbors(seat);
        let left_nick = roster.seats[left].user.nick();
        let right_nick = roster.seats[right].user.nick();

        let targets: SeatSet = match mode {
            MorticianMode::ChooseNeighbor => {
                let candidates = SeatSet::from([left, right]) - view.shielded_seats();
                if candidates.is_empty() {
                    ctx.say("⚰️ Mortician: Both neighbors' cards are shielded — you cannot peek tonight!");
                    SeatSet::EMPTY
                } else {
                    let prompt = irc_format!(
                        "⚰️ Mortician: Reply {} ({}), {} ({}), or {} to peek at one neighbor's card!",
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
                                    "⚰️ Passed — not peeking at any card.",
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
                                    AskMatch::reject("🛡️ That card is shielded!".yellow())
                                }
                                Some(t) => AskMatch::accept_say(
                                    Some(t),
                                    irc_format!(
                                        "⚰️ Locked in peeking at {}'s card — reveals when the phase ends!",
                                        roster.seats[t].user.nick().bold(),
                                    ),
                                ),
                                None => AskMatch::reject("Reply left, right, or pass.".yellow()),
                            }
                        })
                        .await?;
                    ans.ok().flatten().map(SeatSet::single).unwrap_or(SeatSet::EMPTY)
                }
            }
            MorticianMode::LeftOnly | MorticianMode::RightOnly => {
                let (target, dir, nick) = if mode == MorticianMode::LeftOnly {
                    (left, "left", left_nick)
                } else {
                    (right, "right", right_nick)
                };
                if view.is_shielded(target) {
                    ctx.say(format!(
                        "⚰️ Mortician: Your {dir} neighbor ({nick}) is shielded — you cannot peek tonight!"
                    ));
                    SeatSet::EMPTY
                } else {
                    let prompt = irc_format!(
                        "⚰️ Mortician: You may inspect your {dir} neighbor ({nick})! Reply {} (or {}) to peek, or {}!",
                        dir.bold(),
                        "yes".bold(),
                        "pass".bold(),
                    );
                    let ans = ctx
                        .ask(prompt, move |m| {
                            let raw = m.text.trim();
                            if Roster::is_pass(raw) {
                                return AskMatch::accept_say(
                                    false,
                                    "⚰️ Passed — not peeking at any card.",
                                );
                            }
                            if Self::is_affirmative(raw)
                                || raw.eq_ignore_ascii_case(dir)
                                || roster.resolve_nick(raw) == Some(target)
                            {
                                AskMatch::accept_say(
                                    true,
                                    format!(
                                        "⚰️ Locked in peeking at {}'s card — reveals when the phase ends!",
                                        roster.seats[target].user.nick()
                                    ),
                                )
                            } else {
                                AskMatch::reject(format!("Reply {dir}, yes, or pass.").yellow())
                            }
                        })
                        .await?;
                    if ans.ok() == Some(true) {
                        SeatSet::single(target)
                    } else {
                        SeatSet::EMPTY
                    }
                }
            }
            MorticianMode::BothNeighbors => {
                let unshielded = SeatSet::from([left, right]) - view.shielded_seats();
                if unshielded.is_empty() {
                    ctx.say("⚰️ Mortician: Both neighbors' cards are shielded — you cannot peek tonight!");
                    SeatSet::EMPTY
                } else {
                    let prompt = irc_format!(
                        "⚰️ Mortician: You may inspect BOTH neighbors ({left_nick} & {right_nick})! Reply {} (or {}) to peek, or {}!",
                        "both".bold(),
                        "yes".bold(),
                        "pass".bold(),
                    );
                    let ans = ctx
                        .ask(prompt, move |m| {
                            let raw = m.text.trim();
                            if Roster::is_pass(raw) {
                                AskMatch::accept_say(false, "⚰️ Passed — not peeking at any card.")
                            } else if Self::is_affirmative(raw) {
                                AskMatch::accept_say(
                                    true,
                                    "⚰️ Locked in peeking at neighboring cards — reveals when the phase ends!",
                                )
                            } else {
                                AskMatch::reject("Reply both, yes, or pass.".yellow())
                            }
                        })
                        .await?;
                    if ans.ok() == Some(true) {
                        unshielded
                    } else {
                        SeatSet::EMPTY
                    }
                }
            }
            MorticianMode::None => {
                ctx.say(
                    "⚰️ Mortician: Mother Ship clearance DENIED — you may view NONE of your neighbors' cards tonight!",
                );
                SeatSet::EMPTY
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
        rctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = rctx.nick(seat);
        let mut seen_parts = Vec::new();
        for target in targets {
            if let Some(seen) = board.peek_player(seat, target) {
                let target_nick = rctx.nick(target);
                rctx.dm(
                    seat,
                    irc_format!(
                        "⚰️ Mortician Peek: {} holds {}!",
                        target_nick.as_str().bold(),
                        seen.display_for_peek(),
                    ),
                );
                seen_parts.push(irc_format!(
                    "{} ({})",
                    target_nick.bold(),
                    seen.display_for_peek(),
                ));
            }
        }
        if seen_parts.is_empty() {
            rctx.record(irc_format!(
                "⚰️ {actor_label} ({}) did not peek at any card.",
                actor_nick.bold(),
            ));
        } else {
            rctx.record(irc_format!(
                "⚰️ {actor_label} ({}) peeked at {}.",
                actor_nick.bold(),
                Text::join(seen_parts, ", "),
            ));
        }
    }
}

#[cfg(test)]
#[path = "mortician_test.rs"]
mod test;
