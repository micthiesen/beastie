//! Shared interface materials, typography and display-free text layout.
use crate::{HitRegion, Rect, RectCommand, TextCommand};
use serde::{Deserialize, Serialize};

pub(super) const UI_EDGE: [u8; 4] = [71, 96, 94, 255];
pub(super) const UI_PANEL: [u8; 4] = [19, 38, 42, 255];
pub(super) const UI_PANEL_INSET: [u8; 4] = [18, 29, 31, 255];
pub(super) const UI_BUTTON: [u8; 4] = [33, 48, 48, 255];
pub(super) const UI_BUTTON_DISABLED: [u8; 4] = [19, 38, 42, 255];
pub(super) const UI_CORAL: [u8; 4] = [226, 223, 179, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Title,
    Identity,
    Body,
    Secondary,
    Subtitle,
    Control,
    ControlCaption,
    Dialogue,
}
impl TextRole {
    /// Sizes are authored in layout units; large mode is a real, bounded increase.
    pub fn size(self, large: bool) -> f32 {
        let normal = match self {
            Self::Title => 8.0,
            Self::Identity => 6.5,
            Self::Body | Self::Dialogue => 6.0,
            Self::Secondary | Self::Subtitle => 4.8,
            Self::Control => 5.2,
            Self::ControlCaption => 4.5,
        };
        normal * if large { 1.3 } else { 1.0 }
    }
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Secondary | Self::Subtitle | Self::ControlCaption => [168, 191, 188],
            _ => [236, 233, 211],
        }
    }
    pub fn centered(self) -> bool {
        matches!(self, Self::Subtitle | Self::Control | Self::ControlCaption)
    }
}

/// Resolve layout in the semantic view, so rendering never guesses from neighboring labels.
/// Controls own their text bounds; panel copy uses its authored anchor and local padding.
pub(super) fn layout_text(text: &mut [TextCommand], rects: &[RectCommand], hits: &[HitRegion]) {
    let anchors: Vec<_> = text.iter().map(|t| (t.x, t.y, t.layer)).collect();
    for t in text {
        if t.bounds.is_some() {
            continue;
        }
        t.role = if t.id.ends_with("/title") {
            TextRole::Title
        } else if t.id == "compose/summary-name" {
            TextRole::Identity
        } else if t.id == "compose/summary-behavior"
            || t.id == "status/message"
            || t.id == "ui/hover-label"
            || t.id == "reset/detail"
        {
            TextRole::Secondary
        } else if t.id == "speech/text" {
            TextRole::Dialogue
        } else {
            TextRole::Body
        };
        if let Some(control) = hits
            .iter()
            .filter(|h| {
                !h.id.starts_with("world/")
                    && matches!(h.shape, crate::HitShape::Rect)
                    && h.rect.contains(t.x, t.y)
            })
            .min_by_key(|h| h.rect.w * h.rect.h)
        {
            // The compose field keeps left alignment; all button labels use the full inset.
            if control.id != "compose/input" {
                t.role = TextRole::Control;
                t.muted = !control.enabled;
                t.bounds = Some(Rect {
                    x: control.rect.x + 3,
                    y: control.rect.y + 2,
                    w: control.rect.w - 6,
                    h: control.rect.h - 4,
                });
                continue;
            }
        }
        let container = rects
            .iter()
            .filter(|r| {
                !r.outline
                    && r.layer <= t.layer
                    && r.rect.w >= 8
                    && r.rect.h >= 6
                    && r.rect.contains(t.x, t.y)
            })
            .min_by_key(|r| r.rect.w * r.rect.h)
            .map(|r| r.rect)
            .unwrap_or(Rect {
                x: 0,
                y: 0,
                w: 320,
                h: 180,
            });
        let mut right = container.x + container.w - 3;
        let mut bottom = container.y + container.h - 2;
        for h in hits {
            if matches!(h.shape, crate::HitShape::Rect)
                && h.rect.x > t.x
                && h.rect.y <= t.y
                && h.rect.y + h.rect.h > t.y
            {
                right = right.min(h.rect.x - 3);
            }
        }
        for &(x, y, layer) in &anchors {
            if layer != t.layer {
                continue;
            }
            if y == t.y && x > t.x {
                right = right.min(x - 3);
            }
            if y > t.y && x >= t.x && x < right {
                bottom = bottom.min(y - 1);
            }
        }
        let bounds = match t.id.as_str() {
            "compose/input-text" => Rect {
                x: 18,
                y: 155,
                w: 157,
                h: 17,
            },
            "status/message" => Rect {
                x: 84,
                y: 133,
                w: 219,
                h: 12,
            },
            _ => Rect {
                x: t.x,
                y: t.y,
                w: (right - t.x).max(1),
                h: (bottom - t.y).max(1),
            },
        };
        t.bounds = Some(bounds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drop_food_target_does_not_own_instruction_typography() {
        let state = beastie_core::WorldState::new(42, "Mop");
        let scene = crate::plan(
            &state,
            &crate::ViewState {
                mode: crate::UiMode::FoodDrop(beastie_core::FoodId::Berry),
                ..Default::default()
            },
        )
        .0;
        let instruction = scene
            .text
            .iter()
            .find(|t| t.id == "mode/drop-food-label")
            .unwrap();
        assert_eq!(instruction.role, TextRole::Body);
        let bounds = instruction.bounds.unwrap();
        assert!(bounds.x >= 79 && bounds.x + bounds.w <= 241);
    }
    #[test]
    fn settings_pages_have_readable_distinct_control_regions() {
        let state = beastie_core::WorldState::new(42, "Mop");
        for settings_page in 0..3 {
            let scene = crate::plan(
                &state,
                &crate::ViewState {
                    mode: crate::UiMode::Settings,
                    settings_page,
                    text_scale: 2,
                    ..Default::default()
                },
            )
            .0;
            for text in scene
                .text
                .iter()
                .filter(|t| t.id.starts_with("settings/") && t.id.ends_with("-value"))
            {
                let bounds = text.bounds.unwrap();
                assert!(bounds.h >= text.role.size(true).ceil() as i32 + 2);
                assert!(
                    bounds.w as f32
                        >= text.text.chars().count() as f32 * text.role.size(true) * 0.5
                );
            }
        }
    }
}
