use crate::utils::keyvalue::KeyValueNode;
use crate::utils::maths::Vec3;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, Default)]
pub struct StudioConfig {
    pub mirror_shell: bool,
    pub mirror_model: bool,
    pub origin: Vec3,
    pub fov_override: f32,
}

#[derive(Debug, Clone, Default)]
pub struct StudioCache {
    pub name: String,
    pub header_length: i32,
    pub config: StudioConfig,
}

pub static FLUSH_COUNT: AtomicU32 = AtomicU32::new(0);
pub static CACHES: Mutex<Vec<StudioCache>> = Mutex::new(Vec::new());

pub fn update_studio_caches_dummy() {}

pub fn studio_config_flush() {
    FLUSH_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn parse_bool(s: &str) -> bool {
    s.trim().parse::<i32>().map(|n| n != 0).unwrap_or(false)
}

pub fn parse_config_from_str(config_str: &str) -> StudioConfig {
    let mut config = StudioConfig::default();

    if let Some(nodes) = KeyValueNode::parse(config_str) {
        for node in nodes {
            match node.name.as_str() {
                "mirror_shell" => {
                    if let crate::keyvalue::KeyValueValue::String(s) = node.value {
                        config.mirror_shell = parse_bool(&s);
                    }
                }
                "mirror_model" => {
                    if let crate::keyvalue::KeyValueValue::String(s) = node.value {
                        config.mirror_model = parse_bool(&s);
                    }
                }
                "origin" => {
                    if let crate::keyvalue::KeyValueValue::String(s) = node.value {
                        let parts: Vec<&str> = s.split_whitespace().collect();
                        if parts.len() == 3 {
                            if let (Ok(x), Ok(y), Ok(z)) =
                                (parts[0].parse(), parts[1].parse(), parts[2].parse())
                            {
                                config.origin = [x, y, z];
                            }
                        }
                    }
                }
                "fov_override" => {
                    if let crate::keyvalue::KeyValueValue::String(s) = node.value {
                        if let Ok(fov) = s.parse::<f32>() {
                            if (1.0..=179.0).contains(&fov) {
                                config.fov_override = fov;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    config
}
