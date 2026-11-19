//! Async **Secret Hitler** game orchestrator: role DMs, nomination, simultaneous secret DM
//! ballots, silent Legislative Sessions with Veto, Executive Actions, live status queries, and
//! deferred `!reveal` chronicle.

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ircgm::prelude::*;
use tokio::time::Instant;

use crate::lobby::{
    format_role_breakdown, handle_catalog_query, recruit_players, send_active_game_help,
    send_rules_summary,
};
use crate::state::{
    format_policies, hitler_knows_fascists, parse_chancellor_choice, parse_president_discard,
    parse_vote, role_distribution, BoardState, ChancellorChoice, ExecPower, Party, Policy,
    PolicyDeck, Role, Roster, SeatInfo, WinReason, HITLER_TERRITORY_THRESHOLD, MAX_PLAYERS,
    MIN_PLAYERS, VETO_UNLOCK_THRESHOLD,
};

#[derive(Clone, Debug)]
pub struct GameConfig {
    pub lobby_duration: Duration,
    pub nominate_timeout: Duration,
    pub vote_timeout: Duration,
    pub legislative_timeout: Duration,
    pub executive_timeout: Duration,
    pub warn_remaining: Option<Duration>,
}

impl GameConfig {
    const fn secs(lobby: u64, nom: u64, vote: u64, leg: u64, exec: u64, warn: Option<u64>) -> Self {
        Self {
            lobby_duration: Duration::from_secs(lobby),
            nominate_timeout: Duration::from_secs(nom),
            vote_timeout: Duration::from_secs(vote),
            legislative_timeout: Duration::from_secs(leg),
            executive_timeout: Duration::from_secs(exec),
            warn_remaining: match warn {
                Some(w) => Some(Duration::from_secs(w)),
                None => None,
            },
        }
    }

    pub const fn for_tests() -> Self {
        Self::secs(15, 15, 10, 10, 10, None)
    }

    fn ask_cfg(&self, timeout: Duration, warn: impl FnOnce(u64) -> Text) -> AskConfig {
        let mut cfg = AskConfig::new().reply_timeout(timeout);
        if let Some(w) = self.warn_remaining.filter(|&w| w < timeout) {
            cfg = cfg.warn_remaining(w, warn(w.as_secs()));
        }
        cfg
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self::secs(500, 180, 90, 90, 120, Some(20))
    }
}

#[derive(Clone, Debug)]
pub struct GameSetup {
    pub roster: Roster,
    pub deck: PolicyDeck,
    pub initial_president: usize,
}

impl GameSetup {
    pub fn random(mut players: Vec<User>) -> Result<Self> {
        let Some((libs, fas)) = role_distribution(players.len()) else {
            return Err(Error::InvalidInput(
                "Secret Hitler requires between 5 and 10 players",
            ));
        };
        let mut roles = vec![Role::Hitler];
        roles.extend(std::iter::repeat_n(Role::Fascist, fas));
        roles.extend(std::iter::repeat_n(Role::Liberal, libs));
        fastrand::shuffle(&mut roles);
        fastrand::shuffle(&mut players);
        let initial_president = fastrand::usize(..players.len());
        let seats = players
            .into_iter()
            .zip(roles)
            .map(|(user, role)| SeatInfo { user, role })
            .collect();
        Ok(Self {
            roster: Roster { seats },
            deck: PolicyDeck::new_shuffled(),
            initial_president,
        })
    }

