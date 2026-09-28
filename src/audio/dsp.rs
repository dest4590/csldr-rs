use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolatedPropagation {
    pub occlusion: f32,
    pub volume_factor: f32,
    pub reverb_send: f32,
    pub is_source_in_cave: bool,
    pub is_diffracted: bool,
    pub diffracted_origin: [f32; 3],
    pub diffraction_angle_deg: f32,
}

impl Default for InterpolatedPropagation {
    fn default() -> Self {
        Self {
            occlusion: 0.0,
            volume_factor: 1.0,
            reverb_send: 1.0,
            is_source_in_cave: false,
            is_diffracted: false,
            diffracted_origin: [0.0; 3],
            diffraction_angle_deg: 0.0,
        }
    }
}

struct SourceTracker {
    current: InterpolatedPropagation,
    target: InterpolatedPropagation,
    last_eval_time: f32,
}

static PROPAGATION_TRACKERS: Mutex<Option<HashMap<u64, SourceTracker>>> = Mutex::new(None);

#[inline]
fn hash_positions(src: [f32; 3], listener: [f32; 3]) -> u64 {
    let sx = (src[0] / 32.0).round() as i32;
    let sy = (src[1] / 32.0).round() as i32;
    let sz = (src[2] / 32.0).round() as i32;
    let lx = (listener[0] / 32.0).round() as i32;
    let ly = (listener[1] / 32.0).round() as i32;
    let lz = (listener[2] / 32.0).round() as i32;

    let mut h = 0xcbf29ce484222325u64;
    for &val in &[sx, sy, sz, lx, ly, lz] {
        h ^= (val as u64) & 0xffffffff;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[inline]
pub fn interpolate_propagation(
    src: [f32; 3],
    listener: [f32; 3],
    now: f32,
    mut eval_target: impl FnMut() -> InterpolatedPropagation,
) -> InterpolatedPropagation {
    let hash = hash_positions(src, listener);
    let mut guard = match PROPAGATION_TRACKERS.lock() {
        Ok(g) => g,
        Err(_) => return eval_target(),
    };
    let map = guard.get_or_insert_with(HashMap::new);

    const EVAL_INTERVAL: f32 = 0.033;
    const LERP_FACTOR: f32 = 0.15;

    let tracker = map.entry(hash).or_insert_with(|| {
        let initial = eval_target();
        SourceTracker {
            current: initial,
            target: initial,
            last_eval_time: now,
        }
    });

    if (now - tracker.last_eval_time).abs() >= EVAL_INTERVAL {
        tracker.target = eval_target();
        tracker.last_eval_time = now;
    }

    tracker.current.occlusion +=
        (tracker.target.occlusion - tracker.current.occlusion) * LERP_FACTOR;
    tracker.current.volume_factor +=
        (tracker.target.volume_factor - tracker.current.volume_factor) * LERP_FACTOR;
    tracker.current.reverb_send +=
        (tracker.target.reverb_send - tracker.current.reverb_send) * LERP_FACTOR;
    tracker.current.diffraction_angle_deg += (tracker.target.diffraction_angle_deg
        - tracker.current.diffraction_angle_deg)
        * LERP_FACTOR;

    tracker.current.is_source_in_cave = tracker.target.is_source_in_cave;
    tracker.current.is_diffracted = tracker.target.is_diffracted;
    tracker.current.diffracted_origin = tracker.target.diffracted_origin;

    tracker.current
}

pub fn prune_stale_trackers(now: f32) {
    if let Ok(mut guard) = PROPAGATION_TRACKERS.lock() {
        if let Some(map) = guard.as_mut() {
            map.retain(|_, t| (now - t.last_eval_time).abs() < 5.0);
        }
    }
}
