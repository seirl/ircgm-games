//! Daybreak & Bonus **Artifact** tokens (`Claw`, `Brand`, `Cudgel`, `Bow`, `Cloak`, `Sword`,
//! `Mist`, `Dagger`, `Alien Artifact`, `Mask of Muting`, `Shroud of Shame`, `Void`),
//! monster-faction deck gating, and Day/Voting channel-conduct rules (`DayConductTracker`).

use std::collections::HashMap;

use ircgm::prelude::*;
use ircgm::CaseMap;

use crate::board::{deck_has_monster_faction, BoardCards, Roster, SeatSet};
use crate::marks::{apply_traitor_win_override, find_mark};
use crate::role::{
    DeathCtx, DeathEffect, EffectiveAlignment, EndgameModifier, Role, SpecialVictory, Team,
    VictoryCtx, VoteDefense, WinnerFilterCtx, WinnerMap,
};
use crate::roles::find_role;

/// Daybreak & Bonus Roles `Curator` Artifacts placed face-down on a player's card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Artifact {
    /// Turns the holder into a Werewolf (`Team::Werewolf` + primary threat; gated on Werewolf faction in deck).
    ClawOfTheWerewolf,
    /// Turns the holder into a Villager (`Team::Village`, clears threats).
    BrandOfTheVillager,
    /// Turns the holder into a Tanner (`Team::Independent`, wins iff killed).
    CudgelOfTheTanner,
    /// Turns the holder into a Hunter (`Team::Village`, kills voted target on death).
    BowOfTheHunter,
    /// Turns the holder into a Prince (`Team::Village`, royal vote immunity).
    CloakOfThePrince,
    /// Turns the holder into a Bodyguard (`Team::Village`, voted target cannot die).
    SwordOfTheBodyguard,
    /// Turns the holder into a Vampire (`Team::Vampire` + primary threat; gated on Vampire faction in deck).
    MistOfTheVampire,
    /// Turns the holder into a Traitor to their team (wins iff a teammate dies; keeps underlying team/role).
    DaggerOfTheTraitor,
    /// Turns the holder into an Alien (`Team::Alien` + primary threat; gated on Alien faction in deck).
    AlienArtifact,
    /// Holder personally loses if they send any public channel message containing letters/numbers
    /// before voting ends (except restricted `/me` gestures or punctuation-only messages).
    TokenOfMute,
    /// "Facing the Wall": holder must `/me turns around` before speaking in channel and may not
    /// mention any player's nickname in public channel messages before voting ends, or personally loses.
    ShroudOfShame,
    /// Harmless trinket — does nothing.
    VoidOfNothingness,
}

