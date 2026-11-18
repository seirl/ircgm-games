use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, CenterSlot, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ForcedExposerCount(pub Option<usize>);

impl BoardKey for ForcedExposerCount {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(None)
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveExposerCount(usize);

impl BoardKey for ActiveExposerCount {
    const ACCESS: u16 = BoardAccess::NONE;

    fn init() -> Self {
        Self(1)
    }
}

pub struct Exposer;
pub static EXPOSER: Exposer = Exposer;

impl Role for Exposer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "exposer",
            "Exposer",
            GameSet::Alien,
            Team::Village,
            "Wake to flip 1, 2, or 3 center cards face up as instructed by the Mother Ship (all rolled cards or none)!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&EXPOSER, |seat, ctx| Box::pin(Self::prompt_seat(seat, ctx)))
                .copy_on_turn()
                .with_tier_hooks(Self::init_tier, Self::finish_tier)
                .reads(BoardAccess::FACE_UP)
                .writes(BoardAccess::FACE_UP | BoardAccess::PUBLIC_ANNOUNCE)]
        }
    }
}

impl Exposer {
    fn roll_count() -> usize {
        match fastrand::u8(..100) {
            0..=49 => 1,
            50..=84 => 2,
            _ => 3,
        }
    }

    fn init_tier(board: &mut BoardMut<'_>, _roster: &Roster) {
        let rolled = board
            .get_key::<ForcedExposerCount>()
            .0
            .unwrap_or_else(Self::roll_count)
            .clamp(1, 3);
        board.set_key(ActiveExposerCount(rolled));
    }

    fn finish_tier(board: &mut BoardMut<'_>, rctx: &mut ResolveCtx<'_>) {
        let rolled = board.get_key::<ActiveExposerCount>().0;
        rctx.announce(irc_format!(
            "🔦🛸 {} {} was authorized to flip {rolled} center card(s) face up!",
            "Mother Ship (Exposer):".bold().cyan(),
            EXPOSER.info().styled_name(),
        ));
    }

    async fn prompt_seat(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let rolled = ctx.get_key::<ActiveExposerCount>().0;
        let face_down: Vec<CenterSlot> = roster
            .center_slots()
            .into_iter()
            .filter(|&slot| !ctx.is_center_face_up(slot))
            .collect();
        if face_down.is_empty() {
            ctx.say("🔦🛸 Exposer: All center cards are already face up!".yellow());
            return Ok(SeatEffect::new(move |label, _board, rctx| {
                rctx.record(irc_format!(
                    "🔦🛸 {label} ({}) woke, but all center cards were already face up.",
                    rctx.nick(seat).bold(),
                ));
            }));
        }

        let k = rolled.min(face_down.len());
        let avail_str = face_down
            .iter()
            .map(|&s| s.short_label(roster))
            .collect::<Vec<_>>()
            .join(", ");
        let example = face_down
            .iter()
            .take(k)
            .map(|&s| s.token(roster))
            .collect::<Vec<_>>()
            .join(" ");

        let view = ctx.view();
        let prompt = irc_format!(
            "🔦🛸 Exposer: You may flip EXACTLY {k} center card(s) face up (all {k} or none! Available face-down: {avail_str}). Reply {} or {}:",
            example.as_str().bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                let raw = m.text.trim();
                if Roster::is_pass(raw) {
                    return AskMatch::accept_say(
                        Vec::new(),
                        "🔦🛸 Passed — left center cards face down.",
                    );
                }
                if raw.eq_ignore_ascii_case("all") && k == face_down.len() {
                    return AskMatch::accept_say(
                        face_down.clone(),
                        format!("🔦🛸 Locked in exposing {k} center card(s)!"),
                    );
                }
                let parts: Vec<&str> = raw.split_whitespace().collect();
                if parts.len() != k {
                    return AskMatch::reject(
                        format!("Pick EXACTLY {k} face-down center card(s) (e.g. {example}), or pass!")
                            .yellow(),
                    );
                }
                let mut chosen: Vec<CenterSlot> = Vec::with_capacity(k);
                for tok in parts {
                    let Some(slot) = view.roster.parse_center_slot(tok) else {
                        return AskMatch::reject(
                            format!("Center slots must be {}.", view.roster.center_or_pass_hint())
                                .yellow(),
                        );
                    };
                    if view.is_center_face_up(slot) {
                        return AskMatch::reject(
                            format!("🔦 {} is already face up!", slot.label(view.roster)).yellow(),
                        );
                    }
                    if chosen.contains(&slot) {
                        return AskMatch::reject("Pick DISTINCT center slots!".yellow());
                    }
                    chosen.push(slot);
                }
                let labels = chosen
                    .iter()
                    .map(|&s| s.short_label(view.roster))
                    .collect::<Vec<_>>()
                    .join(", ");
                AskMatch::accept_say(
                    chosen,
                    format!("🔦🛸 Locked in exposing Center {labels} — reveals when the phase ends!"),
                )
            })
            .await?;

        let slots = ans.ok().unwrap_or_default();
        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, &slots, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        slots: &[CenterSlot],
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        if slots.is_empty() {
            ctx.record(irc_format!(
                "🔦🛸 {actor_label} ({}) did not expose any center cards.",
                actor_nick.bold(),
            ));
            return;
        }

        let mut parts = Vec::new();
        for &slot in slots {
            let card = board.expose_center(seat, slot);
            parts.push(irc_format!(
                "{} ({})",
                slot.short_label(ctx.roster),
                card.display_for_peek(),
            ));
        }
        let joined = Text::join(&parts, ", ".grey());
        ctx.dm(
            seat,
            irc_format!(
                "🔦🛸 Exposed FACE UP in the center: {}!",
                joined.clone(),
            ),
        );
        ctx.record(irc_format!(
            "🔦🛸 {actor_label} ({}) flipped Center {} face-up!",
            actor_nick.bold(),
            joined,
        ));
    }
}

#[cfg(test)]
#[path = "exposer_test.rs"]
mod test;
