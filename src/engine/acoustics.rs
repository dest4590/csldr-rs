use crate::utils::cvars::{HRTF_ACOUSTICS, HRTF_OCCLUSION, HRTF_REVERB_SCALE};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PMPlane {
    pub normal: [f32; 3],
    pub dist: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PMTrace {
    pub allsolid: c_int,
    pub startsolid: c_int,
    pub inopen: c_int,
    pub inwater: c_int,
    pub fraction: f32,
    pub endpos: [f32; 3],
    pub plane: PMPlane,
    pub ent: c_int,
    pub deltavelocity: [f32; 3],
    pub hitgroup: c_int,
}

pub type FnPMTraceLine = unsafe extern "C" fn(
    start: *const f32,
    end: *const f32,
    flags: c_int,
    usehull: c_int,
    ignore_ent: c_int,
) -> *mut PMTrace;

pub type FnPMTraceTexture =
    unsafe extern "C" fn(ground: c_int, vstart: *const f32, vend: *const f32) -> *const c_char;

pub static G_PM_TRACE_TEXTURE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceMaterial {
    Concrete,
    Metal,
    Wood,
    DirtSand,
    Glass,
}

impl SurfaceMaterial {
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Metal => "MET",
            Self::Concrete => "CON",
            Self::Wood => "WOD",
            Self::DirtSand => "DRT",
            Self::Glass => "GLS",
        }
    }

    pub fn base_damping(&self) -> f32 {
        match self {
            Self::Metal => 0.06,
            Self::Glass => 0.09,
            Self::Concrete => 0.22,
            Self::Wood => 0.42,
            Self::DirtSand => 0.62,
        }
    }
}

