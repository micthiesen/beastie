//! Bounded asynchronous evidence capture. Every image carries its submitted semantic snapshot.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::app::Game;
use crate::feel::PresentationTraceState;
use crate::renderer::{PRESENTATION_HEIGHT, PRESENTATION_WIDTH};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

const MAX_OUTSTANDING: usize = 8;
const WARMUP_FRAMES: u8 = 8;
const READBACK_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) struct FrameSnapshot {
    pub world: beastie_core::WorldState,
    pub view: beastie_view::ViewState,
    pub presentation: PresentationTraceState,
    pub path: Option<PathBuf>,
    pub feel_frame: Option<u64>,
}

struct Pending<T, R> {
    snapshot: T,
    result: Option<R>,
    submitted: Instant,
}

/// Completion order is independent of submission order. A gap always blocks the writer.
struct OrderedFrames<T, R> {
    next_submit: u64,
    next_write: u64,
    pending: BTreeMap<u64, Pending<T, R>>,
}

impl<T, R> Default for OrderedFrames<T, R> {
    fn default() -> Self {
        Self {
            next_submit: 0,
            next_write: 0,
            pending: BTreeMap::new(),
        }
    }
}

impl<T, R> OrderedFrames<T, R> {
    fn submit(&mut self, snapshot: T) -> u64 {
        let sequence = self.next_submit;
        self.next_submit += 1;
        self.pending.insert(
            sequence,
            Pending {
                snapshot,
                result: None,
                submitted: Instant::now(),
            },
        );
        sequence
    }

    fn complete(&mut self, sequence: u64, result: R) -> Result<(), &'static str> {
        let pending = self
            .pending
            .get_mut(&sequence)
            .ok_or("unknown capture completion")?;
        if pending.result.is_some() {
            return Err("duplicate capture completion");
        }
        pending.result = Some(result);
        Ok(())
    }

    fn pop_ready(&mut self) -> Option<(T, R)> {
        self.pending.get(&self.next_write)?.result.as_ref()?;
        let pending = self.pending.remove(&self.next_write)?;
        self.next_write += 1;
        Some((pending.snapshot, pending.result?))
    }

    fn drained(&self) -> bool {
        self.pending.is_empty()
    }
}

#[derive(Resource, Default)]
struct CaptureQueue(OrderedFrames<FrameSnapshot, Result<image::RgbaImage, String>>);

#[derive(Resource, Default)]
struct CaptureWarmup(u8);

#[derive(Component)]
struct CaptureSequence(u64);

pub(crate) struct CapturePlugin;
impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptureQueue>()
            .init_resource::<CaptureWarmup>()
            .add_systems(PostUpdate, submit_frame);
    }
}

fn fail(game: &mut Game, queue: &mut CaptureQueue, error: impl std::fmt::Display) {
    error!("Evidence capture failed: {error}");
    queue.0.pending.clear();
    game.fail_captures();
}

fn submit_frame(
    mut commands: Commands,
    mut queue: ResMut<CaptureQueue>,
    mut game: NonSendMut<Game>,
    mut warmup: ResMut<CaptureWarmup>,
    pacing: Res<crate::host::FramePacing>,
) {
    // Present the initial scene through several extraction/render cycles before advancing
    // scripted input or naming the first screenshot. This allows fonts and GPU pipelines to load.
    if warmup.0 < WARMUP_FRAMES {
        warmup.0 += 1;
        game.frame_pending = warmup.0 < WARMUP_FRAMES;
        return;
    }
    if queue
        .0
        .pending
        .values()
        .any(|pending| pending.submitted.elapsed() > READBACK_TIMEOUT)
    {
        fail(
            &mut game,
            &mut queue,
            "GPU readback exceeded 15 seconds; evidence is incomplete",
        );
        return;
    }
    game.frame_pending = queue.0.pending.len() >= MAX_OUTSTANDING;
    if game.quit_requested {
        if queue.0.drained()
            && let Err(error) = game.finalize_capture()
        {
            fail(&mut game, &mut queue, error);
        }
        return;
    }
    if game.frame_pending || !pacing.ready {
        return;
    }
    if game.needs_capture() {
        let sequence = queue.0.submit(game.begin_capture());
        commands
            .spawn((Screenshot::primary_window(), CaptureSequence(sequence)))
            .observe(capture_ready);
        game.frame_pending = queue.0.pending.len() >= MAX_OUTSTANDING;
    }
    if let Err(error) = game.finish_frame() {
        fail(&mut game, &mut queue, error);
    }
}

fn capture_ready(
    capture: On<ScreenshotCaptured>,
    sequences: Query<&CaptureSequence>,
    mut queue: ResMut<CaptureQueue>,
    mut game: NonSendMut<Game>,
) {
    if game.failed && queue.0.drained() {
        return;
    }
    let Ok(sequence) = sequences.get(capture.entity) else {
        fail(
            &mut game,
            &mut queue,
            "screenshot lost its sequence identity",
        );
        return;
    };
    let image = capture
        .image
        .clone()
        .try_into_dynamic()
        .map(|image| image.to_rgba8())
        .map_err(|error| error.to_string());
    if let Err(error) = queue.0.complete(sequence.0, image) {
        fail(&mut game, &mut queue, error);
        return;
    }
    while let Some((snapshot, result)) = queue.0.pop_ready() {
        let result = result.and_then(|image| write_frame(&mut game, &snapshot, image));
        if let Err(error) = result {
            fail(&mut game, &mut queue, error);
            return;
        }
    }
    game.frame_pending = queue.0.pending.len() >= MAX_OUTSTANDING;
    if game.quit_requested
        && queue.0.drained()
        && let Err(error) = game.finalize_capture()
    {
        fail(&mut game, &mut queue, error);
    }
}

fn write_frame(
    game: &mut Game,
    snapshot: &FrameSnapshot,
    image: image::RgbaImage,
) -> Result<(), String> {
    if let Some(path) = &snapshot.path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        image.save(path).map_err(|error| error.to_string())?;
    }
    let rgba = if image.width() != PRESENTATION_WIDTH as u32
        || image.height() != PRESENTATION_HEIGHT as u32
    {
        image::imageops::resize(
            &image,
            PRESENTATION_WIDTH as u32,
            PRESENTATION_HEIGHT as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    game.record_rendered_frame(snapshot, &rgba)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::OrderedFrames;

    #[test]
    fn out_of_order_readbacks_preserve_image_snapshot_association() {
        let mut queue = OrderedFrames::default();
        let first = queue.submit(("notice", "notice.png", 40));
        let second = queue.submit(("eating", "eating.png", 80));
        queue.complete(second, "second-image").unwrap();
        assert!(queue.pop_ready().is_none());
        assert!(!queue.drained());
        queue.complete(first, "first-image").unwrap();
        assert_eq!(
            queue.pop_ready(),
            Some((("notice", "notice.png", 40), "first-image"))
        );
        assert_eq!(
            queue.pop_ready(),
            Some((("eating", "eating.png", 80), "second-image"))
        );
        assert!(queue.drained());
    }

    #[test]
    fn shutdown_must_wait_for_every_submitted_frame() {
        let mut queue = OrderedFrames::default();
        let first = queue.submit(0);
        let last = queue.submit(1);
        queue.complete(first, ()).unwrap();
        assert_eq!(queue.pop_ready(), Some((0, ())));
        assert!(!queue.drained());
        queue.complete(last, ()).unwrap();
        assert!(!queue.drained());
        assert_eq!(queue.pop_ready(), Some((1, ())));
        assert!(queue.drained());
        assert!(queue.complete(last, ()).is_err());
    }
}
