//! Async **Coup** turn & challenge engine: official 2-player starting card draft, reaction
//! windows (`!challenge` / `!block` / `!pass`), DM prompts, live queries, and deferred `!reveal`.

use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ircgm::prelude::*;
use tokio::time::{sleep_until, Instant};

use crate::actions::{
    parse_action, parse_block_role, parse_influence_choice, pick_role_index, Action, Claim,
};
use crate::lobby::{
    handle_catalog_query, recruit_players, send_active_game_help, send_roles_catalog,
};
use crate::state::{
    copies_per_role, format_numbered_roles, Allegiance, BoardState, GameMode, PlayerState, Role,
    Roster, ASSASSINATE_COST, COUP_COST, MAX_PLAYERS, MIN_PLAYERS,
};

#[derive(Clone, Debug)]
pub struct GameConfig {
    pub lobby_duration: Duration,
    pub action_timeout: Duration,
    pub reaction_timeout: Duration,
    pub dm_choice_timeout: Duration,
    pub warn_remaining: Option<Duration>,
}

impl GameConfig {
    pub const fn for_tests() -> Self {
        Self {
            lobby_duration: Duration::from_secs(15),
            action_timeout: Duration::from_secs(15),
            reaction_timeout: Duration::from_secs(10),
            dm_choice_timeout: Duration::from_secs(10),
            warn_remaining: None,
        }
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
        Self {
            lobby_duration: Duration::from_secs(500),
            action_timeout: Duration::from_secs(90),
            reaction_timeout: Duration::from_secs(20),
            dm_choice_timeout: Duration::from_secs(45),
            warn_remaining: Some(Duration::from_secs(15)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GameSetup {
    pub roster: Roster,
    pub board: BoardState,
}

impl GameSetup {
    pub fn random(mut players: Vec<User>, mode: GameMode) -> Result<Self> {
        let Some(copies) = copies_per_role(players.len()) else {
            return Err(Error::InvalidInput(
                "Coup requires between 2 and 10 players",
            ));
        };
        fastrand::shuffle(&mut players);
        let mut deck: Vec<Role> = mode
            .active_roles()
            .into_iter()
            .flat_map(|r| std::iter::repeat_n(r, copies))
            .collect();
        fastrand::shuffle(&mut deck);

        let first_team = if fastrand::bool() {
            Allegiance::Loyalist
        } else {
            Allegiance::Reformist
        };
        let n = players.len();
        let states = (0..n)
            .map(|i| {
                let cards = [deck.pop().expect("card"), deck.pop().expect("card")];
                let coins = if n == 2 && i == 0 { 1 } else { 2 };
                let team = mode.reformation.then(|| {
                    if i % 2 == 0 {
                        first_team
                    } else {
                        first_team.opposite()
                    }
                });
                PlayerState::new(cards, coins, team)
            })
            .collect();
        Ok(Self {
            roster: Roster { seats: players },
            board: BoardState::new(mode, states, deck, 0, 0),
        })
    }

    pub fn custom(seats: Vec<User>, board: BoardState) -> Self {
        assert_eq!(seats.len(), board.players.len());
        assert!((MIN_PLAYERS..=MAX_PLAYERS).contains(&seats.len()));
        Self {
            roster: Roster { seats },
            board,
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
    pub winner: usize,
    pub chronicle: Vec<Text>,
    pub reveal: GameReveal,
}

#[derive(Clone)]
struct LiveSnapshot {
    board: BoardState,
    phase: &'static str,
    deadline: Option<Instant>,
}

pub fn format_private_hand_dm(board: &BoardState, seat: usize) -> Text {
    let p = &board.players[seat];
    let cards: Vec<Text> = p
        .cards
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if c.revealed {
                irc_format!("{}: {} {}", i + 1, c.role.styled(), "(💀 lost)".grey())
            } else {
                irc_format!("{}: {}", i + 1, c.role.styled())
            }
        })
        .collect();
    let team = p.allegiance.map_or_else(Text::default, |a| {
        irc_format!(" · Allegiance: {}", a.styled())
    });
    irc_format!(
        "🔒 Your cards: {} · Coins: {}{team}",
        Text::join(&cards, " · ".grey()),
        format!("{}c", p.coins).bold().yellow(),
    )
}

fn is_info_cmd(msg: &ChatMessage) -> bool {
    msg.text.trim().split_whitespace().next().is_some_and(|c| {
        matches!(
            c.to_ascii_lowercase().as_str(),
            "!board"
                | "!status"
                | "!players"
                | "!who"
                | "!hand"
                | "!cards"
                | "!myrole"
                | "!role"
                | "!roles"
                | "!rules"
                | "!time"
                | "!help"
                | "!games"
                | "!abort"
                | "!stop"
                | "!quit"
        )
    })
}

pub(crate) async fn ask_user_dm<T: Send>(
    user: &User,
    config: &GameConfig,
    prompt: Text,
    warn_fmt: impl FnOnce(u64) -> Text,
    mut parse: impl FnMut(&ChatMessage) -> std::result::Result<T, String> + Send,
) -> Result<Option<T>> {
    let cfg = config.ask_cfg(config.dm_choice_timeout, warn_fmt);
    match user
        .ask(prompt, move |m| {
            if is_info_cmd(m) {
                AskMatch::ignore()
            } else {
                match parse(m) {
                    Ok(v) => AskMatch::accept(v),
                    Err(e) => AskMatch::reject(e.yellow()),
                }
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

async fn recv_room_until<T>(
    room: &Room,
    deadline: Instant,
    mut on_msg: impl FnMut(&ChatMessage) -> Result<Option<T>>,
) -> Result<Option<T>> {
    let mut inbox = room.inbox();
    match async {
        loop {
            if let Some(val) = on_msg(&inbox.next_chat().await?)? {
                return Ok::<T, Error>(val);
            }
        }
    }
    .until(sleep_until(deadline))
    .await
    {
        Outcome::Completed(res) => Ok(Some(res?)),
        Outcome::Stopped(()) => Ok(None),
    }
}

fn reply_private_hand(roster: &Roster, board: &BoardState, msg: &ChatMessage) {
    let reply = roster.seat_of(&msg.sender).map_or_else(
        || "You are not enrolled as a player in this game.".yellow(),
        |seat| format_private_hand_dm(board, seat),
    );
    let _ = msg.reply(reply);
}

fn handle_live_query(room: &Room, roster: &Roster, live: &Mutex<LiveSnapshot>, msg: &ChatMessage) {
    if !(msg.is_in(room) || msg.is_dm()) {
        return;
    }
    let s = live.lock().expect("live lock").clone();
    if msg.is_cmd("!board") || msg.is_cmd("!status") {
        let _ = msg.reply(s.board.format_board_banner(roster));
        let _ = msg.reply(irc_format!(
            "  {} · Turn: {}",
            format!("Phase: {}", s.phase).grey(),
            roster.nick(s.board.current_turn).bold()
        ));
    } else if msg.is_cmd("!players") || msg.is_cmd("!who") {
        let _ = msg.reply(irc_format!(
            "👥 {} ({}/{} alive): {}",
            "Table order".bold(),
            s.board.alive_count(),
            s.board.players.len(),
            s.board.format_seats(roster, false)
        ));
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
    } else if msg.is_cmd("!hand") || msg.is_cmd("!cards") || msg.is_cmd("!myrole") {
        if msg.is_dm() {
            reply_private_hand(roster, &s.board, msg);
        } else {
            let _ = msg.reply(irc_format!(
                "🔒 Send {} in DM to check your private cards!",
                "!hand".bold().cyan()
            ));
        }
    } else if let Some(arg) = msg.cmd_args("!role") {
        if arg.trim().is_empty() && msg.is_dm() {
            reply_private_hand(roster, &s.board, msg);
        } else {
            let _ = send_roles_catalog(msg, Some(arg), Some(s.board.mode));
        }
    } else if msg.is_cmd("!help") {
        let _ = send_active_game_help(msg, s.board.mode);
    } else {
        let _ = handle_catalog_query(room, msg, Some(s.board.mode));
    }
}

#[derive(Clone, Copy)]
enum ReactionWindow {
    Action {
        actor: usize,
        action: Action,
    },
    PostChallengeBlock {
        actor: usize,
        target: usize,
        action: Action,
    },
    BlockChallenge {
        actor: usize,
        blocker: usize,
        role: Role,
        action: Action,
    },
}

enum ReactionOutcome {
    Passed,
    Challenged(usize),
    Blocked { blocker: usize, role: Role },
}

pub(crate) struct Session<'a> {
    pub(crate) room: &'a Room,
    pub(crate) roster: Roster,
    pub(crate) board: BoardState,
    pub(crate) config: &'a GameConfig,
    mod_guard: Option<ModerationGuard>,
    live: Arc<Mutex<LiveSnapshot>>,
    chronicle: Vec<Text>,
}

impl<'a> Session<'a> {
    pub(crate) fn sync(&self, phase: &'static str, deadline: Option<Instant>) {
        let mut s = self.live.lock().expect("live lock");
        s.board = self.board.clone();
        s.phase = phase;
        s.deadline = deadline;
    }

    pub(crate) fn sync_board(&self) {
        self.live.lock().expect("live lock").board = self.board.clone();
    }

    pub(crate) fn say_and_log(&mut self, entry: Text) -> Result<()> {
        self.room.say(entry.clone())?;
        self.chronicle.push(entry);
        Ok(())
    }

    pub(crate) fn bold(&self, seat: usize) -> Text {
        self.roster.nick(seat).bold()
    }

    pub(crate) fn dm_hand(&self, seat: usize) {
        let _ = self.roster.seats[seat].say(format_private_hand_dm(&self.board, seat));
    }

    async fn prompt_turn_action(&mut self, actor: usize) -> Result<Action> {
        let deadline = Instant::now() + self.config.action_timeout;
        self.sync("Action", Some(deadline));

        let nick = self.roster.nick(actor);
        let coins = self.board.players[actor].coins;
        let secs = self.config.action_timeout.as_secs();
        self.room
            .say(self.board.format_board_banner(&self.roster))?;

        if self.board.must_coup(actor) {
            let targets: Vec<Text> = self
                .board
                .valid_targets(actor)
                .into_iter()
                .map(|t| self.bold(t))
                .collect();
            self.room.say(irc_format!(
                "⚔️ {} has {} (10+) and MUST {}! Targets: {} ({secs}s).",
                nick.as_str().bold().yellow(),
                format!("{coins}c").bold().yellow(),
                "!coup <nick>".bold().red(),
                Text::join(&targets, ", ".grey()),
            ))?;
        } else {
            let mut cmds = vec![
                "!income".bold(),
                "!aid".bold(),
                "!tax".bold().magenta(),
                "!steal <nick>".bold().cyan(),
                "!exchange".bold().green(),
            ];
            if coins >= ASSASSINATE_COST {
                cmds.push("!assassinate <nick>".bold().orange());
            }
            if coins >= COUP_COST {
                cmds.push("!coup <nick>".bold().red());
            }
            if self.board.mode.inquisitor {
                cmds.push("!examine <nick>".bold().yellow());
            }
            if self.board.mode.reformation && coins >= 1 {
                cmds.push("!convert [nick]".bold().cyan());
            }
            if self.board.mode.reformation && self.board.treasury > 0 {
                cmds.push("!embezzle".bold().yellow());
            }
            self.room.say(irc_format!(
                "🎲 {}'s turn ({coins}c) — {} ({secs}s).",
                nick.as_str().bold().yellow(),
                Text::join(&cmds, " · ".grey())
            ))?;
        }

        let (casemap, mode, actor_user) = (
            self.room.casemap(),
            self.board.mode,
            self.roster.seats[actor].clone(),
        );
        let chosen = recv_room_until(self.room, deadline, |msg| {
            if msg.sender != actor_user {
                return Ok(None);
            }
            match parse_action(&msg.text, mode, |t| self.roster.resolve_nick(t, casemap)) {
                Ok(None) => Ok(None),
                Ok(Some(action)) => match action.validate(&self.board, actor) {
                    Ok(()) => Ok(Some(action)),
                    Err(err) => {
                        msg.reply(err.yellow())?;
                        Ok(None)
                    }
                },
                Err(err) => {
                    msg.reply(err.yellow())?;
                    Ok(None)
                }
            }
        })
        .await?;

        if let Some(action) = chosen {
            return Ok(action);
        }
        if self.board.must_coup(actor) {
            let target = self.board.valid_targets(actor)[0];
            self.room.say(irc_format!(
                "⏰ {} timed out with 10+ coins — auto-Couping {}!",
                nick.bold(),
                self.bold(target)
            ))?;
            Ok(Action::Coup(target))
        } else {
            self.room.say(irc_format!(
                "⏰ {} timed out — auto-taking {} (+1c).",
                nick.bold(),
                "Income".bold()
            ))?;
            Ok(Action::Income)
        }
    }

    async fn wait_for_reaction(&mut self, window: ReactionWindow) -> Result<ReactionOutcome> {
        let mode = self.board.mode;
        let secs = self.config.reaction_timeout.as_secs();
        let blocker_names = |a: Action| {
            a.spec(mode)
                .blockers
                .iter()
                .map(|r| r.name())
                .collect::<Vec<_>>()
                .join("/")
        };

        let (phase, action, actor, claimant, allow_block, passers, prompt) = match window {
            ReactionWindow::Action { actor, action } => {
                let spec = action.spec(mode);
                let chal = spec.claim.is_challengeable();
                let passers = self
                    .board
                    .alive_seats()
                    .into_iter()
                    .filter(|&s| {
                        if chal {
                            s != actor
                        } else {
                            self.board.can_block_foreign_aid(actor, s)
                        }
                    })
                    .collect();
                let hint = match (chal, spec.blockers.is_empty()) {
                    (true, true) => irc_format!(
                        "{} or {}",
                        "!challenge".bold().red(),
                        "!pass".bold().green()
                    ),
                    (false, false) => irc_format!(
                        "{} (Duke) or {}",
                        "!block".bold().yellow(),
                        "!pass".bold().green()
                    ),
                    _ => irc_format!(
                        "{}, target may {} ({}), or {}",
                        "!challenge".bold().red(),
                        "!block".bold().yellow(),
                        blocker_names(action),
                        "!pass".bold().green()
                    ),
                };
                let desc = action.describe(&self.board, &self.roster, actor);
                (
                    "Reaction",
                    action,
                    actor,
                    chal.then_some(actor),
                    !spec.blockers.is_empty(),
                    passers,
                    irc_format!("📣 {desc} — {hint} ({secs}s)."),
                )
            }
            ReactionWindow::PostChallengeBlock {
                actor,
                target,
                action,
            } => (
                "Block Window",
                action,
                actor,
                None,
                true,
                vec![target],
                irc_format!(
                    "🛡️ {}, you may still {} ({}) or {} ({secs}s).",
                    self.roster.nick(target).bold().yellow(),
                    "!block".bold().yellow(),
                    blocker_names(action),
                    "!pass".bold().green()
                ),
            ),
            ReactionWindow::BlockChallenge {
                actor,
                blocker,
                role,
                action,
            } => (
                "Block Challenge",
                action,
                actor,
                Some(blocker),
                false,
                self.board
                    .alive_seats()
                    .into_iter()
                    .filter(|&s| s != blocker)
                    .collect(),
                irc_format!(
                    "🛡️ {} claims {} to block {}! Anyone may {} (or {}) ({secs}s).",
                    self.roster.nick(blocker).bold().yellow(),
                    role.styled(),
                    self.bold(actor),
                    "!challenge".bold().red(),
                    "!pass".bold().green()
                ),
            ),
        };

        let deadline = Instant::now() + self.config.reaction_timeout;
        self.sync(phase, Some(deadline));
        self.room.say(prompt)?;

        let mut passed = HashSet::new();
        let res = recv_room_until(self.room, deadline, |msg| {
            let Some(seat) = self.roster.seat_of(&msg.sender).filter(|&s| self.board.is_alive(s)) else {
                return Ok(None);
            };
            if msg.is_cmd("!challenge") || msg.is_cmd("!bs") || msg.is_cmd("!doubt") || msg.is_cmd("!call") {
                match claimant {
                    None => msg.reply("You cannot challenge right now.".yellow())?,
                    Some(c) if !self.board.can_challenge(c, seat) => msg.reply("You cannot challenge your own claim!".yellow())?,
                    Some(_) => return Ok(Some(ReactionOutcome::Challenged(seat))),
                }
                return Ok(None);
            }
            match parse_block_role(&msg.text, action, mode) {
                Ok(Some(role)) => {
                    if !allow_block {
                        msg.reply("You cannot block right now.".yellow())?;
                    } else if matches!(action, Action::ForeignAid) && seat == actor {
                        msg.reply("You cannot block your own Foreign Aid!".yellow())?;
                    } else if matches!(action, Action::ForeignAid) && !self.board.can_block_foreign_aid(actor, seat) {
                        msg.reply("You cannot block a teammate's Foreign Aid while opposing factions remain!".yellow())?;
                    } else if let Action::Assassinate(t) | Action::Steal(t) = action {
                        if seat != t {
                            msg.reply(format_args!("Only {} (the target) can block this action!", self.roster.nick(t)).yellow())?;
                        } else {
                            return Ok(Some(ReactionOutcome::Blocked { blocker: seat, role }));
                        }
                    } else {
                        return Ok(Some(ReactionOutcome::Blocked { blocker: seat, role }));
                    }
                    return Ok(None);
                }
                Err(err) => {
                    if claimant != Some(seat) {
                        msg.reply(err.yellow())?;
                    }
                    return Ok(None);
                }
                Ok(None) => {}
            }
            if (msg.is_cmd("!pass") || msg.is_cmd("!ok") || msg.is_cmd("!allow") || msg.is_cmd("!skip"))
                && passers.contains(&seat)
            {
                passed.insert(seat);
                if passers.iter().all(|s| passed.contains(s)) {
                    return Ok(Some(ReactionOutcome::Passed));
                }
            }
            Ok(None)
        })
        .await?;
        Ok(res.unwrap_or(ReactionOutcome::Passed))
    }

    pub(crate) async fn prompt_choose_live_slot(&self, seat: usize, prefix: &str) -> Result<usize> {
        let live = self.board.players[seat].live_slots();
        if live.len() == 1 || live[0].1 == live[1].1 {
            return Ok(live[0].0);
        }
        let roles: Vec<Role> = live.iter().map(|&(_, r)| r).collect();
        let prompt = irc_format!(
            "{prefix} — reply with {} or role name: {} ({}s)",
            "1 / 2".bold(),
            format_numbered_roles(&roles),
            self.config.dm_choice_timeout.as_secs(),
        );
        let live_parse = live.clone();
        Ok(ask_user_dm(
            &self.roster.seats[seat],
            self.config,
            prompt,
            |s| format!("⏰ {s}s left to choose a card!").yellow(),
            move |m| parse_influence_choice(&m.text, &live_parse),
        )
        .await?
        .unwrap_or(live[0].0))
    }

    pub(crate) async fn lose_influence(
        &mut self,
        seat: usize,
        reason: &str,
    ) -> Result<Option<usize>> {
        if !self.board.is_alive(seat) {
            return Ok(self.board.winner());
        }
        let live = self.board.players[seat].live_slots();
        let slot = if live.len() == 1 || live[0].1 == live[1].1 {
            live[0].0
        } else {
            self.sync(
                "Influence Loss",
                Some(Instant::now() + self.config.dm_choice_timeout),
            );
            self.room.say(irc_format!(
                "💔 {} must lose an influence ({reason}) — choosing in DM...",
                self.roster.nick(seat).bold().yellow()
            ))?;
            self.prompt_choose_live_slot(seat, &format!("💔 You lost an influence ({reason})"))
                .await?
        };

        let lost_role = self.board.players[seat].reveal_slot(slot);
        if self.board.players[seat].live_count() == 0 {
            if let Some(g) = &mut self.mod_guard {
                let _ = g.devoice(&self.roster.seats[seat]);
            }
            self.say_and_log(irc_format!(
                "💀 {} lost their last influence ({}) ({reason}) and is {}!",
                self.roster.nick(seat).bold().red(),
                lost_role.styled(),
                "eliminated".bold().red()
            ))?;
        } else {
            self.say_and_log(irc_format!(
                "💔 {} lost {} ({reason}) — 1 influence remaining.",
                self.bold(seat),
                lost_role.styled()
            ))?;
            self.dm_hand(seat);
        }
        self.sync("Resolving", None);
        Ok(self.board.winner())
    }

    /// Resolves any challenge (`Claim::Role(r)` or Reformation's inverted `Claim::NotDuke`).
    async fn resolve_challenge(
        &mut self,
        claimant: usize,
        claim: Claim,
        challenger: usize,
    ) -> Result<(bool, Option<usize>)> {
        let nick = self.roster.nick(claimant);
        match claim {
            Claim::Role(role) => match self.board.players[claimant].find_live_role(role) {
                Some(slot) => {
                    let new_role = self.board.swap_live_slots(claimant, &[slot])[0];
                    self.sync_board();
                    self.say_and_log(irc_format!(
                        "✅ {} challenged {}'s {} — {} PROVED it and drew a replacement card!",
                        self.bold(challenger),
                        nick.as_str().bold(),
                        role.styled(),
                        nick.as_str().bold().green()
                    ))?;
                    let _ = self.roster.seats[claimant].say(irc_format!(
                        "🔄 You returned {} to the Court Deck and drew: {}.",
                        role.styled(),
                        new_role.styled()
                    ));
                    self.dm_hand(claimant);
                    Ok((
                        true,
                        self.lose_influence(challenger, "lost challenge").await?,
                    ))
                }
                None => {
                    self.say_and_log(irc_format!(
                        "🚨 {} challenged {}'s {} — {} was BLUFFING!",
                        self.bold(challenger),
                        nick.as_str().bold(),
                        role.styled(),
                        nick.as_str().bold().red()
                    ))?;
                    Ok((
                        false,
                        self.lose_influence(claimant, "caught bluffing").await?,
                    ))
                }
            },
            Claim::NotDuke
                if self.board.players[claimant]
                    .find_live_role(Role::Duke)
                    .is_some() =>
            {
                self.say_and_log(irc_format!(
                    "🚨 {} challenged {}'s Embezzle — {} DOES hold a {}! Treasury stays at {}c.",
                    self.bold(challenger),
                    nick.as_str().bold(),
                    nick.as_str().bold().red(),
                    Role::Duke.styled(),
                    self.board.treasury
                ))?;
                Ok((
                    false,
                    self.lose_influence(claimant, "caught embezzling with Duke")
                        .await?,
                ))
            }
            Claim::NotDuke => {
                let live = self.board.players[claimant].live_slots();
                let slots: Vec<usize> = live.iter().map(|&(i, _)| i).collect();
                let shown: Vec<Text> = live.iter().map(|&(_, r)| r.styled()).collect();
                let drawn = self.board.swap_live_slots(claimant, &slots);
                self.sync_board();
                self.say_and_log(irc_format!(
                    "✅ {} challenged {}'s Embezzle — {} revealed {} (no {}) and replaced {} card(s) from the Court Deck!",
                    self.bold(challenger),
                    nick.as_str().bold(),
                    nick.as_str().bold().green(),
                    Text::join(&shown, ", ".grey()),
                    Role::Duke.styled(),
                    drawn.len(),
                ))?;
                self.dm_hand(claimant);
                Ok((
                    true,
                    self.lose_influence(challenger, "lost Embezzle challenge")
                        .await?,
                ))
            }
            Claim::None => Ok((true, None)),
        }
    }

    async fn run_turn(&mut self) -> Result<Option<usize>> {
        let actor = self.board.current_turn;
        let action = self.prompt_turn_action(actor).await?;
        let spec = action.spec(self.board.mode);

        if spec.cost > 0 {
            self.board.players[actor].coins -= spec.cost;
            if spec.goes_to_treasury {
                self.board.treasury += spec.cost;
            }
            self.sync_board();
        }

        // Stage 1: action reaction & optional action challenge (+ post-challenge target block):
        let block_claim = if !spec.claim.is_challengeable() && spec.blockers.is_empty() {
            None
        } else {
            match self
                .wait_for_reaction(ReactionWindow::Action { actor, action })
                .await?
            {
                ReactionOutcome::Passed => None,
                ReactionOutcome::Blocked { blocker, role } => Some((blocker, role)),
                ReactionOutcome::Challenged(challenger) => {
                    let (proved, win) = self
                        .resolve_challenge(actor, spec.claim, challenger)
                        .await?;
                    if !proved {
                        if spec.refund_on_challenge_fail {
                            self.board.players[actor].coins += spec.cost;
                            self.sync_board();
                            self.room.say(irc_format!(
                                "↩️ {}'s {}c Assassination fee is refunded.",
                                self.bold(actor),
                                spec.cost
                            ))?;
                        }
                        if win.is_some() {
                            return Ok(win);
                        }
                        self.board.advance_turn();
                        return Ok(None);
                    }
                    if win.is_some() {
                        return Ok(win);
                    }
                    // Official rule: target still gets a Block Window after a failed action challenge.
                    match spec
                        .target
                        .filter(|&t| !spec.blockers.is_empty() && self.board.is_alive(t))
                    {
                        Some(target) => match self
                            .wait_for_reaction(ReactionWindow::PostChallengeBlock {
                                actor,
                                target,
                                action,
                            })
                            .await?
                        {
                            ReactionOutcome::Blocked { blocker, role } => Some((blocker, role)),
                            _ => None,
                        },
                        None => None,
                    }
                }
            }
        };

        // Stage 2: resolve any claimed block (and optional block challenge):
        if let Some((blocker, role)) = block_claim {
            let (stood, win) = match self
                .wait_for_reaction(ReactionWindow::BlockChallenge {
                    actor,
                    blocker,
                    role,
                    action,
                })
                .await?
            {
                ReactionOutcome::Challenged(challenger) => {
                    self.resolve_challenge(blocker, Claim::Role(role), challenger)
                        .await?
                }
                _ => {
                    self.say_and_log(irc_format!(
                        "🛡️ {}'s {} block stands!",
                        self.bold(blocker),
                        role.styled()
                    ))?;
                    (true, None)
                }
            };
            if win.is_some() {
                return Ok(win);
            }
            if stood {
                self.board.advance_turn();
                return Ok(None);
            }
        }

        // Stage 3: execute the unblocked, unrefuted action:
        if let Some(win) = self.execute_action(actor, action).await? {
            return Ok(Some(win));
        }
        self.sync_board();
        self.board.advance_turn();
        Ok(None)
    }

    fn finish(self, winner: usize) -> Result<GameSummary> {
        self.room.say(irc_format!(
            "🏆 {} {} ({}c, {} surviving influence)!",
            self.roster.nick(winner).bold().green(),
            "wins Coup!".bold().yellow(),
            self.board.players[winner].coins,
            self.board.players[winner].live_count(),
        ))?;
        let final_cards: Vec<Text> = (0..self.roster.len())
            .map(|i| {
                let cards: Vec<Text> = self.board.players[i]
                    .cards
                    .iter()
                    .map(|c| {
                        if c.revealed {
                            irc_format!("💀{}", c.role.styled())
                        } else {
                            c.role.styled()
                        }
                    })
                    .collect();
                irc_format!("{}: {}", self.bold(i), Text::join(&cards, " / ".grey()))
            })
            .collect();
        let mut lines = vec![irc_format!(
            "🃏 {}: {}",
            "Final Cards".bold(),
            Text::join(&final_cards, " · ".grey())
        )];
        if !self.chronicle.is_empty() {
            lines.push(irc_format!("📜 {}:", "What Happened".bold()));
            lines.extend(self.chronicle.iter().cloned());
        }
        Ok(GameSummary {
            roster: self.roster,
            final_board: self.board,
            winner,
            chronicle: self.chronicle,
            reveal: GameReveal { lines },
        })
    }
}

/// Official *Coup* 2-player variant: each player picks 1 character from a 5-card set, then a 3rd
/// shuffled 5-card set deals 1 card each (leaving 3 in Court Deck); seat 0 starts with 1c, seat 1 with 2c.
async fn build_two_player_setup(
    room: &Room,
    mut players: Vec<User>,
    mode: GameMode,
    config: &GameConfig,
) -> Result<GameSetup> {
    fastrand::shuffle(&mut players);
    let active = mode.active_roles();
    let choices = format_numbered_roles(&active);
    room.say(irc_format!(
        "🃏 {} — {} and {}, check your DMs to choose 1 starting character ({}s)!",
        "2-Player Official Variant".bold().cyan(),
        players[0].nick().bold(),
        players[1].nick().bold(),
        config.dm_choice_timeout.as_secs(),
    ))?;

    async fn pick_one(
        user: &User,
        active: [Role; 5],
        choices: &Text,
        config: &GameConfig,
    ) -> Result<Role> {
        let prompt = irc_format!(
            "🃏 2-Player Setup — pick your first character (number 1–5 or role name): {} ({}s)",
            choices.clone(),
            config.dm_choice_timeout.as_secs(),
        );
        let picked = ask_user_dm(
            user,
            config,
            prompt,
            |s| format!("⏰ {s}s left to pick your starting character!").yellow(),
            move |m| {
                pick_role_index(&m.text, &active)
                    .map(|i| active[i])
                    .ok_or_else(|| "Pick a number 1–5 or one of the 5 active role names.".into())
            },
        )
        .await?;
        Ok(picked.unwrap_or_else(|| active[fastrand::usize(..active.len())]))
    }

    let (r0, r1) = tokio::try_join!(
        pick_one(&players[0], active, &choices, config),
        pick_one(&players[1], active, &choices, config),
    )?;
    let mut third_set = active.to_vec();
    fastrand::shuffle(&mut third_set);
    let (d0, d1) = (
        third_set.pop().expect("card"),
        third_set.pop().expect("card"),
    );
    let first_team = if fastrand::bool() {
        Allegiance::Loyalist
    } else {
        Allegiance::Reformist
    };
    let states = vec![
        PlayerState::new([r0, d0], 1, mode.reformation.then_some(first_team)),
        PlayerState::new(
            [r1, d1],
            2,
            mode.reformation.then_some(first_team.opposite()),
        ),
    ];
    Ok(GameSetup {
        roster: Roster { seats: players },
        board: BoardState::new(mode, states, third_set, 0, 0),
    })
}

fn acquire_mod_guard(room: &Room, players: &[User], mode: GameMode) -> Option<ModerationGuard> {
    let mut guard = room.moderation_guard().ok();
    if let Some(g) = &mut guard {
        let _ = g.set_moderated(true);
        let _ = g.voice_many(players);
        let _ = g.set_topic(format!("👑 {} in progress", mode.label()));
    }
    guard
}

async fn watch_departures(
    room: &Room,
    players: &[User],
    mut is_active: impl FnMut(&User) -> bool,
) -> Error {
    let mut inbox = room.inbox_users(players).lossy();
    loop {
        match inbox.next_event().await {
            Ok(
                Event::Parted { user, reason, .. }
                | Event::Kicked { user, reason, .. }
                | Event::Quit { user, reason, .. },
            ) if is_active(&user) => {
                return Error::UserLeft {
                    nick: user.nick(),
                    reason,
                };
            }
            Ok(_) => {}
            Err(_) => std::future::pending::<()>().await,
        }
    }
}

pub async fn run_single_game(
    bot: &Bot,
    room: &Room,
    host: User,
    initial_mode: GameMode,
    config: &GameConfig,
) -> Result<PlayOutcome> {
    let Some((players, mode)) = recruit_players(room, host, initial_mode, config).await? else {
        return Ok(PlayOutcome::NotEnoughPlayers);
    };
    Ok(PlayOutcome::Completed(
        run_live_game(bot, room, players, mode, config)
            .await?
            .reveal,
    ))
}

pub async fn run_live_game(
    bot: &Bot,
    room: &Room,
    players: Vec<User>,
    mode: GameMode,
    config: &GameConfig,
) -> Result<GameSummary> {
    if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&players.len()) {
        return Err(Error::InvalidInput(
            "Coup requires between 2 and 10 players",
        ));
    }
    let mod_guard = acquire_mod_guard(room, &players, mode);
    let setup = if players.len() == 2 {
        let draft_players = players.clone();
        match build_two_player_setup(room, players, mode, config)
            .until(watch_departures(room, &draft_players, |_| true))
            .await
        {
            Outcome::Completed(res) => res?,
            Outcome::Stopped(err) => return Err(err),
        }
    } else {
        GameSetup::random(players, mode)?
    };
    run_dealt_game_with_guard(bot, room, setup, config, mod_guard).await
}

pub async fn run_dealt_game(
    bot: &Bot,
    room: &Room,
    setup: GameSetup,
    config: &GameConfig,
) -> Result<GameSummary> {
    let mod_guard = acquire_mod_guard(room, &setup.roster.seats, setup.board.mode);
    run_dealt_game_with_guard(bot, room, setup, config, mod_guard).await
}

async fn run_dealt_game_with_guard(
    bot: &Bot,
    room: &Room,
    setup: GameSetup,
    config: &GameConfig,
    mod_guard: Option<ModerationGuard>,
) -> Result<GameSummary> {
    let players = setup.roster.seats.clone();
    room.say(irc_format!(
        "👑 {} ({} players, {} cards in Court Deck) — check your DMs for your cards!",
        format!("{} begins!", setup.board.mode.label())
            .bold()
            .green(),
        setup.roster.len(),
        setup.board.deck.len(),
    ))?;
    room.say(irc_format!(
        "👥 {}: {}",
        "Table order".bold(),
        setup.board.format_seats(&setup.roster, false)
    ))?;
    for s in 0..setup.roster.len() {
        let _ = setup.roster.seats[s].say(format_private_hand_dm(&setup.board, s));
    }

    let live = Arc::new(Mutex::new(LiveSnapshot {
        board: setup.board.clone(),
        phase: "Starting",
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
        watch_departures(room, &players, move |u| {
            roster
                .seat_of(u)
                .is_some_and(|s| live.lock().expect("live lock").board.is_alive(s))
        })
    };

    let mut session = Session {
        room,
        roster: setup.roster,
        board: setup.board,
        config,
        mod_guard,
        live,
        chronicle: Vec::new(),
    };
    let game_flow = async {
        let winner = loop {
            if let Some(w) = session.board.winner() {
                break w;
            }
            if let Some(w) = session.run_turn().await? {
                break w;
            }
        };
        session.finish(winner)
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
