//! Vampire expansion **Mark** tokens (`Mark::Clarity`, `Mark::Vampire`, `Mark::Bat`, etc.).

use ircgm::prelude::*;

/// Vampire expansion Marks (every player starts Dusk with `Mark::Clarity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Clarity,
    Vampire,
    Bat,
    Fear,
    Disease,
    Love,
    Traitor,
    Assassin,
}

impl Mark {
    pub fn name(self) -> &'static str {
        match self {
            Mark::Clarity => "Mark of Clarity",
            Mark::Vampire => "Mark of the Vampire",
            Mark::Bat => "Mark of the Bat",
            Mark::Fear => "Mark of Fear",
            Mark::Disease => "Mark of Disease",
            Mark::Love => "Mark of Love",
            Mark::Traitor => "Mark of the Traitor",
            Mark::Assassin => "Mark of the Assassin",
        }
    }

    pub fn styled(self) -> Text {
        match self {
            Mark::Vampire => self.name().bold().red().into(),
            Mark::Fear | Mark::Disease | Mark::Assassin | Mark::Traitor => {
                self.name().bold().orange().into()
            }
            Mark::Love => self.name().bold().yellow().into(),
            Mark::Bat | Mark::Clarity => self.name().bold().light_cyan().into(),
        }
    }
}
