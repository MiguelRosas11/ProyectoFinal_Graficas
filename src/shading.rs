use crate::{
    math::Vec3,
    mesh::{Basis, Y, rgb},
    noise::{fbm, mix, smooth, value},
};
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq)]
pub enum Shader {
    Sun,
    Garden,
    Gas,
    Ice,
    Rock,
    Ocean,
    Ring,
    Model,
}

pub struct Surface {
    pub local: Vec3,
    pub world: Vec3,
    pub normal: Vec3,
}

pub struct Material {
    pub shader: Shader,
    pub basis: Basis,
    pub camera: Vec3,
    pub time: f32,
    pub seed: u32,
}

const MAP_SIZE: usize = 64;

pub struct NormalMap([Vec3; MAP_SIZE * MAP_SIZE]);

impl NormalMap {
    pub fn new() -> Self {
        let height = |x: i32, y: i32| {
            let u = TAU * x as f32 / MAP_SIZE as f32;
            let v = TAU * y as f32 / MAP_SIZE as f32;
            (4.0 * u + 2.0 * v + (v * 3.0).sin()).sin() * 0.68 + (7.0 * u - 5.0 * v).cos() * 0.32
        };
        Self(std::array::from_fn(|i| {
            let (x, y) = ((i % MAP_SIZE) as i32, (i / MAP_SIZE) as i32);
            Vec3::new(
                (height(x - 1, y) - height(x + 1, y)) * 0.8,
                (height(x, y - 1) - height(x, y + 1)) * 0.8,
                1.0,
            )
            .normalized()
        }))
    }

    fn sample(&self, u: f32, v: f32) -> Vec3 {
        let x = u.rem_euclid(1.0) * MAP_SIZE as f32;
        let y = v.rem_euclid(1.0) * MAP_SIZE as f32;
        let (i, j) = (x as usize, y as usize);
        let at = |i: usize, j: usize| self.0[(j % MAP_SIZE) * MAP_SIZE + i % MAP_SIZE];
        at(i, j)
            .lerp(at(i + 1, j), x.fract())
            .lerp(at(i, j + 1).lerp(at(i + 1, j + 1), x.fract()), y.fract())
    }
}

fn lit(base: Vec3, n: Vec3, light: Vec3, view: Vec3, ambient: f32) -> Vec3 {
    let diffuse = n.dot(light).max(0.0);
    let rim = (1.0 - n.dot(view).abs()).powi(3) * (0.18 + diffuse * 0.22);
    base * (ambient + diffuse * (1.07 - ambient)) + rgb(83, 149, 218) * rim
}

pub fn shade(m: &Material, s: Surface, base: Vec3, map: &NormalMap) -> Option<Vec3> {
    let n = s.normal.normalized();
    let light = (Vec3::default() - s.world).normalized();
    let view = (m.camera - s.world).normalized();
    let q = s.local.normalized();
    let out = match m.shader {
        Shader::Sun => {
            let flow = fbm(q * 4.8 + Y * (m.time * 0.29), 319);
            let granules = value(q * 25.0 + Y * (m.time * -0.47), 42);
            let glow = smooth((flow * 0.83 + granules * 0.17 - 0.26) * 2.6);
            let edge = (1.0 - n.dot(view).max(0.0)).powi(3);
            rgb(224, 82, 28).lerp(rgb(255, 246, 161), glow) * (1.08 - 0.23 * edge)
        }
        Shader::Garden => {
            let field = fbm(q * 3.7, m.seed);
            let grain = value(q * 44.0, m.seed ^ 592);
            let meadow = rgb(74, 175, 113).lerp(rgb(186, 205, 110), grain * 0.57);
            let soil = rgb(22, 105, 101).lerp(meadow, smooth((field - 0.36) * 5.1));
            let caps = smooth((q.y.abs() - 0.78) * 6.0 + (field - 0.5) * 0.8);
            lit(soil.lerp(rgb(191, 226, 216), caps), n, light, view, 0.30)
        }
        Shader::Gas => {
            let storm = value(q * 4.0 + Y * (m.time * 0.037), 87);
            let spot = q - Vec3::new(0.42, -0.20, -0.88);
            let vortex = smooth(1.0 - spot.length() * 2.35);
            let bands = (q.y * 26.0 + 3.4 * storm + 6.0 * vortex).sin() * 0.5 + 0.5;
            let clouds = rgb(111, 60, 119)
                .lerp(rgb(251, 171, 122), bands)
                .lerp(rgb(242, 109, 94), vortex * 0.78);
            lit(clouds, n, light, view, 0.31)
        }
        Shader::Ice => {
            let u = q.z.atan2(q.x) / TAU;
            let v = q.y.clamp(-1.0, 1.0).asin() / std::f32::consts::PI;
            let tangent = m.basis.apply(Vec3::new(-q.z, 0.0, q.x));
            let tangent = (tangent - n * n.dot(tangent)).normalized();
            let bitangent = tangent.cross(n);
            let bump = map.sample(u * 7.0, v * 4.0);
            let perturbed = (tangent * bump.x + bitangent * bump.y + n * bump.z).normalized();
            let frost = fbm(q * 5.5, m.seed);
            let ice = rgb(24, 104, 175).lerp(rgb(207, 242, 250), smooth(frost * 1.7 - 0.27));
            let glint = perturbed.dot((light + view).normalized()).max(0.0).powi(36);
            lit(ice, perturbed, light, view, 0.38) + rgb(154, 216, 255) * (glint * 0.55)
        }
        Shader::Rock => {
            let fracture = (q.y * 39.0 + value(q * 8.0, m.seed) * 6.0).sin().abs();
            let ore = fbm(q * 7.0, m.seed ^ 81);
            let rust = rgb(72, 45, 66).lerp(rgb(203, 105, 72), ore);
            let veins = smooth((0.19 - fracture) * 5.3);
            lit(
                rust.lerp(rgb(252, 159, 99), veins * 0.78),
                n,
                light,
                view,
                0.36,
            )
        }
        Shader::Ocean => {
            let land = fbm(q * 3.7, m.seed);
            let ripple = (q.x * 86.0 + q.z * 37.0 + m.time * 3.8).sin() * 0.5 + 0.5;
            let water = rgb(22, 59, 145).lerp(rgb(42, 169, 185), land * 0.74 + ripple * 0.13);
            let island = rgb(244, 218, 145).lerp(rgb(63, 152, 91), smooth((land - 0.54) * 8.0));
            let coast = smooth((land - 0.473) * 37.0);
            let glint = n.dot((light + view).normalized()).max(0.0).powi(54);
            lit(water.lerp(island, coast), n, light, view, 0.31)
                + rgb(145, 215, 255) * (glint * (1.0 - coast) * mix(0.34, 0.81, ripple))
        }
        Shader::Ring => {
            let r = s.local.length();
            if (10.25..10.70).contains(&r) || (11.55..11.77).contains(&r) {
                return None;
            }
            let bands = 0.5 + (r * 23.0).sin() * 0.24 + (r * 6.7).cos() * 0.26;
            rgb(89, 69, 105).lerp(rgb(230, 184, 156), bands) * (0.67 + n.dot(light).abs() * 0.33)
        }
        Shader::Model => lit(base, n, light, view, 0.54),
    };
    Some(out)
}
