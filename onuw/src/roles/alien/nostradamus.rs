use futures::future::try_join_all;
use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardCards, BoardKey, BoardMut, Roster, SeatSet};
use crate::marks::Mark;
use crate::role::{
    EffectiveAlignment, GameSet, GroupEffect, GroupPromptCtx, PromptCtx, ResolveCtx, Role,
    RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep, WinnerFilterCtx, WinnerMap,
};

/// Maps physical card `face_role().id()` (`"nostradamus"`, `"doppelganger"`, `"copycat"`, `"mirror_man"`)
/// to the role whose team/victory that card adopted at `#5E`.
#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveNostradamusRoles(pub Vec<(&'static str, &'static dyn Role)>);

impl ActiveNostradamusRoles {
    fn set(&mut self, face_id: &'static str, adopted: &'static dyn Role) {
        if let Some(entry) = self.0.iter_mut().find(|(id, _)| *id == face_id) {
            entry.1 = adopted;
        } else {
            self.0.push((face_id, adopted));
        }
    }

    fn get_for_card_face(&self, face_id: &str) -> Option<&'static dyn Role> {
        self.0
            .iter()
            .rev()
            .find(|&&(id, _)| id == face_id)
            .or_else(|| self.0.iter().find(|&&(id, _)| id == "nostradamus"))
            .or_else(|| self.0.first())
            .map(|&(_, r)| r)
    }
}

impl BoardKey for ActiveNostradamusRoles {
    const ACCESS: u16 = BoardAccess::ROLE_STATE;

    fn init() -> Self {
        Self(Vec::new())
    }
}

struct FaceGroupVision {
    face_id: &'static str,
    group_seats: SeatSet,
    chosen_seat: usize,
    is_player_copy: bool,
    peeked: Vec<usize>,
    last_role: Option<&'static dyn Role>,
}

pub struct Nostradamus;
pub static NOSTRADAMUS: Nostradamus = Nostradamus;

