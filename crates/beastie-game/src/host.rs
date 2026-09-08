//! Native Bevy event adapter. Authoritative gameplay stays in `GameSession`.
use crate::app::Game;
use crate::error::GameResult;
use crate::renderer::{SceneFrame, TankCamera, Viewport};
use bevy::app::AppExit;
use bevy::input::{
    ButtonState,
    gamepad::GamepadButtonStateChangedEvent,
    keyboard::{Key, KeyboardInput},
};
use bevy::prelude::*;
use bevy::window::{
    MonitorSelection, PrimaryWindow, WindowCloseRequested, WindowFocused, WindowMode,
};
use std::time::{Duration, Instant};

pub(crate) struct KeyStroke {
    pub key: Key,
    pub text: Option<String>,
    pub control: bool,
    pub super_key: bool,
    pub shift: bool,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum HostSet {
    Input,
    Update,
    Publish,
}

/// One gate drives both scripted semantic frames and their corresponding video frames.
#[derive(Resource)]
pub(crate) struct FramePacing {
    epoch: Instant,
    clock: ScriptClock,
    pub ready: bool,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            clock: ScriptClock::default(),
            ready: false,
        }
    }
}

#[derive(Default)]
struct ScriptClock {
    started: Option<Duration>,
    frame: u64,
    last_frame: Duration,
}

impl ScriptClock {
    fn poll(&mut self, now: Duration) -> Option<u64> {
        let started = *self.started.get_or_insert(now);
        let deadline =
            started + Duration::from_nanos(self.frame.saturating_mul(1_000_000_000) / 60);
        if now < deadline {
            return None;
        }
        let delta = if self.frame == 0 {
            16
        } else {
            now.saturating_sub(self.last_frame)
                .as_millis()
                .clamp(1, 250) as u64
        };
        self.frame = self.frame.saturating_add(1);
        self.last_frame = now;
        Some(delta)
    }
}

pub(crate) struct HostPlugin;
impl Plugin for HostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FramePacing>()
            .configure_sets(
                Update,
                (HostSet::Input, HostSet::Update, HostSet::Publish).chain(),
            )
            .add_systems(
                Update,
                (keyboard, pointer, controller, window_events)
                    .chain()
                    .in_set(HostSet::Input),
            )
            .add_systems(Update, update.in_set(HostSet::Update))
            .add_systems(Update, publish.in_set(HostSet::Publish));
    }
}

fn handle(result: GameResult, game: &mut Game) {
    if let Err(error) = result {
        error!("{error}");
        game.quit_requested = true;
        game.failed = true;
    }
}

fn keyboard(
    mut events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut game: NonSendMut<Game>,
) {
    for event in events.read() {
        let input = KeyStroke {
            key: event.logical_key.clone(),
            text: event.text.as_ref().map(ToString::to_string),
            control: keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            super_key: keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]),
            shift: keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
        };
        let result = if event.state == ButtonState::Pressed {
            game.key_down_event(input, event.repeat)
        } else {
            game.key_up_event(input)
        };
        handle(result, &mut game);
    }
}

fn pointer(
    mut commands: Commands,
    window: Query<(Entity, &Window), With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<TankCamera>>,
    buttons: Res<ButtonInput<MouseButton>>,
    frame: Res<SceneFrame>,
    motion: Res<crate::creature::CreatureMotion>,
    mut game: NonSendMut<Game>,
) {
    let Ok((window_entity, window)) = window.single() else {
        return;
    };
    let Ok((camera, transform)) = camera.single() else {
        return;
    };
    let cursor = window.cursor_position();
    let logical = cursor
        .and_then(|p| {
            Viewport::for_drawable(window.width(), window.height()).logical_point(p.x, p.y)
        })
        .map(|(x, y)| (x.floor() as i32, y.floor() as i32));
    let hit =
        cursor.and_then(|p| crate::renderer::pick(&frame.plan, camera, transform, p, &motion));
    let cursor_icon = match hit.as_ref().map(|hit| hit.cursor) {
        Some(beastie_view::CursorKind::Pointer) => bevy::window::SystemCursorIcon::Pointer,
        Some(beastie_view::CursorKind::FoodDrop) => bevy::window::SystemCursorIcon::Crosshair,
        _ => bevy::window::SystemCursorIcon::Default,
    };
    commands
        .entity(window_entity)
        .insert(bevy::window::CursorIcon::System(cursor_icon));
    let world = cursor
        .filter(|_| logical.is_some_and(|(_, y)| y < beastie_view::COMPOSE_BAR_TOP))
        .and_then(|p| crate::renderer::pointer_world(camera, transform, p));
    let result = game.pointer_moved(logical, world, hit.as_ref());
    handle(result, &mut game);
    if buttons.just_pressed(MouseButton::Left) {
        let result = game.pointer_pressed(hit.as_ref().map(|hit| hit.action), world);
        handle(result, &mut game);
    }
    if buttons.just_released(MouseButton::Left) {
        let result = game.pointer_released();
        handle(result, &mut game);
    }
}

