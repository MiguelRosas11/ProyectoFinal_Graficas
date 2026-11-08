use crate::{
    math::Vec3,
    mesh::{self, Basis, Mesh, Prop, X, Y, Z},
    noise::{fbm, smooth, unit, value},
    obj::Model,
    shading::{NormalMap, Shader},
};
use std::f32::consts::TAU;

pub const SUN_RADIUS: f32 = 6.2;
pub const SEED: u32 = 241_274;
pub const LODS: [u8; 3] = [2, 3, 4];

#[derive(Clone, Copy)]
pub struct Planet {
    pub name: &'static str,
    pub description: &'static str,
    pub radius: f32,
    pub orbit: f32,
    pub period: f32,
    pub spin: f32,
    pub phase: f32,
    pub tilt: f32,
    pub relief: f32,
    pub shader: Shader,
}

pub const PLANETS: [Planet; 5] = [
    Planet {
        name: "EDEN",
        description: "BOSQUES Y FAROS",
        radius: 5.9,
        orbit: 25.0,
        period: 119.0,
        spin: 27.0,
        phase: 0.24,
        tilt: 0.09,
        relief: 0.68,
        shader: Shader::Garden,
    },
    Planet {
        name: "VESPER",
        description: "ANILLOS Y TORMENTAS",
        radius: 6.8,
        orbit: 42.0,
        period: 166.0,
        spin: 21.0,
        phase: 2.78,
        tilt: -0.18,
        relief: 0.0,
        shader: Shader::Gas,
    },
    Planet {
        name: "NIVAL",
        description: "GLACIARES Y CRISTALES",
        radius: 4.8,
        orbit: 54.0,
        period: 216.0,
        spin: 39.0,
        phase: 1.66,
        tilt: 0.31,
        relief: 0.55,
        shader: Shader::Ice,
    },
    Planet {
        name: "FERRUM",
        description: "CRATERES Y BASALTO",
        radius: 4.6,
        orbit: 34.0,
        period: 146.0,
        spin: 31.0,
        phase: -0.89,
        tilt: 0.32,
        relief: 0.88,
        shader: Shader::Rock,
    },
    Planet {
        name: "LUMEN",
        description: "OCEANOS Y ARCHIPIELAGOS",
        radius: 5.4,
        orbit: 68.0,
        period: 275.0,
        spin: 34.0,
        phase: 0.56,
        tilt: 0.19,
        relief: 0.42,
        shader: Shader::Ocean,
    },
];

#[derive(Clone, Copy)]
pub struct Body {
    pub center: Vec3,
    pub velocity: Vec3,
    pub basis: Basis,
}

pub fn bodies(time: f32) -> [Body; PLANETS.len()] {
    PLANETS.map(|p| {
        let rate = TAU / p.period;
        let (s, c) = (p.phase + time * rate).sin_cos();
        let (st, ct) = p.tilt.sin_cos();
        let orbit_z = Z * ct + Y * st;
        Body {
            center: (X * c + orbit_z * s) * p.orbit,
            velocity: (orbit_z * c - X * s) * (p.orbit * rate),
            basis: Basis::surface(Vec3::new(st * 0.67, ct, st * 0.74).normalized())
                .compose(Basis::yaw(time * TAU / p.spin)),
        }
    })
}

pub fn terrain(p: Planet, direction: Vec3, seed: u32) -> f32 {
    let hill = fbm(direction * 3.7, seed) - 0.44;
    let height = match p.shader {
        Shader::Ocean => (hill - 0.05).max(0.0) * 2.1,
        Shader::Rock => {
            let crater =
                (direction.dot(Vec3::new(0.72, 0.20, -0.66).normalized())).clamp(-1.0, 1.0);
            let basin = smooth((crater - 0.84) / 0.15);
            let rim = 1.0 - ((crater - 0.88) / 0.08).abs().min(1.0);
            hill * 0.65 - basin * 0.77 + rim * 0.45
        }
        Shader::Ice => (hill.abs() * 2.5 - 0.15) + value(direction * 8.0, seed) * 0.18,
        _ => hill * 1.35,
    };
    p.radius + height * p.relief
}

pub struct Instance {
    pub model: usize,
    pub local_position: Vec3,
    pub local_basis: Basis,
    pub scale: f32,
}

pub struct World {
    pub ship: Mesh,
    pub sun: Mesh,
    pub planets: [[Mesh; 3]; PLANETS.len()],
    pub ring: Mesh,
    pub props: [Mesh; 4],
    pub instances: [Vec<Instance>; PLANETS.len()],
    pub normal_map: NormalMap,
    pub stars: Vec<(Vec3, f32)>,
    pub seed: u32,
}

impl World {
    pub fn new(model: &Model, seed: u32) -> Self {
        let planets = std::array::from_fn(|i| {
            LODS.map(|level| mesh::icosphere(level, |n| terrain(PLANETS[i], n, seed_for(seed, i))))
        });
        let instances = std::array::from_fn(|i| populate(i, seed_for(seed, i)));
        let stars = (0..1550u32)
            .map(|i| (direction(i * 6 + 8_010), unit(i * 13 + 2_740)))
            .collect();
        Self {
            ship: Mesh::from_obj(model),
            sun: mesh::icosphere(4, |_| SUN_RADIUS),
            planets,
            ring: mesh::ring(128, 8.4, 13.4),
            props: [Prop::Tree, Prop::Beacon, Prop::Crystal, Prop::Rock].map(mesh::prop),
            instances,
            normal_map: NormalMap::new(),
            stars,
            seed,
        }
    }
}

pub fn seed_for(seed: u32, index: usize) -> u32 {
    seed ^ (index as u32 * 1021 + 37)
}

fn direction(seed: u32) -> Vec3 {
    let y = unit(seed) * 2.0 - 1.0;
    let (s, c) = (unit(seed.wrapping_add(1)) * TAU).sin_cos();
    let horizontal = (1.0 - y * y).sqrt();
    Vec3::new(c * horizontal, y, s * horizontal)
}

fn populate(index: usize, seed: u32) -> Vec<Instance> {
    let p = PLANETS[index];
    let (first, count, threshold, size) = match index {
        0 => (0, 290, 0.48, 0.75),
        2 => (2, 185, 0.47, 0.95),
        3 => (3, 190, 0.46, 0.95),
        4 => (1, 120, 0.63, 0.58),
        _ => return vec![],
    };
    (0..count)
        .filter_map(|i| {
            let key = seed.wrapping_add(i * 17);
            let normal = direction(key);
            let density = fbm(normal * 3.7, seed);
            if density < threshold {
                return None;
            }
            let model = if index == 0 && i % 9 == 0 { 1 } else { first };
            let scale = size * (0.44 + unit(key.wrapping_add(7)) * 0.64);
            let local_position = normal * (terrain(p, normal, seed) - 0.055);
            Some(Instance {
                model,
                local_position,
                scale,
                local_basis: Basis::surface(normal)
                    .compose(Basis::yaw(unit(key.wrapping_add(9)) * TAU)),
            })
        })
        .collect()
}
