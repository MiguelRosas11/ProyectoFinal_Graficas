use crate::math::Vec3;

pub fn hash(mut n: u32) -> u32 {
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^ (n >> 16)
}

pub fn unit(n: u32) -> f32 {
    (hash(n) >> 8) as f32 / 16_777_216.0
}

pub fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn value(p: Vec3, seed: u32) -> f32 {
    let x = p.x.floor();
    let y = p.y.floor();
    let z = p.z.floor();
    let (fx, fy, fz) = (smooth(p.x - x), smooth(p.y - y), smooth(p.z - z));
    let lattice = (x as i32 as u32).wrapping_mul(73_856_093)
        ^ (y as i32 as u32).wrapping_mul(19_349_663)
        ^ (z as i32 as u32).wrapping_mul(83_492_791);
    // Sumar en cada eje antes del XOR también para las esquinas vecinas.
    let corner = |dx: i32, dy: i32, dz: i32| {
        unit(
            seed ^ ((x as i32 + dx) as u32).wrapping_mul(73_856_093)
                ^ ((y as i32 + dy) as u32).wrapping_mul(19_349_663)
                ^ ((z as i32 + dz) as u32).wrapping_mul(83_492_791),
        )
    };
    let low = mix(
        mix(unit(seed ^ lattice), corner(1, 0, 0), fx),
        mix(corner(0, 1, 0), corner(1, 1, 0), fx),
        fy,
    );
    let high = mix(
        mix(corner(0, 0, 1), corner(1, 0, 1), fx),
        mix(corner(0, 1, 1), corner(1, 1, 1), fx),
        fy,
    );
    mix(low, high, fz)
}

pub fn fbm(p: Vec3, seed: u32) -> f32 {
    0.57 * value(p, seed) + 0.29 * value(p * 2.03, seed ^ 73) + 0.14 * value(p * 4.07, seed ^ 199)
}