fn controller(
    mut events: MessageReader<GamepadButtonStateChangedEvent>,
    mut game: NonSendMut<Game>,
) {
    for event in events.read() {
        let result = if event.state == ButtonState::Pressed {
            game.gamepad_button_down_event(event.button)
        } else {
            game.gamepad_button_up_event(event.button)
        };
        handle(result, &mut game);
    }
}

fn window_events(
    mut focus: MessageReader<WindowFocused>,
    mut close: MessageReader<WindowCloseRequested>,
    mut game: NonSendMut<Game>,
) {
    for event in focus.read() {
        let result = game.focus_event(event.focused);
        handle(result, &mut game);
    }
    if close.read().next().is_some() {
        game.quit_requested = true;
    }
}

fn update(
    time: Res<Time>,
    mut pacing: ResMut<FramePacing>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
    mut game: NonSendMut<Game>,
) {
    let native_delta_ms = time.delta().as_millis().clamp(1, 250) as u64;
    let frame_delta = if game.is_scripted() {
        if game.frame_pending {
            None
        } else {
            let elapsed = pacing.epoch.elapsed();
            pacing.clock.poll(elapsed)
        }
    } else {
        Some(native_delta_ms)
    };
    pacing.ready = frame_delta.is_some();
    let result = game.update(frame_delta.unwrap_or(native_delta_ms), pacing.ready);
    handle(result, &mut game);
    if game.take_window_settings_dirty()
        && let Ok(mut window) = window.single_mut()
    {
        let (fullscreen, scale) = game.window_settings();
        window.mode = if fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        if !fullscreen {
            window
                .resolution
                .set(640.0 * f32::from(scale), 360.0 * f32::from(scale));
        }
    }
    if game.quit_requested && game.captures_drained() {
        let result = game.finalize_capture();
        handle(result, &mut game);
        if let Err(error) = game.quit_event() {
            error!("Could not save on shutdown: {error}");
            game.failed = true;
        }
        exit.write(if game.failed {
            AppExit::error()
        } else {
            AppExit::Success
        });
    }
}

fn publish(mut frame: ResMut<SceneFrame>, game: NonSend<Game>) {
    frame.plan = game.render_plan();
}

#[cfg(test)]
mod pacing_tests {
    use super::ScriptClock;
    use std::time::Duration;

    #[test]
    fn ninety_and_one_twenty_hz_hosts_produce_sixty_semantic_frames_per_second() {
        for host_hz in [90_u64, 120, 144] {
            let mut clock = ScriptClock::default();
            let mut frames = 0;
            for host_frame in 0..host_hz * 10 {
                let now = Duration::from_nanos(host_frame * 1_000_000_000 / host_hz);
                frames += usize::from(clock.poll(now).is_some());
            }
            assert_eq!(frames, 600, "host refresh {host_hz}");
        }
    }

    #[test]
    fn slow_frame_never_skips_a_scenario_step_or_advances_twice_in_one_update() {
        let mut clock = ScriptClock::default();
        assert!(clock.poll(Duration::ZERO).is_some());
        assert!(clock.poll(Duration::from_millis(4)).is_none());
        assert!(clock.poll(Duration::from_millis(100)).is_some());
        assert_eq!(clock.frame, 2);
        assert!(clock.poll(Duration::from_millis(110)).is_some());
        assert_eq!(clock.frame, 3);
    }
}
