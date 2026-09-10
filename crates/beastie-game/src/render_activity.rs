//! Suppress hidden-window GPU work without changing gameplay or visible pacing.
use bevy::{
    prelude::*,
    render::{
        RenderApp,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
    },
    window::{PrimaryWindow, WindowFocused, WindowOccluded},
};

/// Missing platform occlusion events fail open. Focus loss alone never hides a
/// still-visible aquarium. This policy does not suppress or throttle automation.
#[derive(Resource, Clone, Copy, Default, ExtractResource)]
pub(crate) struct RenderActivity {
    occluded: bool,
    force_render: bool,
}

impl RenderActivity {
    pub(crate) fn should_render(&self) -> bool {
        self.force_render || !self.occluded
    }
}

pub(crate) struct RenderActivityPlugin {
    force_render: bool,
}

impl RenderActivityPlugin {
    pub(crate) fn from_args(args: &crate::args::Args) -> Self {
        Self {
            force_render: args.script.is_some()
                || args.capture_dir.is_some()
                || args.feel_dir.is_some()
                || args.render_report.is_some()
                || args.smoke,
        }
    }
}

impl Plugin for RenderActivityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderActivity {
            force_render: self.force_render,
            ..default()
        })
        .init_resource::<bevy::winit::WinitSettings>()
        .add_systems(PreUpdate, (update_activity, update_hidden_pacing).chain());
        if app.get_sub_app(RenderApp).is_some() {
            app.add_plugins(ExtractResourcePlugin::<RenderActivity>::default());
        }
    }
}

fn update_activity(
    primary: Query<Entity, With<PrimaryWindow>>,
    mut occlusion: MessageReader<WindowOccluded>,
    mut focus: MessageReader<WindowFocused>,
    mut activity: ResMut<RenderActivity>,
) {
    let primary = primary.single().ok();
    let mut occluded = primary.is_some() && activity.occluded;
    for event in occlusion.read() {
        if Some(event.window) == primary {
            occluded = event.occluded;
        }
    }
    // A restored focus is also a recovery path if a backend omits the matching
    // unoccluded event. Prefer rendering on ambiguous simultaneous events.
    for event in focus.read() {
        if Some(event.window) == primary && event.focused {
            occluded = false;
        }
    }
    if activity.occluded != occluded {
        activity.occluded = occluded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(force_render: bool) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<WindowOccluded>()
            .add_message::<WindowFocused>()
            .add_plugins(RenderActivityPlugin { force_render });
        let window = app.world_mut().spawn(PrimaryWindow).id();
        (app, window)
    }

    fn renders(app: &App) -> bool {
        app.world().resource::<RenderActivity>().should_render()
    }

    #[test]
    fn missing_events_and_visible_unfocused_windows_keep_rendering() {
        let (mut app, window) = app(false);
        app.update();
        assert!(renders(&app));
        app.world_mut().write_message(WindowFocused {
            window,
            focused: false,
        });
        app.update();
        assert!(renders(&app));
        let other = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(WindowOccluded {
            window: other,
            occluded: true,
        });
        app.update();
        assert!(renders(&app));
    }

    #[test]
    fn confirmed_occlusion_suppresses_until_restore_or_focus_recovery() {
        let (mut app, window) = app(false);
        for restored_by_focus in [false, true] {
            app.world_mut().write_message(WindowOccluded {
                window,
                occluded: true,
            });
            app.update();
            assert!(!renders(&app));
            if restored_by_focus {
                app.world_mut().write_message(WindowFocused {
                    window,
                    focused: true,
                });
            } else {
                app.world_mut().write_message(WindowOccluded {
                    window,
                    occluded: false,
                });
            }
            app.update();
            assert!(renders(&app));
        }
    }

    #[test]
    fn automation_is_not_suppressed_when_occluded() {
        let (mut app, window) = app(true);
        app.world_mut().write_message(WindowOccluded {
            window,
            occluded: true,
        });
        app.update();
        assert!(renders(&app));
    }

    #[test]
    fn automation_flags_select_override() {
        use clap::Parser;
        for flags in [vec![], vec!["--smoke"], vec!["--script", "fixture.jsonl"]] {
            let expected = !flags.is_empty();
            let args =
                crate::args::Args::try_parse_from(std::iter::once("beastie").chain(flags)).unwrap();
            assert_eq!(
                RenderActivityPlugin::from_args(&args).force_render,
                expected
            );
        }
    }
}

/// Preserve the caller's exact visible pacing policy. Only explicit occlusion
/// reduces wakeups; window events still wake immediately, including restore.
fn update_hidden_pacing(
    activity: Res<RenderActivity>,
    mut settings: ResMut<bevy::winit::WinitSettings>,
    mut visible: Local<Option<bevy::winit::WinitSettings>>,
) {
    if !activity.should_render() {
        if visible.is_none() {
            *visible = Some(settings.clone());
            let hidden =
                bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_millis(100));
            settings.focused_mode = hidden;
            settings.unfocused_mode = hidden;
        }
    } else if let Some(original) = visible.take() {
        *settings = original;
    }
}

#[cfg(test)]
mod hidden_pacing_tests {
    use super::*;
    use bevy::winit::{UpdateMode, WinitSettings};
    fn setup(force_render: bool) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<WindowOccluded>()
            .add_message::<WindowFocused>()
            .add_plugins(RenderActivityPlugin { force_render });
        let window = app.world_mut().spawn(PrimaryWindow).id();
        (app, window)
    }
    #[test]
    fn hidden_pacing_preserves_visible_policies_and_restores_on_window_events() {
        let (mut app, window) = setup(false);
        let focused = UpdateMode::Continuous;
        let unfocused = UpdateMode::reactive(std::time::Duration::from_millis(23));
        app.insert_resource(WinitSettings {
            focused_mode: focused,
            unfocused_mode: unfocused,
        });
        app.world_mut().write_message(WindowFocused {
            window,
            focused: false,
        });
        app.update();
        assert_eq!(
            app.world().resource::<WinitSettings>().unfocused_mode,
            unfocused
        );
        for focus_restore in [false, true] {
            app.world_mut().write_message(WindowOccluded {
                window,
                occluded: true,
            });
            app.update();
            let hidden = UpdateMode::reactive_low_power(std::time::Duration::from_millis(100));
            assert_eq!(app.world().resource::<WinitSettings>().focused_mode, hidden);
            assert_eq!(
                app.world().resource::<WinitSettings>().unfocused_mode,
                hidden
            );
            app.update(); // A repeated hidden update must not overwrite saved modes.
            if focus_restore {
                app.world_mut().write_message(WindowFocused {
                    window,
                    focused: true,
                });
            } else {
                app.world_mut().write_message(WindowOccluded {
                    window,
                    occluded: false,
                });
            }
            app.update();
            let restored = app.world().resource::<WinitSettings>();
            assert_eq!(restored.focused_mode, focused);
            assert_eq!(restored.unfocused_mode, unfocused);
        }
    }
    #[test]
    fn automation_does_not_throttle_when_occluded() {
        let (mut app, window) = setup(true);
        let original = app.world().resource::<WinitSettings>().clone();
        app.world_mut().write_message(WindowOccluded {
            window,
            occluded: true,
        });
        app.update();
        let current = app.world().resource::<WinitSettings>();
        assert_eq!(current.focused_mode, original.focused_mode);
        assert_eq!(current.unfocused_mode, original.unfocused_mode);
    }
}
