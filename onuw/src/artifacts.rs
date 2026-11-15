//! Daybreak **Artifact** tokens (`Claw`, `Brand`, `Cudgel`, `Token of Mute`, `Shroud of Shame`, `Void`)
//! and their Day/Voting channel-conduct rules (`DayConductTracker`).

use std::collections::{HashMap, HashSet};

use ircgm::prelude::*;
use ircgm::CaseMap;

use crate::board::Roster;
use crate::role::{Team, VictoryCtx};

/// Daybreak `Curator` Artifacts placed face-down on a player's card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Artifact {
    /// Turns the holder into a Werewolf (`Team::Werewolf` + primary threat).
    ClawOfTheWerewolf,
    /// Turns the holder into a Villager (`Team::Village`, clears threats).
    BrandOfTheVillager,
    /// Turns the holder into a Tanner (`Team::Independent`, wins iff killed).
    CudgelOfTheTanner,
    /// Holder personally loses if they send any public channel message containing letters/numbers
    /// before voting ends (except restricted `/me` gestures or punctuation-only messages).
    TokenOfMute,
    /// "Facing the Wall": holder must `/me turns around` before speaking in channel and may not
    /// mention any player's nickname in public channel messages before voting ends, or personally loses.
    ShroudOfShame,
    /// Harmless trinket — does nothing.
    VoidOfNothingness,
}

const TANNER_BLOCKED_TEAMS: &[Team] = &[Team::Werewolf, Team::Vampire];

impl Artifact {
    pub const ALL: &'static [Artifact] = &[
        Artifact::ClawOfTheWerewolf,
        Artifact::BrandOfTheVillager,
        Artifact::CudgelOfTheTanner,
        Artifact::TokenOfMute,
        Artifact::ShroudOfShame,
        Artifact::VoidOfNothingness,
    ];

    pub fn random() -> Artifact {
        Self::ALL[fastrand::usize(..Self::ALL.len())]
    }

    pub fn id(self) -> &'static str {
        match self {
            Artifact::ClawOfTheWerewolf => "claw_of_the_werewolf",
            Artifact::BrandOfTheVillager => "brand_of_the_villager",
            Artifact::CudgelOfTheTanner => "cudgel_of_the_tanner",
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
            Artifact::TokenOfMute => "Token of Mute",
            Artifact::ShroudOfShame => "Shroud of Shame",
            Artifact::VoidOfNothingness => "Void of Nothingness",
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Artifact::ClawOfTheWerewolf => self.name().bold().red().into(),
            Artifact::CudgelOfTheTanner => self.name().bold().orange().into(),
            Artifact::TokenOfMute | Artifact::ShroudOfShame => self.name().bold().yellow().into(),
            Artifact::BrandOfTheVillager | Artifact::VoidOfNothingness => {
                self.name().bold().light_cyan().into()
            }
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
            Artifact::VoidOfNothingness => irc_format!(
                "🏺 An {} was placed on your card! You secretly hold the {} — it does nothing!",
                "Artifact token".bold().yellow(),
                self.styled(),
            ),
        }
    }
}

impl Artifact {
    pub fn team_override(self) -> Option<Team> {
        match self {
            Artifact::ClawOfTheWerewolf => Some(Team::Werewolf),
            Artifact::BrandOfTheVillager => Some(Team::Village),
            Artifact::CudgelOfTheTanner => Some(Team::Independent),
            Artifact::TokenOfMute | Artifact::ShroudOfShame | Artifact::VoidOfNothingness => None,
        }
    }

    pub fn wolf_threat_override(self) -> Option<bool> {
        match self {
            Artifact::ClawOfTheWerewolf => Some(true),
            Artifact::BrandOfTheVillager | Artifact::CudgelOfTheTanner => Some(false),
            Artifact::TokenOfMute | Artifact::ShroudOfShame | Artifact::VoidOfNothingness => None,
        }
    }

    pub fn vampire_threat_override(self) -> Option<bool> {
        match self {
            Artifact::ClawOfTheWerewolf
            | Artifact::BrandOfTheVillager
            | Artifact::CudgelOfTheTanner => Some(false),
            Artifact::TokenOfMute | Artifact::ShroudOfShame | Artifact::VoidOfNothingness => None,
        }
    }

    pub fn check_independent_win(
        self,
        seat: usize,
        ctx: &VictoryCtx<'_>,
    ) -> Option<(&'static str, &'static [Team])> {
        match self {
            Artifact::CudgelOfTheTanner if ctx.is_dead(seat) => {
                Some(("Cudgel of the Tanner", TANNER_BLOCKED_TEAMS))
            }
            _ => None,
        }
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

/// Evaluates public channel messages from Daybreak until voting ends against `TokenOfMute`
/// and `ShroudOfShame` conduct rules.
#[derive(Clone, Debug, Default)]
pub struct DayConductTracker {
    mute_violations: HashMap<usize, String>,
    turned_around: HashSet<usize>,
    spoke_before_turning: HashSet<usize>,
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

        if !self.turned_around.contains(&seat) && !self.spoke_before_turning.contains(&seat) {
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
        if self.spoke_before_turning.contains(&seat) {
            return Some("spoke in channel before /me turns around".to_owned());
        }
        if !self.turned_around.contains(&seat) {
            return Some("never faced the wall with /me turns around".to_owned());
        }
        None
    }
}

/// True iff `msg` is a public game-control command (`!vote <valid_nick>`, `!ready`, `!time`,
/// `!players`, `!who`, `!deck`, `!help`, `!role <name>`) that is exempt from both `TokenOfMute` and `ShroudOfShame`.
pub fn is_exempt_public_command(msg: &ChatMessage, roster: &Roster) -> bool {
    if msg.is_action() {
        return false;
    }
    let trimmed = msg.text.trim();
    if matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "!ready" | "!time" | "!players" | "!who" | "!deck" | "!help"
    ) {
        return true;
    }
    if msg.cmd_arg("!role").is_some() || msg.cmd_arg("!info").is_some() {
        return true;
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
            let before_ok = text[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !is_nick_word_char(c));
            let after_ok = text[end..]
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
