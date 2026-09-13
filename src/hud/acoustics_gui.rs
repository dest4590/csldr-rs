use crate::engine::acoustics::{
    get_current_acoustics, trace_world_ray, RoomType, LAST_ACOUSTIC_RAYS,
};
use crate::engine::acoustics_debug::ACTIVE_SOUND_NODES;
use crate::engine::cdll_int::CLIENT_TIME;
use crate::engine::native_hrtf::NATIVE_HRTF_ENGINE;
use crate::engine::sound_hook::get_listener_state;
use crate::render::draw2d::*;
use crate::render::gl::*;
use crate::render::studiorenderer::CAN_OPENGL;
use crate::utils::cvars::{HRTF_DEBUG, HRTF_DEBUG_RAYS};

unsafe fn find_debug_bend_path(listener: [f32; 3], sound: [f32; 3]) -> Option<([f32; 3], f32)> {
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

pub unsafe fn draw_acoustic_rays_3d() {
    let debug_rays_enabled = HRTF_DEBUG_RAYS.value() != 0.0
        || HRTF_DEBUG.value() != 0.0
        || crate::render::camera::is_freecam_active();

    if !debug_rays_enabled {
        return;
    }

    gl_disable(GL_TEXTURE_2D);
    gl_disable(GL_DEPTH_TEST);
    gl_enable(GL_BLEND);
    gl_blend_func(GL_SRC_ALPHA, GL_ONE);

    if let Ok(rays) = LAST_ACOUSTIC_RAYS.lock() {
        gl_line_width(2.0);
        gl_begin(GL_LINES);

        for ray in rays.iter() {
            let target = if ray.hit { ray.hit_pos } else { ray.end };

            if ray.hit {
                match ray.material {
                    crate::engine::acoustics::SurfaceMaterial::Metal => {
                        gl_color4f(0.2, 0.9, 1.0, 0.70)
                    }
                    crate::engine::acoustics::SurfaceMaterial::Wood => {
                        gl_color4f(0.9, 0.55, 0.2, 0.70)
                    }
                    crate::engine::acoustics::SurfaceMaterial::DirtSand => {
                        gl_color4f(0.6, 0.8, 0.3, 0.70)
                    }
                    crate::engine::acoustics::SurfaceMaterial::Glass => {
                        gl_color4f(0.9, 0.9, 1.0, 0.80)
                    }
                    _ => gl_color4f(1.0, 0.45, 0.15, 0.70),
                }
            } else {
                gl_color4f(0.0, 0.85, 1.0, 0.35);
            }

            gl_vertex3f(ray.start[0], ray.start[1], ray.start[2]);
            gl_vertex3f(target[0], target[1], target[2]);

            if ray.hit {
                const CROSS_SZ: f32 = 3.0;
                gl_vertex3f(target[0] - CROSS_SZ, target[1], target[2]);
                gl_vertex3f(target[0] + CROSS_SZ, target[1], target[2]);
                gl_vertex3f(target[0], target[1] - CROSS_SZ, target[2]);
                gl_vertex3f(target[0], target[1] + CROSS_SZ, target[2]);
                gl_vertex3f(target[0], target[1], target[2] - CROSS_SZ);
                gl_vertex3f(target[0], target[1], target[2] + CROSS_SZ);
            }
        }

        gl_end();
    }

    let listener = get_listener_state();
    let now = CLIENT_TIME.lock().map(|t| *t).unwrap_or(0.0);

    if let Ok(nodes) = ACTIVE_SOUND_NODES.lock() {
        for node in nodes.iter() {
            if node.is_expired(now) {
                continue;
            }

            let start_pos = [
                listener.origin[0],
                listener.origin[1],
                listener.origin[2] + 8.0,
            ];
            let sound_pos = node.origin;

            let direct_tr = trace_world_ray(start_pos, sound_pos);
            let direct_blocked = if !node.occluded {
                false
            } else if let Some(tr) = direct_tr {
                if tr.fraction >= 0.97 {
                    false
                } else {
                    let dx = sound_pos[0] - tr.endpos[0];
                    let dy = sound_pos[1] - tr.endpos[1];
                    let dz = sound_pos[2] - tr.endpos[2];
                    let dist_to_target = (dx * dx + dy * dy + dz * dz).sqrt();
                    dist_to_target > 28.0
                }
            } else {
                false
            };

            if direct_blocked {
                let wall_hit = direct_tr.map(|t| t.endpos).unwrap_or(sound_pos);

                gl_line_width(1.5);
                gl_begin(GL_LINES);
                gl_color4f(1.0, 0.2, 0.2, 0.45);
                gl_vertex3f(start_pos[0], start_pos[1], start_pos[2]);
                gl_vertex3f(wall_hit[0], wall_hit[1], wall_hit[2]);

                gl_color4f(0.8, 0.1, 0.1, 0.20);
                gl_vertex3f(wall_hit[0], wall_hit[1], wall_hit[2]);
                gl_vertex3f(sound_pos[0], sound_pos[1], sound_pos[2]);

                gl_color4f(1.0, 0.1, 0.1, 0.90);
                const HIT_SZ: f32 = 4.0;
                gl_vertex3f(wall_hit[0] - HIT_SZ, wall_hit[1], wall_hit[2]);
                gl_vertex3f(wall_hit[0] + HIT_SZ, wall_hit[1], wall_hit[2]);
                gl_vertex3f(wall_hit[0], wall_hit[1] - HIT_SZ, wall_hit[2]);
                gl_vertex3f(wall_hit[0], wall_hit[1] + HIT_SZ, wall_hit[2]);
                gl_end();

                if let Some((corner_pos, _angle)) = find_debug_bend_path(start_pos, sound_pos) {
                    gl_line_width(3.5);
                    gl_begin(GL_LINES);
                    gl_color4f(0.95, 0.20, 1.0, 0.95);
                    gl_vertex3f(start_pos[0], start_pos[1], start_pos[2]);
                    gl_vertex3f(corner_pos[0], corner_pos[1], corner_pos[2]);

                    gl_color4f(1.0, 0.85, 0.10, 0.95);
                    gl_vertex3f(corner_pos[0], corner_pos[1], corner_pos[2]);
                    gl_vertex3f(sound_pos[0], sound_pos[1], sound_pos[2]);

                    gl_color4f(1.0, 0.95, 0.10, 1.0);
                    const CRN_SZ: f32 = 6.0;
                    gl_vertex3f(corner_pos[0] - CRN_SZ, corner_pos[1], corner_pos[2]);
                    gl_vertex3f(corner_pos[0] + CRN_SZ, corner_pos[1], corner_pos[2]);
                    gl_vertex3f(corner_pos[0], corner_pos[1] - CRN_SZ, corner_pos[2]);
                    gl_vertex3f(corner_pos[0], corner_pos[1] + CRN_SZ, corner_pos[2]);
                    gl_vertex3f(corner_pos[0], corner_pos[1], corner_pos[2] - CRN_SZ);
                    gl_vertex3f(corner_pos[0], corner_pos[1], corner_pos[2] + CRN_SZ);
                    gl_end();
                }
            } else {
                gl_line_width(3.0);
                gl_begin(GL_LINES);
                gl_color4f(0.15, 1.0, 0.35, 0.85);
                gl_vertex3f(start_pos[0], start_pos[1], start_pos[2]);
                gl_vertex3f(sound_pos[0], sound_pos[1], sound_pos[2]);
                gl_end();
            }

            gl_line_width(2.0);
            gl_begin(GL_LINES);
            gl_color4f(0.2, 0.9, 1.0, 0.90);
            const SRC_SZ: f32 = 5.0;
            gl_vertex3f(sound_pos[0] - SRC_SZ, sound_pos[1], sound_pos[2]);
            gl_vertex3f(sound_pos[0] + SRC_SZ, sound_pos[1], sound_pos[2]);
            gl_vertex3f(sound_pos[0], sound_pos[1] - SRC_SZ, sound_pos[2]);
            gl_vertex3f(sound_pos[0], sound_pos[1] + SRC_SZ, sound_pos[2]);
            gl_vertex3f(sound_pos[0], sound_pos[1], sound_pos[2] - SRC_SZ);
            gl_vertex3f(sound_pos[0], sound_pos[1], sound_pos[2] + SRC_SZ);
            gl_end();
        }
    }

    gl_line_width(1.0);
    gl_disable(GL_BLEND);
    gl_enable(GL_DEPTH_TEST);
    gl_enable(GL_TEXTURE_2D);
}

pub unsafe fn draw_acoustics_gui() {
    if !CAN_OPENGL.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    let freecam_active = crate::render::camera::is_freecam_active();
    let debug_rays_enabled =
        HRTF_DEBUG_RAYS.value() != 0.0 || HRTF_DEBUG.value() != 0.0 || freecam_active;
    let debug_gui_enabled = HRTF_DEBUG.value() != 0.0 || freecam_active;

    if !debug_gui_enabled && !debug_rays_enabled {
        return;
    }

    let listener = get_listener_state();
    let acoustics = get_current_acoustics();
    let now = CLIENT_TIME.lock().map(|t| *t).unwrap_or(0.0);

    if debug_rays_enabled {
        if let Ok(rays) = LAST_ACOUSTIC_RAYS.lock() {
            for ray in rays.iter() {
                let target = if ray.hit { ray.hit_pos } else { ray.end };

                if let Some(hit_scr) = world_to_screen(target) {
                    let dx = target[0] - ray.start[0];
                    let dy = target[1] - ray.start[1];
                    let dz = target[2] - ray.start[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                    if ray.hit {
                        let text = format!("{:.0}u [{}]", dist, ray.material.tag());
                        let (r, g, b) = match ray.material {
                            crate::engine::acoustics::SurfaceMaterial::Metal => (0.3, 0.9, 1.0),
                            crate::engine::acoustics::SurfaceMaterial::Wood => (1.0, 0.65, 0.25),
                            crate::engine::acoustics::SurfaceMaterial::DirtSand => {
                                (0.65, 0.9, 0.35)
                            }
                            crate::engine::acoustics::SurfaceMaterial::Glass => (0.9, 0.95, 1.0),
                            _ => (1.0, 0.50, 0.15),
                        };
                        draw_rect_2d(hit_scr[0] - 2.5, hit_scr[1] - 2.5, 5.0, 5.0, r, g, b, 0.95);
                        draw_text_2d(
                            (hit_scr[0] + 5.0) as i32,
                            (hit_scr[1] - 4.0) as i32,
                            &text,
                            r,
                            g,
                            b,
                        );
                    }
                }
            }
        }
    }

    if !debug_gui_enabled {
        return;
    }

    let spatial = NATIVE_HRTF_ENGINE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|e| e.last_spatial))
        .unwrap_or_default();

    if let Ok(nodes) = ACTIVE_SOUND_NODES.lock() {
        for node in nodes.iter() {
            let age = node.age(now);
            if node.is_expired(now) {
                continue;
            }

            let alpha = (1.0 - (age / 2.5)).clamp(0.15, 1.0);
            let dx = node.origin[0] - listener.origin[0];
            let dy = node.origin[1] - listener.origin[1];
            let dz = node.origin[2] - listener.origin[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

            if node.occluded {
                if let Some((corner_pos, angle_deg)) =
                    find_debug_bend_path(listener.origin, node.origin)
                {
                    if let Some(corner_scr) = world_to_screen(corner_pos) {
                        let crn_text = format!("* BEND CORNER ({:.0} deg)", angle_deg);
                        draw_rect_2d(
                            corner_scr[0] - 3.0,
                            corner_scr[1] - 3.0,
                            6.0,
                            6.0,
                            0.95,
                            0.20,
                            1.0,
                            alpha,
                        );
                        draw_text_2d(
                            (corner_scr[0] + 8.0) as i32,
                            (corner_scr[1] - 4.0) as i32,
                            &crn_text,
                            0.95,
                            0.85,
                            0.20,
                        );
                    }
                }
            }

            if let Some(src_screen) = world_to_screen(node.origin) {
                let short_name = node
                    .sound_name
                    .rsplit('/')
                    .next()
                    .unwrap_or(&node.sound_name);
                let diff_tag = if node.occluded { " [WALL]" } else { "" };
                let elev_arrow = if dz > 60.0 {
                    " ^"
                } else if dz < -60.0 {
                    " v"
                } else {
                    ""
                };
                let text_line = format!("{} {:.0}u{}{}", short_name, dist, diff_tag, elev_arrow);

                draw_rect_2d(
                    src_screen[0] - 2.0,
                    src_screen[1] - 2.0,
                    4.0,
                    4.0,
                    0.2,
                    0.9,
                    1.0,
                    alpha,
                );
                let panel_w = get_text_width(&text_line) + 14.0;
                draw_rect_2d(
                    src_screen[0] + 6.0,
                    src_screen[1] - 9.0,
                    panel_w,
                    20.0,
                    0.05,
                    0.06,
                    0.08,
                    alpha * 0.85,
                );
                draw_text_2d(
                    (src_screen[0] + 12.0) as i32,
                    (src_screen[1] - 3.0) as i32,
                    &text_line,
                    0.92,
                    0.92,
                    0.94,
                );
            }
        }
    }

    let info_w = 185.0;
    let info_h = 100.0;
    let info_x = 12.0;
    let info_y = 12.0;

    draw_rect_2d(info_x, info_y, info_w, info_h, 0.04, 0.05, 0.07, 0.90);
    draw_rect_outline_2d(info_x, info_y, info_w, info_h, 0.2, 0.24, 0.28, 0.70);

    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 4.0) as i32,
        "HRTF ACOUSTIC ENGINE",
        0.0,
        0.85,
        1.0,
    );

    let room_type_name = match acoustics.room_type {
        RoomType::Outdoor => "Outdoor",
        RoomType::Indoor => "Indoor",
        RoomType::Tunnel => "Tunnel",
    };
    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 18.0) as i32,
        &format!(
            "ROOM     : {} (T60: {:.2}s)",
            room_type_name, acoustics.t60_reverb_time
        ),
        0.88,
        0.88,
        0.90,
    );
    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 30.0) as i32,
        &format!(
            "MATERIAL : {} (Met:{:.0}% Wod:{:.0}%)",
            acoustics.primary_material.tag(),
            acoustics.metal_ratio * 100.0,
            acoustics.wood_ratio * 100.0
        ),
        0.88,
        0.88,
        0.90,
    );
    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 42.0) as i32,
        &format!(
            "DAMPING  : {:.2} | MIX: {:.0}%",
            acoustics.damping,
            acoustics.reflection_mix * 100.0
        ),
        0.88,
        0.88,
        0.90,
    );

    let rear_status = if spatial.rear_shadow > 0.05 {
        format!("ACTIVE ({:.1}dB)", spatial.rear_shadow * -2.5)
    } else {
        "CLEAR (0dB)".to_string()
    };
    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 54.0) as i32,
        &format!("REAR PINNA : {}", rear_status),
        1.0,
        0.75,
        0.25,
    );

    let elev_deg = spatial.elevation.to_degrees();
    let elev_status = if elev_deg > 8.0 {
        format!("+Airy (+{:.0} deg)", elev_deg)
    } else if elev_deg < -8.0 {
        format!("-Notch 7.2k ({:.0} deg)", elev_deg)
    } else {
        format!("{:.0} deg", elev_deg)
    };
    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 66.0) as i32,
        &format!("ELEVATION  : {}", elev_status),
        0.35,
        0.90,
        0.45,
    );

    let diff_status = if spatial.is_diffracted {
        format!("BENT ({:.0} deg)", spatial.diffraction_angle_deg)
    } else {
        "DIRECT".to_string()
    };
    draw_text_2d(
        (info_x + 8.0) as i32,
        (info_y + 78.0) as i32,
        &format!("DIFFRACT   : {}", diff_status),
        0.90,
        0.40,
        0.85,
    );

    let radar_x = info_x + info_w + 10.0;
    let radar_y = info_y;
    let radar_size = 100.0;
    let center_x = radar_x + radar_size * 0.5;
    let center_y = radar_y + radar_size * 0.5;

    draw_rect_2d(
        radar_x, radar_y, radar_size, radar_size, 0.04, 0.05, 0.07, 0.90,
    );
    draw_rect_outline_2d(
        radar_x, radar_y, radar_size, radar_size, 0.2, 0.24, 0.28, 0.70,
    );

    draw_rect_2d(
        radar_x + 2.0,
        center_y,
        radar_size - 4.0,
        (radar_size * 0.5) - 2.0,
        0.20,
        0.05,
        0.05,
        0.25,
    );

    draw_rect_2d(
        center_x - 0.5,
        radar_y + 4.0,
        1.0,
        radar_size - 8.0,
        0.2,
        0.25,
        0.3,
        0.5,
    );
    draw_rect_2d(
        radar_x + 4.0,
        center_y - 0.5,
        radar_size - 8.0,
        1.0,
        0.2,
        0.25,
        0.3,
        0.5,
    );
    draw_rect_2d(
        center_x - 1.5,
        center_y - 1.5,
        3.0,
        3.0,
        0.0,
        0.85,
        1.0,
        1.0,
    );

    const RADAR_SCALE: f32 = 0.040;

    if let Ok(rays) = LAST_ACOUSTIC_RAYS.lock() {
        for ray in rays.iter() {
            let target = if ray.hit { ray.hit_pos } else { ray.end };
            let dx = target[0] - ray.start[0];
            let dy = target[1] - ray.start[1];

            let local_x = dx * listener.right[0] + dy * listener.right[1];
            let local_y = dx * listener.forward[0] + dy * listener.forward[1];

            let pt_x = center_x + (local_x * RADAR_SCALE).clamp(-46.0, 46.0);
            let pt_y = center_y - (local_y * RADAR_SCALE).clamp(-46.0, 46.0);

            let (r, g, b) = match ray.material {
                crate::engine::acoustics::SurfaceMaterial::Metal => (0.3, 0.9, 1.0),
                crate::engine::acoustics::SurfaceMaterial::Wood => (1.0, 0.6, 0.2),
                crate::engine::acoustics::SurfaceMaterial::DirtSand => (0.6, 0.85, 0.3),
                _ => (1.0, 0.45, 0.2),
            };
            draw_rect_2d(pt_x - 1.0, pt_y - 1.0, 2.0, 2.0, r, g, b, 0.85);
        }
    }

    if let Ok(nodes) = ACTIVE_SOUND_NODES.lock() {
        for node in nodes.iter() {
            if node.is_expired(now) {
                continue;
            }

            let s_dx = node.origin[0] - listener.origin[0];
            let s_dy = node.origin[1] - listener.origin[1];
            let s_lx = s_dx * listener.right[0] + s_dy * listener.right[1];
            let s_ly = s_dx * listener.forward[0] + s_dy * listener.forward[1];
            let s_x = center_x + (s_lx * RADAR_SCALE).clamp(-46.0, 46.0);
            let s_y = center_y - (s_ly * RADAR_SCALE).clamp(-46.0, 46.0);

            if let Some((corner_pos, _)) = find_debug_bend_path(listener.origin, node.origin) {
                let c_dx = corner_pos[0] - listener.origin[0];
                let c_dy = corner_pos[1] - listener.origin[1];
                let c_lx = c_dx * listener.right[0] + c_dy * listener.right[1];
                let c_ly = c_dx * listener.forward[0] + c_dy * listener.forward[1];
                let c_x = center_x + (c_lx * RADAR_SCALE).clamp(-46.0, 46.0);
                let c_y = center_y - (c_ly * RADAR_SCALE).clamp(-46.0, 46.0);

                draw_rect_2d(c_x - 2.0, c_y - 2.0, 4.0, 4.0, 0.95, 0.20, 1.0, 0.95);
            }

            let (sr, sg, sb) = if node.occluded {
                (1.0, 0.2, 0.2)
            } else {
                (0.2, 0.9, 1.0)
            };
            draw_rect_2d(s_x - 2.0, s_y - 2.0, 4.0, 4.0, sr, sg, sb, 0.95);
        }
    }
}
