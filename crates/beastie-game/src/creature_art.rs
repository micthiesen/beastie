//! Authored creature art. These values change presentation, never simulation truth.
use beastie_core::{ActivityRecipe, Mood};
use beastie_view::PresentationCueKind;

pub(super) struct CreatureArt {
    pub segments: usize,
    pub head_radii: [i32; 3],
    pub head_cell: f32,
    pub fin_cell: f32,
    pub eye_spacing: f32,
    pub body: [u8; 3],
    pub belly: [u8; 3],
    pub crown: [u8; 3],
    pub freckle: [u8; 3],
    pub fin_base: [u8; 3],
    pub fin_tip: [u8; 3],
    pub eye: [u8; 3],
    pub pupil: [u8; 3],
    pub glint: [u8; 3],
    pub brow: [u8; 3],
    pub mouth: [u8; 3],
    pub cheek: [u8; 3],
    pub lip: [u8; 3],
    pub tongue: [u8; 3],
}

pub(super) const CREATURE: CreatureArt = CreatureArt {
    segments: 12,
    head_radii: [12, 10, 9],
    head_cell: 0.055,
    fin_cell: 0.045,
    eye_spacing: 0.27,
    body: [245, 174, 58],
    belly: [250, 206, 112],
    crown: [255, 194, 70],
    freckle: [235, 151, 46],
    fin_base: [210, 127, 49],
    fin_tip: [246, 190, 87],
    eye: [255, 242, 201],
    pupil: [30, 40, 44],
    glint: [255, 253, 230],
    brow: [121, 73, 41],
    mouth: [85, 49, 40],
    cheek: [234, 128, 65],
    lip: [224, 151, 72],
    tongue: [212, 113, 100],
};

pub(super) struct MotionArt {
    pub gesture_arrival_ms: f32,
    pub blink_period: f32,
    pub blink_duration: f32,
    pub reduced_gesture: f32,
    pub idle_gait: f32,
    pub swim_gait: f32,
}