pub fn classify_texture(name: &str) -> SurfaceMaterial {
    let lower = name.to_lowercase();
    if lower.contains("metal")
        || lower.contains("iron")
        || lower.contains("pipe")
        || lower.contains("duct")
        || lower.contains("vent")
        || lower.contains("steel")
        || lower.contains("grate")
        || lower.starts_with("m_")
    {
        SurfaceMaterial::Metal
    } else if lower.contains("wood")
        || lower.contains("crate")
        || lower.contains("plank")
        || lower.contains("door")
        || lower.contains("board")
        || lower.starts_with("w_")
    {
        SurfaceMaterial::Wood
    } else if lower.contains("dirt")
        || lower.contains("sand")
        || lower.contains("grass")
        || lower.contains("mud")
        || lower.contains("gravel")
        || lower.starts_with("d_")
    {
        SurfaceMaterial::DirtSand
    } else if lower.contains("glass") || lower.contains("window") || lower.starts_with('{') {
        SurfaceMaterial::Glass
    } else {
        SurfaceMaterial::Concrete
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DebugRay {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub hit_pos: [f32; 3],
    pub hit: bool,
    pub label: &'static str,
    pub spawn_time: f32,
    pub material: SurfaceMaterial,
}

pub static LAST_ACOUSTIC_RAYS: Mutex<Vec<DebugRay>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomType {
    Outdoor,
    Indoor,
    Tunnel,
}

#[derive(Clone, Copy, Debug)]
pub struct AcousticEnvironment {
    pub room_type: RoomType,
    pub t60_reverb_time: f32,
    pub damping: f32,
    pub reflection_mix: f32,
    pub enclosure_factor: f32,
    pub estimated_volume: f32,
    pub primary_material: SurfaceMaterial,
    pub metal_ratio: f32,
    pub wood_ratio: f32,
}

impl Default for AcousticEnvironment {
    fn default() -> Self {
        Self {
            room_type: RoomType::Outdoor,
            t60_reverb_time: 0.05,
            damping: 0.1,
            reflection_mix: 0.0,
            enclosure_factor: 0.0,
            estimated_volume: 5_000_000.0,
            primary_material: SurfaceMaterial::Concrete,
            metal_ratio: 0.0,
            wood_ratio: 0.0,
        }
    }
}

pub static CURRENT_ENVIRONMENT: Mutex<AcousticEnvironment> = Mutex::new(AcousticEnvironment {
    room_type: RoomType::Outdoor,
    t60_reverb_time: 0.05,
    damping: 0.1,
    reflection_mix: 0.0,
    enclosure_factor: 0.0,
    estimated_volume: 5_000_000.0,
    primary_material: SurfaceMaterial::Concrete,
    metal_ratio: 0.0,
    wood_ratio: 0.0,
});

static ROOM_DEBOUNCE: Mutex<(RoomType, u32, f32, u32)> = Mutex::new((RoomType::Outdoor, 0, 0.0, 0));
const DEBOUNCE_THRESHOLD: u32 = 4;
const FLICKER_TOLERANCE: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub struct SoundPropagation {
    pub occlusion: f32,
    pub volume_factor: f32,
    pub is_source_in_cave: bool,
    pub reverb_send: f32,
    pub is_diffracted: bool,
    pub diffracted_origin: [f32; 3],
    pub diffraction_angle_deg: f32,
}

pub unsafe fn trace_world_ray(start: [f32; 3], end: [f32; 3]) -> Option<PMTrace> {
    let trace_fn_ptr = crate::engine::cdll_int::G_ENGFUNCS.PM_TraceLine;
    if !trace_fn_ptr.is_null() {
        let trace_fn: FnPMTraceLine = std::mem::transmute(trace_fn_ptr);
        let ptr = trace_fn(start.as_ptr(), end.as_ptr(), 0, 0, 1);
        if !ptr.is_null() {
            let tr = *ptr;
            if tr.allsolid == 0 {
                return Some(tr);
            }
        }
    }
    None
}

pub unsafe fn query_surface_material(start: [f32; 3], end: [f32; 3]) -> SurfaceMaterial {
    let fn_ptr = G_PM_TRACE_TEXTURE.load(Ordering::Relaxed);
    if !fn_ptr.is_null() {
        let trace_tex: FnPMTraceTexture = std::mem::transmute(fn_ptr);
        let name_ptr = trace_tex(0, start.as_ptr(), end.as_ptr());
        if !name_ptr.is_null() {
            if let Ok(c_str) = CStr::from_ptr(name_ptr).to_str() {
                return classify_texture(c_str);
            }
        }
    }
    SurfaceMaterial::Concrete
}

static LAST_UPDATE_TIME: Mutex<f32> = Mutex::new(0.0);

pub unsafe fn update_room_acoustics(player_pos: [f32; 3]) {
    if HRTF_ACOUSTICS.value_or(1.0) == 0.0 {
        return;
    }

    let now = crate::engine::cdll_int::CLIENT_TIME
        .lock()
        .map(|t| *t)
        .unwrap_or(0.0);

    {
        let Ok(mut last_time) = LAST_UPDATE_TIME.lock() else {
            return;
        };
        if (now - *last_time).abs() < 0.10 {
            return;
        }
        *last_time = now;
    }

    const MAX_RAY_DIST: f32 = 1024.0;
    const MAX_CEILING_DIST: f32 = 2048.0;
    const MAX_FLOOR_DIST: f32 = 512.0;
    const MAX_DIAG_DIST: f32 = 1400.0;
    const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;

    let ray_start = [player_pos[0], player_pos[1], player_pos[2] + 32.0];

    let directions: [([f32; 3], &'static str); 18] = [
        ([0.0, MAX_RAY_DIST, 0.0], "+Y North"),
        (
            [INV_SQRT2 * MAX_RAY_DIST, INV_SQRT2 * MAX_RAY_DIST, 0.0],
            "+X+Y NorthEast",
        ),
        ([MAX_RAY_DIST, 0.0, 0.0], "+X East"),
        (
            [INV_SQRT2 * MAX_RAY_DIST, -INV_SQRT2 * MAX_RAY_DIST, 0.0],
            "+X-Y SouthEast",
        ),
        ([0.0, -MAX_RAY_DIST, 0.0], "-Y South"),
        (
            [-INV_SQRT2 * MAX_RAY_DIST, -INV_SQRT2 * MAX_RAY_DIST, 0.0],
            "-X-Y SouthWest",
        ),
        ([-MAX_RAY_DIST, 0.0, 0.0], "-X West"),
        (
            [-INV_SQRT2 * MAX_RAY_DIST, INV_SQRT2 * MAX_RAY_DIST, 0.0],
            "-X+Y NorthWest",
        ),
        ([0.0, 0.0, MAX_CEILING_DIST], "+Z Ceiling"),
        ([0.0, 0.0, -MAX_FLOOR_DIST], "-Z Floor"),
        (
            [0.0, INV_SQRT2 * MAX_DIAG_DIST, INV_SQRT2 * MAX_DIAG_DIST],
            "+Y+Z UpNorth",
        ),
        (
            [INV_SQRT2 * MAX_DIAG_DIST, 0.0, INV_SQRT2 * MAX_DIAG_DIST],
            "+X+Z UpEast",
        ),
        (
            [0.0, -INV_SQRT2 * MAX_DIAG_DIST, INV_SQRT2 * MAX_DIAG_DIST],
            "-Y+Z UpSouth",
        ),
        (
            [-INV_SQRT2 * MAX_DIAG_DIST, 0.0, INV_SQRT2 * MAX_DIAG_DIST],
            "-X+Z UpWest",
        ),
        (
            [0.0, INV_SQRT2 * MAX_DIAG_DIST, -INV_SQRT2 * MAX_DIAG_DIST],
            "+Y-Z DownNorth",
        ),
        (
            [INV_SQRT2 * MAX_DIAG_DIST, 0.0, -INV_SQRT2 * MAX_DIAG_DIST],
            "+X-Z DownEast",
        ),
        (
            [0.0, -INV_SQRT2 * MAX_DIAG_DIST, -INV_SQRT2 * MAX_DIAG_DIST],
            "-Y-Z DownSouth",
        ),
        (
            [-INV_SQRT2 * MAX_DIAG_DIST, 0.0, -INV_SQRT2 * MAX_DIAG_DIST],
            "-X-Z DownWest",
        ),
    ];

    let mut ray_distances = [MAX_RAY_DIST; 18];
    let mut close_walls_count = 0;
    let mut upper_sky_rays = 0;
    let mut debug_rays = Vec::with_capacity(18);

    let mut metal_hits = 0;
    let mut wood_hits = 0;
    let mut total_hits = 0;
    let mut accumulated_damping = 0.0;

    for (i, (dir, label)) in directions.iter().enumerate() {
        let end_pos = [
            ray_start[0] + dir[0],
            ray_start[1] + dir[1],
            ray_start[2] + dir[2],
        ];

        if let Some(tr) = trace_world_ray(ray_start, end_pos) {
            let max_len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
            let dist = tr.fraction * max_len;
            ray_distances[i] = dist;

            let hit = tr.fraction < 0.98;
            let material = if hit {
                query_surface_material(ray_start, tr.endpos)
            } else {
                SurfaceMaterial::Concrete
            };

            if hit {
                total_hits += 1;
                accumulated_damping += material.base_damping();
                match material {
                    SurfaceMaterial::Metal => metal_hits += 1,
                    SurfaceMaterial::Wood => wood_hits += 1,
                    _ => {}
                }
            }

            debug_rays.push(DebugRay {
                start: ray_start,
                end: end_pos,
                hit_pos: tr.endpos,
                hit,
                label,
                spawn_time: now,
                material,
            });

            if i < 8 && dist < 320.0 && hit {
                close_walls_count += 1;
            }

            if (i == 8 || (10..=13).contains(&i)) && (!hit || dist > 550.0) {
                upper_sky_rays += 1;
            }
        }
    }

    if let Ok(mut rays_guard) = LAST_ACOUSTIC_RAYS.lock() {
        *rays_guard = debug_rays;
    }

    let ceiling_dist = ray_distances[8];
    let floor_dist = ray_distances[9];
    let ns_dist = ray_distances[0] + ray_distances[4];
    let ew_dist = ray_distances[2] + ray_distances[6];
    let diag1_dist = ray_distances[1] + ray_distances[5];
    let diag2_dist = ray_distances[3] + ray_distances[7];

    let height = ceiling_dist + floor_dist;
    let target_volume = ns_dist * ew_dist * height;
    let min_span = ns_dist.min(ew_dist).min(diag1_dist).min(diag2_dist);
    let max_span = ns_dist.max(ew_dist).max(diag1_dist).max(diag2_dist);

    const CORRIDOR_WIDTH: f32 = 260.0;
    let ns_is_corridor = ray_distances[0] < CORRIDOR_WIDTH && ray_distances[4] < CORRIDOR_WIDTH;
    let ew_is_corridor = ray_distances[2] < CORRIDOR_WIDTH && ray_distances[6] < CORRIDOR_WIDTH;
    let diag1_is_corridor = ray_distances[1] < CORRIDOR_WIDTH && ray_distances[5] < CORRIDOR_WIDTH;
    let diag2_is_corridor = ray_distances[3] < CORRIDOR_WIDTH && ray_distances[7] < CORRIDOR_WIDTH;
    let has_opposing_walls =
        ns_is_corridor || ew_is_corridor || diag1_is_corridor || diag2_is_corridor;

    let is_open_sky = upper_sky_rays >= 2 || ceiling_dist > 500.0;

    let mut longitudinal_continuity: f32 = 0.0;
    if has_opposing_walls && !is_open_sky {
        let (long_dir, forward_clearance, backward_clearance) = if ew_is_corridor {
            ([0.0, 1.0, 0.0], ray_distances[0], ray_distances[4])
        } else if ns_is_corridor {
            ([1.0, 0.0, 0.0], ray_distances[2], ray_distances[6])
        } else if diag1_is_corridor {
            (
                [-INV_SQRT2, INV_SQRT2, 0.0],
                ray_distances[7],
                ray_distances[3],
            )
        } else {
            (
                [INV_SQRT2, INV_SQRT2, 0.0],
                ray_distances[1],
                ray_distances[5],
            )
        };

        let probe_dist_fwd = 140.0f32.min(forward_clearance * 0.6);
        let probe_dist_back = 140.0f32.min(backward_clearance * 0.6);

        let p1_start = [
            ray_start[0] + long_dir[0] * probe_dist_fwd,
            ray_start[1] + long_dir[1] * probe_dist_fwd,
            ray_start[2],
        ];
        let p2_start = [
            ray_start[0] - long_dir[0] * probe_dist_back,
            ray_start[1] - long_dir[1] * probe_dist_back,
            ray_start[2],
        ];

        let p1_enclosed = if probe_dist_fwd > 40.0 {
            trace_world_ray(
                p1_start,
                [p1_start[0], p1_start[1], p1_start[2] + MAX_CEILING_DIST],
            )
            .map(|tr| {
                tr.startsolid == 0 && tr.fraction < 0.98 && (tr.fraction * MAX_CEILING_DIST) < 300.0
            })
            .unwrap_or(false)
        } else {
            true
        };

        let p2_enclosed = if probe_dist_back > 40.0 {
            trace_world_ray(
                p2_start,
                [p2_start[0], p2_start[1], p2_start[2] + MAX_CEILING_DIST],
            )
            .map(|tr| {
                tr.startsolid == 0 && tr.fraction < 0.98 && (tr.fraction * MAX_CEILING_DIST) < 300.0
            })
            .unwrap_or(false)
        } else {
            true
        };

        longitudinal_continuity = match (p1_enclosed, p2_enclosed) {
            (true, true) => 1.0,
            (true, false) | (false, true) => 0.4,
            (false, false) => 0.0,
        };
    }

    let is_tunnel = !is_open_sky
        && has_opposing_walls
        && min_span < 240.0
        && ceiling_dist < 250.0
        && max_span < 750.0
        && longitudinal_continuity >= 0.4;

    let is_indoor = !is_open_sky && ceiling_dist < 450.0 && close_walls_count >= 2;

    let (target_room_type, mut target_t60, base_damping, mut target_mix, enclosure_val) =
        if is_tunnel {
            (RoomType::Tunnel, 2.10, 0.18, 0.60, 1.0)
        } else if is_indoor {
            (RoomType::Indoor, 0.75, 0.28, 0.35, 0.55)
        } else {
            (RoomType::Outdoor, 0.04, 0.05, 0.0, 0.0)
        };

    let hits_f = total_hits.max(1) as f32;
    let metal_ratio = metal_hits as f32 / hits_f;
    let wood_ratio = wood_hits as f32 / hits_f;
    let mat_damping = if total_hits > 0 {
        accumulated_damping / hits_f
    } else {
        base_damping
    };
    let target_damping = (base_damping * 0.5 + mat_damping * 0.5).clamp(0.04, 0.70);

    let mat_t60_mult = 1.0 + (metal_ratio * 0.35) - (wood_ratio * 0.25);
    target_t60 *= mat_t60_mult;

    let reverb_scale = HRTF_REVERB_SCALE.value_or(1.0);
    target_t60 *= reverb_scale;
    target_mix = (target_mix * reverb_scale).clamp(0.0, 0.75);

    const LERP_RATE: f32 = 0.08;
    let mut current_room = target_room_type;

    let primary_mat = if metal_ratio > 0.4 {
        SurfaceMaterial::Metal
    } else if wood_ratio > 0.4 {
        SurfaceMaterial::Wood
    } else {
        SurfaceMaterial::Concrete
    };

    if let Ok(mut cur) = CURRENT_ENVIRONMENT.lock() {
        cur.t60_reverb_time += (target_t60 - cur.t60_reverb_time) * LERP_RATE;
        cur.damping += (target_damping - cur.damping) * LERP_RATE;
        cur.reflection_mix += (target_mix - cur.reflection_mix) * LERP_RATE;
        cur.enclosure_factor += (enclosure_val - cur.enclosure_factor) * LERP_RATE;
        cur.estimated_volume += (target_volume - cur.estimated_volume) * LERP_RATE;
        cur.primary_material = primary_mat;
        cur.metal_ratio = metal_ratio;
        cur.wood_ratio = wood_ratio;
        current_room = cur.room_type;
    }

    if let Ok(mut deb) = ROOM_DEBOUNCE.lock() {
        if deb.0 == target_room_type {
            deb.1 = deb.1.saturating_add(1);
            deb.3 = 0;
        } else {
            deb.3 = deb.3.saturating_add(1);
            if deb.3 > FLICKER_TOLERANCE {
                deb.0 = target_room_type;
                deb.1 = 0;
                deb.3 = 0;
            }
        }

        if deb.1 >= DEBOUNCE_THRESHOLD && current_room != deb.0 {
            let target_room = deb.0;
            if let Ok(mut cur) = CURRENT_ENVIRONMENT.lock() {
                cur.room_type = target_room;
            }

            let engine_room_id: f32 = match target_room {
                RoomType::Outdoor => 0.0,
                RoomType::Indoor => 12.0,
                RoomType::Tunnel => 18.0,
            };

            deb.2 = engine_room_id;

            if HRTF_ACOUSTICS.value_or(1.0) == 0.0 {
                let cvar_set_ptr = crate::engine::cdll_int::G_ENGFUNCS.Cvar_SetValue;
                if !cvar_set_ptr.is_null() {
                    let cvar_set: unsafe extern "C" fn(*const c_char, f32) =
                        std::mem::transmute(cvar_set_ptr);
                    cvar_set(c"room_type".as_ptr(), engine_room_id);
                }
            }
        }
    }
}

pub fn get_current_acoustics() -> AcousticEnvironment {
    CURRENT_ENVIRONMENT.lock().map(|e| *e).unwrap_or_default()
}

unsafe fn find_diffraction_corner(listener: [f32; 3], sound: [f32; 3]) -> Option<([f32; 3], f32)> {
    let dx = sound[0] - listener[0];
    let dy = sound[1] - listener[1];
    let dist_xy = (dx * dx + dy * dy).sqrt();
    if dist_xy < 1.0 {
        return None;
    }

    let perp_x = -dy / dist_xy;
    let perp_y = dx / dist_xy;

    let offsets = [50.0, -50.0, 100.0, -100.0, 160.0, -160.0, 240.0, -240.0];

    for off in offsets {
        let edge_pos = [
            listener[0] + dx * 0.5 + perp_x * off,
            listener[1] + dy * 0.5 + perp_y * off,
            listener[2] + 16.0,
        ];

        let path1_clear = trace_world_ray(listener, edge_pos)
            .map(|t| t.fraction > 0.94)
            .unwrap_or(false);

        if path1_clear {
            let path2_clear = trace_world_ray(edge_pos, sound)
                .map(|t| t.fraction > 0.94)
                .unwrap_or(false);

            if path2_clear {
                let v1 = [edge_pos[0] - listener[0], edge_pos[1] - listener[1]];
                let v2 = [sound[0] - edge_pos[0], sound[1] - edge_pos[1]];
                let len1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
                let len2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt();

                if len1 > 0.1 && len2 > 0.1 {
                    let dot = ((v1[0] * v2[0] + v1[1] * v2[1]) / (len1 * len2)).clamp(-1.0, 1.0);
                    let angle_deg = dot.acos().to_degrees();
                    return Some((edge_pos, angle_deg));
                }
            }
        }
    }

    None
}

const OCCLUSION_CACHE_TTL: f32 = 0.10;
const OCCLUSION_CACHE_MAX_ENTRIES: usize = 128;

struct OcclusionCacheEntry {
    key: (i32, i32, i32, i32, i32, i32),
    result: SoundPropagation,
    computed_at: f32,
}

static OCCLUSION_CACHE: Mutex<Vec<OcclusionCacheEntry>> = Mutex::new(Vec::new());

fn quantize_pos(pos: [f32; 3]) -> (i32, i32, i32) {
    (
        (pos[0] / 4.0).round() as i32,
        (pos[1] / 4.0).round() as i32,
        (pos[2] / 4.0).round() as i32,
    )
}

fn cache_key(sound_origin: [f32; 3], listener_pos: [f32; 3]) -> (i32, i32, i32, i32, i32, i32) {
    let (sx, sy, sz) = quantize_pos(sound_origin);
    let (lx, ly, lz) = quantize_pos(listener_pos);
    (sx, sy, sz, lx, ly, lz)
}

pub unsafe fn evaluate_sound_propagation(
    sound_origin: [f32; 3],
    listener_pos: [f32; 3],
) -> SoundPropagation {
    if HRTF_OCCLUSION.value_or(1.0) == 0.0 {
        return SoundPropagation {
            occlusion: 0.0,
            volume_factor: 1.0,
            is_source_in_cave: false,
            reverb_send: 1.0,
            is_diffracted: false,
            diffracted_origin: sound_origin,
            diffraction_angle_deg: 0.0,
        };
    }

    let now = crate::engine::cdll_int::CLIENT_TIME
        .lock()
        .map(|t| *t)
        .unwrap_or(0.0);
    let key = cache_key(sound_origin, listener_pos);

    if let Ok(mut cache) = OCCLUSION_CACHE.lock() {
        if let Some(entry) = cache.iter().find(|e| e.key == key) {
            let age = now - entry.computed_at;
            if (0.0..OCCLUSION_CACHE_TTL).contains(&age) {
                return entry.result;
            }
        }
        cache.retain(|e| (0.0..OCCLUSION_CACHE_TTL).contains(&(now - e.computed_at)));
    }

    let result = compute_sound_propagation_uncached(sound_origin, listener_pos);

    if let Ok(mut cache) = OCCLUSION_CACHE.lock() {
        if let Some(entry) = cache.iter_mut().find(|e| e.key == key) {
            entry.result = result;
            entry.computed_at = now;
        } else {
            if cache.len() >= OCCLUSION_CACHE_MAX_ENTRIES {
                if let Some((idx, _)) = cache
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.computed_at.total_cmp(&b.1.computed_at))
                {
                    cache.remove(idx);
                }
            }
            cache.push(OcclusionCacheEntry {
                key,
                result,
                computed_at: now,
            });
        }
    }

    result
}

unsafe fn compute_sound_propagation_uncached(
    sound_origin: [f32; 3],
    listener_pos: [f32; 3],
) -> SoundPropagation {
    let start_pos = listener_pos;

    let direct_tr = trace_world_ray(start_pos, sound_origin);

    let is_direct_clear = if let Some(tr) = direct_tr {
        if tr.fraction >= 0.97 {
            true
        } else {
            let dx = sound_origin[0] - tr.endpos[0];
            let dy = sound_origin[1] - tr.endpos[1];
            let dz = sound_origin[2] - tr.endpos[2];
            let dist_to_target = (dx * dx + dy * dy + dz * dz).sqrt();
            dist_to_target <= 28.0
        }
    } else {
        true
    };

    let alt_clear = if !is_direct_clear {
        let target_up = [sound_origin[0], sound_origin[1], sound_origin[2] + 40.0];
        trace_world_ray(start_pos, target_up)
            .map(|tr| tr.fraction >= 0.96)
            .unwrap_or(false)
    } else {
        true
    };

    if is_direct_clear || alt_clear {
        return SoundPropagation {
            occlusion: 0.0,
            volume_factor: 1.0,
            is_source_in_cave: false,
            reverb_send: 1.0,
            is_diffracted: false,
            diffracted_origin: sound_origin,
            diffraction_angle_deg: 0.0,
        };
    }

    let diffraction = find_diffraction_corner(listener_pos, sound_origin);
    let (is_diffracted, _diffracted_origin, diffraction_angle_deg) = match diffraction {
        Some((edge, angle)) => (true, edge, angle),
        None => (false, sound_origin, 0.0),
    };

    let mut blocked_rays = 1;
    let side_offsets = [
        [0.0, 0.0, 48.0],
        [0.0, 0.0, -16.0],
        [32.0, 32.0, 0.0],
        [-32.0, -32.0, 0.0],
        [32.0, -32.0, 0.0],
        [-32.0, 32.0, 0.0],
    ];
    for off in &side_offsets {
        let offset_start = [
            start_pos[0] + off[0],
            start_pos[1] + off[1],
            start_pos[2] + off[2],
        ];
        let offset_end = [
            sound_origin[0] + off[0],
            sound_origin[1] + off[1],
            sound_origin[2] + off[2],
        ];
        if let Some(tr) = trace_world_ray(offset_start, offset_end) {
            if tr.fraction < 0.95 {
                blocked_rays += 1;
            }
        }
    }

    let ceiling_test = [sound_origin[0], sound_origin[1], sound_origin[2] + 280.0];
    let source_has_low_ceiling = if let Some(tr) = trace_world_ray(sound_origin, ceiling_test) {
        tr.fraction < 0.95
    } else {
        false
    };

    let mut wall_hit = [false; 4];
    let offsets = [
        [240.0, 0.0, 0.0],
        [-240.0, 0.0, 0.0],
        [0.0, 240.0, 0.0],
        [0.0, -240.0, 0.0],
    ];

    for (idx, off) in offsets.iter().enumerate() {
        let side = [
            sound_origin[0] + off[0],
            sound_origin[1] + off[1],
            sound_origin[2] + off[2],
        ];
        if let Some(tr) = trace_world_ray(sound_origin, side) {
            if tr.fraction < 0.90 {
                wall_hit[idx] = true;
            }
        }
    }

    let x_axis_enclosed = wall_hit[0] && wall_hit[1];
    let y_axis_enclosed = wall_hit[2] && wall_hit[3];
    let total_walls_hit = wall_hit.iter().filter(|&&h| h).count();
    let has_enclosing_pair = x_axis_enclosed || y_axis_enclosed || total_walls_hit >= 3;

    let is_source_in_cave = source_has_low_ceiling && has_enclosing_pair;

    let (mut occlusion, mut volume_factor) = if is_diffracted {
        let bend_penalty = (diffraction_angle_deg / 180.0).clamp(0.0, 1.0) * 0.40;
        (0.35 + bend_penalty, 0.70 - bend_penalty * 0.3)
    } else if blocked_rays >= 4 || (blocked_rays >= 3 && is_source_in_cave) {
        (0.85, 0.35)
    } else if is_source_in_cave {
        (0.75, 0.45)
    } else {
        (0.60, 0.55)
    };

    let scale = HRTF_OCCLUSION.value_or(1.0);
    occlusion = (occlusion * scale).clamp(0.0, 0.95);
    volume_factor = (1.0 - (1.0 - volume_factor) * scale).clamp(0.30, 1.0);

    let reverb_send = (1.0 - occlusion * 0.8).clamp(0.15, 1.0);

    SoundPropagation {
        occlusion,
        volume_factor,
        is_source_in_cave,
        reverb_send,
        is_diffracted,
        diffracted_origin: sound_origin,
        diffraction_angle_deg,
    }
}

pub fn print_ray_state() {
    let acoustics = get_current_acoustics();

    let mut msg = String::new();
    msg.push_str("\n=================== ACOUSTIC RAY SYSTEM STATE ===================\n");
    msg.push_str(&format!(
        "Environment: {:?} | T60: {:.2}s | Mix: {:.0}% | Damp: {:.0}% | Vol: {:.0} u³ | Mat: {:?} (Met:{:.0}% Wod:{:.0}%)\n",
        acoustics.room_type,
        acoustics.t60_reverb_time,
        acoustics.reflection_mix * 100.0,
        acoustics.damping * 100.0,
        acoustics.estimated_volume,
        acoustics.primary_material,
        acoustics.metal_ratio * 100.0,
        acoustics.wood_ratio * 100.0,
    ));

    if let Ok(rays) = LAST_ACOUSTIC_RAYS.lock() {
        if rays.is_empty() {
            msg.push_str("No acoustic rays traced yet (move or spawn in-game).\n");
        } else {
            let start = rays[0].start;
            msg.push_str(&format!(
                "Player Center Origin X Y Z: [{:.1}, {:.1}, {:.1}]\n",
                start[0], start[1], start[2]
            ));
            msg.push_str("---------------------------------------------------------------------------------\n");
            msg.push_str(
                " ID | Direction Label  | Target (Hit/End)         | Dist  | Mat | Status\n",
            );
            msg.push_str("---------------------------------------------------------------------------------\n");

            let mut hit_count = 0;
            let mut open_count = 0;

            for (i, ray) in rays.iter().enumerate() {
                let target = if ray.hit { ray.hit_pos } else { ray.end };
                let dx = target[0] - ray.start[0];
                let dy = target[1] - ray.start[1];
                let dz = target[2] - ray.start[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                let status_str = if ray.hit {
                    hit_count += 1;
                    "HIT (Wall)"
                } else {
                    open_count += 1;
                    "OPEN (Sky/Space)"
                };

                msg.push_str(&format!(
                    "[{:02}] {:<18} | [{:7.1}, {:7.1}, {:7.1}] | {:5.0}u | [{}] | {}\n",
                    i,
                    ray.label,
                    target[0],
                    target[1],
                    target[2],
                    dist,
                    ray.material.tag(),
                    status_str
                ));
            }

            msg.push_str("---------------------------------------------------------------------------------\n");
            msg.push_str(&format!(
                "Total Rays: {} | Hits: {} | Open: {}\n",
                rays.len(),
                hit_count,
                open_count
            ));
        }
    }
    msg.push_str(
        "=================================================================================\n\n",
    );

    crate::utils::console::con_print_str(&msg);
}
