use beastie_view::{CursorKind, HitRegion, RenderPlan, UiAction};

pub const MAX_TALK_CHARACTERS: usize = 512;

#[must_use]
pub fn action_at(plan: &RenderPlan, x: f32, y: f32) -> Option<UiAction> {
    region_at(plan, x, y).map(|hit| hit.action)
}

#[must_use]
pub fn region_at(plan: &RenderPlan, x: f32, y: f32) -> Option<&HitRegion> {
    plan.hit_regions
        .iter()
        .rev()
        .find(|hit| hit.enabled && hit.rect.contains(x.floor() as i32, y.floor() as i32))
}

#[must_use]
pub fn cursor_at(plan: &RenderPlan, x: f32, y: f32) -> CursorKind {
    region_at(plan, x, y).map_or(CursorKind::Default, |hit| hit.cursor)
}

#[must_use]
pub fn move_focus(plan: &RenderPlan, current: Option<&str>, delta: i32) -> Option<String> {
    let enabled = plan
        .hit_regions
        .iter()
        .filter(|hit| hit.enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return None;
    }
    if current.is_none()
        && let Some(preferred) = enabled.iter().find(|hit| {
            hit.id.starts_with("action/")
                || hit.id.starts_with("keyboard/")
                || hit.id.starts_with("settings/")
                || hit.id == "world/drop-food"
        })
    {
        return Some(preferred.id.clone());
    }
    let current_index = current
        .and_then(|id| enabled.iter().position(|hit| hit.id == id))
        .unwrap_or(if delta < 0 { 0 } else { enabled.len() - 1 });
    let next = (i32::try_from(current_index).unwrap_or_default() + delta)
        .rem_euclid(i32::try_from(enabled.len()).unwrap_or(1));
    Some(
        enabled[usize::try_from(next).unwrap_or_default()]
            .id
            .clone(),
    )
}

#[must_use]
pub fn focused_action(plan: &RenderPlan, focused: Option<&str>) -> Option<UiAction> {
    let focused = focused?;
    plan.hit_regions
        .iter()
        .find(|hit| hit.enabled && hit.id == focused)
        .map(|hit| hit.action)
}

pub fn append_text(buffer: &mut String, text: &str) {
    let remaining = MAX_TALK_CHARACTERS.saturating_sub(buffer.chars().count());
    buffer.extend(
        text.chars()
            .filter(|character| !character.is_control())
            .take(remaining),
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use beastie_core::{ToyId, WorldState};
    use beastie_view::{UiMode, UiTarget, ViewState, plan};

    use super::*;

    #[test]
    fn modal_focus_starts_on_the_first_relevant_action() {
        let world = WorldState::new(42, "Mop");
        for (mode, expected) in [
            (UiMode::Context(UiTarget::Creature), "action/comfort"),
            (UiMode::FoodChoice, "action/select-berry"),
            (UiMode::ToyChoice, "action/play-ball"),
            (UiMode::OnScreenKeyboard, "keyboard/a"),
        ] {
            let view = ViewState {
                mode,
                ..ViewState::default()
            };
            let render = plan(&world, &view).0;
            assert_eq!(move_focus(&render, None, 1).as_deref(), Some(expected));
        }
    }

    #[test]
    fn compose_is_the_persistent_default_focus() {
        let view = ViewState::default();
        assert_eq!(view.mode, UiMode::Compose);
        assert_eq!(view.focused_region.as_deref(), Some("compose/input"));
    }

    #[test]
    fn text_is_unicode_safe_bounded_and_strips_controls() {
        let mut buffer = "a".repeat(MAX_TALK_CHARACTERS - 1);
        append_text(&mut buffer, "é\nextra");
        assert_eq!(buffer.chars().count(), MAX_TALK_CHARACTERS);
        assert!(buffer.ends_with('é'));
        buffer.pop();
        assert_eq!(buffer.chars().count(), MAX_TALK_CHARACTERS - 1);
    }

    #[test]
    fn toy_choice_mouse_and_focus_select_the_same_typed_actions() {
        let world = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::ToyChoice,
            ..ViewState::default()
        };
        let (render, _) = plan(&world, &view);
        let ball = render
            .hit_regions
            .iter()
            .find(|hit| hit.id == "action/play-ball")
            .expect("ball choice");
        let sock = render
            .hit_regions
            .iter()
            .find(|hit| hit.id == "action/play-sock")
            .expect("sock choice");
        assert_eq!(
            action_at(&render, ball.rect.x as f32 + 1.0, ball.rect.y as f32 + 1.0),
            Some(UiAction::Play(ToyId::Ball))
        );
        assert_eq!(
            action_at(&render, sock.rect.x as f32 + 1.0, sock.rect.y as f32 + 1.0),
            Some(UiAction::Play(ToyId::Sock))
        );
        assert_eq!(
            focused_action(&render, Some("action/play-ball")),
            Some(UiAction::Play(ToyId::Ball))
        );
        assert_eq!(
            focused_action(&render, Some("action/play-sock")),
            Some(UiAction::Play(ToyId::Sock))
        );
        assert_eq!(
            move_focus(&render, Some("action/play-ball"), 1).as_deref(),
            Some("action/play-bell")
        );
    }

    #[test]
    fn controller_keyboard_focus_types_deletes_submits_and_cancels() {
        let world = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::OnScreenKeyboard,
            text_buffer: "hi".to_owned(),
            ..ViewState::default()
        };
        let render = plan(&world, &view).0;
        for (id, expected) in [
            ("keyboard/question", UiAction::TypeCharacter('?')),
            ("keyboard/delete", UiAction::Backspace),
            ("keyboard/send", UiAction::SubmitText),
            ("keyboard/cancel", UiAction::CancelMode),
        ] {
            assert_eq!(focused_action(&render, Some(id)), Some(expected), "{id}");
        }
        assert_eq!(
            move_focus(&render, Some("keyboard/exclamation"), 1).as_deref(),
            Some("keyboard/delete")
        );
        assert_eq!(
            move_focus(&render, Some("keyboard/delete"), 1).as_deref(),
            Some("keyboard/cancel")
        );
        assert_eq!(
            move_focus(&render, Some("keyboard/cancel"), 1).as_deref(),
            Some("keyboard/send")
        );
    }

    #[test]
    fn controller_focus_visits_every_enabled_semantic_region_and_wraps() {
        let world = WorldState::new(42, "Mop");
        for mode in [
            UiMode::Context(UiTarget::Creature),
            UiMode::FoodChoice,
            UiMode::ToyChoice,
            UiMode::Settings,
            UiMode::DataManagement,
            UiMode::ConfirmReset,
            UiMode::OnScreenKeyboard,
        ] {
            let render = plan(
                &world,
                &ViewState {
                    mode,
                    ..ViewState::default()
                },
            )
            .0;
            let enabled = render
                .hit_regions
                .iter()
                .filter(|region| region.enabled)
                .count();
            assert!(enabled > 0);

            let first = move_focus(&render, None, 1).expect("initial controller focus");
            let mut focused = first.clone();
            let mut visited = BTreeSet::new();
            for _ in 0..enabled {
                assert!(
                    visited.insert(focused.clone()),
                    "duplicate focus before wrap"
                );
                assert!(focused_action(&render, Some(&focused)).is_some());
                focused = move_focus(&render, Some(&focused), 1).expect("next focus");
            }
            assert_eq!(visited.len(), enabled);
            assert_eq!(focused, first);
        }
    }
}
