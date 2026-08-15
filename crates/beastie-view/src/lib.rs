//! Declarative visual and audio projection of authoritative state.

use beastie_core::{Intention, WorldState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteCommand {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextCommand {
    pub text: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderPlan {
    pub sprites: Vec<SpriteCommand>,
    pub text: Vec<TextCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPlan {
    pub events: Vec<String>,
}

#[must_use]
pub fn plan(state: &WorldState, speech: Option<&str>) -> (RenderPlan, AudioPlan) {
    let creature_id = match state.creature.current_intention {
        Intention::Sleep => "creature/sleep",
        Intention::Eat => "creature/eat",
        Intention::RejectFood => "creature/annoyed",
        Intention::Play => "creature/play",
        Intention::ApproachPlayer | Intention::Idle => "creature/idle",
    };
    let mut text = Vec::new();
    if let Some(speech) = speech {
        text.push(TextCommand {
            text: speech.to_owned(),
            x: 96,
            y: 28,
        });
    }
    (
        RenderPlan {
            sprites: vec![
                SpriteCommand {
                    id: "room/background".to_owned(),
                    x: 0,
                    y: 0,
                    layer: 0,
                },
                SpriteCommand {
                    id: creature_id.to_owned(),
                    x: 144,
                    y: 96,
                    layer: 3,
                },
            ],
            text,
        },
        AudioPlan { events: Vec::new() },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_intent_has_a_structured_render_command() {
        let mut state = WorldState::new(42, "Mop");
        state.creature.current_intention = Intention::Sleep;
        let (render, _) = plan(&state, None);
        assert_eq!(render.sprites[1].id, "creature/sleep");
        assert_eq!(render.sprites[1].layer, 3);
    }

    #[test]
    fn rejected_food_has_an_annoyed_render_command() {
        let mut state = WorldState::new(42, "Mop");
        state.creature.current_intention = Intention::RejectFood;
        let (render, _) = plan(&state, None);
        assert_eq!(render.sprites[1].id, "creature/annoyed");
    }
}