impl Artifact {
    pub const ALL: &'static [Artifact] = &[
        Artifact::ClawOfTheWerewolf,
        Artifact::BrandOfTheVillager,
        Artifact::CudgelOfTheTanner,
        Artifact::BowOfTheHunter,
        Artifact::CloakOfThePrince,
        Artifact::SwordOfTheBodyguard,
        Artifact::MistOfTheVampire,
        Artifact::DaggerOfTheTraitor,
        Artifact::AlienArtifact,
        Artifact::TokenOfMute,
        Artifact::ShroudOfShame,
        Artifact::VoidOfNothingness,
    ];

    /// Returns `Some(team)` for monster-converting tokens that are only included in the Curator's
    /// stack when at least one role from that monster faction is present in the deck.
    pub fn required_monster_faction(self) -> Option<Team> {
        match self {
            Artifact::ClawOfTheWerewolf => Some(Team::Werewolf),
            Artifact::MistOfTheVampire => Some(Team::Vampire),
            Artifact::AlienArtifact => Some(Team::Alien),
            Artifact::BrandOfTheVillager
            | Artifact::CudgelOfTheTanner
            | Artifact::BowOfTheHunter
            | Artifact::CloakOfThePrince
            | Artifact::SwordOfTheBodyguard
            | Artifact::DaggerOfTheTraitor
            | Artifact::TokenOfMute
            | Artifact::ShroudOfShame
            | Artifact::VoidOfNothingness => None,
        }
    }

    /// True iff `self` is eligible to be drawn by the Curator for `deck`.
    pub fn is_enabled_for_deck(self, deck: &[&'static dyn Role]) -> bool {
        match self.required_monster_faction() {
            Some(team) => deck_has_monster_faction(deck, team),
            None => true,
        }
    }

    /// Returns all Curator `Artifact` tokens active for `deck` (the 9 non-monster tokens plus any
    /// monster tokens whose faction is present in `deck`).
    pub fn pool_for_deck(deck: &[&'static dyn Role]) -> Vec<Artifact> {
        Self::ALL
            .iter()
            .copied()
            .filter(|art| art.is_enabled_for_deck(deck))
            .collect()
    }

    pub fn random() -> Artifact {
        Self::ALL[fastrand::usize(..Self::ALL.len())]
    }

    /// Draws a random Artifact valid for `deck` not yet present in `existing`
    /// (without replacement when multiple Curators act).
    pub fn random_unused_for_deck(
        deck: &[&'static dyn Role],
        existing: &[Option<Artifact>],
    ) -> Artifact {
        let pool = Self::pool_for_deck(deck);
        let unused: Vec<Artifact> = pool
            .iter()
            .copied()
            .filter(|cand| !existing.contains(&Some(*cand)))
            .collect();
        if unused.is_empty() {
            pool[fastrand::usize(..pool.len())]
        } else {
            unused[fastrand::usize(..unused.len())]
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Artifact::ClawOfTheWerewolf => "claw_of_the_werewolf",
            Artifact::BrandOfTheVillager => "brand_of_the_villager",
            Artifact::CudgelOfTheTanner => "cudgel_of_the_tanner",
            Artifact::BowOfTheHunter => "bow_of_the_hunter",
            Artifact::CloakOfThePrince => "cloak_of_the_prince",
            Artifact::SwordOfTheBodyguard => "sword_of_the_bodyguard",
            Artifact::MistOfTheVampire => "mist_of_the_vampire",
            Artifact::DaggerOfTheTraitor => "dagger_of_the_traitor",
            Artifact::AlienArtifact => "alien_artifact",
            Artifact::TokenOfMute => "token_of_mute",
            Artifact::ShroudOfShame => "shroud_of_shame",
            Artifact::VoidOfNothingness => "void_of_nothingness",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Artifact::ClawOfTheWerewolf => "Claw of the Werewolf",
            Artifact::BrandOfTheVillager => "Brand of the Villager",
            Artifact::CudgelOfTheTanner => "Cudgel of the Tanner",
            Artifact::BowOfTheHunter => "Bow of the Hunter",
            Artifact::CloakOfThePrince => "Cloak of the Prince",
            Artifact::SwordOfTheBodyguard => "Sword of the Bodyguard",
            Artifact::MistOfTheVampire => "Mist of the Vampire",
            Artifact::DaggerOfTheTraitor => "Dagger of the Traitor",
            Artifact::AlienArtifact => "Alien Artifact",
            Artifact::TokenOfMute => "Mask of Muting",
            Artifact::ShroudOfShame => "Shroud of Shame",
            Artifact::VoidOfNothingness => "Void of Nothingness",
        }
    }

    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Artifact::ClawOfTheWerewolf => &["claw", "werewolf", "wolf", "claw of werewolf"],
            Artifact::BrandOfTheVillager => &["brand", "villager", "brand of villager"],
            Artifact::CudgelOfTheTanner => &["cudgel", "tanner", "club", "cudgel of tanner"],
            Artifact::BowOfTheHunter => &["bow", "hunter", "bow of hunter"],
            Artifact::CloakOfThePrince => &["cloak", "prince", "cloak of prince"],
            Artifact::SwordOfTheBodyguard => &["sword", "bodyguard", "sword of bodyguard"],
            Artifact::MistOfTheVampire => &["mist", "vampire", "vamp", "mist of vampire"],
            Artifact::DaggerOfTheTraitor => &["dagger", "traitor", "dagger of traitor"],
            Artifact::AlienArtifact => &["alien", "artifact of the alien", "artifact of alien"],
            Artifact::TokenOfMute => &[
                "mask",
                "mute",
                "muting",
                "mask of mute",
                "token of muting",
            ],
            Artifact::ShroudOfShame => &[
                "shroud",
                "shame",
                "shroud of the shame",
                "facing the wall",
                "wall",
            ],
            Artifact::VoidOfNothingness => &["void", "nothing", "nothingness", "void of nothing"],
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Artifact::ClawOfTheWerewolf => {
                "Turns you into a Werewolf (only in pool when Werewolf faction is in the deck)."
            }
            Artifact::BrandOfTheVillager => {
                "Turns you into an ordinary Villager on the Village team."
            }
            Artifact::CudgelOfTheTanner => {
                "Turns you into a Tanner (Independent — you win ONLY if you get killed)."
            }
            Artifact::BowOfTheHunter => {
                "Turns you into a Hunter on the Village team (if you die, whoever you vote for also dies)."
            }
            Artifact::CloakOfThePrince => {
                "Turns you into a Prince on the Village team (votes against you do not count)."
            }
            Artifact::SwordOfTheBodyguard => {
                "Turns you into a Bodyguard on the Village team (whoever you vote for cannot die)."
            }
            Artifact::MistOfTheVampire => {
                "Turns you into a Vampire (only in pool when Vampire faction is in the deck)."
            }
            Artifact::DaggerOfTheTraitor => {
                "Makes you a Traitor to your team: if you have teammates, you win ONLY if a teammate dies."
            }
            Artifact::AlienArtifact => {
                "Turns you into an Alien (only in pool when Alien faction is in the deck)."
            }
            Artifact::TokenOfMute => {
                "You personally lose if you send letters/numbers in channel before voting ends (except allowed /me gestures, !ready, !vote)."
            }
            Artifact::ShroudOfShame => {
                "Facing the Wall: before voting ends you MUST send /me turns around before speaking and never mention any player's nick in channel, or you personally lose."
            }
            Artifact::VoidOfNothingness => "Harmless trinket — does nothing.",
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Artifact::ClawOfTheWerewolf => self.name().bold().red().into(),
            Artifact::MistOfTheVampire => self.name().bold().magenta().into(),
            Artifact::AlienArtifact => self.name().bold().green().into(),
            Artifact::CudgelOfTheTanner | Artifact::DaggerOfTheTraitor => {
                self.name().bold().orange().into()
            }
            Artifact::TokenOfMute | Artifact::ShroudOfShame => self.name().bold().yellow().into(),
            Artifact::BrandOfTheVillager
            | Artifact::BowOfTheHunter
            | Artifact::CloakOfThePrince
            | Artifact::SwordOfTheBodyguard
            | Artifact::VoidOfNothingness => self.name().bold().light_cyan().into(),
        }
    }

    /// Secret DM sent to the recipient when the Curator places this Artifact on their card.
    pub fn recipient_dm(self) -> Text {
        match self {
            Artifact::TokenOfMute => irc_format!(
                "🏺 You have the {}. If you send any public channel message containing letters/numbers before voting ends, you personally lose (even if your team wins). Allowed in channel: punctuation-only messages (..., !, ?), {} gestures ({}, {}, {}, {}, {}, {}, {}), {}, and {}.",
                self.styled(),
                "/me".bold(),
                "/me nods".bold(),
                "/me shakes head".bold(),
                "/me shrugs".bold(),
                "/me waves".bold(),
                "/me thumbs up".bold(),
                "/me thumbs down".bold(),
                "/me points at <nick>".bold(),
                "!ready".bold().green(),
                "!vote <nick>".bold().green(),
            ),
            Artifact::ShroudOfShame => irc_format!(
                "🏺 You have the {} (Facing the Wall). Before speaking (and before voting ends), you MUST send {} in channel, and NONE of your public channel messages may contain any player's nickname (except {}) — otherwise you personally lose (even if your team wins)!",
                self.styled(),
                "/me turns around".bold().cyan(),
                "!vote <nick>".bold().green(),
            ),
            Artifact::ClawOfTheWerewolf => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now a Werewolf!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::BrandOfTheVillager => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now on the Village team!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::CudgelOfTheTanner => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you now win ONLY if you get killed!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::BowOfTheHunter => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now a Hunter on the Village team (if you die, whoever you point at also dies)!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::CloakOfThePrince => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now a Prince on the Village team (votes against you do not count)!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::SwordOfTheBodyguard => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now a Bodyguard on the Village team (whoever you point at during the vote cannot die)!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::MistOfTheVampire => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now a Vampire!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::DaggerOfTheTraitor => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now a Traitor to your team and ONLY win if a teammate dies!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::AlienArtifact => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — you are now an Alien!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
            Artifact::VoidOfNothingness => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — it does nothing!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
        }
    }
}

