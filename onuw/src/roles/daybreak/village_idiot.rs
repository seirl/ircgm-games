use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake};

pub struct VillageIdiot;
pub static VILLAGE_IDIOT: VillageIdiot = VillageIdiot;

#[derive(Clone, Copy)]
enum RotateDir {
    Left,
    Right,
}

impl Role for VillageIdiot {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "village_idiot",
            "Village Idiot",
            GameSet::Daybreak,
            Team::Village,
            "May rotate ALL other unshielded players' cards one step left or right around the table (your own card stays put!).",
        )
        .aliases(&["vi", "idiot"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(172, |seat, ctx| Box::pin(Self::prompt(seat, ctx))).mutates())
    }
}

impl VillageIdiot {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let prompt = irc_format!(
            "🤪 Village Idiot: Reply {} ({}), {} ({}), or {} to rotate all other unshielded cards! Players: {}",
            "left".bold(),
            "-1".bold(),
            "right".bold(),
            "+1".bold(),
            "pass".bold(),
            ctx.roster.format_players_marking_shields(ctx.shielded),
        );
        let ans = ctx
            .ask(prompt, |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🤪 Passed — left all cards in place.");
                }
                match m.text.trim().to_ascii_lowercase().as_str() {
                    "left" | "l" | "-1" => AskMatch::accept_say(
                        Some(RotateDir::Left),
                        "🤪 Locked in rotating other cards LEFT!",
                    ),
                    "right" | "r" | "+1" => AskMatch::accept_say(
                        Some(RotateDir::Right),
                        "🤪 Locked in rotating other cards RIGHT!",
                    ),
                    _ => AskMatch::reject("Reply left, right, or pass.".yellow()),
                }
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<RotateDir>>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        let Answer::Ok(Some(dir)) = ans else {
            ctx.record(irc_format!(
                "🤪 {actor_label} ({}) did not rotate cards.",
                actor_nick.bold(),
            ));
            return;
        };

        let ring: Vec<usize> = (0..ctx.roster.len())
            .filter(|&i| i != seat && !board.is_shielded(i))
            .collect();
        if ring.len() >= 2 {
            board.record_card_interaction(seat);
            let snapshot: Vec<_> = ring.iter().map(|&i| board.players[i]).collect();
            let k = ring.len();
            for (pos, &dst_seat) in ring.iter().enumerate() {
                let src_pos = match dir {
                    RotateDir::Right => (pos + k - 1) % k,
                    RotateDir::Left => (pos + 1) % k,
                };
                board.players[dst_seat] = snapshot[src_pos];
            }
            let dir_str = match dir {
                RotateDir::Left => "left",
                RotateDir::Right => "right",
            };
            ctx.record(irc_format!(
                "🤪 {actor_label} ({}) rotated {} unshielded cards {dir_str}.",
                actor_nick.bold(),
                ring.len(),
            ));
        }
    }
}
