use crate::engine::cdll_int::CLIENT_TIME;
use std::sync::{Mutex, MutexGuard};

pub const MAX_NODE_LIFETIME: f32 = 2.5;
pub const UPDATE_WINDOW: f32 = 0.3;
pub const MAX_SOUND_NODES: usize = 32;
pub const MAX_SPATIAL_MATCH_DIST_SQ: f32 = 400.0 * 400.0;
pub const MAX_VELOCITY_MAGNITUDE: f32 = 3500.0;

fn vec3_dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn vec3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundDebugNode {
    pub sound_name: String,
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub volume: f32,
    pub spawn_time: f32,
    pub occluded: bool,
}

impl SoundDebugNode {
    pub fn new(
        sound_name: String,
        origin: [f32; 3],
        volume: f32,
        spawn_time: f32,
        occluded: bool,
    ) -> Self {
        Self {
            sound_name,
            origin,
            velocity: [0.0, 0.0, 0.0],
            volume,
            spawn_time,
            occluded,
        }
    }

    pub fn age(&self, now: f32) -> f32 {
        (now - self.spawn_time).max(0.0)
    }

    pub fn is_expired(&self, now: f32) -> bool {
        let age = now - self.spawn_time;
        age < 0.0 || age >= MAX_NODE_LIFETIME
    }

    pub fn speed(&self) -> f32 {
        vec3_len(self.velocity)
    }

    pub fn update(&mut self, origin: [f32; 3], volume: f32, now: f32, occluded: bool, dt: f32) {
        let safe_dt = dt.max(0.001);
        let mut inst_vel = [
            (origin[0] - self.origin[0]) / safe_dt,
            (origin[1] - self.origin[1]) / safe_dt,
            (origin[2] - self.origin[2]) / safe_dt,
        ];

        let speed = vec3_len(inst_vel);
        if speed > MAX_VELOCITY_MAGNITUDE {
            let scale = MAX_VELOCITY_MAGNITUDE / speed;
            inst_vel[0] *= scale;
            inst_vel[1] *= scale;
            inst_vel[2] *= scale;
        }

        if self.velocity == [0.0, 0.0, 0.0] {
            self.velocity = inst_vel;
        } else {
            const ALPHA: f32 = 0.7;
            self.velocity = [
                self.velocity[0] * (1.0 - ALPHA) + inst_vel[0] * ALPHA,
                self.velocity[1] * (1.0 - ALPHA) + inst_vel[1] * ALPHA,
                self.velocity[2] * (1.0 - ALPHA) + inst_vel[2] * ALPHA,
            ];
        }

        self.origin = origin;
        self.volume = volume;
        self.spawn_time = now;
        self.occluded = occluded;
    }
}

pub static ACTIVE_SOUND_NODES: Mutex<Vec<SoundDebugNode>> = Mutex::new(Vec::new());

fn lock_nodes() -> MutexGuard<'static, Vec<SoundDebugNode>> {
    match ACTIVE_SOUND_NODES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub fn clear_sound_nodes() {
    lock_nodes().clear();
}

pub fn track_3d_sound(name: &str, origin: [f32; 3], volume: f32, occluded: bool) {
    let now = CLIENT_TIME.lock().map(|t| *t).unwrap_or(0.0);
    track_3d_sound_at_time(name, origin, volume, occluded, now);
}

pub fn track_3d_sound_at_time(name: &str, origin: [f32; 3], volume: f32, occluded: bool, now: f32) {
    let mut nodes = lock_nodes();

    let matching_node = nodes.iter_mut().find(|node| {
        let dt = now - node.spawn_time;
        node.sound_name == name
            && (0.0..UPDATE_WINDOW).contains(&dt)
            && vec3_dist_sq(origin, node.origin) <= MAX_SPATIAL_MATCH_DIST_SQ
    });

    if let Some(node) = matching_node {
        let dt = now - node.spawn_time;
        node.update(origin, volume, now, occluded, dt);
    } else {
        nodes.push(SoundDebugNode::new(
            name.to_string(),
            origin,
            volume,
            now,
            occluded,
        ));
    }

    nodes.retain(|n| !n.is_expired(now));

    if nodes.len() > MAX_SOUND_NODES {
        let overflow = nodes.len() - MAX_SOUND_NODES;
        nodes.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_test<'a>() -> std::sync::MutexGuard<'a, ()> {
        match TEST_LOCK.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn lock_nodes<'a>() -> std::sync::MutexGuard<'a, Vec<SoundDebugNode>> {
        match ACTIVE_SOUND_NODES.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    #[test]
    fn test_track_3d_sound_creation_and_pruning() {
        let _guard = lock_test();
        clear_sound_nodes();

        track_3d_sound_at_time("test_sound.wav", [100.0, 0.0, 0.0], 1.0, false, 1.0);
        {
            let nodes = lock_nodes();
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].sound_name, "test_sound.wav");
            assert_eq!(nodes[0].origin, [100.0, 0.0, 0.0]);
            assert_eq!(nodes[0].velocity, [0.0, 0.0, 0.0]);
            assert_eq!(nodes[0].speed(), 0.0);
        }

        track_3d_sound_at_time("other_sound.wav", [0.0, 0.0, 0.0], 0.5, true, 3.6);
        {
            let nodes = lock_nodes();
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].sound_name, "other_sound.wav");
        }
    }

    #[test]
    fn test_spatial_matching_and_velocity() {
        let _guard = lock_test();
        clear_sound_nodes();

        track_3d_sound_at_time("footstep.wav", [0.0, 0.0, 0.0], 1.0, false, 10.0);

        track_3d_sound_at_time("footstep.wav", [10.0, 0.0, 0.0], 1.0, false, 10.1);

        {
            let nodes = lock_nodes();
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].origin, [10.0, 0.0, 0.0]);
            assert!((nodes[0].velocity[0] - 100.0).abs() < 1e-2);
            assert!((nodes[0].speed() - 100.0).abs() < 1e-2);
        }

        track_3d_sound_at_time("footstep.wav", [1000.0, 0.0, 0.0], 1.0, false, 10.15);
        {
            let nodes = lock_nodes();
            assert_eq!(nodes.len(), 2);
        }
    }

    #[test]
    fn test_time_reset_pruning() {
        let _guard = lock_test();
        clear_sound_nodes();

        track_3d_sound_at_time("old_map.wav", [10.0, 0.0, 0.0], 1.0, false, 100.0);
        track_3d_sound_at_time("new_map.wav", [0.0, 0.0, 0.0], 1.0, false, 0.5);

        {
            let nodes = lock_nodes();
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].sound_name, "new_map.wav");
        }
    }

    #[test]
    fn test_max_nodes_capacity() {
        let _guard = lock_test();
        clear_sound_nodes();

        for i in 0..40 {
            let name = format!("sound_{}.wav", i);
            track_3d_sound_at_time(&name, [i as f32, 0.0, 0.0], 1.0, false, 1.0);
        }

        let nodes = lock_nodes();
        assert_eq!(nodes.len(), MAX_SOUND_NODES);
        assert_eq!(nodes[0].sound_name, "sound_8.wav");
    }
}