pub(super) const MOTION: MotionArt = MotionArt {
    gesture_arrival_ms: 180.0,
    blink_period: 5.7,
    blink_duration: 0.15,
    reduced_gesture: 0.3,
    idle_gait: 0.009,
    swim_gait: 0.075,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Acting {
    pub eye_open: f32,
    pub brow: f32,
    pub smile: f32,
    pub mouth: f32,
    pub mouth_width: f32,
    pub asymmetry: f32,
    pub tilt: f32,
    pub nod: f32,
    pub fin: f32,
}

const CONTENT: Acting = Acting {
    eye_open: 0.9,
    brow: 0.0,
    smile: 0.055,
    mouth: 0.1,
    mouth_width: 1.0,
    asymmetry: 0.0,
    tilt: 0.0,
    nod: 0.0,
    fin: 0.0,
};
const CURIOUS: Acting = Acting {
    eye_open: 1.12,
    brow: 0.18,
    smile: 0.02,
    mouth: 0.25,
    mouth_width: 0.76,
    asymmetry: 0.08,
    tilt: 0.10,
    nod: 0.04,
    fin: 0.12,
};
const SLEEP: Acting = Acting {
    eye_open: 0.055,
    brow: -0.05,
    smile: 0.02,
    mouth: 0.08,
    mouth_width: 0.8,
    asymmetry: 0.0,
    tilt: -0.07,
    nod: -0.13,
    fin: -0.2,
};
const AFFECTION: Acting = Acting {
    eye_open: 0.64,
    brow: 0.08,
    smile: 0.1,
    mouth: 0.14,
    mouth_width: 1.2,
    asymmetry: 0.0,
    tilt: 0.12,
    nod: 0.06,
    fin: 0.18,
};
const SUSPICIOUS: Acting = Acting {
    eye_open: 0.56,
    brow: -0.24,
    smile: -0.035,
    mouth: 0.05,
    mouth_width: 0.85,
    asymmetry: 0.13,
    tilt: 0.15,
    nod: -0.045,
    fin: -0.2,
};
const REFUSAL: Acting = Acting {
    eye_open: 0.48,
    brow: -0.3,
    smile: -0.07,
    mouth: 0.6,
    mouth_width: 0.7,
    asymmetry: 0.0,
    tilt: 0.0,
    nod: -0.15,
    fin: -0.28,
};

pub(super) fn mood(mood: Mood) -> Acting {
    match mood {
        Mood::Content => CONTENT,
        Mood::Curious => CURIOUS,
        Mood::Hungry => Acting {
            eye_open: 0.93,
            brow: 0.13,
            smile: -0.02,
            mouth: 0.38,
            tilt: -0.03,
            fin: 0.05,
            ..CONTENT
        },
        Mood::Sleepy => Acting {
            eye_open: 0.34,
            nod: -0.07,
            fin: -0.1,
            ..SLEEP
        },
        Mood::Lonely => Acting {
            eye_open: 0.68,
            brow: 0.3,
            smile: -0.055,
            mouth: 0.08,
            tilt: 0.06,
            nod: -0.05,
            fin: -0.12,
            ..CONTENT
        },
        Mood::Resentful => Acting {
            eye_open: 0.52,
            brow: -0.28,
            smile: -0.025,
            mouth: 0.05,
            tilt: -0.07,
            nod: -0.01,
            fin: -0.1,
            ..CONTENT
        },
    }
}

/// Whole poses prevent an unrelated underlying mood from leaking into an owned reaction.
pub(super) fn expression(cue: PresentationCueKind) -> Option<Acting> {
    Some(match cue {
        PresentationCueKind::Recoil | PresentationCueKind::Spit => REFUSAL,
        PresentationCueKind::Suspicion | PresentationCueKind::FoodSuspicion => SUSPICIOUS,
        PresentationCueKind::Affection => AFFECTION,
        PresentationCueKind::Comfort => Acting {
            eye_open: 0.72,
            smile: 0.075,
            tilt: -0.07,
            fin: 0.06,
            ..AFFECTION
        },
        PresentationCueKind::Delight => Acting {
            eye_open: 0.82,
            smile: 0.1,
            mouth: 0.5,
            mouth_width: 1.12,
            fin: 0.3,
            nod: 0.12,
            ..CONTENT
        },
        PresentationCueKind::Notice
        | PresentationCueKind::PositiveNotice
        | PresentationCueKind::PlaceNotice
        | PresentationCueKind::Wake => Acting {
            eye_open: 1.12,
            brow: 0.2,
            nod: 0.09,
            ..CURIOUS
        },
        PresentationCueKind::Curious => Acting {
            eye_open: 1.08,
            brow: 0.28,
            tilt: 0.22,
            nod: 0.04,
            ..CURIOUS
        },
        PresentationCueKind::WordLearned => Acting {
            eye_open: 0.9,
            smile: 0.12,
            mouth: 0.62,
            mouth_width: 1.18,
            fin: 0.45,
            nod: 0.16,
            ..CONTENT
        },
        PresentationCueKind::Sleep => SLEEP,
        PresentationCueKind::Crumbs => Acting {
            mouth: 0.3,
            ..CONTENT
        },
        _ => return None,
    })
}

pub(super) struct MouthShape {
    pub open: f32,
    pub width: f32,
}
// Closed consonant, open vowel, rounded vowel. No invented phoneme or transcript inference.
pub(super) const SPEECH: [MouthShape; 3] = [
    MouthShape {
        open: 0.12,
        width: 1.15,
    },
    MouthShape {
        open: 0.72,
        width: 0.95,
    },
    MouthShape {
        open: 0.42,
        width: 0.62,
    },
];

/// Autonomous acting uses the selected recipe only while its exact owner is active.
pub(super) fn activity(recipe: ActivityRecipe, elapsed_ms: u64) -> Acting {
    let beat = (elapsed_ms as f32 * 0.003).sin();
    match recipe {
        ActivityRecipe::BallNudge => Acting {
            nod: beat.abs() * 0.15,
            fin: 0.2,
            ..CURIOUS
        },
        ActivityRecipe::BellStrike => Acting {
            tilt: beat * 0.15,
            eye_open: 1.1,
            asymmetry: 0.1,
            ..CURIOUS
        },
        ActivityRecipe::SockTug => Acting {
            nod: -beat.abs() * 0.16,
            mouth: 0.25,
            mouth_width: 1.15,
            fin: -0.12,
            ..CONTENT
        },
        ActivityRecipe::CaveShelter => Acting {
            eye_open: 0.6,
            nod: -0.07,
            fin: -0.2,
            ..CONTENT
        },
        ActivityRecipe::PlantOrbit => Acting {
            tilt: beat * 0.14,
            brow: 0.2,
            eye_open: 1.05,
            ..CURIOUS
        },
        ActivityRecipe::BottomForage => Acting {
            nod: -0.18,
            mouth: 0.1 + beat.abs() * 0.2,
            ..CURIOUS
        },
        ActivityRecipe::OpenWaterDrift => Acting {
            tilt: beat * 0.04,
            eye_open: 0.8,
            ..CONTENT
        },
    }
}
