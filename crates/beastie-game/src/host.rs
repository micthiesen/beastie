//! Native Bevy event adapter. Authoritative gameplay stays in `GameSession`.
use crate::app::Game;
use crate::error::GameResult;
use crate::input::PasteOwner;
use crate::renderer::{SceneFrame, TankCamera, Viewport};
use bevy::app::AppExit;
use bevy::clipboard::{Clipboard, ClipboardRead};
use bevy::input::{
    ButtonState,
    gamepad::GamepadButtonStateChangedEvent,
    keyboard::{Key, KeyboardInput},
};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowCloseRequested, WindowMode};
use bevy::winit::{RawWinitWindowEvent, WINIT_WINDOWS, converters::convert_keyboard_input};
use std::time::{Duration, Instant};
use winit::{event::WindowEvent as NativeWindowEvent, keyboard::ModifiersState};

#[derive(Debug)]
pub(crate) struct KeyStroke {
    pub key: Key,
    pub text: Option<String>,
    pub control: bool,
    pub super_key: bool,
    pub shift: bool,
}

#[derive(Resource, Default)]
struct OrderedModifiers {
    state: ModifiersState,
    control_keys: u8,
    super_keys: u8,
    shift_keys: u8,
}

impl OrderedModifiers {
    fn observe(&mut self, event: &NativeWindowEvent) {
        match event {
            NativeWindowEvent::ModifiersChanged(modifiers) => {
                self.state = modifiers.state();
                if !self.state.control_key() {
                    self.control_keys = 0;
                }
                if !self.state.super_key() {
                    self.super_keys = 0;
                }
                if !self.state.shift_key() {
                    self.shift_keys = 0;
                }
            }
            NativeWindowEvent::Focused(false) => *self = Self::default(),
            _ => {}
        }
    }

    fn stroke(&mut self, event: &KeyboardInput) -> KeyStroke {
        let physical = match event.key_code {
            KeyCode::ControlLeft => Some((&mut self.control_keys, ModifiersState::CONTROL, 1)),
            KeyCode::ControlRight => Some((&mut self.control_keys, ModifiersState::CONTROL, 2)),
            KeyCode::SuperLeft => Some((&mut self.super_keys, ModifiersState::SUPER, 1)),
            KeyCode::SuperRight => Some((&mut self.super_keys, ModifiersState::SUPER, 2)),
            KeyCode::ShiftLeft => Some((&mut self.shift_keys, ModifiersState::SHIFT, 1)),
            KeyCode::ShiftRight => Some((&mut self.shift_keys, ModifiersState::SHIFT, 2)),
            _ => None,
        };
        if let Some((held, flag, side)) = physical {
            if event.state == ButtonState::Pressed {
                *held |= side;
            } else {
                *held &= !side;
            }
            self.state.set(flag, *held != 0);
        }
        KeyStroke {
            key: event.logical_key.clone(),
            text: event.text.as_ref().map(ToString::to_string),
            control: self.state.control_key(),
            super_key: self.state.super_key(),
            shift: self.state.shift_key(),
        }
    }
}

#[derive(Resource, Default)]
struct PendingPaste(Option<(PasteOwner, ClipboardRead)>);

