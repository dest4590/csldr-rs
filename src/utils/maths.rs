use std::f32::consts::PI;

pub const F_PI: f32 = PI;

pub type Vec3 = [f32; 3];
pub type Mat3x4 = [[f32; 4]; 3];

#[inline]
pub fn rint(x: f32) -> i32 {
    if x < 0.0 {
        (x - 0.5) as i32
    } else {
        (x + 0.5) as i32
    }
}

#[inline]
pub fn degrees(rad: f32) -> f32 {
    rad * (180.0 / F_PI)
}

#[inline]
pub fn radians(deg: f32) -> f32 {
    deg * (F_PI / 180.0)
}

#[inline]
pub fn dot_product(a: &Vec3, b: &Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn vector_length(v: &Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[inline]
pub fn vector_length_2d(v: &Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

#[inline]
pub fn vector_normalize(v: &mut Vec3) {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len_sq > 0.0 {
        let s = 1.0 / len_sq.sqrt();
        v[0] *= s;
        v[1] *= s;
        v[2] *= s;
    }
}

#[inline]
pub fn cross_product(a: &Vec3, b: &Vec3, dst: &mut Vec3) {
    dst[0] = a[1] * b[2] - a[2] * b[1];
    dst[1] = a[2] * b[0] - a[0] * b[2];
    dst[2] = a[0] * b[1] - a[1] * b[0];
}

#[inline]
pub fn vector_copy(v: &Vec3, dst: &mut Vec3) {
    dst[0] = v[0];
    dst[1] = v[1];
    dst[2] = v[2];
}

#[inline]
pub fn vector_clear(v: &mut Vec3) {
    v[0] = 0.0;
    v[1] = 0.0;
    v[2] = 0.0;
}

#[inline]
pub fn vector_negate(v: &mut Vec3) {
    v[0] = -v[0];
    v[1] = -v[1];
    v[2] = -v[2];
}

#[inline]
pub fn vector_add(a: &Vec3, b: &Vec3, dst: &mut Vec3) {
    dst[0] = a[0] + b[0];
    dst[1] = a[1] + b[1];
    dst[2] = a[2] + b[2];
}

#[inline]
pub fn vector_subtract(a: &Vec3, b: &Vec3, dst: &mut Vec3) {
    dst[0] = a[0] - b[0];
    dst[1] = a[1] - b[1];
    dst[2] = a[2] - b[2];
}

#[inline]
pub fn vector_scale(a: &Vec3, s: f32, dst: &mut Vec3) {
    dst[0] = a[0] * s;
    dst[1] = a[1] * s;
    dst[2] = a[2] * s;
}

#[inline]
pub fn vector_ma(v: &Vec3, s: f32, dst: &mut Vec3) {
    dst[0] += v[0] * s;
    dst[1] += v[1] * s;
    dst[2] += v[2] * s;
}

#[inline]
pub fn vector_ma_2(v: &Vec3, s: f32, dst: &mut Vec3) {
    dst[0] += v[0] * s;
    dst[1] += v[1] * s;
    dst[2] -= v[2] * s;
}

pub fn angle_vectors(
    angles: &Vec3,
    forward: Option<&mut Vec3>,
    side: Option<&mut Vec3>,
    up: Option<&mut Vec3>,
) {
    let yaw = radians(angles[1]);
    let pitch = radians(angles[0]);
    let roll = radians(angles[2]);

    let cy = yaw.cos();
    let sy = yaw.sin();
    let cp = pitch.cos();
    let sp = pitch.sin();
    let cr = roll.cos();
    let sr = roll.sin();

    if let Some(fwd) = forward {
        fwd[0] = cy * cp;
        fwd[1] = cp * sy;
        fwd[2] = -sp;
    }

    if let Some(s) = side {
        s[0] = cr * sy - cy * sp * sr;
        s[1] = -(sy * sp * sr + cy * cr);
        s[2] = -(cp * sr);
    }

    if let Some(u) = up {
        u[0] = cy * cr * sp + sy * sr;
        u[1] = cr * sy * sp - cy * sr;
        u[2] = cp * cr;
    }
}

pub fn angle_lerp(a: &Vec3, b: &Vec3, t: f32, dst: &mut Vec3) {
    for i in 0..3 {
        let mut dt = b[i] - a[i];

        if dt < -180.0 {
            dt += 360.0;
        } else if dt > 180.0 {
            dt -= 360.0;
        }

        dst[i] = a[i] + t * dt;
    }
}

pub fn normalize_angle(mut angle: f32) -> f32 {
    while angle > 180.0 {
        angle -= 360.0;
    }
    while angle < -180.0 {
        angle += 360.0;
    }
    angle
}