impl Role for Nostradamus {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "nostradamus",
            "Nostradamus",
            GameSet::Alien,
            Team::Independent,
            "Look at up to 2 players' cards (3 at 6+ players), stopping immediately if you see a non-Village card: the LAST card you view sets your team (announced publicly), and you MUST survive to win!",
        )
        .aliases(&["nostra"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::group(&NOSTRADAMUS, |seats, ctx| Box::pin(Self::wake(seats, ctx)))
                    .copy_on_turn()
                    .run_when_unheld()
                    .reads(BoardAccess::PLAYER_FACES)
                    .writes(ActiveNostradamusRoles::ACCESS | BoardAccess::PUBLIC_ANNOUNCE),
            ]
        }
    }

    fn adjust_alignment(
        &self,
        seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur.threat_team = None;
        let face_id = board.player_card(seat).face_role().id();
        if let Some(adopted) = board
            .get_key::<ActiveNostradamusRoles>()
            .get_for_card_face(face_id)
        {
            if adopted.id() != self.info().id {
                let mut adopted_align = adopted.adjust_alignment(
                    seat,
                    board,
                    EffectiveAlignment {
                        team: adopted.team(),
                        threat_team: None,
                    },
                );
                adopted_align.threat_team = None;
                return adopted_align;
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
        let team = ctx.board.effective_team(holder);
        if ctx.dead_seats.contains(holder) || team.is_collective() {
            return;
        }
        let face_id = ctx.board.player_card(holder).face_role().id();
        let Some(adopted) = ctx
            .board
            .get_key::<ActiveNostradamusRoles>()
            .get_for_card_face(face_id)
        else {
            return;
        };
        if adopted.id() == self.info().id {
            return;
        }
        let vctx = VictoryCtx {
            roster: ctx.roster,
            board: ctx.board,
            votes: ctx.votes,
            dead_seats: ctx.dead_seats,
        };
        if let Some(SpecialVictory::WithTeam(t)) = adopted.special_victory(holder, &vctx) {
            if ctx.winning_teams.contains(&t) {
                winners.insert(holder, "Nostradamus");
            }
            return;
        }
        if ctx.hunted_active {
            if matches!(
                adopted.special_victory(holder, &vctx),
                Some(SpecialVictory::Hunted)
            ) {
                if !ctx.hunted_seats.is_empty() && ctx.hunted_seats.is_disjoint(ctx.dead_seats) {
                    winners.insert(holder, "Nostradamus");
                } else {
                    winners.remove(&holder);
                }
            }
            return;
        }

        let candidates = if team != Team::Independent {
            ctx.board.seats_on_team(team).without(holder)
        } else {
            ctx.board.active_holders(adopted)
        };
        if candidates.into_iter().any(|s| ctx.base_winners.contains_key(&s)) {
            winners.insert(holder, "Nostradamus");
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
    fn format_adopted_banner(role: &'static dyn Role, board: &BoardCards) -> Text {
        let team = role
            .adjust_alignment(
                0,
                board,
                EffectiveAlignment {
                    team: role.team(),
                    threat_team: None,
                },
            )
            .team;
        if team == Team::Independent {
            role.styled_name()
        } else {
            team.style_text(team.win_label())
        }
    }

    fn pick_random_deck_role(roster: &Roster) -> &'static dyn Role {
        let candidates: Vec<&'static dyn Role> = roster
            .deck
            .iter()
            .copied()
            .filter(|r| r.id() != "nostradamus")
            .collect();
        if candidates.is_empty() {
            &NOSTRADAMUS
        } else {
            candidates[fastrand::usize(..candidates.len())]
        }
    }

    async fn wake(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        // Group waking seats by printed `initial_role.id()` (`"nostradamus"`, `"copycat"`, `"mirror_man"`, `"doppelganger"`):
        // if `nostradamus*2` or `doppelganger*2` causes 2+ seats to share the same card face,
        // draw 1 random seer per face group and share its announced destiny across that group!
        let mut face_groups: Vec<(&'static str, bool, SeatSet)> = Vec::new();
        for pass_player_copy in [false, true] {
            for s in active_seats {
                let is_player_copy = ctx.copied_from(s).is_some();
                if is_player_copy != pass_player_copy {
                    continue;
                }
                let face_id = ctx.roster.seats[s].initial_role.id();
                if let Some((_, _, set)) = face_groups.iter_mut().find(|(id, _, _)| *id == face_id)
                {
                    set.insert(s);
                } else {
                    face_groups.push((face_id, is_player_copy, SeatSet::single(s)));
                }
            }
        }
        // Keep `"nostradamus"` first among non-player-copy groups:
        face_groups.sort_by_key(|&(face_id, is_player_copy, _)| {
            (is_player_copy, face_id != "nostradamus")
        });

        let mut chosen_groups: Vec<(&'static str, bool, SeatSet, usize)> =
            Vec::with_capacity(face_groups.len());
        for (face_id, is_player_copy, group_seats) in face_groups {
            let chosen = group_seats
                .choose_random()
                .expect("group_seats is non-empty");
            let role_title = if is_player_copy {
                "Doppelgänger-Nostradamus"
            } else {
                "Nostradamus"
            };
            for extra in group_seats.without(chosen) {
                ctx.dm(
                    extra,
                    format!(
                        "🔭 {role_title}: Another {role_title} is gazing into the future tonight — you share whatever team they pick (and MUST survive to win)!"
                    ),
                );
            }
            chosen_groups.push((face_id, is_player_copy, group_seats, chosen));
        }

        let ctx_ref = &ctx;
        let visions = try_join_all(chosen_groups.into_iter().map(
            |(face_id, is_player_copy, group_seats, chosen_seat)| async move {
                let pctx = ctx_ref.for_seat(chosen_seat).await?;
                let (peeked, last_role) = Self::prompt_seat(chosen_seat, pctx).await?;
                Ok::<_, Error>(FaceGroupVision {
                    face_id,
                    group_seats,
                    chosen_seat,
                    is_player_copy,
                    peeked,
                    last_role,
                })
            },
        ))
        .await?;

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(&visions, board, rctx);
        }))
    }

    async fn prompt_seat(
        seat: usize,
        mut ctx: PromptCtx<'_>,
    ) -> Result<(Vec<usize>, Option<&'static dyn Role>)> {
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
                    Self::format_adopted_banner(role, ctx.board_for_peek()),
                ));
                break;
            }

            ctx.say(irc_format!(
                "🔭 {} holds {} (current destiny: {})!",
                ctx.nick(target).bold(),
                card.display_for_peek(),
                Self::format_adopted_banner(role, ctx.board_for_peek()),
            ));
        }

        if let Some(role) = last_role {
            ctx.say(irc_format!(
                "🔭 Destiny sealed: you join {} (and MUST survive to win)!",
                Self::format_adopted_banner(role, ctx.board_for_peek()),
            ));
        }

        Ok((peeked, last_role))
    }

    fn resolve_single_vision(
        v: &FaceGroupVision,
        destinies: &mut ActiveNostradamusRoles,
        board: &mut BoardMut<'_>,
        rctx: &mut ResolveCtx<'_>,
    ) -> &'static dyn Role {
        if !v.peeked.is_empty() {
            board.record_prompt_peek(v.chosen_seat);
        }
        let label = rctx.wakers.actor_label(rctx.roster, v.chosen_seat);
        let actor_nick = rctx.nick(v.chosen_seat);
        let adopted = match v.last_role {
            Some(role) => {
                let names = rctx.roster.format_seats(v.peeked.iter().copied());
                rctx.record(irc_format!(
                    "🔭 {label} ({}) viewed {} — joined {}!",
                    actor_nick.bold(),
                    names.bold(),
                    Self::format_adopted_banner(role, board.cards()),
                ));
                role
            }
            None => {
                let picked = Self::pick_random_deck_role(rctx.roster);
                let banner = Self::format_adopted_banner(picked, board.cards());
                rctx.dm(
                    v.chosen_seat,
                    irc_format!(
                        "🔭 No card viewed — the Mother Ship assigned you to {} (survive to win)!",
                        banner.clone(),
                    ),
                );
                rctx.record(irc_format!(
                    "🔭 {label} ({}) viewed no cards — randomly assigned to {}!",
                    actor_nick.bold(),
                    banner,
                ));
                picked
            }
        };
        let banner = Self::format_adopted_banner(adopted, board.cards());
        let role_title = if v.is_player_copy {
            "Doppelgänger-Nostradamus"
        } else {
            "Nostradamus"
        };
        for extra in v.group_seats.without(v.chosen_seat) {
            rctx.dm(
                extra,
                irc_format!(
                    "🔭 Shared Prophecy: {role_title} joined {} (survive to win)!",
                    banner.clone(),
                ),
            );
        }
        destinies.set(v.face_id, adopted);
        adopted
    }

    fn resolve(
        visions: &[FaceGroupVision],
        board: &mut BoardMut<'_>,
        rctx: &mut ResolveCtx<'_>,
    ) {
        let mut destinies = board.get_key::<ActiveNostradamusRoles>();

        let mut primary_adopted: Option<&'static dyn Role> = None;
        let mut dg_adopted: Vec<&'static dyn Role> = Vec::new();

        for v in visions {
            let adopted = Self::resolve_single_vision(v, &mut destinies, board, rctx);
            if v.is_player_copy {
                dg_adopted.push(adopted);
            } else if primary_adopted.is_none() {
                primary_adopted = Some(adopted);
            }
        }

        // 1. Primary Nostradamus prophecy (also sets `"nostradamus"` for center/Copycat/Drunk):
        let main_role = match primary_adopted {
            Some(role) => {
                destinies.set("nostradamus", role);
                role
            }
            None => {
                let picked = Self::pick_random_deck_role(rctx.roster);
                destinies.set("nostradamus", picked);
                rctx.record(irc_format!(
                    "🔭 Nostradamus (in center) randomly joined {}.",
                    Self::format_adopted_banner(picked, board.cards()),
                ));
                picked
            }
        };
        rctx.announce(irc_format!(
            "🔭 {} {} has joined {}!",
            "Nostradamus Prophecy:".bold().orange(),
            NOSTRADAMUS.info().styled_name(),
            Self::format_adopted_banner(main_role, board.cards()),
        ));

        // 2. Separate Doppelgänger-Nostradamus prophecy whenever a player-copier is in the deck
        //    (emits a random decoy team when Doppelgänger copied someone else or is in the center):
        let deck_has_player_copier = rctx.roster.deck.iter().any(|r| r.is_player_copier());
        if deck_has_player_copier {
            let dg_label = Team::Independent.style_text("Doppelgänger-Nostradamus");
            if dg_adopted.is_empty() {
                let decoy = Self::pick_random_deck_role(rctx.roster);
                for &r in &rctx.roster.deck {
                    if r.is_player_copier() {
                        destinies.set(r.id(), decoy);
                    }
                }
                let decoy_banner = Self::format_adopted_banner(decoy, board.cards());
                rctx.announce(irc_format!(
                    "🔭 {} {} has joined {}!",
                    "Nostradamus Prophecy:".bold().orange(),
                    dg_label,
                    decoy_banner.clone(),
                ));
                rctx.record(irc_format!(
                    "🔭 Doppelgänger-Nostradamus randomly joined {} (did not wake to gaze).",
                    decoy_banner,
                ));
            } else {
                for role in dg_adopted {
                    rctx.announce(irc_format!(
                        "🔭 {} {} has joined {}!",
                        "Nostradamus Prophecy:".bold().orange(),
                        dg_label.clone(),
                        Self::format_adopted_banner(role, board.cards()),
                    ));
                }
            }
        }

        board.set_key(destinies);
    }
}

#[cfg(test)]
#[path = "nostradamus_test.rs"]
mod test;