impl Artifact {
    pub fn replaces_role_card(self) -> bool {
        matches!(
            self,
            Artifact::ClawOfTheWerewolf
                | Artifact::BrandOfTheVillager
                | Artifact::CudgelOfTheTanner
                | Artifact::BowOfTheHunter
                | Artifact::CloakOfThePrince
                | Artifact::SwordOfTheBodyguard
                | Artifact::MistOfTheVampire
                | Artifact::AlienArtifact
        )
    }

    pub fn check_day_conduct_forfeit(
        self,
        seat: usize,
        roster: &Roster,
        casemap: CaseMap,
        channel_log: &[ChatMessage],
    ) -> Option<Text> {
        let mut tracker = DayConductTracker::default();
        for msg in channel_log {
            tracker.observe_channel_msg(msg, roster, casemap);
        }
        match self {
            Artifact::TokenOfMute => tracker.mute_violation(seat).map(|reason| {
                irc_format!(
                    "🤐 {} broke the {} ({reason}) and personally loses!",
                    roster.seats[seat].user.nick().bold(),
                    self.styled(),
                )
            }),
            Artifact::ShroudOfShame => tracker.shame_violation(seat).map(|reason| {
                irc_format!(
                    "🙈 {} broke the {} ({reason}) and personally loses!",
                    roster.seats[seat].user.nick().bold(),
                    self.styled(),
                )
            }),
            _ => None,
        }
    }
}

