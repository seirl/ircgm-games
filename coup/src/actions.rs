//! Single source of truth for every **Coup** / **Inquisitor** / **Reformation** action:
//! declarative [`ActionSpec`], validation, chat/DM parsers, description, and async execution.

use ircgm::prelude::*;
use tokio::time::Instant;

use crate::game::{ask_user_dm, Session};
use crate::state::{
    format_numbered_roles, parse_role, BoardState, GameMode, Role, Roster, ASSASSINATE_COST,
    CONVERT_OTHER_COST, CONVERT_SELF_COST, COUP_COST,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim {
    None,
    Role(Role),
    /// *Coup: Reformation* Embezzle: publicly claims NOT to hold the Duke.
    NotDuke,
}

impl Claim {
    pub const fn is_challengeable(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionSpec {
    pub cost: u32,
    pub goes_to_treasury: bool,
    pub refund_on_challenge_fail: bool,
    pub claim: Claim,
    pub blockers: &'static [Role],
    pub target: Option<usize>,
}

impl ActionSpec {
    const fn basic(
        cost: u32,
        claim: Claim,
        blockers: &'static [Role],
        target: Option<usize>,
    ) -> Self {
        Self {
            cost,
            goes_to_treasury: false,
            refund_on_challenge_fail: false,
            claim,
            blockers,
            target,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Income,
    ForeignAid,
    Coup(usize),
    Tax,
    Assassinate(usize),
    Steal(usize),
    Exchange,
    Examine(usize),
    ConvertSelf,
    ConvertOther(usize),
    Embezzle,
}

impl Action {
    pub const fn spec(self, mode: GameMode) -> ActionSpec {
        const DUKE: &[Role] = &[Role::Duke];
        const CONTESSA: &[Role] = &[Role::Contessa];
        const STEAL_AMB: &[Role] = &[Role::Captain, Role::Ambassador];
        const STEAL_INQ: &[Role] = &[Role::Captain, Role::Inquisitor];

        match self {
            Self::Income => ActionSpec::basic(0, Claim::None, &[], None),
            Self::ForeignAid => ActionSpec::basic(0, Claim::None, DUKE, None),
            Self::Coup(t) => ActionSpec::basic(COUP_COST, Claim::None, &[], Some(t)),
            Self::Tax => ActionSpec::basic(0, Claim::Role(Role::Duke), &[], None),
            Self::Assassinate(t) => ActionSpec {
                refund_on_challenge_fail: true,
                ..ActionSpec::basic(
                    ASSASSINATE_COST,
                    Claim::Role(Role::Assassin),
                    CONTESSA,
                    Some(t),
                )
            },
            Self::Steal(t) => ActionSpec::basic(
                0,
                Claim::Role(Role::Captain),
                if mode.inquisitor {
                    STEAL_INQ
                } else {
                    STEAL_AMB
                },
                Some(t),
            ),
            Self::Exchange => ActionSpec::basic(
                0,
                Claim::Role(if mode.inquisitor {
                    Role::Inquisitor
                } else {
                    Role::Ambassador
                }),
                &[],
                None,
            ),
            Self::Examine(t) => ActionSpec::basic(0, Claim::Role(Role::Inquisitor), &[], Some(t)),
            Self::ConvertSelf => ActionSpec {
                goes_to_treasury: true,
                ..ActionSpec::basic(CONVERT_SELF_COST, Claim::None, &[], None)
            },
            Self::ConvertOther(t) => ActionSpec {
                goes_to_treasury: true,
                ..ActionSpec::basic(CONVERT_OTHER_COST, Claim::None, &[], Some(t))
            },
            Self::Embezzle => ActionSpec::basic(0, Claim::NotDuke, &[], None),
        }
    }

    pub fn validate(
        self,
        board: &BoardState,
        actor: usize,
    ) -> std::result::Result<(), &'static str> {
        if !board.is_alive(actor) {
            return Err("Eliminated players cannot act.");
        }
        if board.must_coup(actor) && !matches!(self, Self::Coup(_)) {
            return Err("You have 10+ coins and MUST launch a Coup (!coup <nick>)!");
        }
        if matches!(self, Self::Examine(_)) && !board.mode.inquisitor {
            return Err("Examine is only available in Inquisitor mode!");
        }
        if matches!(
            self,
            Self::ConvertSelf | Self::ConvertOther(_) | Self::Embezzle
        ) && !board.mode.reformation
        {
            return Err("That action is only available in Reformation mode!");
        }

        let spec = self.spec(board.mode);
        if board.players[actor].coins < spec.cost {
            return Err(match self {
                Self::Coup(_) => "Coup costs 7 coins!",
                Self::Assassinate(_) => "Assassinate costs 3 coins!",
                Self::ConvertSelf => "Converting yourself costs 1 coin!",
                _ => "Converting another player costs 2 coins!",
            });
        }
        if let Some(t) = spec.target {
            if t == actor {
                return Err(if matches!(self, Self::ConvertOther(_)) {
                    "Use !convert (1 coin) to convert yourself!"
                } else {
                    "You cannot target yourself!"
                });
            }
            if t >= board.players.len() || !board.is_alive(t) {
                return Err("Target player is already eliminated!");
            }
            // Official FAQ: 0-coin Steal is legal; ConvertOther may target teammates.
            if !matches!(self, Self::ConvertOther(_)) && !board.can_target(actor, t) {
                return Err("You cannot target a player of the same Allegiance while opposing factions remain!");
            }
        }
        match self {
            Self::Exchange if board.deck.len() < board.mode.exchange_draw_count() => {
                Err("Court Deck has too few cards to Exchange.")
            }
            Self::Embezzle if board.treasury == 0 => {
                Err("The Treasury Reserve is empty (0 coins)!")
            }
            _ => Ok(()),
        }
    }

    pub fn describe(self, board: &BoardState, roster: &Roster, actor: usize) -> Text {
        let a = roster.nick(actor).bold();
        let tgt = |t: usize| roster.nick(t).bold();
        match self {
            Self::Income => irc_format!("{a} takes {} (+1c)", "Income".bold()),
            Self::ForeignAid => irc_format!("{a} attempts {} (+2c)", "Foreign Aid".bold()),
            Self::Coup(t) => irc_format!(
                "{a} launches a {} against {} (7c)",
                "Coup".bold().red(),
                tgt(t)
            ),
            Self::Tax => irc_format!(
                "{a} claims {} to {} (+3c)",
                Role::Duke.styled(),
                "Tax".bold()
            ),
            Self::Assassinate(t) => irc_format!(
                "{a} pays 3c and claims {} to {} {}",
                Role::Assassin.styled(),
                "Assassinate".bold().orange(),
                tgt(t),
            ),
            Self::Steal(t) => irc_format!(
                "{a} claims {} to {} from {}",
                Role::Captain.styled(),
                "Steal".bold().cyan(),
                tgt(t),
            ),
            Self::Exchange => {
                let role = if board.mode.inquisitor {
                    Role::Inquisitor
                } else {
                    Role::Ambassador
                };
                irc_format!("{a} claims {} to {}", role.styled(), "Exchange".bold())
            }
            Self::Examine(t) => irc_format!(
                "{a} claims {} to {} {}",
                Role::Inquisitor.styled(),
                "Examine".bold().yellow(),
                tgt(t),
            ),
            Self::ConvertSelf => irc_format!(
                "{a} pays 1c to Treasury to {} their own Allegiance",
                "Convert".bold().cyan()
            ),
            Self::ConvertOther(t) => irc_format!(
                "{a} pays 2c to Treasury to {} {}'s Allegiance",
                "Convert".bold().cyan(),
                tgt(t),
            ),
            Self::Embezzle => irc_format!(
                "{a} claims NOT to hold {} and attempts to {} the Treasury ({}c)",
                Role::Duke.styled(),
                "Embezzle".bold().yellow(),
                board.treasury,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExamineDecision {
    Keep,
    ForceSwap,
}

pub fn parse_action<F>(
    raw: &str,
    mode: GameMode,
    resolve_nick: F,
) -> std::result::Result<Option<Action>, String>
where
    F: Fn(&str) -> Option<usize>,
{
    let mut parts = raw.trim().split_whitespace();
    let Some(cmd) = parts.next().filter(|c| c.starts_with('!')) else {
        return Ok(None);
    };
    let arg = parts.next();
    let need_target = |usage: &str| -> std::result::Result<usize, String> {
        let tok = arg.ok_or_else(|| format!("Usage: {usage}"))?;
        resolve_nick(tok).ok_or_else(|| format!("Unknown player '{tok}'."))
    };

    match cmd.to_ascii_lowercase().as_str() {
        "!income" | "!inc" => Ok(Some(Action::Income)),
        "!aid" | "!foreignaid" | "!foreign" | "!fa" => Ok(Some(Action::ForeignAid)),
        "!tax" | "!duke" => Ok(Some(Action::Tax)),
        "!coup" => Ok(Some(Action::Coup(need_target("!coup <nick>")?))),
        "!assassinate" | "!ass" | "!kill" | "!assassin" => Ok(Some(Action::Assassinate(
            need_target("!assassinate <nick>")?,
        ))),
        "!steal" | "!captain" => Ok(Some(Action::Steal(need_target("!steal <nick>")?))),
        "!exchange" | "!swap" | "!ambassador" => Ok(Some(Action::Exchange)),
        "!examine" | "!inquisitor" | "!peek" => {
            if !mode.inquisitor {
                return Err("Examine is only available in Inquisitor mode!".into());
            }
            Ok(Some(Action::Examine(need_target("!examine <nick>")?)))
        }
        "!convert" => {
            if !mode.reformation {
                return Err("Convert is only available in Reformation mode!".into());
            }
            match arg {
                None => Ok(Some(Action::ConvertSelf)),
                Some(t) if matches!(t.to_ascii_lowercase().as_str(), "self" | "me") => {
                    Ok(Some(Action::ConvertSelf))
                }
                Some(t) => Ok(Some(Action::ConvertOther(
                    resolve_nick(t).ok_or_else(|| format!("Unknown player '{t}'."))?,
                ))),
            }
        }
        "!embezzle" | "!emb" => {
            if !mode.reformation {
                return Err("Embezzle is only available in Reformation mode!".into());
            }
            Ok(Some(Action::Embezzle))
        }
        _ => Ok(None),
    }
}

pub fn parse_block_role(
    raw: &str,
    action: Action,
    mode: GameMode,
) -> std::result::Result<Option<Role>, String> {
    let valid = action.spec(mode).blockers;
    let mut parts = raw.trim().split_whitespace();
    let Some(cmd) = parts.next().filter(|c| c.starts_with('!')) else {
        return Ok(None);
    };
    let lower = cmd.to_ascii_lowercase();
    let valid_names = || {
        valid
            .iter()
            .map(|r| r.name())
            .collect::<Vec<_>>()
            .join(" or ")
    };

    let role = if matches!(lower.as_str(), "!block" | "!counter" | "!counteract") {
        if valid.is_empty() {
            return Err("This action cannot be blocked!".into());
        }
        match parts.next() {
            None if valid.len() == 1 => return Ok(Some(valid[0])),
            None => {
                let cmds = valid
                    .iter()
                    .map(|r| format!("!block {}", r.name().to_ascii_lowercase()))
                    .collect::<Vec<_>>()
                    .join(" or ");
                return Err(format!("Specify which role you claim to block: {cmds}"));
            }
            Some(arg) => parse_role(arg).ok_or_else(|| {
                format!(
                    "Unknown role '{arg}' — valid blocker(s): {}.",
                    valid_names()
                )
            })?,
        }
    } else if !valid.is_empty() {
        match lower.as_str() {
            "!duke" => Role::Duke,
            "!contessa" => Role::Contessa,
            "!captain" => Role::Captain,
            "!ambassador" => Role::Ambassador,
            "!inquisitor" => Role::Inquisitor,
            _ => return Ok(None),
        }
    } else {
        return Ok(None);
    };

    valid.contains(&role).then_some(Some(role)).ok_or_else(|| {
        format!(
            "{} cannot block this action — claim {}.",
            role.name(),
            valid_names()
        )
    })
}

pub fn pick_role_index(raw: &str, roles: &[Role]) -> Option<usize> {
    let tok = raw
        .trim()
        .split_whitespace()
        .next()?
        .trim_start_matches('!');
    if let Ok(n) = tok.parse::<usize>() {
        if (1..=roles.len()).contains(&n) {
            return Some(n - 1);
        }
    }
    let role = parse_role(tok)?;
    roles.iter().position(|&r| r == role)
}

pub fn parse_influence_choice(
    raw: &str,
    live: &[(usize, Role)],
) -> std::result::Result<usize, String> {
    let roles: Vec<Role> = live.iter().map(|&(_, r)| r).collect();
    if let Some(idx) = pick_role_index(raw, &roles) {
        return Ok(live[idx].0);
    }
    let choices = live
        .iter()
        .enumerate()
        .map(|(i, &(_, r))| format!("{} ({})", i + 1, r.name()))
        .collect::<Vec<_>>()
        .join(" or ");
    Err(format!("Choose {choices}."))
}

pub fn parse_exchange_keep(
    raw: &str,
    pool: &[Role],
    keep_count: usize,
) -> std::result::Result<Vec<usize>, String> {
    let mut tokens: Vec<&str> = raw
        .trim()
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '+'))
        .filter(|s| !s.is_empty())
        .collect();
    if tokens
        .first()
        .is_some_and(|t| matches!(t.to_ascii_lowercase().as_str(), "!keep" | "keep"))
    {
        tokens.remove(0);
    }
    if tokens.len() != keep_count {
        return Err(format!(
            "Pick exactly {keep_count} card(s) to keep (by number 1–{} or role name).",
            pool.len()
        ));
    }

    let mut chosen = Vec::with_capacity(keep_count);
    for tok in tokens {
        let clean = tok.trim_start_matches('!');
        let idx = if let Ok(n) = clean.parse::<usize>() {
            if !(1..=pool.len()).contains(&n) {
                return Err(format!("Card number {n} out of range (1–{}).", pool.len()));
            }
            if chosen.contains(&(n - 1)) {
                return Err(format!(
                    "Card #{n} was selected twice — pick distinct cards."
                ));
            }
            n - 1
        } else if let Some(role) = parse_role(clean) {
            pool.iter()
                .enumerate()
                .position(|(i, &r)| r == role && !chosen.contains(&i))
                .ok_or_else(|| {
                    format!(
                        "You don't have another {} in your exchange choices.",
                        role.name()
                    )
                })?
        } else {
            return Err(format!(
                "Unknown card '{tok}' — use numbers 1–{} or role names.",
                pool.len()
            ));
        };
        chosen.push(idx);
    }
    Ok(chosen)
}

pub fn parse_examine_decision(raw: &str) -> Option<ExamineDecision> {
    let tok = raw
        .trim()
        .split_whitespace()
        .next()?
        .trim_start_matches('!')
        .to_ascii_lowercase();
    match tok.as_str() {
        "keep" | "return" | "allow" | "ok" | "pass" | "k" => Some(ExamineDecision::Keep),
        "force" | "swap" | "replace" | "exchange" | "discard" | "change" | "f" | "s" => {
            Some(ExamineDecision::ForceSwap)
        }
        _ => None,
    }
}

impl Session<'_> {
    pub(crate) async fn execute_action(
        &mut self,
        actor: usize,
        action: Action,
    ) -> Result<Option<usize>> {
        match action {
            Action::Income | Action::ForeignAid | Action::Tax => {
                let (delta, verb, label) = match action {
                    Action::Income => (1, "took", "Income".bold()),
                    Action::ForeignAid => (2, "took", "Foreign Aid".bold()),
                    _ => (3, "collected", "Tax".bold().magenta()),
                };
                self.board.players[actor].coins += delta;
                self.say_and_log(irc_format!(
                    "💰 {} {verb} {label} (+{delta}c → {}c).",
                    self.bold(actor),
                    self.board.players[actor].coins,
                ))?;
            }
            Action::Coup(target) => {
                self.say_and_log(irc_format!(
                    "⚔️ {} paid 7c to {} {}!",
                    self.bold(actor),
                    "Coup".bold().red(),
                    self.bold(target)
                ))?;
                return self.lose_influence(target, "Couped").await;
            }
            Action::Assassinate(target) if self.board.is_alive(target) => {
                self.say_and_log(irc_format!(
                    "🗡️ {}'s {} hits {}!",
                    self.bold(actor),
                    "Assassination".bold().orange(),
                    self.bold(target)
                ))?;
                return self.lose_influence(target, "assassinated").await;
            }
            Action::Assassinate(_) => {}
            Action::Steal(target) => {
                let taken = self.board.players[target].coins.min(2);
                self.board.players[target].coins -= taken;
                self.board.players[actor].coins += taken;
                let now = self.board.players[actor].coins;
                self.say_and_log(irc_format!(
                    "🏴‍☠️ {} stole {taken}c from {} ({}c → {now}c).",
                    self.bold(actor),
                    self.bold(target),
                    now - taken
                ))?;
            }
            Action::Exchange => self.execute_exchange(actor).await?,
            Action::Examine(target) => self.execute_examine(actor, target).await?,
            Action::ConvertSelf | Action::ConvertOther(_) => {
                let (target, cost) = match action {
                    Action::ConvertOther(t) => (t, CONVERT_OTHER_COST),
                    _ => (actor, CONVERT_SELF_COST),
                };
                let next = self.board.players[target]
                    .allegiance
                    .expect("Reformation allegiance")
                    .opposite();
                self.board.players[target].allegiance = Some(next);
                let who = if target == actor {
                    Text::default()
                } else {
                    irc_format!("{} ", self.bold(target))
                };
                self.say_and_log(irc_format!(
                    "🏛️ {} paid {cost}c to Treasury (now {}c) and converted {who}to {}!",
                    self.bold(actor),
                    self.board.treasury,
                    next.styled(),
                ))?;
            }
            Action::Embezzle => {
                let taken = self.board.treasury;
                self.board.treasury = 0;
                self.board.players[actor].coins += taken;
                self.say_and_log(irc_format!(
                    "🏛️ {} {} {taken}c from the Treasury Reserve (now {}c)!",
                    self.bold(actor),
                    "embezzled".bold().yellow(),
                    self.board.players[actor].coins,
                ))?;
            }
        }
        Ok(self.board.winner())
    }

    async fn execute_exchange(&mut self, actor: usize) -> Result<()> {
        let live = self.board.players[actor].live_slots();
        let keep_count = live.len();
        if keep_count == 0 {
            return Ok(());
        }
        let drawn: Vec<Role> = (0..self.board.mode.exchange_draw_count())
            .filter_map(|_| self.board.deck.pop())
            .collect();
        let mut pool: Vec<Role> = live.iter().map(|&(_, r)| r).collect();
        pool.extend_from_slice(&drawn);

        self.sync(
            "Exchange",
            Some(Instant::now() + self.config.dm_choice_timeout),
        );
        self.room.say(irc_format!(
            "🔄 {} draws {} card(s) from the Court Deck and chooses in DM...",
            self.bold(actor),
            drawn.len()
        ))?;

        let prompt = irc_format!(
            "🔄 Exchange — pick {keep_count} card(s) to KEEP (numbers 1–{} or role names): {} ({}s)",
            pool.len(),
            format_numbered_roles(&pool),
            self.config.dm_choice_timeout.as_secs(),
        );
        let pool_parse = pool.clone();
        let chosen = ask_user_dm(
            &self.roster.seats[actor],
            self.config,
            prompt,
            |s| format!("⏰ {s}s left to pick Exchange card(s)!").yellow(),
            move |m| parse_exchange_keep(&m.text, &pool_parse, keep_count),
        )
        .await?
        .unwrap_or_else(|| (0..keep_count).collect());

        for (pos, &idx) in chosen.iter().enumerate() {
            self.board.players[actor].cards[live[pos].0].role = pool[idx];
        }
        for (idx, &role) in pool.iter().enumerate() {
            if !chosen.contains(&idx) {
                self.board.deck.push(role);
            }
        }
        fastrand::shuffle(&mut self.board.deck);
        self.sync_board();
        self.dm_hand(actor);
        self.say_and_log(irc_format!(
            "🔄 {} completed Exchange and returned {} card(s) to the Court Deck.",
            self.bold(actor),
            drawn.len()
        ))
    }

    async fn execute_examine(&mut self, actor: usize, target: usize) -> Result<()> {
        if !self.board.is_alive(target) {
            return Ok(());
        }
        self.sync(
            "Examine",
            Some(Instant::now() + self.config.dm_choice_timeout),
        );
        let target_live = self.board.players[target].live_slots();
        let shown_slot = if target_live.len() == 1 || target_live[0].1 == target_live[1].1 {
            target_live[0].0
        } else {
            self.room.say(irc_format!(
                "🔍 {} must choose one of their cards to show to {} in DM...",
                self.bold(target),
                self.bold(actor)
            ))?;
            let prefix = format!(
                "🔍 {} (Inquisitor) examines you — choose a card to show",
                self.roster.nick(actor)
            );
            self.prompt_choose_live_slot(target, &prefix).await?
        };

        let shown_role = self.board.players[target].cards[shown_slot].role;
        self.sync(
            "Examine Decision",
            Some(Instant::now() + self.config.dm_choice_timeout),
        );
        self.room.say(irc_format!(
            "🔍 {} showed a card to {} — waiting for Inquisitor's decision in DM...",
            self.bold(target),
            self.bold(actor)
        ))?;

        let prompt = irc_format!(
            "🔍 {} showed you: {} — reply {} to hand it back or {} to force them to draw a new card ({}s).",
            self.bold(target),
            shown_role.styled(),
            "!keep".bold().green(),
            "!force".bold().orange(),
            self.config.dm_choice_timeout.as_secs(),
        );
        let decision = ask_user_dm(
            &self.roster.seats[actor],
            self.config,
            prompt,
            |s| format!("⏰ {s}s left to reply !keep or !force!").yellow(),
            |m| parse_examine_decision(&m.text).ok_or_else(|| "Reply !keep or !force.".into()),
        )
        .await?
        .unwrap_or(ExamineDecision::Keep);

        match decision {
            ExamineDecision::Keep => {
                let _ = self.roster.seats[target].say(irc_format!(
                    "🔍 {} handed your {} back.",
                    self.bold(actor),
                    shown_role.styled()
                ));
                self.say_and_log(irc_format!(
                    "🔍 {} allowed {} to keep their examined card.",
                    self.bold(actor),
                    self.bold(target)
                ))
            }
            ExamineDecision::ForceSwap => {
                let (old, new_role) = self.board.force_replace_examined_card(target, shown_slot);
                self.sync_board();
                let _ = self.roster.seats[target].say(irc_format!(
                    "🔍 {} forced you to replace {} — you drew: {}!",
                    self.bold(actor),
                    old.styled(),
                    new_role.styled()
                ));
                self.dm_hand(target);
                self.say_and_log(irc_format!(
                    "🔍 {} forced {} to swap their examined card with the Court Deck!",
                    self.bold(actor),
                    self.bold(target)
                ))
            }
        }
    }
}
