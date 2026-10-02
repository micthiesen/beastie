//! Shared interface materials, typography and display-free text layout.
use crate::{HitRegion, Rect, RectCommand, TextCommand};
use serde::{Deserialize, Serialize};

pub(super) const UI_EDGE: [u8; 4] = [80, 115, 112, 255];
pub(super) const UI_PANEL: [u8; 4] = [12, 42, 47, 255];
pub(super) const UI_PANEL_INSET: [u8; 4] = [9, 33, 39, 255];
pub(super) const UI_BUTTON: [u8; 4] = [26, 59, 63, 255];
pub(super) const UI_BUTTON_DISABLED: [u8; 4] = [19, 42, 46, 255];
pub(super) const UI_CORAL: [u8; 4] = [227, 204, 148, 255];
pub(super) const UI_PRIMARY: [u8; 4] = [240, 230, 192, 255];
pub(super) const UI_SELECTED: [u8; 4] = [58, 86, 72, 255];
pub(super) const UI_DANGER: [u8; 4] = [200, 128, 107, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    Title,
    Identity,
    Body,
    Secondary,
    Subtitle,
    Control,
    PrimaryControl,
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
            Self::Control | Self::PrimaryControl => 5.2,
            Self::ControlCaption => 4.5,
        };
        normal * if large { 1.3 } else { 1.0 }
    }
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::PrimaryControl => [13, 45, 49],
            Self::Secondary | Self::Subtitle | Self::ControlCaption => [168, 191, 188],
            _ => [236, 233, 211],
        }
    }
    pub fn centered(self) -> bool {
        matches!(
            self,
            Self::Subtitle | Self::Control | Self::PrimaryControl | Self::ControlCaption
        )
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
    fn controller_name_entry_owns_its_field_and_explicit_submission() {
        let world = beastie_core::WorldState::new(42, "Mop");
        for value in ["", "Gob"] {
            let scene = crate::plan(
                &world,
                &crate::ViewState {
                    mode: crate::UiMode::OnScreenKeyboard,
                    renaming_with_osk: true,
                    text_buffer: value.into(),
                    ..Default::default()
                },
            )
            .0;
            let field = scene
                .text
                .iter()
                .find(|t| t.id == "keyboard/input-text")
                .unwrap();
            assert_eq!(
                field.text,
                if value.is_empty() {
                    "New name…"
                } else {
                    value
                }
            );
            let submit = scene
                .hit_regions
                .iter()
                .find(|h| h.id == "keyboard/send")
                .unwrap();
            assert_eq!(submit.action, crate::UiAction::SubmitName);
            assert_eq!(submit.label, "Save name");
            assert_eq!(submit.enabled, !value.is_empty());
            assert!(!scene.hit_regions.iter().any(|h| {
                h.enabled && matches!(h.id.as_str(), "compose/input" | "compose/send")
            }));
        }
    }

    #[test]
    fn secondary_dialog_headers_and_binding_rows_align_with_their_controls() {
        let state = beastie_core::WorldState::new(42, "Mop");
        for mode in [crate::UiMode::Bindings, crate::UiMode::DataManagement] {
            for text_scale in [1, 2] {
                let scene = crate::plan(
                    &state,
                    &crate::ViewState {
                        mode,
                        text_scale,
                        ..Default::default()
                    },
                )
                .0;
                let header = scene
                    .text
                    .iter()
                    .find(|t| t.id.ends_with("/title"))
                    .unwrap();
                let close = scene
                    .hit_regions
                    .iter()
                    .find(|h| h.id == "modal/close")
                    .unwrap()
                    .rect;
                let bounds = header.bounds.unwrap();
                assert!(header.vertical_centered);
                assert!((2 * bounds.y + bounds.h - 2 * close.y - close.h).abs() <= 1);
                assert!(bounds.x + bounds.w < close.x);
                for name in scene
                    .text
                    .iter()
                    .filter(|t| t.id.starts_with("bindings/") && t.id.ends_with("-name"))
                {
                    let id = name.id.strip_suffix("-name").unwrap();
                    let control = scene.hit_regions.iter().find(|h| h.id == id).unwrap().rect;
                    let bounds = name.bounds.unwrap();
                    assert!(name.vertical_centered);
                    assert!(!name.role.centered());
                    assert_eq!(2 * bounds.y + bounds.h, 2 * control.y + control.h);
                    assert!(bounds.x + bounds.w < control.x);
                }
            }
        }
    }

    #[test]
    fn action_sheets_have_equal_outer_gutters_and_clear_focus_rims() {
        let state = beastie_core::WorldState::new(42, "Mop");
        for mode in [
            crate::UiMode::FoodChoice,
            crate::UiMode::ToyChoice,
            crate::UiMode::Context(crate::UiTarget::Creature),
        ] {
            let scene = crate::plan(
                &state,
                &crate::ViewState {
                    mode,
                    ..Default::default()
                },
            )
            .0;
            let panel = scene
                .rects
                .iter()
                .find(|r| r.id.starts_with("mode/") && r.id.ends_with("-panel"))
                .unwrap()
                .rect;
            let controls: Vec<_> = scene
                .hit_regions
                .iter()
                .filter(|h| h.id.starts_with("action/"))
                .collect();
            let first = controls.first().unwrap().rect;
            let last = controls.last().unwrap().rect;
            let gutter = first.x - panel.x;
            assert_eq!(gutter, panel.x + panel.w - last.x - last.w);
            assert_eq!(gutter, panel.y + panel.h - last.y - last.h);
            // Focus stays inside its control; preserve clear gutters around each button.
            assert!(gutter > 2);
            for pair in controls.windows(2) {
                assert!(pair[0].rect.x + pair[0].rect.w + 2 < pair[1].rect.x);
            }
            assert!(panel.x >= 0 && panel.x + panel.w <= crate::LOGICAL_WIDTH);
        }
    }

    #[test]
    fn settings_rows_share_a_vertical_center_without_centering_names_horizontally() {
        let state = beastie_core::WorldState::new(42, "Mop");
        for text_scale in [1, 2] {
            for settings_page in 0..2 {
                let scene = crate::plan(
                    &state,
                    &crate::ViewState {
                        mode: crate::UiMode::Settings,
                        settings_page,
                        text_scale,
                        ..Default::default()
                    },
                )
                .0;
                for name in scene
                    .text
                    .iter()
                    .filter(|t| t.id.starts_with("settings/") && t.id.ends_with("-name"))
                {
                    let id = name.id.strip_suffix("-name").unwrap();
                    let value = scene
                        .text
                        .iter()
                        .find(|t| t.id == format!("{id}-value"))
                        .unwrap();
                    let area = scene.hit_regions.iter().find(|h| h.id == id).unwrap().rect;
                    let center = 2 * area.y + area.h;
                    assert!(name.vertical_centered);
                    assert!(!name.role.centered());
                    for bounds in [name.bounds.unwrap(), value.bounds.unwrap()] {
                        assert_eq!(2 * bounds.y + bounds.h, center);
                    }
                    for part in scene
                        .rects
                        .iter()
                        .filter(|r| r.id == format!("{id}-track") || r.id == format!("{id}-thumb"))
                    {
                        assert_eq!(2 * part.rect.y + part.rect.h, center);
                    }
                }
                for category in scene
                    .text
                    .iter()
                    .filter(|t| t.id.starts_with("settings/page-") && t.id.ends_with("-label"))
                {
                    let id = category.id.strip_suffix("-label").unwrap();
                    let area = scene.hit_regions.iter().find(|h| h.id == id).unwrap().rect;
                    let bounds = category.bounds.unwrap();
                    assert!(category.vertical_centered);
                    assert_eq!(2 * bounds.y + bounds.h, 2 * area.y + area.h);
                }
            }
        }
    }

    #[test]
    fn compose_input_and_header_keep_their_control_centers() {
        let state = beastie_core::WorldState::new(42, "Mop");
        let scene = crate::plan(
            &state,
            &crate::ViewState {
                mode: crate::UiMode::Settings,
                ..Default::default()
            },
        )
        .0;
        let input = scene
            .text
            .iter()
            .find(|t| t.id == "compose/input-text")
            .unwrap();
        let field = scene
            .hit_regions
            .iter()
            .find(|h| h.id == "compose/input")
            .unwrap()
            .rect;
        let bounds = input.bounds.unwrap();
        assert!(input.vertical_centered);
        assert!(!input.role.centered());
        assert_eq!(2 * bounds.y + bounds.h, 2 * field.y + field.h);
        assert!(bounds.x >= field.x + 3);
        assert!(bounds.x + bounds.w <= field.x + field.w - 3);
        let header = scene
            .text
            .iter()
            .find(|t| t.id == "settings/title")
            .unwrap()
            .bounds
            .unwrap();
        let close = scene
            .hit_regions
            .iter()
            .find(|h| h.id == "modal/close")
            .unwrap()
            .rect;
        assert!((2 * header.y + header.h - 2 * close.y - close.h).abs() <= 1);
    }

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
        let cancel = scene
            .hit_regions
            .iter()
            .find(|h| h.id == "mode/drop-cancel")
            .unwrap();
        assert!(bounds.x >= 55 && bounds.x + bounds.w < cancel.rect.x);
        assert!(bounds.h as f32 >= instruction.role.size(false) * 1.2);
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