impl EndgameModifier for Artifact {
    fn modifier_name(&self) -> &'static str {
        self.name()
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        _board: &BoardCards,
        cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        match self {
            Artifact::ClawOfTheWerewolf => EffectiveAlignment {
                team: Team::Werewolf,
                threat_team: Some(Team::Werewolf),
            },
            Artifact::MistOfTheVampire => EffectiveAlignment {
                team: Team::Vampire,
                threat_team: Some(Team::Vampire),
            },
            Artifact::AlienArtifact => EffectiveAlignment {
                team: Team::Alien,
                threat_team: Some(Team::Alien),
            },
            Artifact::BrandOfTheVillager
            | Artifact::BowOfTheHunter
            | Artifact::CloakOfThePrince
            | Artifact::SwordOfTheBodyguard => EffectiveAlignment {
                team: Team::Village,
                threat_team: None,
            },
            Artifact::CudgelOfTheTanner => EffectiveAlignment {
                team: Team::Independent,
                threat_team: None,
            },
            Artifact::DaggerOfTheTraitor
            | Artifact::TokenOfMute
            | Artifact::ShroudOfShame
            | Artifact::VoidOfNothingness => cur,
        }
    }

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        match self {
            Artifact::CloakOfThePrince => Some(VoteDefense::immune(
                seat,
                false,
                irc_format!(
                    "👑 {} ({}) holds royal immunity and cannot be executed!",
                    ctx.roster.seats[seat].user.nick().bold(),
                    self.styled(),
                ),
            )),
            Artifact::SwordOfTheBodyguard => {
                let target = ctx.votes[seat]?;
                Some(VoteDefense::immune(
                    target,
                    true,
                    irc_format!(
                        "🛡️ {} ({}) shielded {} from execution!",
                        ctx.roster.seats[seat].user.nick().bold(),
                        self.styled(),
                        ctx.roster.seats[target].user.nick().bold(),
                    ),
                ))
            }
            _ => None,
        }
    }

    fn on_death(&self, dead_seat: usize, ctx: &DeathCtx<'_>) -> Vec<DeathEffect> {
        match self {
            Artifact::BowOfTheHunter => match ctx.votes[dead_seat] {
                Some(target_seat) => {
                    let hunter_nick = ctx.roster.seats[dead_seat].user.nick();
                    let target_nick = ctx.roster.seats[target_seat].user.nick();
                    vec![DeathEffect::Kill {
                        seat: target_seat,
                        respects_protection: true,
                        announcement: irc_format!(
                            "{} {} died with the {} and took down {}!",
                            "🏹 HUNTER SHOT!".bold().orange(),
                            hunter_nick.bold(),
                            self.styled(),
                            target_nick.bold().red(),
                        ),
                    }]
                }
                None => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn victory_claim(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        match self {
            Artifact::CudgelOfTheTanner if ctx.is_dead(seat) => {
                Some(SpecialVictory::independent_blocking_monsters())
            }
            _ => None,
        }
    }

    fn override_win(&self, holder: usize, ctx: &WinnerFilterCtx<'_>, winners: &mut WinnerMap) {
        if *self == Artifact::DaggerOfTheTraitor {
            apply_traitor_win_override(holder, ctx, winners);
        }
    }

    fn apply_forfeits(
        &self,
        holder: usize,
        ctx: &mut WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        if let Some(forfeit_msg) =
            self.check_day_conduct_forfeit(holder, ctx.roster, ctx.casemap, ctx.day_channel_log)
        {
            winners.remove(&holder);
            ctx.announcements.push(forfeit_msg);
        }
    }
}

/// Evaluates public channel messages from Daybreak until voting ends against `TokenOfMute`
/// and `ShroudOfShame` conduct rules.
#[derive(Clone, Debug, Default)]
pub struct DayConductTracker {
    mute_violations: HashMap<usize, String>,
    turned_around: SeatSet,
    spoke_before_turning: SeatSet,
    named_player_violations: HashMap<usize, String>,
}

impl DayConductTracker {
    pub fn observe_channel_msg(
        &mut self,
        msg: &ChatMessage,
        roster: &Roster,
        casemap: CaseMap,
    ) {
        let Some(seat) = roster
            .seats
            .iter()
            .position(|s| s.user.id() == msg.sender.id())
        else {
            return;
        };

        // Public game mechanics (`!vote <nick>`, `!ready`, `!time`, `!players`, `!deck`) are not speech/chat:
        if is_exempt_public_command(msg, roster) {
            return;
        }

        if !is_allowed_mute_channel_msg(msg, roster) {
            self.mute_violations.entry(seat).or_insert_with(|| {
                if msg.is_action() {
                    format!("sent disallowed /me {}", msg.text.trim())
                } else {
                    "spoke in channel".to_owned()
                }
            });
        }

        let is_turn = is_turn_around_action(msg);
        if !is_turn {
            if let Some(mentioned) = find_mentioned_player_nick(&msg.text, roster, casemap) {
                self.named_player_violations.entry(seat).or_insert(mentioned);
            }
        }

        if !self.turned_around.contains(seat) && !self.spoke_before_turning.contains(seat) {
            if is_turn {
                self.turned_around.insert(seat);
            } else {
                self.spoke_before_turning.insert(seat);
            }
        }
    }

    pub fn mute_violation(&self, seat: usize) -> Option<&str> {
        self.mute_violations.get(&seat).map(String::as_str)
    }

    pub fn shame_violation(&self, seat: usize) -> Option<String> {
        if let Some(mentioned) = self.named_player_violations.get(&seat) {
            return Some(format!("mentioned player {mentioned}"));
        }
        if self.spoke_before_turning.contains(seat) {
            return Some("spoke in channel before /me turns around".to_owned());
        }
        if !self.turned_around.contains(seat) {
            return Some("never faced the wall with /me turns around".to_owned());
        }
        None
    }
}

fn normalize_artifact_key(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Resolves an [`Artifact`] by ID, display name, or alias (case- and punctuation-insensitive).
pub fn find_artifact(query: &str) -> Option<Artifact> {
    let key = normalize_artifact_key(query);
    if key.is_empty() {
        return None;
    }
    for &art in Artifact::ALL {
        if normalize_artifact_key(art.id()) == key || normalize_artifact_key(art.name()) == key {
            return Some(art);
        }
        for &alias in art.aliases() {
            if normalize_artifact_key(alias) == key {
                return Some(art);
            }
        }
    }
    None
}

/// True iff `msg` is a public game-control command (`!vote <valid_nick>`, `!ready`, `!time`,
/// `!players`, `!who`, `!deck`, `!order`, `!help`, `!role <name>`, `!mark(s)`, `!artifact(s)`)
/// that is exempt from both `TokenOfMute` and `ShroudOfShame`.
pub fn is_exempt_public_command(msg: &ChatMessage, roster: &Roster) -> bool {
    if msg.is_action() {
        return false;
    }
    let trimmed = msg.text.trim();
    if matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "!ready"
            | "!time"
            | "!players"
            | "!who"
            | "!deck"
            | "!order"
            | "!help"
            | "!marks"
            | "!mark"
            | "!artifacts"
            | "!artifact"
    ) {
        return true;
    }
    if let Some(arg) = msg.cmd_arg("!role") {
        return find_role(arg).is_some();
    }
    if let Some(arg) = msg.cmd_arg("!mark").or_else(|| msg.cmd_arg("!marks")) {
        return find_mark(arg).is_some();
    }
    if let Some(arg) = msg
        .cmd_arg("!artifact")
        .or_else(|| msg.cmd_arg("!artifacts"))
    {
        return find_artifact(arg).is_some();
    }
    if let Some(arg) = msg.cmd_arg("!info") {
        return find_role(arg).is_some() || find_mark(arg).is_some() || find_artifact(arg).is_some();
    }
    if let Some(arg) = msg.cmd_arg("!vote") {
        return !arg.contains(char::is_whitespace) && roster.resolve_nick(arg).is_some();
    }
    false
}

/// True iff `msg` is allowed under `Artifact::TokenOfMute`:
/// - Non-action messages may contain zero letters/numbers (`!msg.text.chars().any(char::is_alphanumeric)`),
///   allowing punctuation-only (`...`, `!`, `?`) or emoji reactions.
/// - `/me` (`Action`) messages must match the restricted non-verbal gesture list (including `/me points at <nick>`).
pub fn is_allowed_mute_channel_msg(msg: &ChatMessage, roster: &Roster) -> bool {
    if !msg.is_action() {
        return !msg.text.chars().any(char::is_alphanumeric);
    }

    let trimmed = msg.text.trim().trim_end_matches(['.', '!', '?']).trim();
    if trimmed.is_empty() || !trimmed.chars().any(char::is_alphanumeric) {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "nods"
            | "nod"
            | "shakes head"
            | "shakes their head"
            | "shakes his head"
            | "shakes her head"
            | "shrugs"
            | "shrug"
            | "waves"
            | "wave"
            | "thumbs up"
            | "gives a thumbs up"
            | "thumbs down"
            | "gives a thumbs down"
            | "facepalms"
            | "facepalm"
            | "sighs"
            | "sigh"
            | "gasps"
            | "gasp"
            | "laughs"
            | "laugh"
            | "cries"
            | "cry"
            | "points"
            | "points at self"
            | "points at themselves"
    ) {
        return true;
    }

    for prefix in ["points at ", "points to ", "point at ", "point to "] {
        if lower.starts_with(prefix) {
            let target_raw = trimmed[prefix.len()..].trim();
            return !target_raw.contains(char::is_whitespace)
                && roster.resolve_nick(target_raw).is_some();
        }
    }

    false
}

/// True iff `msg` is a `/me turns around` (or equivalent wall-facing) posture emote.
pub fn is_turn_around_action(msg: &ChatMessage) -> bool {
    if !msg.is_action() {
        return false;
    }
    let lower = msg
        .text
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "turns around"
            | "turn around"
            | "turns their chair around"
            | "turns his chair around"
            | "turns her chair around"
            | "turns chair around"
            | "turns away"
            | "turns their back"
            | "faces the wall"
            | "faces away"
    )
}

fn is_nick_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Case-folded word-boundary scan returning the first enrolled player nickname mentioned in `text`.
pub fn find_mentioned_player_nick(
    text: &str,
    roster: &Roster,
    casemap: CaseMap,
) -> Option<String> {
    let folded_hay = casemap.fold(text);
    let hay = folded_hay.as_str();
    for seat in &roster.seats {
        let nick = seat.user.nick();
        if nick.is_empty() {
            continue;
        }
        let folded_needle = casemap.fold(&nick);
        let needle = folded_needle.as_str();
        for (start, _) in hay.match_indices(needle) {
            let end = start + needle.len();
            let before_ok = hay[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !is_nick_word_char(c));
            let after_ok = hay[end..]
                .chars()
                .next()
                .is_none_or(|c| !is_nick_word_char(c));
            if before_ok && after_ok {
                return Some(nick);
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "artifacts_test.rs"]
mod test;
