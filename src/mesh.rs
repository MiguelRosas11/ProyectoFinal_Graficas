use crate::{math::Vec3, obj::Model};
use std::{collections::HashMap, f32::consts::TAU};

pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

#[derive(Clone, Copy)]
pub struct Basis {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Basis {
    pub const IDENTITY: Self = Self { x: X, y: Y, z: Z };

    pub fn yaw(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            x: Vec3::new(c, 0.0, -s),
            y: Y,
            z: Vec3::new(s, 0.0, c),
        }
    }

    pub fn surface(normal: Vec3) -> Self {
        let helper = if normal.y.abs() > 0.92 { X } else { Y };
        let x = helper.cross(normal).normalized();
        Self {
            x,
            y: normal,
            z: x.cross(normal),
        }
    }

    pub fn ship(forward: Vec3, roll: f32) -> Self {
        let right = forward.cross(Y).normalized();
        let up = right.cross(forward);
        let (s, c) = roll.sin_cos();
        Self {
            // El OBJ de Aster tiene el morro hacia +Z. Giramos también X para preservar la orientación.
            x: (right * c + up * s) * -1.0,
            y: up * c - right * s,
            z: forward,
        }
    }

    pub fn apply(self, v: Vec3) -> Vec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    pub fn compose(self, other: Self) -> Self {
        Self {
            x: self.apply(other.x),
            y: self.apply(other.y),
            z: self.apply(other.z),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
}

pub struct Face {
    pub indices: [usize; 3],
    pub color: Vec3,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Face>,
    pub radius: f32,
}

impl Mesh {
    fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            faces: Vec::new(),
            radius: 0.0,
        }
    }

    fn triangle(&mut self, points: [Vec3; 3], color: Vec3) {
        let start = self.vertices.len();
        let [a, b, c] = points;
        let normal = (b - a).cross(c - a).normalized();
        for position in points {
            self.radius = self.radius.max(position.length());
            self.vertices.push(Vertex { position, normal });
        }
        self.faces.push(Face {
            indices: [start, start + 1, start + 2],
            color,
        });
    }

    pub fn from_obj(model: &Model) -> Self {
        let mut mesh = Self::empty();
        for face in &model.faces {
            let points = face.indices.map(|i| model.vertices[i]);
            let crate::color::Color(r, g, b) = model
                .materials
                .get(&face.material)
                .copied()
                .unwrap_or(crate::color::Color(200, 220, 240));
            mesh.triangle(points, rgb(r, g, b));
        }
        mesh
    }
}

pub fn rgb(r: u8, g: u8, b: u8) -> Vec3 {
    Vec3::new(r as f32, g as f32, b as f32) * (1.0 / 255.0)
}

