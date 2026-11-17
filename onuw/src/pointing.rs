//! Silent Dusk/Night tabletop pointing (`👉 <nick> points at ...`) for multi-player monster wakes.
//!
//! In *One Night Ultimate*, talking at night is strictly forbidden — waking groups (`Vampires`,
//! `Aliens`) coordinate shared actions purely by pointing silently at player cards or center slots.

use futures::future::try_join_all;
use ircgm::prelude::*;

use crate::board::{Roster, SeatSet};
use crate::role::GroupPromptCtx;

/// Order-independent pair of distinct player seats (`(a, b) == (b, a)`) so two packmates
/// pointing at `"Carol Dave"` and `"Dave Carol"` tally toward the exact same swap!
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SeatPair(usize, usize);

impl SeatPair {
    pub fn new(a: usize, b: usize) -> Option<Self> {
        (a != b).then(|| Self(a.min(b), a.max(b)))
    }

    pub fn seats(self) -> (usize, usize) {
        (self.0, self.1)
    }
}

/// Runs silent tabletop pointing across `seats` (rejecting free-text night chatter):
/// - If `seats` has 1 player, prompts them once and locks in their choice immediately.
/// - If `seats` has 2+ players, opens an exclusive DM prompt to each player until `ctx.deadline`,
///   broadcasts `"👉 <nick> <gesture>"` to all `seats` whenever someone points so partners can
///   silently align (or adjust their finger), and tallies plurality (breaking ties by latest pointer).
pub(crate) async fn silent_point<T, P, M>(
    ctx: &GroupPromptCtx<'_>,
    seats: SeatSet,
    prompt_for: P,
    parse_gesture: M,
) -> Result<Option<(usize, T)>>
where
    T: Clone + PartialEq + Send + Sync,
    P: Fn(usize) -> Text,
    M: Fn(usize, &str) -> std::result::Result<(String, Option<T>), Text> + Send + Sync,
{
    for s in seats {
        ctx.maybe_emit_table_summary_to(s);
    }

    let roster = ctx.roster;
    if let Some(solo) = seats.only() {
        let Some(mut dm) = roster.seats[solo].user.exclusive_available().await? else {
            return Ok(None);
        };
        let nick = roster.seats[solo].user.nick();
        let ans = dm
            .ask(prompt_for(solo), |m| match parse_gesture(solo, m.text.trim()) {
                Ok((desc, Some(choice))) => AskMatch::accept_say(
                    choice,
                    irc_format!("👉 {} {desc}", nick.as_str().bold()),
                ),
                Ok((_, None)) => AskMatch::reject("Invalid target.".yellow()),
                Err(err) => AskMatch::reject(err),
            })
            .config(ctx.ask_cfg)
            .await?;
        return Ok(ans.ok().map(|c| (solo, c)));
    }

    let state = std::sync::Mutex::new(PointBallot::<T>::new());
    let cfg = AskConfig::new()
        .reply_deadline(ctx.deadline)
        .no_timeout_message();
    let parse_ref = &parse_gesture;
    let state_ref = &state;
    let cfg_ref = &cfg;

    let futs = seats.into_iter().map(|seat| {
        let prompt = prompt_for(seat);
        async move {
            let Some(mut dm) = roster.seats[seat].user.exclusive_available().await? else {
                return Ok::<(), Error>(());
            };
            let nick = roster.seats[seat].user.nick();
            let _ = dm
                .ask(prompt, |m| -> AskMatch<()> {
                    match parse_ref(seat, m.text.trim()) {
                        Ok((desc, valid_choice)) => {
                            state_ref
                                .lock()
                                .expect("point ballot lock")
                                .record(seat, valid_choice);
                            let line = irc_format!("👉 {} {desc}", nick.as_str().bold());
                            for peer in seats {
                                let _ = roster.seats[peer].user.say(line.clone());
                            }
                            AskMatch::ignore()
                        }
                        Err(err) => AskMatch::reject(err),
                    }
                })
                .config(cfg_ref)
                .await?;
            Ok(())
        }
    });

    try_join_all(futs).await?;

    Ok(state.into_inner().expect("point ballot lock").tally())
}