fn poll_paste(pending: &mut PendingPaste, game: &mut Game) {
    let Some((owner, read)) = &mut pending.0 else {
        return;
    };
    if let Some(text) = read.poll_result() {
        game.apply_clipboard_text(*owner, text.map_err(|_| ()));
        pending.0 = None;
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum HostSet {
    Input,
    Update,
    Publish,
}

#[derive(Resource, Default, Clone, Copy)]
pub(crate) enum HostInputPolicy {
    #[default]
    Native,
    ScriptOnly,
}

impl HostInputPolicy {
    pub(crate) const fn from_script_only(script_only: bool) -> Self {
        if script_only {
            Self::ScriptOnly
        } else {
            Self::Native
        }
    }

    const fn allows_native(self) -> bool {
        matches!(self, Self::Native)
    }

    fn suppress_messages<M: Message>(self, messages: &mut MessageReader<M>) -> bool {
        if self.allows_native() {
            return false;
        }
        messages.clear();
        true
    }
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
            .init_resource::<HostInputPolicy>()
            .init_resource::<OrderedModifiers>()
            .init_resource::<PendingPaste>()
            .configure_sets(
                Update,
                (HostSet::Input, HostSet::Update, HostSet::Publish).chain(),
            )
            .add_systems(
                Update,
                (
                    keyboard,
                    pointer,
                    controller,
                    window_events,
                    clipboard_results,
                )
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
    mut events: MessageReader<RawWinitWindowEvent>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut modifiers: ResMut<OrderedModifiers>,
    mut clipboard: ResMut<Clipboard>,
    mut pending: ResMut<PendingPaste>,
    policy: Res<HostInputPolicy>,
    mut game: NonSendMut<Game>,
) {
    if policy.suppress_messages(&mut events) {
        *modifiers = OrderedModifiers::default();
        pending.0 = None;
        return;
    }
    let Ok(primary) = window.single() else {
        events.clear();
        *modifiers = OrderedModifiers::default();
        pending.0 = None;
        return;
    };
    // This is the only game keyboard/focus dispatcher. Bevy's derived KeyboardInput and
    // WindowFocused messages still serve its own systems but must not be replayed here.
    for event in events.read() {
        let source =
            WINIT_WINDOWS.with_borrow(|windows| windows.get_window_entity(event.window_id));
        if source != Some(primary) {
            continue;
        }
        modifiers.observe(&event.event);
        match &event.event {
            NativeWindowEvent::KeyboardInput { event, .. } => {
                let translated = convert_keyboard_input(event, primary);
                let input = modifiers.stroke(&translated);
                let result = if translated.state == ButtonState::Pressed {
                    game.key_down_event(input, translated.repeat)
                } else {
                    game.key_up_event(input)
                };
                handle(result, &mut game);
                if let Some(owner) = game.take_paste_request() {
                    pending.0 = Some((owner, clipboard.fetch_text()));
                    // Desktop reads are ready now, so paste precedes the next ordered key.
                    poll_paste(&mut pending, &mut game);
                }
            }
            NativeWindowEvent::Focused(focused) => {
                if !focused {
                    pending.0 = None;
                }
                let result = game.focus_event(*focused);
                handle(result, &mut game);
            }
            _ => {}
        }
    }
}

fn clipboard_results(
    policy: Res<HostInputPolicy>,
    mut pending: ResMut<PendingPaste>,
    mut game: NonSendMut<Game>,
) {
    if policy.allows_native() {
        poll_paste(&mut pending, &mut game);
    } else {
        pending.0 = None;
    }
}

#[allow(clippy::too_many_arguments)] // Independent Bevy input, scene and cached geometry resources.
fn pointer(
    mut commands: Commands,
    window: Query<(Entity, &Window), With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<TankCamera>>,
    buttons: Res<ButtonInput<MouseButton>>,
    frame: Res<SceneFrame>,
    motion: Res<crate::creature::CreatureMotion>,
    scenery: Res<crate::renderer::SceneryPicking>,
    policy: Res<HostInputPolicy>,
    mut game: NonSendMut<Game>,
) {
    if !policy.allows_native() {
        return;
    }
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
    let hit = cursor
        .and_then(|p| crate::renderer::pick(&frame.plan, camera, transform, p, &motion, &scenery));
    let cursor_icon = match hit.as_ref().map(|hit| hit.cursor) {
        Some(beastie_view::CursorKind::Pointer) => bevy::window::SystemCursorIcon::Pointer,
        _ => bevy::window::SystemCursorIcon::Default,
    };
    commands
        .entity(window_entity)
        .insert(bevy::window::CursorIcon::System(cursor_icon));
    let on_world_object = hit
        .as_ref()
        .is_some_and(|hit| matches!(hit.shape, beastie_view::HitShape::World(_)));
    let world = cursor
        .filter(|_| logical.is_some_and(|(_, y)| y < beastie_view::COMPOSE_BAR_TOP))
        .and_then(|p| {
            crate::renderer::pointer_world(camera, transform, p).or_else(|| {
                on_world_object
                    .then(|| crate::renderer::pointer_world_clamped(camera, transform, p))
                    .flatten()
            })
        });
    let result = game.pointer_moved(logical, world, hit.as_ref());
    handle(result, &mut game);
    if buttons.just_pressed(MouseButton::Left) {
        let result = game.pointer_pressed(
            hit.as_ref().filter(|hit| hit.enabled).map(|hit| hit.action),
            world,
        );
        handle(result, &mut game);
    }
    if buttons.just_released(MouseButton::Left) {
        let result = game.pointer_released();
        handle(result, &mut game);
    }
}

fn controller(
    mut events: MessageReader<GamepadButtonStateChangedEvent>,
    policy: Res<HostInputPolicy>,
    mut game: NonSendMut<Game>,
) {
    if policy.suppress_messages(&mut events) {
        return;
    }
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
    mut close: MessageReader<WindowCloseRequested>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut game: NonSendMut<Game>,
) {
    let primary = window.single().ok();
    if close.read().any(|event| Some(event.window) == primary) {
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
mod ordered_keyboard_tests {
    use super::*;

    fn key(code: KeyCode, logical: Key, state: ButtonState) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: logical,
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn fast_complete_chord_uses_modifiers_at_each_event_not_the_end_of_the_frame() {
        let mut modifiers = OrderedModifiers::default();
        let events = [
            key(KeyCode::SuperLeft, Key::Super, ButtonState::Pressed),
            key(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Pressed,
            ),
            key(
                KeyCode::KeyA,
                Key::Character("a".into()),
                ButtonState::Released,
            ),
            key(KeyCode::SuperLeft, Key::Super, ButtonState::Released),
        ];
        let strokes = events
            .iter()
            .map(|event| modifiers.stroke(event))
            .collect::<Vec<_>>();
        assert!(strokes[1].super_key);
        assert!(strokes[2].super_key);
        assert!(!strokes[3].super_key);
        assert!(!modifiers.state.super_key());
    }

    #[test]
    fn held_modifiers_survive_frame_boundaries_and_preserve_both_sides() {
        let mut modifiers = OrderedModifiers::default();
        modifiers.stroke(&key(
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Pressed,
        ));
        modifiers.stroke(&key(
            KeyCode::ControlRight,
            Key::Control,
            ButtonState::Pressed,
        ));
        // No per-frame ButtonInput state is consulted or reset.
        assert!(
            modifiers
                .stroke(&key(
                    KeyCode::KeyV,
                    Key::Character("v".into()),
                    ButtonState::Pressed
                ))
                .control
        );
        modifiers.stroke(&key(
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Released,
        ));
        assert!(
            modifiers
                .stroke(&key(
                    KeyCode::KeyA,
                    Key::Character("a".into()),
                    ButtonState::Pressed
                ))
                .control
        );
        modifiers.stroke(&key(
            KeyCode::ControlRight,
            Key::Control,
            ButtonState::Released,
        ));
        assert!(!modifiers.state.control_key());
    }

    #[test]
    fn synthetic_modifiers_changed_without_modifier_keys_is_authoritative() {
        let mut modifiers = OrderedModifiers::default();
        modifiers.observe(&NativeWindowEvent::ModifiersChanged(
            (ModifiersState::SUPER | ModifiersState::SHIFT).into(),
        ));
        let stroke = modifiers.stroke(&key(
            KeyCode::KeyA,
            Key::Character("a".into()),
            ButtonState::Pressed,
        ));
        assert!(stroke.super_key && stroke.shift);
        modifiers.observe(&NativeWindowEvent::ModifiersChanged(
            ModifiersState::empty().into(),
        ));
        let stroke = modifiers.stroke(&key(
            KeyCode::KeyA,
            Key::Character("a".into()),
            ButtonState::Released,
        ));
        assert!(!stroke.super_key && !stroke.shift);
    }

    #[test]
    fn focus_loss_clears_physical_and_synthetic_modifier_state() {
        let mut modifiers = OrderedModifiers::default();
        modifiers.stroke(&key(
            KeyCode::ControlLeft,
            Key::Control,
            ButtonState::Pressed,
        ));
        modifiers.observe(&NativeWindowEvent::ModifiersChanged(
            (ModifiersState::SUPER | ModifiersState::CONTROL).into(),
        ));
        modifiers.observe(&NativeWindowEvent::Focused(false));
        let stroke = modifiers.stroke(&key(
            KeyCode::KeyA,
            Key::Character("a".into()),
            ButtonState::Pressed,
        ));
        assert!(!stroke.super_key && !stroke.control && !stroke.shift);
        assert_eq!(modifiers.control_keys, 0);
    }
}

#[cfg(test)]
mod input_policy_tests {
    use super::HostInputPolicy;
    use bevy::ecs::system::SystemState;
    use bevy::prelude::*;

    #[derive(Message)]
    struct NativeInput;

    #[test]
    fn script_only_drains_native_messages_without_replaying_them_later() {
        let mut world = World::new();
        world.init_resource::<Messages<NativeInput>>();
        let mut reader = SystemState::<MessageReader<NativeInput>>::new(&mut world);
        world
            .resource_mut::<Messages<NativeInput>>()
            .write(NativeInput);
        let mut events = reader.get_mut(&mut world).unwrap();
        assert!(HostInputPolicy::ScriptOnly.suppress_messages(&mut events));
        assert_eq!(events.read().count(), 0);
        assert!(!HostInputPolicy::Native.suppress_messages(&mut events));
        assert_eq!(events.read().count(), 0);

        world
            .resource_mut::<Messages<NativeInput>>()
            .write(NativeInput);
        let mut events = reader.get_mut(&mut world).unwrap();
        assert!(!HostInputPolicy::default().suppress_messages(&mut events));
        assert_eq!(events.read().count(), 1);
    }

    #[test]
    fn native_input_is_available_unless_script_isolation_is_explicit() {
        assert!(HostInputPolicy::default().allows_native());
        assert!(HostInputPolicy::from_script_only(false).allows_native());
        assert!(!HostInputPolicy::from_script_only(true).allows_native());
    }
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
