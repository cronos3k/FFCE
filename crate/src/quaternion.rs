//! Quaternion helpers for motion intent in the trace / future fields.
//!
//! Faithful port of `quaternion.py`. Quaternions are stored as [w, x, y, z].

pub type Quat = [f32; 4];

pub fn identity() -> Quat {
    [1.0, 0.0, 0.0, 0.0]
}

fn norm4(q: &Quat) -> f32 {
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt()
}

fn normalize(q: Quat) -> Quat {
    let n = norm4(&q);
    if n < 1e-8 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn from_two_vectors(a: [f32; 3], b: [f32; 3]) -> Quat {
    let na = norm3(a) + 1e-8;
    let nb = norm3(b) + 1e-8;
    let a = [a[0] / na, a[1] / na, a[2] / na];
    let b = [b[0] / nb, b[1] / nb, b[2] / nb];
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    if dot < -0.999999 {
        let mut axis = [1.0, 0.0, 0.0];
        if a[0].abs() > 0.9 {
            axis = [0.0, 1.0, 0.0];
        }
        let mut ax = cross(a, axis);
        let n = norm3(ax) + 1e-8;
        ax = [ax[0] / n, ax[1] / n, ax[2] / n];
        return [0.0, ax[0], ax[1], ax[2]];
    }
    let v = cross(a, b);
    let w = 1.0 + dot;
    normalize([w, v[0], v[1], v[2]])
}

fn mul(q1: Quat, q2: Quat) -> Quat {
    let [w1, x1, y1, z1] = q1;
    let [w2, x2, y2, z2] = q2;
    [
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
    ]
}

fn conj(q: Quat) -> Quat {
    [q[0], -q[1], -q[2], -q[3]]
}

fn rotate(q: Quat, v: [f32; 3]) -> [f32; 3] {
    let vq = [0.0, v[0], v[1], v[2]];
    let r = mul(mul(q, vq), conj(q));
    [r[1], r[2], r[3]]
}

fn slerp(q1: Quat, q2: Quat, t: f32) -> Quat {
    let q1 = normalize(q1);
    let mut q2 = normalize(q2);
    let mut dot = q1[0] * q2[0] + q1[1] * q2[1] + q1[2] * q2[2] + q1[3] * q2[3];
    if dot < 0.0 {
        q2 = [-q2[0], -q2[1], -q2[2], -q2[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        let q = [
            q1[0] + t * (q2[0] - q1[0]),
            q1[1] + t * (q2[1] - q1[1]),
            q1[2] + t * (q2[2] - q1[2]),
            q1[3] + t * (q2[3] - q1[3]),
        ];
        return normalize(q);
    }
    let theta_0 = dot.acos();
    let theta = theta_0 * t;
    let s0 = theta_0.sin();
    let s1 = theta.sin();
    let s2 = (theta_0 - theta).sin();
    [
        q1[0] * (s2 / s0) + q2[0] * (s1 / s0),
        q1[1] * (s2 / s0) + q2[1] * (s1 / s0),
        q1[2] * (s2 / s0) + q2[2] * (s1 / s0),
        q1[3] * (s2 / s0) + q2[3] * (s1 / s0),
    ]
}

/// Blend the previous intent quaternion toward the new motion vector.
pub fn update(q_prev: Quat, dx: i32, dy: i32, z0: f32, alpha: f32) -> Quat {
    let f = [0.0, 0.0, 1.0];
    let v = [dx as f32, dy as f32, z0];
    let q_star = from_two_vectors(f, v);
    slerp(q_prev, q_star, alpha)
}

/// Return the normalized XY direction of a quaternion-rotated forward axis.
pub fn forward_dir_xy(q: &Quat) -> [f32; 2] {
    let f = [0.0, 0.0, 1.0];
    let v = rotate(*q, f);
    let xy = [v[0], v[1]];
    let n = (xy[0] * xy[0] + xy[1] * xy[1]).sqrt();
    if n < 1e-6 {
        return [0.0, 0.0];
    }
    [xy[0] / n, xy[1] / n]
}