/// Parses a single-player point (`<nick>`, optionally prefixed with `cmd_word` such as `"mark"` or `"scan"`).
///
/// When `enforce_valid_in_prompt` is `false` (e.g. a wide-awake observer is present), pointing at an
/// ineligible non-shielded player still broadcasts the physical gesture while recording `None` for the ballot.
pub(crate) fn parse_single_player_point(
    roster: &Roster,
    raw: &str,
    cmd_word: &str,
    valid_targets: SeatSet,
    shielded: SeatSet,
    enforce_valid_in_prompt: bool,
    invalid_target_msg: &'static str,
    usage_msg: &'static str,
) -> std::result::Result<(String, Option<usize>), Text> {
    let stripped = raw.strip_prefix('!').unwrap_or(raw).trim();
    let parts: Vec<&str> = stripped.split_whitespace().collect();
    let tok = match parts.as_slice() {
        [w, arg] if !cmd_word.is_empty() && w.eq_ignore_ascii_case(cmd_word) => *arg,
        [arg] => *arg,
        _ => return Err(usage_msg.yellow().into()),
    };
    let Some(idx) = roster.resolve_nick(tok) else {
        return Err(usage_msg.yellow().into());
    };
    if shielded.contains(idx) {
        return Err("🛡️ That player's card is shielded!".yellow().into());
    }
    if enforce_valid_in_prompt && !valid_targets.contains(idx) {
        return Err(invalid_target_msg.yellow().into());
    }
    Ok((
        format!("points at {}!", roster.seats[idx].user.nick()),
        valid_targets.contains(idx).then_some(idx),
    ))
}

/// Parses a simultaneous two-player point (`<nick1> <nick2>`, order-independent via [`SeatPair`]).
pub(crate) fn parse_two_player_point(
    roster: &Roster,
    raw: &str,
    valid_targets: SeatSet,
    shielded: SeatSet,
    enforce_valid_in_prompt: bool,
    invalid_target_msg: &'static str,
    usage_msg: &'static str,
) -> std::result::Result<(String, Option<SeatPair>), Text> {
    let stripped = raw.strip_prefix('!').unwrap_or(raw).trim();
    let parts: Vec<&str> = stripped.split_whitespace().collect();
    let (t1, t2) = match parts.as_slice() {
        [w, a, b] if w.eq_ignore_ascii_case("beam") || w.eq_ignore_ascii_case("swap") => (*a, *b),
        [a, b] => (*a, *b),
        _ => return Err(usage_msg.yellow().into()),
    };
    let (Some(a), Some(b)) = (roster.resolve_nick(t1), roster.resolve_nick(t2)) else {
        return Err(usage_msg.yellow().into());
    };
    let Some(pair) = SeatPair::new(a, b) else {
        return Err("Pick two DIFFERENT players!".yellow().into());
    };
    if shielded.contains(a) || shielded.contains(b) {
        return Err("🛡️ Shielded players cannot be targeted!".yellow().into());
    }
    let both_valid = valid_targets.contains(a) && valid_targets.contains(b);
    if enforce_valid_in_prompt && !both_valid {
        return Err(invalid_target_msg.yellow().into());
    }
    let (lo, hi) = pair.seats();
    Ok((
        format!(
            "points at {} and {}!",
            roster.seats[lo].user.nick(),
            roster.seats[hi].user.nick()
        ),
        both_valid.then_some(pair),
    ))
}

struct PointBallot<T> {
    next_seq: u64,
    per_seat: [Option<(u64, T)>; SeatSet::MAX_SEATS],
}

impl<T: Clone + PartialEq> PointBallot<T> {
    fn new() -> Self {
        Self {
            next_seq: 0,
            per_seat: std::array::from_fn(|_| None),
        }
    }

    fn record(&mut self, seat: usize, valid_choice: Option<T>) {
        if let Some(choice) = valid_choice {
            let seq = self.next_seq;
            self.next_seq += 1;
            self.per_seat[seat] = Some((seq, choice));
        } else {
            self.per_seat[seat] = None;
        }
    }

    fn tally(self) -> Option<(usize, T)> {
        let mut tallies: Vec<(T, usize, u64, usize)> = Vec::new();
        for (s, entry_opt) in self.per_seat.into_iter().enumerate() {
            if let Some((seq, choice)) = entry_opt {
                if let Some(entry) = tallies.iter_mut().find(|(c, _, _, _)| *c == choice) {
                    entry.1 += 1;
                    if seq > entry.2 {
                        entry.2 = seq;
                        entry.3 = s;
                    }
                } else {
                    tallies.push((choice, 1, seq, s));
                }
            }
        }
        tallies
            .into_iter()
            .max_by_key(|&(_, count, latest_seq, _)| (count, latest_seq))
            .map(|(choice, _, _, actor)| (actor, choice))
    }
}

#[cfg(test)]
#[path = "pointing_test.rs"]
mod test;