pub fn icosphere(level: u8, radius_at: impl Fn(Vec3) -> f32) -> Mesh {
    let g = (1.0 + 5.0_f32.sqrt()) * 0.5;
    let mut points = [
        [-1., g, 0.],
        [1., g, 0.],
        [-1., -g, 0.],
        [1., -g, 0.],
        [0., -1., g],
        [0., 1., g],
        [0., -1., -g],
        [0., 1., -g],
        [g, 0., -1.],
        [g, 0., 1.],
        [-g, 0., -1.],
        [-g, 0., 1.],
    ]
    .map(|[x, y, z]| Vec3::new(x, y, z).normalized())
    .to_vec();
    let mut faces = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..level {
        let mut edges = HashMap::new();
        let mut split = Vec::with_capacity(faces.len() * 4);
        let mut midpoint = |a: usize, b: usize| {
            *edges.entry((a.min(b), a.max(b))).or_insert_with(|| {
                points.push((points[a] + points[b]).normalized());
                points.len() - 1
            })
        };
        for [a, b, c] in faces {
            let (ab, bc, ca) = (midpoint(a, b), midpoint(b, c), midpoint(c, a));
            split.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = split;
    }

    // Primero subdividimos una esfera unitaria. Después deformamos la geometría.
    let mut mesh = Mesh::empty();
    for point in points {
        let radius = radius_at(point);
        mesh.radius = mesh.radius.max(radius);
        mesh.vertices.push(Vertex {
            position: point * radius,
            normal: Vec3::default(),
        });
    }
    for indices in faces {
        let [a, b, c] = indices.map(|i| mesh.vertices[i].position);
        let normal = (b - a).cross(c - a);
        for i in indices {
            mesh.vertices[i].normal = mesh.vertices[i].normal + normal;
        }
        mesh.faces.push(Face {
            indices,
            color: rgb(255, 255, 255),
        });
    }
    for v in &mut mesh.vertices {
        v.normal = v.normal.normalized();
    }
    mesh
}

pub fn ring(segments: usize, inner: f32, outer: f32) -> Mesh {
    let mut mesh = Mesh::empty();
    for i in 0..segments {
        let edge = |j: usize| {
            let (s, c) = (TAU * j as f32 / segments as f32).sin_cos();
            Vec3::new(c, 0.0, s)
        };
        let (a, b) = (edge(i), edge(i + 1));
        for points in [
            [a * inner, b * inner, b * outer],
            [a * inner, b * outer, a * outer],
        ] {
            mesh.triangle(points, rgb(255, 255, 255));
        }
    }
    mesh
}

#[derive(Clone, Copy)]
pub enum Prop {
    Tree,
    Beacon,
    Crystal,
    Rock,
}

fn column(mesh: &mut Mesh, center: Vec3, shape: &[(f32, f32, f32)], sides: usize, color: Vec3) {
    let point = |(height, radius, offset): (f32, f32, f32), i: usize| {
        let (s, c) = (i as f32 / sides as f32 * TAU + offset).sin_cos();
        center + Vec3::new(c * radius, height, s * radius)
    };
    for pair in shape.windows(2) {
        for i in 0..sides {
            let (a, b) = (point(pair[0], i), point(pair[0], i + 1));
            let (c, d) = (point(pair[1], i), point(pair[1], i + 1));
            let shade = 0.72 + 0.28 * (i % 3) as f32 * 0.5;
            mesh.triangle([a, c, b], color * shade);
            mesh.triangle([b, c, d], color * shade);
        }
    }
}

pub fn prop(kind: Prop) -> Mesh {
    let mut mesh = Mesh::empty();
    let mut stack = |shape: &[(f32, f32, f32)], sides, color| {
        column(&mut mesh, Vec3::default(), shape, sides, color);
    };
    match kind {
        Prop::Tree => {
            stack(
                &[(-0.27, 0.12, 0.), (1.04, 0.08, 0.), (1.07, 0., 0.)],
                5,
                rgb(115, 63, 55),
            );
            stack(
                &[(0.40, 0., 0.), (0.50, 0.47, 0.), (1.32, 0., 0.)],
                6,
                rgb(35, 184, 140),
            );
            stack(
                &[(0.73, 0., 0.), (0.83, 0.35, 0.24), (1.63, 0., 0.)],
                6,
                rgb(107, 230, 159),
            );
        }
        Prop::Beacon => {
            stack(
                &[
                    (-0.30, 0.58, 0.),
                    (0.18, 0.58, 0.),
                    (0.38, 0.30, 0.),
                    (1.28, 0.25, 0.),
                    (1.45, 0.47, 0.),
                    (1.54, 0., 0.),
                ],
                6,
                rgb(214, 227, 240),
            );
            stack(
                &[(1.49, 0.25, 0.), (1.82, 0.17, 0.2), (1.98, 0., 0.)],
                6,
                rgb(255, 123, 56),
            );
        }
        Prop::Crystal => {
            for (x, z, size) in [(0., 0., 1.0), (0.30, 0.16, 0.64), (-0.25, -0.20, 0.73)] {
                column(
                    &mut mesh,
                    Vec3::new(x, 0., z),
                    &[
                        (-0.25, 0.22, 0.),
                        (0.82 * size, 0.23, 0.18),
                        (1.28 * size, 0., 0.),
                    ],
                    5,
                    rgb(98, 211, 255),
                );
            }
        }
        Prop::Rock => {
            stack(
                &[
                    (-0.26, 0.48, 0.),
                    (0.25, 0.59, 0.19),
                    (0.64, 0.34, 0.41),
                    (0.72, 0., 0.),
                ],
                5,
                rgb(123, 64, 59),
            );
        }
    }
    mesh
}
