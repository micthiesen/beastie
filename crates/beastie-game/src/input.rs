use beastie_view::{RenderPlan, UiAction};

pub const MAX_TALK_CHARACTERS: usize = 512;

#[must_use]
pub fn action_at(plan: &RenderPlan, x: f32, y: f32) -> Option<UiAction> {
    plan.hit_regions
        .iter()
        .rev()
        .find(|hit| hit.enabled && hit.rect.contains(x.floor() as i32, y.floor() as i32))
        .map(|hit| hit.action)
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
    use beastie_core::WorldState;
    use beastie_view::{UiMode, ViewState, plan};

    use super::*;

    #[test]
    fn focus_cycles_only_enabled_regions() {
        let world = WorldState::new(42, "Mop");
        let view = ViewState {
            mode: UiMode::TextEntry,
            ..ViewState::default()
        };
        let (plan, _) = plan(&world, &view);
        assert_eq!(move_focus(&plan, None, 1).as_deref(), Some("target/bowl"));
        let focused = move_focus(&plan, Some("text-entry/cancel"), 1);
        assert_ne!(focused.as_deref(), Some("text-entry/send"));
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
}
