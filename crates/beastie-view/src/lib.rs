//! Display-free projection of authoritative state into a small, fixed room.

use beastie_core::{FoodId, Intention, Reaction, RoomSpot, WorldState};
use serde::{Deserialize, Serialize};

pub const LOGICAL_WIDTH: i32 = 320;
pub const LOGICAL_HEIGHT: i32 = 180;

const ACTION_WIDTH: i32 = 64;
const ACTION_HEIGHT: i32 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    #[must_use]
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTarget {
    Window,
    Bed,
    Bowl,
    Toy,
    Clutter,
    Creature,
    Food(FoodId),
    Reaction(Reaction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAction {
    OpenContext(UiTarget),
    CloseContext,
    OpenFoodChoice,
    Feed(FoodId),
    Play,
    Tidy,
    Comfort,
    Talk,
    React(Reaction),
    TypeCharacter(char),
    Backspace,
    SubmitText,
    CancelText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    #[default]
    Idle,
    Context(UiTarget),
    FoodChoice,
    TextEntry,
    OnScreenKeyboard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ViewState {
    pub mode: UiMode,
    /// Stable [`HitRegion::id`] selected by keyboard or controller navigation.
    pub focused_region: Option<String>,
    pub text_buffer: String,
    pub pending: bool,
    pub speech: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteCommand {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
    /// Zero-based deterministic animation frame. Runtime assets use
    /// `<id>-<frame>.png`, falling back to the unnumbered `<id>.png`.
    #[serde(default)]
    pub frame: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RectCommand {
    pub id: String,
    pub rect: Rect,
    pub color: [u8; 4],
    pub layer: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextCommand {
    pub id: String,
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub layer: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitRegion {
    pub id: String,
    pub target: Option<UiTarget>,
    pub action: UiAction,
    pub rect: Rect,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderPlan {
    pub sprites: Vec<SpriteCommand>,
    pub rects: Vec<RectCommand>,
    pub text: Vec<TextCommand>,
    pub hit_regions: Vec<HitRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPlan {
    pub events: Vec<String>,
}

#[must_use]
pub fn contextual_actions(target: UiTarget) -> Vec<UiAction> {
    match target {
        UiTarget::Bowl => vec![UiAction::OpenFoodChoice],
        UiTarget::Toy => vec![UiAction::Play],
        UiTarget::Clutter => vec![UiAction::Tidy],
        UiTarget::Creature => vec![UiAction::Comfort, UiAction::Talk],
        UiTarget::Window | UiTarget::Bed | UiTarget::Food(_) | UiTarget::Reaction(_) => Vec::new(),
    }
}

#[must_use]
pub fn plan(state: &WorldState, view: &ViewState) -> (RenderPlan, AudioPlan) {
    let (creature_x, creature_y) = creature_position(state);
    let mut sprites = room_sprites(state, creature_x, creature_y);
    let mut rects = vec![RectCommand {
        id: "room/floor-shadow".to_owned(),
        rect: Rect {
            x: 0,
            y: 137,
            w: LOGICAL_WIDTH,
            h: 43,
        },
        color: [38, 35, 44, 96],
        layer: 1,
    }];
    add_room_lighting(state, &mut rects);
    let mut text = Vec::new();
    let mut hit_regions = room_hit_regions(creature_x, creature_y);

    match view.mode {
        UiMode::Idle => {}
        UiMode::Context(target) => {
            add_context_menu(target, &mut rects, &mut text, &mut hit_regions)
        }
        UiMode::FoodChoice => add_food_choice(&mut rects, &mut text, &mut hit_regions),
        UiMode::TextEntry => add_text_entry(view, false, &mut rects, &mut text, &mut hit_regions),
        UiMode::OnScreenKeyboard => {
            add_text_entry(view, true, &mut rects, &mut text, &mut hit_regions)
        }
    }

    if let Some(speech) = view
        .speech
        .as_deref()
        .filter(|speech| !speech.trim().is_empty())
    {
        add_speech(speech, &mut rects, &mut text, &mut hit_regions);
    }

    if let Some(focused) = view.focused_region.as_deref()
        && let Some(rect) = hit_regions
            .iter()
            .find(|hit| hit.id == focused)
            .map(|hit| hit.rect)
    {
        rects.push(RectCommand {
            id: "ui/focus".to_owned(),
            rect: grow(rect, 2),
            color: [255, 225, 133, 180],
            layer: 22,
        });
    }

    // Keep ordering deterministic even when UI modes append overlays.
    sprites.sort_by_key(|command| command.layer);
    rects.sort_by_key(|command| command.layer);
    text.sort_by_key(|command| command.layer);
    (
        RenderPlan {
            sprites,
            rects,
            text,
            hit_regions,
        },
        AudioPlan { events: Vec::new() },
    )
}

fn room_sprites(state: &WorldState, creature_x: i32, creature_y: i32) -> Vec<SpriteCommand> {
    let creature_id = match state.creature.current_intention {
        Intention::Sleep => "creature/sleep",
        Intention::Eat => "creature/eat",
        Intention::RejectFood => "creature/annoyed",
        Intention::Play => "creature/play",
        Intention::ApproachPlayer => "creature/walk",
        Intention::Idle if state.creature.movement.is_some() => "creature/walk",
        Intention::Idle => "creature/idle",
    };
    let creature_frame = animation_frame(creature_id, state.elapsed_ms);
    let (creature_x_offset, creature_y_offset) = pose_offset(creature_id, creature_frame);
    let mut sprites = vec![
        sprite("room/background", 0, 0, 0),
        framed_sprite("room/window", 140, 18, 2, window_frame(state.elapsed_ms)),
        sprite("room/bed", 22, 103, 2),
        sprite("room/bowl", 238, 132, 3),
        sprite("room/toy", 95, 139, 3),
        sprite(
            if state.room.tidy {
                "room/clutter-tidy"
            } else {
                "room/clutter"
            },
            278,
            125,
            3,
        ),
    ];
    if let Some(food) = state.room.food_in_bowl {
        sprites.push(sprite(
            match food {
                FoodId::Berry => "food/berry",
                FoodId::Mushroom => "food/mushroom",
                FoodId::Pellet => "food/pellet",
            },
            245,
            132,
            4,
        ));
    }
    sprites.push(framed_sprite(
        creature_id,
        creature_x + creature_x_offset,
        creature_y + creature_y_offset,
        5,
        creature_frame,
    ));
    sprites
}

fn sprite(id: &str, x: i32, y: i32, layer: i16) -> SpriteCommand {
    framed_sprite(id, x, y, layer, 0)
}

fn framed_sprite(id: &str, x: i32, y: i32, layer: i16, frame: u8) -> SpriteCommand {
    SpriteCommand {
        id: id.to_owned(),
        x,
        y,
        layer,
        frame,
    }
}

fn animation_frame(pose: &str, elapsed_ms: u64) -> u8 {
    let frame_ms = match pose {
        "creature/walk" => 250,
        "creature/eat" => 400,
        "creature/play" => 300,
        "creature/annoyed" => 500,
        "creature/sleep" => 1_600,
        _ => 1_200,
    };
    u8::try_from((elapsed_ms / frame_ms) % 4).unwrap_or_default()
}

fn pose_offset(pose: &str, frame: u8) -> (i32, i32) {
    if frame.is_multiple_of(2) {
        return (0, 0);
    }
    match pose {
        "creature/walk" | "creature/idle" | "creature/eat" => (0, -1),
        "creature/play" => (0, -2),
        "creature/annoyed" => (-1, 0),
        "creature/sleep" => (1, 0),
        _ => (0, 0),
    }
}

fn window_frame(elapsed_ms: u64) -> u8 {
    let phase = elapsed_ms % beastie_core::ACTIVE_DAY_MS;
    let third = beastie_core::ACTIVE_DAY_MS / 3;
    if phase < third {
        0
    } else if phase < third * 2 {
        1
    } else {
        2
    }
}

fn add_room_lighting(state: &WorldState, rects: &mut Vec<RectCommand>) {
    let (window_color, room_color) = match window_frame(state.elapsed_ms) {
        0 => ([151, 210, 230, 24], [255, 230, 184, 7]),
        1 => ([224, 137, 100, 38], [114, 62, 75, 18]),
        _ => ([67, 84, 142, 54], [23, 30, 66, 42]),
    };
    rects.push(RectCommand {
        id: "room/window-light".to_owned(),
        rect: Rect {
            x: 146,
            y: 23,
            w: 76,
            h: 61,
        },
        color: window_color,
        layer: 7,
    });
    rects.push(RectCommand {
        id: "room/lighting".to_owned(),
        rect: Rect {
            x: 0,
            y: 0,
            w: LOGICAL_WIDTH,
            h: LOGICAL_HEIGHT,
        },
        color: room_color,
        layer: 8,
    });
}

fn room_hit_regions(creature_x: i32, creature_y: i32) -> Vec<HitRegion> {
    [
        (
            UiTarget::Window,
            Rect {
                x: 140,
                y: 18,
                w: 87,
                h: 72,
            },
        ),
        (
            UiTarget::Bed,
            Rect {
                x: 18,
                y: 98,
                w: 64,
                h: 48,
            },
        ),
        (
            UiTarget::Bowl,
            Rect {
                x: 232,
                y: 125,
                w: 43,
                h: 29,
            },
        ),
        (
            UiTarget::Toy,
            Rect {
                x: 88,
                y: 130,
                w: 32,
                h: 28,
            },
        ),
        (
            UiTarget::Clutter,
            Rect {
                x: 272,
                y: 116,
                w: 42,
                h: 40,
            },
        ),
        (
            UiTarget::Creature,
            Rect {
                x: creature_x - 4,
                y: creature_y - 4,
                w: 42,
                h: 48,
            },
        ),
    ]
    .into_iter()
    .map(|(target, rect)| HitRegion {
        id: format!("target/{}", target_name(target)),
        target: Some(target),
        action: UiAction::OpenContext(target),
        rect,
        enabled: !contextual_actions(target).is_empty(),
    })
    .collect()
}

fn add_context_menu(
    target: UiTarget,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let actions = contextual_actions(target);
    if actions.is_empty() {
        return;
    }
    let width = ACTION_WIDTH * i32::try_from(actions.len()).unwrap_or(1);
    rects.push(panel(
        "ui/context-panel",
        Rect {
            x: (LOGICAL_WIDTH - width) / 2,
            y: 154,
            w: width,
            h: 22,
        },
    ));
    for (index, action) in actions.into_iter().enumerate() {
        let x =
            (LOGICAL_WIDTH - width) / 2 + i32::try_from(index).unwrap_or_default() * ACTION_WIDTH;
        add_button(action, x, 156, text, hits);
    }
}

fn add_food_choice(
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    rects.push(panel(
        "ui/food-panel",
        Rect {
            x: 61,
            y: 152,
            w: 198,
            h: 24,
        },
    ));
    for (index, food) in [FoodId::Berry, FoodId::Mushroom, FoodId::Pellet]
        .into_iter()
        .enumerate()
    {
        let action = UiAction::Feed(food);
        add_button(
            action,
            64 + i32::try_from(index).unwrap_or_default() * 64,
            155,
            text,
            hits,
        );
    }
}

fn add_speech(
    speech: &str,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    rects.push(panel(
        "ui/speech-bubble",
        Rect {
            x: 76,
            y: 12,
            w: 168,
            h: 48,
        },
    ));
    text.push(TextCommand {
        id: "speech/text".to_owned(),
        text: speech.to_owned(),
        x: 84,
        y: 20,
        layer: 21,
    });
    for (index, reaction) in [Reaction::Laugh, Reaction::Disapprove, Reaction::Comfort]
        .into_iter()
        .enumerate()
    {
        let rect = Rect {
            x: 77 + i32::try_from(index).unwrap_or_default() * 56,
            y: 62,
            w: 54,
            h: ACTION_HEIGHT,
        };
        hits.push(HitRegion {
            id: format!("reaction/{}", reaction_name(reaction)),
            target: Some(UiTarget::Reaction(reaction)),
            action: UiAction::React(reaction),
            rect,
            enabled: true,
        });
        text.push(TextCommand {
            id: format!("reaction/{}/label", reaction_name(reaction)),
            text: action_label(UiAction::React(reaction)).to_owned(),
            x: rect.x + 4,
            y: rect.y + 4,
            layer: 21,
        });
    }
}

fn add_text_entry(
    view: &ViewState,
    show_keyboard: bool,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let entry_rect = if show_keyboard {
        Rect {
            x: 16,
            y: 8,
            w: 288,
            h: 25,
        }
    } else {
        Rect {
            x: 40,
            y: 126,
            w: 240,
            h: 30,
        }
    };
    rects.push(panel("ui/text-entry", entry_rect));
    text.push(TextCommand {
        id: "text-entry/prompt".to_owned(),
        text: if view.pending {
            "Thinking...".to_owned()
        } else if view.text_buffer.is_empty() {
            "Say something".to_owned()
        } else {
            view.text_buffer.clone()
        },
        x: entry_rect.x + 7,
        y: entry_rect.y + 7,
        layer: 21,
    });
    if view.pending {
        return;
    }
    if show_keyboard {
        add_on_screen_keyboard(!view.text_buffer.trim().is_empty(), rects, text, hits);
    } else {
        add_text_control(
            "text-entry/cancel",
            "Cancel",
            UiAction::CancelText,
            Rect {
                x: 42,
                y: 158,
                w: 60,
                h: 18,
            },
            true,
            rects,
            text,
            hits,
        );
        add_text_control(
            "text-entry/send",
            "Send",
            UiAction::SubmitText,
            Rect {
                x: 218,
                y: 158,
                w: 60,
                h: 18,
            },
            !view.text_buffer.trim().is_empty(),
            rects,
            text,
            hits,
        );
    }
}

fn add_on_screen_keyboard(
    can_submit: bool,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    const KEYS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ ";
    for (index, character) in KEYS.chars().enumerate() {
        let column = i32::try_from(index % 9).unwrap_or_default();
        let row = i32::try_from(index / 9).unwrap_or_default();
        let rect = Rect {
            x: 17 + column * 32,
            y: 40 + row * 27,
            w: 29,
            h: 23,
        };
        let key_name = if character == ' ' {
            "space".to_owned()
        } else {
            character.to_ascii_lowercase().to_string()
        };
        let label = if character == ' ' {
            "Space".to_owned()
        } else {
            character.to_string()
        };
        add_text_control(
            &format!("keyboard/{key_name}"),
            &label,
            UiAction::TypeCharacter(character.to_ascii_lowercase()),
            rect,
            true,
            rects,
            text,
            hits,
        );
    }
    for (id, label, action, rect) in [
        (
            "keyboard/delete",
            "Delete",
            UiAction::Backspace,
            Rect {
                x: 17,
                y: 124,
                w: 78,
                h: 23,
            },
        ),
        (
            "keyboard/cancel",
            "Cancel",
            UiAction::CancelText,
            Rect {
                x: 121,
                y: 124,
                w: 78,
                h: 23,
            },
        ),
        (
            "keyboard/send",
            "Send",
            UiAction::SubmitText,
            Rect {
                x: 225,
                y: 124,
                w: 78,
                h: 23,
            },
        ),
    ] {
        add_text_control(
            id,
            label,
            action,
            rect,
            action != UiAction::SubmitText || can_submit,
            rects,
            text,
            hits,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_text_control(
    id: &str,
    label: &str,
    action: UiAction,
    rect: Rect,
    enabled: bool,
    rects: &mut Vec<RectCommand>,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    rects.push(RectCommand {
        id: format!("{id}/background"),
        rect,
        color: if enabled {
            [63, 57, 75, 235]
        } else {
            [45, 42, 50, 160]
        },
        layer: 20,
    });
    text.push(TextCommand {
        id: format!("{id}/label"),
        text: label.to_owned(),
        x: rect.x + 6,
        y: rect.y + 6,
        layer: 21,
    });
    hits.push(HitRegion {
        id: id.to_owned(),
        target: None,
        action,
        rect,
        enabled,
    });
}

fn add_button(
    action: UiAction,
    x: i32,
    y: i32,
    text: &mut Vec<TextCommand>,
    hits: &mut Vec<HitRegion>,
) {
    let rect = Rect {
        x,
        y,
        w: 60,
        h: ACTION_HEIGHT,
    };
    let id = action_name(action);
    hits.push(HitRegion {
        id: format!("action/{id}"),
        target: action_target(action),
        action,
        rect,
        enabled: true,
    });
    text.push(TextCommand {
        id: format!("action/{id}/label"),
        text: action_label(action).to_owned(),
        x: x + 4,
        y: y + 4,
        layer: 21,
    });
}

fn panel(id: &str, rect: Rect) -> RectCommand {
    RectCommand {
        id: id.to_owned(),
        rect,
        color: [25, 23, 31, 224],
        layer: 20,
    }
}

fn grow(rect: Rect, amount: i32) -> Rect {
    Rect {
        x: rect.x - amount,
        y: rect.y - amount,
        w: rect.w + amount * 2,
        h: rect.h + amount * 2,
    }
}

fn creature_position(state: &WorldState) -> (i32, i32) {
    let spot = |spot| match spot {
        RoomSpot::Bed => (42.0, 104.0),
        RoomSpot::Bowl => (224.0, 108.0),
        RoomSpot::Toy => (100.0, 105.0),
        RoomSpot::Player => (154.0, 114.0),
        RoomSpot::Center => (144.0, 96.0),
    };
    let (x, y) = if let Some(movement) = state.creature.movement {
        let (from_x, from_y) = spot(movement.from);
        let (to_x, to_y) = spot(movement.to);
        let progress = movement.progress();
        (
            from_x + (to_x - from_x) * progress,
            from_y + (to_y - from_y) * progress,
        )
    } else {
        spot(state.creature.position)
    };
    (x.round() as i32, y.round() as i32)
}

fn action_target(action: UiAction) -> Option<UiTarget> {
    match action {
        UiAction::Feed(food) => Some(UiTarget::Food(food)),
        UiAction::React(reaction) => Some(UiTarget::Reaction(reaction)),
        UiAction::OpenContext(target) => Some(target),
        _ => None,
    }
}

fn target_name(target: UiTarget) -> &'static str {
    match target {
        UiTarget::Window => "window",
        UiTarget::Bed => "bed",
        UiTarget::Bowl => "bowl",
        UiTarget::Toy => "toy",
        UiTarget::Clutter => "clutter",
        UiTarget::Creature => "creature",
        UiTarget::Food(FoodId::Berry) => "berry",
        UiTarget::Food(FoodId::Mushroom) => "mushroom",
        UiTarget::Food(FoodId::Pellet) => "pellet",
        UiTarget::Reaction(reaction) => reaction_name(reaction),
    }
}

fn action_name(action: UiAction) -> &'static str {
    match action {
        UiAction::OpenContext(_) => "open-context",
        UiAction::CloseContext => "close-context",
        UiAction::OpenFoodChoice => "choose-food",
        UiAction::Feed(FoodId::Berry) => "feed-berry",
        UiAction::Feed(FoodId::Mushroom) => "feed-mushroom",
        UiAction::Feed(FoodId::Pellet) => "feed-pellet",
        UiAction::Play => "play",
        UiAction::Tidy => "tidy",
        UiAction::Comfort => "comfort",
        UiAction::Talk => "talk",
        UiAction::React(Reaction::Laugh) => "react-laugh",
        UiAction::React(Reaction::Disapprove) => "react-disapprove",
        UiAction::React(Reaction::Comfort) => "react-comfort",
        UiAction::TypeCharacter(_) => "type-character",
        UiAction::Backspace => "backspace",
        UiAction::SubmitText => "submit-text",
        UiAction::CancelText => "cancel-text",
    }
}

fn action_label(action: UiAction) -> &'static str {
    match action {
        UiAction::OpenFoodChoice => "Feed",
        UiAction::Feed(FoodId::Berry) => "Berry",
        UiAction::Feed(FoodId::Mushroom) => "Mushroom",
        UiAction::Feed(FoodId::Pellet) => "Pellet",
        UiAction::Play => "Play",
        UiAction::Tidy => "Tidy",
        UiAction::Comfort => "Comfort",
        UiAction::Talk => "Talk",
        UiAction::React(Reaction::Laugh) => "Laugh",
        UiAction::React(Reaction::Disapprove) => "Nope",
        UiAction::React(Reaction::Comfort) => "Comfort",
        UiAction::TypeCharacter(_) => "Type",
        UiAction::Backspace => "Delete",
        UiAction::SubmitText => "Send",
        UiAction::CancelText => "Cancel",
        UiAction::OpenContext(_) | UiAction::CloseContext => "Back",
    }
}

fn reaction_name(reaction: Reaction) -> &'static str {
    match reaction {
        Reaction::Laugh => "laugh",
        Reaction::Disapprove => "disapprove",
        Reaction::Comfort => "comfort",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beastie_core::Movement;

    fn sprite<'a>(plan: &'a RenderPlan, id: &str) -> &'a SpriteCommand {
        plan.sprites
            .iter()
            .find(|command| command.id == id)
            .expect("expected sprite")
    }

    #[test]
    fn room_plan_contains_the_whole_play_space() {
        let state = WorldState::new(42, "Mop");
        let (render, _) = plan(&state, &ViewState::default());
        for id in [
            "room/background",
            "room/window",
            "room/bed",
            "room/bowl",
            "room/toy",
            "room/clutter-tidy",
            "creature/idle",
        ] {
            sprite(&render, id);
        }
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.target == Some(UiTarget::Bowl))
        );
        assert_eq!(LOGICAL_WIDTH, 320);
        assert_eq!(LOGICAL_HEIGHT, 180);
    }

    #[test]
    fn contextual_actions_match_room_targets() {
        assert_eq!(
            contextual_actions(UiTarget::Bowl),
            vec![UiAction::OpenFoodChoice]
        );
        assert_eq!(contextual_actions(UiTarget::Toy), vec![UiAction::Play]);
        assert_eq!(contextual_actions(UiTarget::Clutter), vec![UiAction::Tidy]);
        assert_eq!(
            contextual_actions(UiTarget::Creature),
            vec![UiAction::Comfort, UiAction::Talk]
        );
    }

    #[test]
    fn food_choice_exposes_all_authoritative_food_actions() {
        let state = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::FoodChoice,
            ..ViewState::default()
        };
        let (render, _) = plan(&state, &view);
        let actions: Vec<_> = render.hit_regions.iter().map(|hit| hit.action).collect();
        assert!(actions.contains(&UiAction::Feed(FoodId::Berry)));
        assert!(actions.contains(&UiAction::Feed(FoodId::Mushroom)));
        assert!(actions.contains(&UiAction::Feed(FoodId::Pellet)));
    }

    #[test]
    fn movement_is_interpolated_and_uses_walk_pose() {
        let mut state = WorldState::new(42, "Mop");
        state.creature.movement = Some(Movement {
            from: RoomSpot::Bed,
            to: RoomSpot::Bowl,
            elapsed_ms: 500,
            duration_ms: 1_000,
        });
        let (render, _) = plan(&state, &ViewState::default());
        let creature = sprite(&render, "creature/walk");
        assert_eq!((creature.x, creature.y), (133, 106));
    }

    #[test]
    fn intention_selects_pose_at_semantic_position() {
        let mut state = WorldState::new(42, "Mop");
        state.creature.position = RoomSpot::Bed;
        state.creature.current_intention = Intention::Sleep;
        let (render, _) = plan(&state, &ViewState::default());
        let creature = sprite(&render, "creature/sleep");
        assert_eq!((creature.x, creature.y), (42, 104));
    }

    #[test]
    fn pose_frames_are_deterministic_for_authoritative_time() {
        let cases = [
            (Intention::Idle, "creature/idle", 1_200),
            (Intention::Eat, "creature/eat", 400),
            (Intention::Sleep, "creature/sleep", 1_600),
            (Intention::Play, "creature/play", 300),
            (Intention::RejectFood, "creature/annoyed", 500),
            (Intention::ApproachPlayer, "creature/walk", 250),
        ];
        for (intention, id, elapsed_ms) in cases {
            let mut state = WorldState::new(42, "Mop");
            state.creature.current_intention = intention;
            state.elapsed_ms = elapsed_ms;
            let (render, _) = plan(&state, &ViewState::default());
            assert_eq!(sprite(&render, id).frame, 1, "{id}");
        }
    }

    #[test]
    fn identical_state_produces_an_identical_lit_plan() {
        let mut state = WorldState::new(42, "Mop");
        state.elapsed_ms = beastie_core::ACTIVE_DAY_MS * 2 / 3;
        let first = plan(&state, &ViewState::default()).0;
        let second = plan(&state, &ViewState::default()).0;
        assert_eq!(first, second);
        assert_eq!(sprite(&first, "room/window").frame, 2);
        assert!(
            first
                .rects
                .iter()
                .any(|rect| { rect.id == "room/lighting" && rect.color == [23, 30, 66, 42] })
        );
    }

    #[test]
    fn reactions_exist_only_while_speech_is_visible() {
        let state = WorldState::new(42, "Mop");
        let (idle, _) = plan(&state, &ViewState::default());
        assert!(
            !idle
                .hit_regions
                .iter()
                .any(|hit| matches!(hit.action, UiAction::React(_)))
        );

        let speaking = ViewState {
            speech: Some("Berry again? Bold.".to_owned()),
            ..ViewState::default()
        };
        let (speaking, _) = plan(&state, &speaking);
        assert_eq!(
            speaking
                .hit_regions
                .iter()
                .filter(|hit| matches!(hit.action, UiAction::React(_)))
                .count(),
            3
        );
    }

    #[test]
    fn idle_chrome_recedes_and_no_bars_are_projected() {
        let state = WorldState::new(42, "Mop");
        let (render, _) = plan(&state, &ViewState::default());
        assert!(
            render
                .rects
                .iter()
                .all(|command| !command.id.starts_with("ui/"))
        );
        assert!(render.sprites.iter().all(|command| {
            !command.id.contains("need")
                && !command.id.contains("relationship")
                && !command.id.contains("bar")
        }));
        assert!(render.text.is_empty());
    }

    #[test]
    fn focus_is_a_declarative_highlight() {
        let state = WorldState::new(42, "Mop");
        let view = ViewState {
            focused_region: Some("target/toy".to_owned()),
            ..ViewState::default()
        };
        let (render, _) = plan(&state, &view);
        assert!(render.rects.iter().any(|command| command.id == "ui/focus"));
    }

    #[test]
    fn on_screen_keyboard_has_stable_character_and_edit_actions() {
        let state = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::OnScreenKeyboard,
            text_buffer: "hi".to_owned(),
            focused_region: Some("keyboard/send".to_owned()),
            ..ViewState::default()
        };
        let (render, _) = plan(&state, &view);
        assert!(
            render.hit_regions.iter().any(|hit| {
                hit.id == "keyboard/a" && hit.action == UiAction::TypeCharacter('a')
            })
        );
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.id == "keyboard/space")
        );
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.action == UiAction::Backspace)
        );
        assert!(
            render
                .hit_regions
                .iter()
                .any(|hit| hit.action == UiAction::SubmitText)
        );
        assert!(render.rects.iter().any(|command| command.id == "ui/focus"));
    }

    #[test]
    fn pending_text_entry_has_no_acceptance_regions() {
        let state = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::TextEntry,
            text_buffer: "hello".to_owned(),
            pending: true,
            ..ViewState::default()
        };
        let (render, _) = plan(&state, &view);
        assert!(
            render
                .text
                .iter()
                .any(|command| command.text == "Thinking...")
        );
        assert!(!render.hit_regions.iter().any(|hit| {
            matches!(
                hit.action,
                UiAction::SubmitText | UiAction::TypeCharacter(_) | UiAction::Backspace
            )
        }));
    }
}