    pub fn custom(seats: Vec<SeatInfo>, deck: PolicyDeck, initial_president: usize) -> Self {
        assert!(
            (MIN_PLAYERS..=MAX_PLAYERS).contains(&seats.len()) && initial_president < seats.len()
        );
        Self {
            roster: Roster { seats },
            deck,
            initial_president,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReveal {
    lines: Vec<Text>,
}

impl GameReveal {
    pub fn lines(&self) -> &[Text] {
        &self.lines
    }

    pub fn into_lines(self) -> Vec<Text> {
        self.lines
    }

    pub fn announce(&self, room: &Room) -> Result<()> {
        for line in &self.lines {
            room.say(line.clone())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayOutcome {
    Completed(GameReveal),
    NotEnoughPlayers,
}

#[derive(Debug)]
pub struct GameSummary {
    pub roster: Roster,
    pub final_board: BoardState,
    pub win_reason: WinReason,
    pub winning_party: Party,
    pub winners: Vec<(usize, &'static str)>,
    pub chronicle: Vec<Text>,
    pub reveal: GameReveal,
}

#[derive(Clone)]
struct LiveSnapshot {
    board: BoardState,
    phase: &'static str,
    chancellor: Option<usize>,
    deadline: Option<Instant>,
}

fn is_info_cmd(msg: &ChatMessage) -> bool {
    let Some(cmd) = msg.text.trim().split_whitespace().next() else {
        return false;
    };
    matches!(
        cmd.to_ascii_lowercase().as_str(),
        "!board"
            | "!status"
            | "!players"
            | "!who"
            | "!eligible"
            | "!tracks"
            | "!track"
            | "!boards"
            | "!powers"
            | "!rules"
            | "!role"
            | "!myrole"
            | "!time"
            | "!help"
            | "!games"
            | "!abort"
            | "!stop"
            | "!quit"
    )
}

fn format_eligible_line(roster: &Roster, board: &BoardState) -> Text {
    let elig = board.eligible_chancellors();
    let tl = board.term_limited_seats();
    if tl.is_empty() {
        irc_format!(
            "✅ {}: {}",
            "Eligible Chancellors".bold(),
            roster.format_seats(&elig)
        )
    } else {
        irc_format!(
            "✅ {}: {} {} {}: {}",
            "Eligible Chancellors".bold(),
            roster.format_seats(&elig),
            "·".grey(),
            "Term-limited".yellow(),
            roster.format_seats(&tl),
        )
    }
}

pub fn format_secret_role_dm(roster: &Roster, seat: usize, board: &BoardState) -> Text {
    let n = roster.len();
    let dead = if board.is_alive(seat) {
        Text::default()
    } else {
        " (💀 Executed)".grey()
    };
    let hitler = roster.nick(roster.hitler_seat());
    let reg_fas: Vec<usize> = (0..n)
        .filter(|&i| roster.seats[i].role == Role::Fascist)
        .collect();

    match roster.seats[seat].role {
        Role::Liberal => irc_format!(
            "🔒 Your Secret Role: {} (Party: {}){dead} — Enact 5 Liberal Policies or execute Hitler to win!",
            "LIBERAL".bold().cyan(),
            Party::Liberal.styled(),
        ),
        Role::Fascist => {
            let fellow: Vec<usize> = reg_fas.into_iter().filter(|&i| i != seat).collect();
            let vis = if hitler_knows_fascists(n) {
                "Hitler knows who you are"
            } else {
                "Hitler does NOT know who the Fascists are"
            };
            let peers = if fellow.is_empty() {
                irc_format!(
                    "Hitler is {} ({vis}). Enact 6 Fascist Policies or elect Hitler Chancellor at 3+ Fascist Policies!",
                    hitler.as_str().bold().orange()
                )
            } else {
                irc_format!(
                    "Hitler: {} · Fellow Fascist(s): {} ({vis}).",
                    hitler.as_str().bold().orange(),
                    roster.format_seats(&fellow)
                )
            };
            irc_format!(
                "🔒 Your Secret Role: {} (Party: {}){dead} — {peers}",
                "FASCIST".bold().red(),
                Party::Fascist.styled()
            )
        }
        Role::Hitler if hitler_knows_fascists(n) => irc_format!(
            "🔒 Your Secret Role: {} (Party: {}){dead} — Your Fascist teammate is {}. If elected Chancellor at 3+ Fascist Policies, Fascists win; if executed, Liberals win!",
            "HITLER".bold().orange(),
            Party::Fascist.styled(),
            roster.format_seats(&reg_fas),
        ),
        Role::Hitler => irc_format!(
            "🔒 Your Secret Role: {} (Party: {}){dead} — You do NOT know who the Fascists are, though they know who you are. If elected Chancellor at 3+ Fascist Policies, Fascists win; if executed, Liberals win!",
            "HITLER".bold().orange(),
            Party::Fascist.styled(),
        ),
    }
}

fn handle_live_query(room: &Room, roster: &Roster, live: &Mutex<LiveSnapshot>, msg: &ChatMessage) {
    if !(msg.is_in(room) || msg.is_dm()) {
        return;
    }
    let s = live.lock().expect("live lock").clone();
    if msg.is_cmd("!board") || msg.is_cmd("!status") {
        let _ = msg.reply(s.board.format_board_banner(roster));
        let phase = match s.chancellor {
            Some(c) => format!("Phase: {} (Chancellor: {})", s.phase, roster.nick(c)),
            None => format!("Phase: {}", s.phase),
        };
        let _ = msg.reply(irc_format!(
            "  {} · {}",
            phase.grey(),
            format_eligible_line(roster, &s.board)
        ));
    } else if msg.is_cmd("!players") || msg.is_cmd("!who") {
        let _ = msg.reply(irc_format!(
            "👥 {} ({}/{} alive): {}",
            "Table order".bold(),
            s.board.alive_count(),
            s.board.total_players,
            roster.format_table_order(&s.board),
        ));
    } else if msg.is_cmd("!eligible") {
        let _ = msg.reply(format_eligible_line(roster, &s.board));
    } else if msg.is_cmd("!time") {
        let line = match s.deadline {
            Some(dl) => format!(
                "⏳ {}s remaining in {}.",
                dl.saturating_duration_since(Instant::now()).as_secs(),
                s.phase
            ),
            None => format!("⏳ Currently in {}.", s.phase),
        };
        let _ = msg.reply(line);
    } else if msg.is_cmd("!role") || msg.is_cmd("!myrole") {
        if msg.is_dm() {
            let reply = match roster.seat_of(&msg.sender) {
                Some(seat) => format_secret_role_dm(roster, seat, &s.board),
                None => "You are not enrolled as a player in this game.".yellow(),
            };
            let _ = msg.reply(reply);
        } else if let Some(b) = format_role_breakdown(roster.len()) {
            let _ = msg.reply(irc_format!(
                "🃏 {}-player setup: {b} — send {} in DM to check your own secret role!",
                roster.len(),
                "!role".bold().cyan(),
            ));
        }
    } else if msg.is_cmd("!help") {
        let _ = send_active_game_help(msg);
    } else if msg.is_cmd("!rules") {
        let _ = send_rules_summary(msg);
    } else {
        let _ = handle_catalog_query(room, msg);
    }
}

struct Session<'a> {
    bot: &'a Bot,
    room: &'a Room,
    roster: Roster,
    board: BoardState,
    config: &'a GameConfig,
    mod_guard: Option<ModerationGuard>,
    live: Arc<Mutex<LiveSnapshot>>,
    chronicle: Vec<Text>,
}

impl<'a> Session<'a> {
    fn bold(&self, seat: usize) -> Text {
        self.roster.nick(seat).bold().into()
    }

    fn set_phase(&mut self, phase: &'static str, chancellor: Option<usize>, timeout: Duration) {
        {
            let mut s = self.live.lock().expect("live lock");
            s.board = self.board.clone();
            s.phase = phase;
            s.chancellor = chancellor;
            s.deadline = Some(Instant::now() + timeout);
        }
        if let Some(g) = &mut self.mod_guard {
            let _ = g.set_topic(self.board.format_topic(&self.roster, phase));
        }
    }

    async fn ask_target(
        &self,
        eligible: &[usize],
        cmds: &[&str],
        prompt: Text,
        validate: impl Fn(usize) -> Option<&'static str> + Send,
        timeout: Duration,
        timeout_verb: &str,
    ) -> Result<usize> {
        let pres = self.board.current_president;
        let pres_u = &self.roster.seats[pres].user;
        let primary = cmds[0];
        let cfg = self.config.ask_cfg(timeout, |r| {
            irc_format!(
                "⏳ {}: {r}s left to use {}!",
                self.bold(pres),
                format!("{primary} <nick>").bold().yellow()
            )
        });
        match self
            .room
            .ask_user(pres_u, prompt, move |m| {
                let Some(raw) = cmds.iter().find_map(|c| m.cmd_args(c)) else {
                    return AskMatch::ignore();
                };
                let arg = raw.trim();
                if arg.is_empty() {
                    return AskMatch::reject(format_args!("Usage: {primary} <nick>.").yellow());
                }
                let Some(seat) = self.roster.resolve_nick(arg) else {
                    return AskMatch::reject("Unknown player nickname.".yellow());
                };
                match validate(seat) {
                    Some(reason) => AskMatch::reject(reason.yellow()),
                    None => AskMatch::accept(seat),
                }
            })
            .config(cfg)
            .await?
        {
            Answer::Ok(seat) => Ok(seat),
            Answer::TimedOut => {
                let rnd = eligible[fastrand::usize(..eligible.len())];
                self.room.say(irc_format!(
                    "⏳ {} ran out of time and randomly {timeout_verb} {}!",
                    self.bold(pres),
                    self.bold(rnd),
                ))?;
                Ok(rnd)
            }
            other => other.required(),
        }
    }

    async fn ask_power_target(
        &self,
        eligible: &[usize],
        cmds: &[&str],
        prompt: Text,
        invalid_msg: &'static str,
        timeout_verb: &str,
    ) -> Result<usize> {
        let pres = self.board.current_president;
        self.ask_target(
            eligible,
            cmds,
            prompt,
            |s| {
                if s == pres {
                    Some("You cannot target yourself!")
                } else if !eligible.contains(&s) {
                    Some(invalid_msg)
                } else {
                    None
                }
            },
            self.config.executive_timeout,
            timeout_verb,
        )
        .await
    }

    async fn ask_dm<T: Send>(
        &self,
        seat: usize,
        prompt: Text,
        warn_fmt: impl FnOnce(u64) -> Text,
        mut matcher: impl FnMut(&ChatMessage) -> AskMatch<T> + Send,
    ) -> Result<Option<T>> {
        let cfg = self
            .config
            .ask_cfg(self.config.legislative_timeout, warn_fmt);
        match self.roster.seats[seat]
            .user
            .ask(prompt, move |m| {
                if is_info_cmd(m) {
                    AskMatch::ignore()
                } else {
                    matcher(m)
                }
            })
            .config(cfg)
            .await?
        {
            Answer::Ok(v) => Ok(Some(v)),
            Answer::TimedOut => Ok(None),
            other => other.required().map(Some),
        }
    }

    async fn hold_election(&self, chan: usize) -> Result<(usize, usize, bool)> {
        let pres = self.board.current_president;
        let bot_nick = self.bot.nick();
        let hz = if self.board.fas_policies >= HITLER_TERRITORY_THRESHOLD {
            irc_format!(
                " {}",
                "⚠️ Hitler Territory: if the Chancellor is Hitler and this passes, Fascists win!"
                    .bold()
                    .orange()
            )
        } else {
            Text::default()
        };
        self.room.say(irc_format!(
            "🗳️ {} nominates {} as Chancellor! Vote secretly via {} or {} ({}s).{hz}",
            self.roster.nick(pres).bold().yellow(),
            self.roster.nick(chan).bold().cyan(),
            format!("/msg {bot_nick} ja").bold().green(),
            "nein".bold().red(),
            self.config.vote_timeout.as_secs(),
        ))?;

        let alive = self.board.alive_seats();
        let living_users: Vec<User> = alive
            .iter()
            .map(|&s| self.roster.seats[s].user.clone())
            .collect();
        let mut dm_inbox = self.bot.inbox_dms_from(&living_users).lossy();
        let mut chan_inbox = self.room.inbox().lossy();
        for &s in &alive {
            let _ = self.roster.seats[s].user.say(irc_format!(
                "🗳️ Round {}: Approve President {} + Chancellor {}? Reply {} or {}.",
                self.board.round,
                self.bold(pres),
                self.bold(chan),
                "ja".bold().green(),
                "nein".bold().red(),
            ));
        }

        let deadline = Instant::now() + self.config.vote_timeout;
        let warn_at = self
            .config
            .warn_remaining
            .filter(|&w| w < self.config.vote_timeout)
            .map(|w| deadline - w);
        let mut warned = warn_at.is_none();
        let mut ballots = vec![None; self.roster.len()];

        loop {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(deadline) => break,
                _ = async { tokio::time::sleep_until(warn_at.unwrap()).await }, if !warned => {
                    warned = true;
                    let rem = self.config.warn_remaining.unwrap().as_secs();
                    self.room.say(format_args!("⏳ {rem}s left to cast secret ballots in DM!").yellow())?;
                    for &s in &alive {
                        if ballots[s].is_none() {
                            let _ = self.roster.seats[s].user.say(format_args!("⏳ {rem}s left to vote — reply ja or nein!").yellow());
                        }
                    }
                }
                dm = dm_inbox.next_chat() => {
                    let msg = dm?;
                    if is_info_cmd(&msg) { continue; }
                    let Some(seat) = self.roster.seat_of(&msg.sender).filter(|&s| self.board.is_alive(s)) else { continue; };
                    match parse_vote(&msg.text) {
                        Some(vote) => {
                            let updated = ballots[seat].replace(vote).is_some();
                            let word = if vote { "JA!".bold().green() } else { "NEIN!".bold().red() };
                            if !updated && alive.iter().all(|&s| ballots[s].is_some()) {
                                msg.reply(irc_format!("🗳️ Ballot recorded: {word}"))?;
                                break;
                            } else if updated {
                                msg.reply(irc_format!("🗳️ Ballot updated: {word}"))?;
                            } else {
                                msg.reply(irc_format!("🗳️ Ballot recorded: {word} {}", "(you may update it until all players vote)".grey()))?;
                            }
                        }
                        None => msg.reply("Reply ja (yes) or nein (no) to vote on the government.".yellow())?,
                    }
                }
                ch = chan_inbox.next_chat() => {
                    let msg = ch?;
                    if self.roster.seat_of(&msg.sender).is_some_and(|s| self.board.is_alive(s))
                        && ["!vote", "!ja", "!nein", "!yes", "!no"].iter().any(|c| msg.is_cmd(c))
                    {
                        msg.reply(irc_format!(
                            "🗳️ Ballots are secret — vote via {} or {}!",
                            format!("/msg {bot_nick} ja").bold().green(),
                            "nein".bold().red(),
                        ))?;
                    }
                }
            }
        }

        let timed_out: Vec<usize> = alive
            .iter()
            .copied()
            .filter(|&s| ballots[s].is_none())
            .collect();
        if !timed_out.is_empty() {
            self.room.say(irc_format!(
                "⏳ Did not vote in time (counted as {}): {}",
                "NEIN!".bold().red(),
                self.roster.format_seats(&timed_out),
            ))?;
        }
        let ja = alive.iter().filter(|&&s| ballots[s] == Some(true)).count();
        let nein = alive.len() - ja;
        let items: Vec<Text> = alive
            .iter()
            .map(|&s| {
                let v = if ballots[s] == Some(true) {
                    "JA!".bold().green()
                } else {
                    "NEIN!".bold().red()
                };
                irc_format!("{}: {v}", self.bold(s))
            })
            .collect();
        self.room.say(irc_format!(
            "📊 Votes ({ja} Ja / {nein} Nein) — {}",
            Text::join(&items, " · ".grey())
        ))?;
        Ok((ja, nein, ja > nein))
    }

    fn maybe_reshuffle(&mut self) -> Result<()> {
        if let Some(n) = self.board.deck.reshuffle_if_needed() {
            self.room.say(irc_format!(
                "🔀 Fewer than 3 tiles remained in the draw pile — reshuffled with discard pile ({} policies in deck).",
                n.to_string().bold(),
            ))?;
            self.chronicle
                .push(irc_format!("🔀 Policy deck reshuffled ({n} tiles)."));
        }
        Ok(())
    }

    fn announce_milestones(&self, policy: Policy) -> Result<()> {
        if policy == Policy::Fascist && self.board.fas_policies == HITLER_TERRITORY_THRESHOLD {
            self.room.say(
                "⚠️ 3rd Fascist Policy enacted — Hitler Territory is now active! Electing Hitler as Chancellor wins for the Fascists."
                    .bold()
                    .orange(),
            )?;
        } else if policy == Policy::Fascist && self.board.fas_policies == VETO_UNLOCK_THRESHOLD {
            self.room.say(
                "🛑 5th Fascist Policy enacted — Veto Power is now permanently unlocked!"
                    .bold()
                    .yellow(),
            )?;
        }
        Ok(())
    }

    fn enact_chaos(&mut self, reason: &str) -> Result<Option<WinReason>> {
        let (policy, win) = self.board.enact_chaos_policy();
        self.room.say(irc_format!(
            "🔥 {} {reason} — top policy enacted automatically: {}! (Term limits & Election Tracker reset; executive power ignored.)",
            "CHAOS!".bold().orange(),
            policy.label(),
        ))?;
        self.chronicle.push(irc_format!(
            "🔥 Chaos enacted {} (term limits cleared).",
            policy.label()
        ));
        if win.is_some() {
            return Ok(win);
        }
        self.announce_milestones(policy)?;
        self.maybe_reshuffle()?;
        Ok(None)
    }

    async fn run_legislative_and_resolve(
        &mut self,
        chan: usize,
        ja: usize,
        nein: usize,
    ) -> Result<Option<WinReason>> {
        let pres = self.board.current_president;
        let drawn = self.board.deck.draw_three();
        let mut hand = Vec::from(drawn);

        if let Some(g) = &mut self.mod_guard {
            let _ = g.devoice_many([&self.roster.seats[pres].user, &self.roster.seats[chan].user]);
        }
        let session_res = self
            .collect_legislative_choices(pres, chan, &mut hand)
            .await;
        if let Some(g) = &mut self.mod_guard {
            let _ = g.voice_many([&self.roster.seats[pres].user, &self.roster.seats[chan].user]);
        }
        let (pres_discarded, chan_received, veto_tried, enacted_idx_opt) = session_res?;

        let Some(enacted_idx) = enacted_idx_opt else {
            let chaos = self.board.record_consented_veto(chan_received);
            self.chronicle.push(irc_format!(
                "R{}: {} + {} ({ja} Ja / {nein} Nein) — {} drew [{}], discarded {} → {} vetoed [{}] with consent (Tracker {}/3).",
                self.board.round, self.bold(pres), self.bold(chan), self.bold(pres),
                format_policies(&drawn), pres_discarded.short(), self.bold(chan),
                format_policies(&chan_received), self.board.election_tracker,
            ));
            self.maybe_reshuffle()?;
            if !chaos {
                let warn = if self.board.election_tracker == 2 {
                    " — next failed vote or veto triggers Chaos!"
                } else {
                    ""
                };
                self.room.say(irc_format!(
                    "🛑 President {} consents to the {}! Both policies discarded — Election Tracker advances to {}/3{warn}",
                    self.bold(pres), "VETO".bold().yellow(), self.board.election_tracker,
                ))?;
                return Ok(None);
            }
            self.room.say(irc_format!(
                "🛑 President {} consents to the {} — Election Tracker reaches 3/3!",
                self.bold(pres),
                "VETO".bold().yellow(),
            ))?;
            return self.enact_chaos("Election Tracker reached 3/3");
        };

        let enacted = hand.remove(enacted_idx);
        let chan_discarded = hand[0];
        self.board.deck.discard([chan_discarded]);
        let vn = if veto_tried {
            " (after refused veto)"
        } else {
            ""
        };
        self.chronicle.push(irc_format!(
            "R{}: {} + {} ({ja} Ja / {nein} Nein) — {} drew [{}], discarded {} → {} received [{}], discarded {}, enacted {}{vn}.",
            self.board.round, self.bold(pres), self.bold(chan), self.bold(pres),
            format_policies(&drawn), pres_discarded.short(), self.bold(chan),
            format_policies(&chan_received), chan_discarded.short(), enacted.label(),
        ));
        self.room.say(irc_format!(
            "⚖️ Government ({} & {}) enacts a {} policy!",
            self.bold(pres),
            self.bold(chan),
            enacted.label(),
        ))?;

        let (win, power) = self.board.apply_enacted_policy(enacted, false);
        if win.is_some() {
            return Ok(win);
        }
        self.announce_milestones(enacted)?;
        // Reshuffle BEFORE Executive Action so Policy Peek inspects post-reshuffle tiles:
        self.maybe_reshuffle()?;
        if let Some(power) = power {
            self.set_phase(power.name(), Some(chan), self.config.executive_timeout);
            return self.run_power(power).await;
        }
        Ok(None)
    }

    async fn collect_legislative_choices(
        &mut self,
        pres: usize,
        chan: usize,
        hand: &mut Vec<Policy>,
    ) -> Result<(Policy, [Policy; 2], bool, Option<usize>)> {
        let pres_idx = match self
            .ask_dm(
                pres,
                irc_format!(
                    "📜 Legislative Session: You drew [{}]. Choose 1 policy to {}: reply {} or {}.",
                    format_policies(hand),
                    "DISCARD".bold().yellow(),
                    "liberal".bold().cyan(),
                    "fascist".bold().red(),
                ),
                |r| format!("⏳ {r}s left to discard 1 policy (liberal / fascist)!").yellow(),
                |m| match parse_president_discard(&m.text) {
                    Ok(p) => match hand.iter().position(|&c| c == p) {
                        Some(i) => AskMatch::accept_say(
                            i,
                            irc_format!(
                                "✔ Discarded {} — passing 2 policies to Chancellor {}.",
                                p.label(),
                                self.bold(chan)
                            ),
                        ),
                        None => AskMatch::reject(
                            format_args!("You do not hold a {} policy in this hand!", p.name())
                                .yellow(),
                        ),
                    },
                    Err(h) => AskMatch::reject(h.yellow()),
                },
            )
            .await?
        {
            Some(i) => i,
            None => {
                let _ = self.roster.seats[pres]
                    .user
                    .say("⏳ Timed out — 1 policy discarded at random.".yellow());
                self.room.say(irc_format!(
                    "⏳ {} took too long; 1 policy discarded at random and 2 passed to {}.",
                    self.bold(pres),
                    self.bold(chan),
                ))?;
                fastrand::usize(..hand.len())
            }
        };

        let pres_discarded = hand.remove(pres_idx);
        self.board.deck.discard([pres_discarded]);
        let chan_received = [hand[0], hand[1]];
        let mut veto_tried = false;

        loop {
            let can_veto = self.board.fas_policies >= VETO_UNLOCK_THRESHOLD && !veto_tried;
            let veto_hint = if can_veto {
                irc_format!(", or reply {}", "veto".bold().yellow())
            } else {
                Text::default()
            };
            let decision = match self
                .ask_dm(
                    chan,
                    irc_format!(
                        "📜 President {} passed you [{}]. Choose 1 policy to {}: reply {} or {}{veto_hint}.",
                        self.bold(pres),
                        format_policies(hand),
                        "ENACT".bold().green(),
                        "liberal".bold().cyan(),
                        "fascist".bold().red(),
                    ),
                    |r| format!("⏳ {r}s left to enact a policy (liberal / fascist)!").yellow(),
                    |m| match parse_chancellor_choice(&m.text, can_veto) {
                        Ok(ChancellorChoice::ProposeVeto) => AskMatch::accept_say(
                            None,
                            irc_format!("⏳ Veto requested — asking President {}...", self.bold(pres))
                                .yellow(),
                        ),
                        Ok(ChancellorChoice::Enact(p)) => match hand.iter().position(|&c| c == p) {
                            Some(i) => AskMatch::accept_say(
                                Some(i),
                                irc_format!("✔ Enacting {} policy!", p.label()),
                            ),
                            None => AskMatch::reject(
                                format_args!("You do not hold a {} policy in this hand!", p.name())
                                    .yellow(),
                            ),
                        },
                        Err(h) => AskMatch::reject(h.yellow()),
                    },
                )
                .await?
            {
                Some(d) => d,
                None => {
                    let _ = self.roster.seats[chan]
                        .user
                        .say("⏳ Timed out — 1 policy enacted at random.".yellow());
                    self.room.say(irc_format!(
                        "⏳ {} took too long; 1 policy enacted at random!",
                        self.bold(chan)
                    ))?;
                    Some(fastrand::usize(..hand.len()))
                }
            };

            if let Some(idx) = decision {
                return Ok((pres_discarded, chan_received, veto_tried, Some(idx)));
            }

            veto_tried = true;
            self.room.say(irc_format!(
                "🛑 Chancellor {} proposes to {} the agenda! Waiting for President {} in DM...",
                self.bold(chan),
                "VETO".bold().yellow(),
                self.bold(pres),
            ))?;
            let accepted = self
                .ask_dm(
                    pres,
                    irc_format!(
                        "🛑 Chancellor {} proposes to VETO [{}]. Consent to discard both and advance the Election Tracker? Reply {} or {}.",
                        self.bold(chan),
                        format_policies(hand),
                        "ja".bold().green(),
                        "nein".bold().red(),
                    ),
                    |r| format!("⏳ {r}s left to reply ja or nein on the Veto request!").yellow(),
                    |m| match parse_vote(&m.text) {
                        Some(true) => AskMatch::accept_say(true, "🛑 You consented to the Veto.".yellow()),
                        Some(false) => AskMatch::accept_say(false, "❌ You refused the Veto.".cyan()),
                        None => AskMatch::reject("Reply ja (yes) or nein (no).".yellow()),
                    },
                )
                .await?
                .unwrap_or(false);

            if accepted {
                return Ok((pres_discarded, chan_received, veto_tried, None));
            }
            self.room.say(irc_format!(
                "❌ President {} refuses the Veto! Chancellor {} must enact a policy.",
                self.bold(pres),
                self.bold(chan),
            ))?;
        }
    }

    async fn run_power(&mut self, power: ExecPower) -> Result<Option<WinReason>> {
        let pres = self.board.current_president;
        match power {
            ExecPower::Peek => {
                let top3 = self.board.deck.peek_top_three();
                self.roster.seats[pres].user.say(irc_format!(
                    "🔮 Policy Peek — Top 3 policies in draw pile (first → third): [{}].",
                    format_policies(&top3),
                ))?;
                self.room.say(irc_format!(
                    "🔮 {} — {} privately examines the top 3 policies in the draw pile.",
                    "Policy Peek".bold().yellow(),
                    self.bold(pres),
                ))?;
                self.chronicle.push(irc_format!(
                    "🔮 Policy Peek: {} saw [{}].",
                    self.bold(pres),
                    format_policies(&top3),
                ));
            }
            ExecPower::Investigate => {
                let elig = self.board.eligible_investigation_targets();
                if elig.is_empty() {
                    return Ok(None);
                }
                let target = self
                    .ask_power_target(
                        &elig,
                        &["!investigate", "!inspect"],
                        irc_format!(
                            "🔍 {}: inspect a player's Party Membership with {} — Eligible: {}",
                            self.roster.nick(pres).bold().yellow(),
                            "!investigate <nick>".bold().cyan(),
                            self.roster.format_seats(&elig),
                        ),
                        "That player is dead or has already been investigated in this game!",
                        "investigates",
                    )
                    .await?;
                self.board.mark_investigated(target);
                self.live.lock().expect("live lock").board = self.board.clone();
                let party = self.roster.seats[target].role.party();
                self.roster.seats[pres].user.say(irc_format!(
                    "🔍 Loyalty Investigation: {}'s Party Membership card is {}.",
                    self.bold(target),
                    party.styled(),
                ))?;
                self.room.say(irc_format!(
                    "🔍 {} — {} inspected {}'s Party Membership card!",
                    "Investigate Loyalty".bold().cyan(),
                    self.bold(pres),
                    self.bold(target),
                ))?;
                self.chronicle.push(irc_format!(
                    "🔍 Investigate Loyalty: {} inspected {} (saw Party: {}).",
                    self.bold(pres),
                    self.bold(target),
                    party.styled(),
                ));
            }
            ExecPower::SpecialElection => {
                let elig = self.board.eligible_other_living();
                let chosen = self
                    .ask_power_target(
                        &elig,
                        &["!elect", "!special"],
                        irc_format!(
                            "🎖️ {}: choose the next Presidential Candidate with {} — Eligible: {}",
                            self.roster.nick(pres).bold().yellow(),
                            "!elect <nick>".bold().green(),
                            self.roster.format_seats(&elig),
                        ),
                        "Choose another living player!",
                        "appoints",
                    )
                    .await?;
                self.board.special_president = Some(chosen);
                self.room.say(irc_format!(
                    "🎖️ {} — {} hands the next Presidency to {}! (Normal clockwise rotation resumes afterward.)",
                    "Special Election".bold().yellow(),
                    self.bold(pres),
                    self.roster.nick(chosen).bold().yellow(),
                ))?;
                self.chronicle.push(irc_format!(
                    "🎖️ Special Election: {} appointed {} as next Presidential Candidate.",
                    self.bold(pres),
                    self.bold(chosen),
                ));
            }
            ExecPower::Execution => {
                let elig = self.board.eligible_other_living();
                let victim = self
                    .ask_power_target(
                        &elig,
                        &["!kill", "!execute", "!shoot"],
                        irc_format!(
                            "🔫 {}: formally execute a player with {} — Living targets: {}",
                            self.roster.nick(pres).bold().red(),
                            "!kill <nick>".bold().red(),
                            self.roster.format_seats(&elig),
                        ),
                        "Choose another living player!",
                        "executes",
                    )
                    .await?;
                let vrole = self.roster.seats[victim].role;
                let win = self.board.execute_player(victim, vrole);
                self.live.lock().expect("live lock").board = self.board.clone();
                if let Some(g) = &mut self.mod_guard {
                    let _ = g.devoice(&self.roster.seats[victim].user);
                }
                if win.is_some() {
                    self.chronicle.push(irc_format!(
                        "🎯 Execution: {} executed {} ({}) — Liberals win!",
                        self.bold(pres),
                        self.bold(victim),
                        Role::Hitler.styled(),
                    ));
                    return Ok(win);
                }
                self.room.say(irc_format!(
                    "💀 {} formally executes {}! {} was {} (eliminated players may not speak, vote, or reveal their role).",
                    self.bold(pres),
                    self.bold(victim),
                    self.bold(victim),
                    "NOT Hitler".bold().yellow(),
                ))?;
                if self.board.alive_count() == 5 && self.board.total_players > 5 {
                    self.room.say(
                        "ℹ️ 5 players remain alive — only the last elected Chancellor is now term-limited!"
                            .cyan(),
                    )?;
                }
                self.chronicle.push(irc_format!(
                    "💀 Execution: {} executed {} ({}, not Hitler).",
                    self.bold(pres),
                    self.bold(victim),
                    vrole.styled(),
                ));
            }
        }
        Ok(None)
    }

    async fn run_round(&mut self) -> Result<Option<WinReason>> {
        let pres = self.board.advance_president();
        self.set_phase("Nomination", None, self.config.nominate_timeout);
        self.room
            .say(self.board.format_board_banner(&self.roster))?;

        let elig = self.board.eligible_chancellors();
        let tl = self.board.term_limited_seats();
        let tl_note = if tl.is_empty() {
            Text::default()
        } else {
            irc_format!(" (Term-limited: {})", self.roster.format_seats(&tl))
        };
        let chan = self
            .ask_target(
                &elig,
                &["!chancellor", "!chancelor", "!nominate", "!nom", "!chan"],
                irc_format!(
                    "🎩 {}: nominate a Chancellor with {} — Eligible: {}{tl_note}",
                    self.roster.nick(pres).bold().yellow(),
                    "!chancellor <nick>".bold().green(),
                    self.roster.format_seats(&elig),
                ),
                |s| self.board.chancellor_ineligible_reason(s),
                self.config.nominate_timeout,
                "nominates as Chancellor",
            )
            .await?;

        self.set_phase("Election", Some(chan), self.config.vote_timeout);
        let (ja, nein, passed) = self.hold_election(chan).await?;

        if !passed {
            let chaos = self.board.record_failed_election();
            self.chronicle.push(irc_format!(
                "R{}: {} + {} rejected ({ja} Ja / {nein} Nein, Tracker {}/3).",
                self.board.round,
                self.bold(pres),
                self.bold(chan),
                self.board.election_tracker,
            ));
            if !chaos {
                let warn = if self.board.election_tracker == 2 {
                    " — next failed vote or veto triggers Chaos!"
                } else {
                    ""
                };
                self.room.say(
                    format_args!(
                        "❌ Election failed ({}/3){warn}",
                        self.board.election_tracker
                    )
                    .yellow(),
                )?;
                return Ok(None);
            }
            return self.enact_chaos("Three failed governments in a row");
        }

        let in_hz = self.board.fas_policies >= HITLER_TERRITORY_THRESHOLD;
        if let Some(win) = self
            .board
            .record_elected_government(chan, self.roster.seats[chan].role)
        {
            self.chronicle.push(irc_format!(
                "R{}: {} + {} ({}) elected ({ja} Ja / {nein} Nein) in Hitler Territory — Fascists win!",
                self.board.round,
                self.bold(pres),
                self.bold(chan),
                Role::Hitler.styled(),
            ));
            return Ok(Some(win));
        }
        let hz_ok = if in_hz {
            irc_format!(
                " ({} is confirmed {}!)",
                self.bold(chan),
                "NOT Hitler".bold().cyan()
            )
        } else {
            Text::default()
        };
        self.room.say(irc_format!(
            "✅ Government elected: President {} & Chancellor {}!{hz_ok}",
            self.bold(pres),
            self.bold(chan),
        ))?;

        // Rule (Legislative Session): President & Chancellor may not communicate while in session.
        self.set_phase(
            "Legislative Session",
            Some(chan),
            self.config.legislative_timeout,
        );
        self.room.say(irc_format!(
            "🤫 Legislative Session — {} draws 3 policies in DM (President & Chancellor may not speak while in session).",
            self.bold(pres),
        ))?;
        self.run_legislative_and_resolve(chan, ja, nein).await
    }

    fn finish(mut self, win_reason: WinReason) -> Result<GameSummary> {
        if let Some(g) = self.mod_guard.take() {
            let _ = g.restore();
        }
        let winning_party = win_reason.winning_party();
        match win_reason {
            WinReason::FiveLiberalPolicies => self.room.say(
                "🕊️ 5 LIBERAL POLICIES ENACTED — LIBERALS WIN!"
                    .bold()
                    .cyan(),
            )?,
            WinReason::SixFascistPolicies => self
                .room
                .say("💀 6 FASCIST POLICIES ENACTED — FASCISTS WIN!".bold().red())?,
            WinReason::HitlerElectedChancellor { chancellor } => self.room.say(irc_format!(
                "💀 {} ({}) WAS ELECTED CHANCELLOR IN HITLER TERRITORY — FASCISTS WIN!",
                self.roster.nick(chancellor).bold().orange(),
                "HITLER".bold().orange(),
            ))?,
            WinReason::HitlerExecuted { president, hitler } => self.room.say(irc_format!(
                "🎯 {} EXECUTED {} ({}) — LIBERALS WIN!",
                self.roster.nick(president).bold().cyan(),
                self.roster.nick(hitler).bold().orange(),
                "HITLER".bold().orange(),
            ))?,
        }
        let winners: Vec<(usize, &'static str)> = (0..self.roster.len())
            .filter(|&i| self.roster.seats[i].role.party() == winning_party)
            .map(|i| (i, winning_party.name()))
            .collect();
        let w_str = winners
            .iter()
            .map(|&(i, l)| format!("{} ({l})", self.roster.nick(i)))
            .collect::<Vec<_>>()
            .join(", ");
        self.room.say(irc_format!(
            "{} {}",
            "🏆 Winners:".bold().green(),
            w_str.bold()
        ))?;

        let roles: Vec<Text> = (0..self.roster.len())
            .map(|i| {
                let dead = if self.board.is_alive(i) { "" } else { " 💀" };
                irc_format!(
                    "{}: {}{dead}",
                    self.bold(i),
                    self.roster.seats[i].role.styled()
                )
            })
            .collect();
        let mut lines: Vec<Text> = roles
            .chunks(5)
            .enumerate()
            .map(|(i, c)| {
                let pfx: Text = if i == 0 {
                    irc_format!("{} ", "🃏 Secret Roles:".bold())
                } else {
                    "  ".into()
                };
                pfx + Text::join(c, " · ".grey())
            })
            .collect();
        lines.push(irc_format!(
            "🏛️ {} Liberal {}/5 · Fascist {}/6 · Deck ({} draw, {} discard)",
            "Final Board:".bold(),
            self.board.lib_policies,
            self.board.fas_policies,
            self.board.deck.draw_len(),
            self.board.deck.discard_len(),
        ));
        lines.push("📜 What Happened:".bold().cyan().into());
        for (i, e) in self.chronicle.iter().enumerate() {
            lines.push(irc_format!("  {}. {e}", i + 1));
        }

        Ok(GameSummary {
            roster: self.roster,
            final_board: self.board,
            win_reason,
            winning_party,
            winners,
            chronicle: self.chronicle,
            reveal: GameReveal { lines },
        })
    }
}

pub async fn run_single_game(
    bot: &Bot,
    room: &Room,
    host: User,
    config: &GameConfig,
) -> Result<PlayOutcome> {
    let Some(players) = recruit_players(room, host, config).await? else {
        return Ok(PlayOutcome::NotEnoughPlayers);
    };
    let summary = run_dealt_game(bot, room, GameSetup::random(players)?, config).await?;
    Ok(PlayOutcome::Completed(summary.reveal))
}

/// Runs a complete game of **Secret Hitler** from a [`GameSetup`].
pub async fn run_dealt_game(
    bot: &Bot,
    room: &Room,
    setup: GameSetup,
    config: &GameConfig,
) -> Result<GameSummary> {
    let players: Vec<User> = setup.roster.seats.iter().map(|s| s.user.clone()).collect();
    let mut mod_guard = room.moderation_guard().ok();
    if let Some(g) = &mut mod_guard {
        let _ = g.set_moderated(true);
        let _ = g.voice_many(&players);
    }

    let board = BoardState::new(setup.roster.len(), setup.initial_president, setup.deck);
    if let Some(b) = format_role_breakdown(setup.roster.len()) {
        room.say(irc_format!(
            "🎩 {} ({} players) — {b}",
            "Secret Hitler begins!".bold().green(),
            setup.roster.len()
        ))?;
    }
    room.say(irc_format!(
        "👥 {}: {}",
        "Table order".bold(),
        setup.roster.format_table_order(&board)
    ))?;
    for s in 0..setup.roster.len() {
        let _ = setup.roster.seats[s]
            .user
            .say(format_secret_role_dm(&setup.roster, s, &board));
    }

    let live = Arc::new(Mutex::new(LiveSnapshot {
        board: board.clone(),
        phase: "Starting",
        chancellor: None,
        deadline: None,
    }));

    let info_responder = {
        let (room, roster, live) = (room.clone(), setup.roster.clone(), Arc::clone(&live));
        bot.inbox().on_map(move |msg| {
            handle_live_query(&room, &roster, &live, msg);
            None::<Infallible>
        })
    };

    let departure = {
        let (roster, live) = (setup.roster.clone(), Arc::clone(&live));
        let mut inbox = room.inbox_users(&players).lossy();
        async move {
            loop {
                match inbox.next_event().await {
                    Ok(
                        Event::Parted { user, reason, .. }
                        | Event::Kicked { user, reason, .. }
                        | Event::Quit { user, reason, .. },
                    ) => {
                        if roster
                            .seat_of(&user)
                            .is_some_and(|s| live.lock().expect("live lock").board.is_alive(s))
                        {
                            return Error::UserLeft {
                                nick: user.nick(),
                                reason,
                            };
                        }
                    }
                    Ok(_) => {}
                    Err(_) => std::future::pending::<()>().await,
                }
            }
        }
    };

    let mut session = Session {
        bot,
        room,
        roster: setup.roster,
        board,
        config,
        mod_guard,
        live,
        chronicle: Vec::new(),
    };

    let game_flow = async {
        let win_reason = loop {
            if let Some(win) = session.run_round().await? {
                break win;
            }
        };
        session.finish(win_reason)
    };

    enum Stop {
        Departed(Error),
        Info(Infallible),
    }
    match game_flow
        .until(async { Stop::Departed(departure.await) })
        .or_until(async { Stop::Info(info_responder.await) })
        .await
    {
        Outcome::Completed(res) => res,
        Outcome::Stopped(Stop::Departed(err)) => Err(err),
        Outcome::Stopped(Stop::Info(never)) => match never {},
    }
}

#[cfg(test)]
#[path = "game_test.rs"]
mod test;
