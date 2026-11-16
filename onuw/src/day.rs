//! Timed **Day Discussion** and public **Voting Phase** (`!ready`, `!time`, `!vote <nick>`).

use std::collections::HashSet;
use std::convert::Infallible;
use std::time::Duration;

use ircgm::prelude::*;
use tokio::time::Instant;

use crate::board::{BoardCards, Roster};

pub(crate) fn is_active_in_room(room: &Room, user: &User) -> bool {
    user.is_online() && room.has_user(user)
}

pub async fn run_day_phase(
    room: &Room,
    roster: &Roster,
    board: &BoardCards,
    day_duration: Duration,
) -> Result<()> {
    let shielded = board.shielded_seats();
    if !shielded.is_empty() {
        room.say(irc_format!(
            "🛡️ {}: {}",
            "Shield Token rests on".bold(),
            roster.format_indices(&shielded).bold().light_cyan(),
        ))?;
    }

    let artifacts = board.artifact_seats();
    if !artifacts.is_empty() {
        room.say(irc_format!(
            "🏺 {}: {}",
            "Artifact Token rests on".bold(),
            roster.format_indices(&artifacts).bold().yellow(),
        ))?;
    }

    for seat in 0..roster.len() {
        if board.is_player_face_up(seat) {
            room.say(irc_format!(
                "{} {}'s card is publicly {}!",
                "🔦 FACE-UP CARD:".bold().yellow(),
                roster.seats[seat].user.nick().bold(),
                board.player_card(seat).display_for_peek(),
            ))?;
        }
    }

    for c_idx in 0..3 {
        if board.is_center_face_up(c_idx) {
            room.say(irc_format!(
                "🔦 {}: {}",
                format!("FACE-UP CENTER #{}", c_idx + 1).bold().yellow(),
                board.center_card(c_idx).display_for_peek(),
            ))?;
        }
    }

    let deadline = Instant::now() + day_duration;
    let total_secs = day_duration.as_secs();
    let dur_label = if total_secs % 60 == 0 && total_secs >= 60 {
        format!("{} minutes", total_secs / 60)
    } else {
        format!("{total_secs}s")
    };
    let prompt = irc_format!(
        "{} You have {dur_label} to debate! Type {} in channel when ready to vote early ({} checks clock).",
        "☀️ EVERYONE WAKE UP!".bold().yellow(),
        "!ready".bold().green(),
        "!time".bold().cyan(),
    );

    let mut day_cfg = AskConfig::new().reply_deadline(deadline);
    if day_duration > Duration::from_secs(30) {
        day_cfg = day_cfg.warn_remaining(
            Duration::from_secs(30),
            "⏳ 30 seconds left in Day discussion!".yellow(),
        );
    }

    let enrolled: Vec<User> = roster.seats.iter().map(|s| s.user.clone()).collect();
    let mut ready_seats: HashSet<usize> = HashSet::new();

    let answers = room
        .ask_many(prompt, |msg| {
            let Some(seat_idx) = roster
                .seats
                .iter()
                .position(|s| s.user.id() == msg.sender.id())
            else {
                return AskMatch::ignore();
            };
            if msg.is_cmd("!time") {
                let rem = deadline.saturating_duration_since(Instant::now()).as_secs();
                let _ = msg.reply(format!("⏳ {rem}s remaining in Day discussion."));
                return AskMatch::ignore();
            }
            if !msg.is_cmd("!ready") {
                return AskMatch::ignore();
            }
            ready_seats.insert(seat_idx);
            let mut active_total = 0usize;
            let mut ready_count = 0usize;
            for (i, seat) in roster.seats.iter().enumerate() {
                if is_active_in_room(room, &seat.user) {
                    active_total += 1;
                    if ready_seats.contains(&i) {
                        ready_count += 1;
                    }
                }
            }
            AskMatch::accept_say(
                (),
                irc_format!(
                    "🗳️ {} is ready to vote ({ready_count}/{active_total}).",
                    msg.sender.nick().bold(),
                ),
            )
        })
        .from_users(&enrolled)
        .config(day_cfg)
        .await?;

    let active_now = roster
        .seats
        .iter()
        .filter(|s| is_active_in_room(room, &s.user))
        .count();
    if active_now > 0 && answers.len() >= active_now {
        room.say("🗳️ Everyone is ready — moving to the vote!".bold().green())?;
    }
    Ok(())
}

pub async fn run_voting_phase(
    bot: &Bot,
    room: &Room,
    roster: &Roster,
    vote_timeout: Duration,
    warn_remaining: Option<Duration>,
) -> Result<Vec<Option<usize>>> {
    let enrolled: Vec<User> = roster.seats.iter().map(|s| s.user.clone()).collect();
    let players_str = roster.format_players();

    let mut vote_cfg = AskConfig::new().reply_timeout(vote_timeout);
    if let Some(w) = warn_remaining.filter(|&w| w < vote_timeout) {
        vote_cfg = vote_cfg.warn_remaining(
            w,
            irc_format!(
                "⏳ {} {}",
                format!("Hurry — {}s left to", w.as_secs()).yellow(),
                "!vote <nick>!".bold().yellow(),
            ),
        );
    }

    let dm_reminder = bot.inbox_dms_from(&enrolled).on_map(|msg| {
        if msg.is_cmd("!vote") || roster.resolve_nick(&msg.text).is_some() {
            let _ = msg.reply(irc_format!(
                "🗳️ {} {} {}",
                "Voting is public — type".yellow(),
                "!vote <nick>".bold().yellow(),
                format!("in {}!", room.name()).yellow(),
            ));
        }
        None::<Infallible>
    });

    let prompt = irc_format!(
        "{} Type {} in channel ({}s)! Players: {players_str}",
        "🗳️ 3... 2... 1... POINT!".bold().red(),
        "!vote <nick>".bold().green(),
        vote_timeout.as_secs(),
    );

    let vote_fut = room
        .ask_many(prompt, |msg| {
            let Some(voter_idx) = roster
                .seats
                .iter()
                .position(|s| s.user.id() == msg.sender.id())
            else {
                return AskMatch::ignore();
            };
            let Some(raw_args) = msg.cmd_args("!vote") else {
                return AskMatch::ignore();
            };
            if raw_args.is_empty() {
                return AskMatch::reject(irc_format!(
                    "{} {}",
                    "Usage:".yellow(),
                    "!vote <nick>.".bold().yellow(),
                ));
            }
            match roster.resolve_nick(raw_args) {
                Some(idx) if idx == voter_idx => {
                    AskMatch::reject("You cannot point at yourself!".yellow())
                }
                Some(idx) => AskMatch::accept_say(
                    idx,
                    irc_format!(
                        "👉 {} points at {}!",
                        msg.sender.nick().bold(),
                        roster.seats[idx].user.nick().bold(),
                    ),
                ),
                None => AskMatch::reject("Unknown player nickname.".yellow()),
            }
        })
        .from_users(&enrolled)
        .config(vote_cfg);

    let Outcome::Completed(res) = vote_fut.until(dm_reminder).await;
    let answers = res?;

    Ok(roster
        .seats
        .iter()
        .map(|seat| answers.get(&seat.user).copied())
        .collect())
}
