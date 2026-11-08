use crate::{
    flight::{SHIP_SCALE, View},
    framebuffer::Framebuffer,
    math::Vec3,
    mesh::{Basis, Mesh, Y, rgb},
    shading::{self, Material, NormalMap, Shader, Surface},
    world::{self, PLANETS, SUN_RADIUS, World},
};

const NEAR: f32 = 0.16;
const FAR: f32 = 240.0;
const TAN_Y: f32 = 0.60;

#[derive(Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    pub forward: Vec3,
    right: Vec3,
    up: Vec3,
}

impl Camera {
    pub fn new(eye: Vec3, forward: Vec3) -> Self {
        let right = forward.cross(Y).normalized();
        Self {
            eye,
            forward,
            right,
            up: right.cross(forward),
        }
    }

    fn vector(self, v: Vec3) -> Vec3 {
        Vec3::new(v.dot(self.right), v.dot(self.up), v.dot(self.forward))
    }

    fn point(self, p: Vec3) -> Vec3 {
        self.vector(p - self.eye)
    }
}

#[derive(Clone, Copy)]
pub struct Projection {
    focal: f32,
    half_w: f32,
    half_h: f32,
}

impl Projection {
    pub fn new(width: usize, height: usize) -> Self {
        let (half_w, half_h) = (width as f32 * 0.5, height as f32 * 0.5);
        Self {
            focal: half_h / TAN_Y,
            half_w,
            half_h,
        }
    }

    pub fn screen(self, p: Vec3) -> (f32, f32) {
        (
            self.half_w + p.x * self.focal / p.z,
            self.half_h - p.y * self.focal / p.z,
        )
    }

    pub fn marker(self, camera: Camera, point: Vec3) -> (f32, f32, bool) {
        let p = camera.point(point);
        let edge_x = self.half_w - 26.0;
        let edge_y = self.half_h - 85.0;
        if p.z > NEAR {
            let (x, y) = self.screen(p);
            if (x - self.half_w).abs() < edge_x && (y - self.half_h).abs() < edge_y {
                return (x, y, true);
            }
        }
        let dx = if p.x.abs() < 0.001 && p.y.abs() < 0.001 {
            1.0
        } else {
            p.x
        };
        let dy = -p.y;
        let divisor = (dx.abs() / edge_x).max(dy.abs() / edge_y);
        (
            self.half_w + dx / divisor,
            self.half_h + dy / divisor,
            false,
        )
    }

    fn outside(self, p: Vec3, radius: f32) -> bool {
        let tan_x = self.half_w / self.focal;
        let x_slack = radius * (1.0 + tan_x * tan_x).sqrt();
        let y_slack = radius * (1.0 + TAN_Y * TAN_Y).sqrt();
        p.z + radius < NEAR
            || p.z - radius > FAR
            || p.x.abs() > p.z * tan_x + x_slack
            || p.y.abs() > p.z * TAN_Y + y_slack
    }
}

#[derive(Default)]
pub struct Stats {
    pub submitted: u32,
    pub rasterized: u32,
    pub culled: u32,
    pub lod: [u32; 3],
}

pub struct Options {
    pub culling: bool,
    pub lod: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            culling: true,
            lod: true,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Vertex {
    view: Vec3,
    surface: Vec3,
    normal: Vec3,
    world: Vec3,
}

impl Vertex {
    fn lerp(self, b: Self, t: f32) -> Self {
        Self {
            view: self.view.lerp(b.view, t),
            surface: self.surface.lerp(b.surface, t),
            normal: self.normal.lerp(b.normal, t),
            world: self.world.lerp(b.world, t),
        }
    }
}

fn clip_near(tri: [Vertex; 3]) -> ([Vertex; 4], usize) {
    let mut points = [Vertex::default(); 4];
    let mut count = 0;
    let mut a = tri[2];
    for b in tri {
        let a_inside = a.view.z >= NEAR;
        let b_inside = b.view.z >= NEAR;
        if a_inside != b_inside {
            let t = (NEAR - a.view.z) / (b.view.z - a.view.z);
            points[count] = a.lerp(b, t);
            points[count].view.z = NEAR;
            count += 1;
        }
        if b_inside {
            points[count] = b;
            count += 1;
        }
        a = b;
    }
    (points, count)
}

#[derive(Clone, Copy)]
struct Screen {
    x: f32,
    y: f32,
    inv_z: f32,
    vertex: Vertex,
}

fn pack(color: Vec3) -> u32 {
    let channel = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    channel(color.x) << 16 | channel(color.y) << 8 | channel(color.z)
}

fn edge(a: Screen, b: Screen, x: f32, y: f32) -> f32 {
    (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
}

struct Frame<'a> {
    buffer: &'a mut Framebuffer,
    normal_map: &'a NormalMap,
    projection: Projection,
    camera: Camera,
    options: &'a Options,
    stats: Stats,
    vertices: Vec<Vertex>,
}

struct Draw<'a> {
    mesh: &'a Mesh,
    position: Vec3,
    scale: f32,
    material: Material,
}

impl Frame<'_> {
    fn draw(&mut self, draw: Draw<'_>) {
        let radius = draw.mesh.radius * draw.scale;
        if self.options.culling
            && self
                .projection
                .outside(self.camera.point(draw.position), radius)
        {
            self.stats.culled += 1;
            return;
        }
        self.stats.submitted += draw.mesh.faces.len() as u32;
        self.vertices.clear();
        self.vertices
            .extend(draw.mesh.vertices.iter().map(|vertex| {
                let world = draw.position + draw.material.basis.apply(vertex.position * draw.scale);
                Vertex {
                    view: self.camera.point(world),
                    world,
                    normal: draw.material.basis.apply(vertex.normal),
                    surface: vertex.position,
                }
            }));
        for face in &draw.mesh.faces {
            let tri = face.indices.map(|i| self.vertices[i]);
            if tri.iter().all(|v| v.view.z >= NEAR) {
                self.triangle(tri, &draw.material, face.color);
            } else if tri.iter().any(|v| v.view.z >= NEAR) {
                let (points, count) = clip_near(tri);
                for i in 1..count.saturating_sub(1) {
                    self.triangle(
                        [points[0], points[i], points[i + 1]],
                        &draw.material,
                        face.color,
                    );
                }
            }
        }
    }

    fn triangle(&mut self, points: [Vertex; 3], material: &Material, base: Vec3) {
        let [a, b, c] = points.map(|vertex| {
            let (x, y) = self.projection.screen(vertex.view);
            Screen {
                x,
                y,
                inv_z: vertex.view.z.recip(),
                vertex,
            }
        });
        let area = edge(a, b, c.x, c.y);
        if area.abs() < 1e-5 || (area > 0.0 && material.shader != Shader::Ring) {
            return;
        }
        let left = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
        let top = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
        let right =
            a.x.max(b.x)
                .max(c.x)
                .ceil()
                .min(self.buffer.width as f32 - 1.0) as i32;
        let bottom =
            a.y.max(b.y)
                .max(c.y)
                .ceil()
                .min(self.buffer.height as f32 - 1.0) as i32;
        if left > right || top > bottom {
            return;
        }
        self.stats.rasterized += 1;

        let inv_area = area.recip();
        let du = (b.y - c.y) * inv_area;
        let dv = (c.y - a.y) * inv_area;
        let x0 = left as f32 + 0.5;
        for y in top..=bottom {
            let y0 = y as f32 + 0.5;
            let mut u = edge(b, c, x0, y0) * inv_area;
            let mut v = edge(c, a, x0, y0) * inv_area;
            let row = y as usize * self.buffer.width;
            for x in left..=right {
                let w = 1.0 - u - v;
                if u >= -1e-5 && v >= -1e-5 && w >= -1e-5 {
                    let (au, bv, cw) = (u * a.inv_z, v * b.inv_z, w * c.inv_z);
                    let inv_z = au + bv + cw;
                    let index = row + x as usize;
                    if inv_z > self.buffer.depth[index] && inv_z >= FAR.recip() {
                        let correction = inv_z.recip();
                        let interp =
                            |x: Vec3, y: Vec3, z: Vec3| (x * au + y * bv + z * cw) * correction;
                        let surface = Surface {
                            local: interp(a.vertex.surface, b.vertex.surface, c.vertex.surface),
                            world: interp(a.vertex.world, b.vertex.world, c.vertex.world),
                            normal: interp(a.vertex.normal, b.vertex.normal, c.vertex.normal),
                        };
                        if let Some(color) =
                            shading::shade(material, surface, base, self.normal_map)
                        {
                            self.buffer.depth[index] = inv_z;
                            self.buffer.pixels[index] = pack(color);
                        }
                    }
                }
                u += du;
                v += dv;
            }
        }
    }

    fn sky(&mut self, world: &World) {
        self.buffer.depth.fill(0.0);
        let sun = self.camera.point(Vec3::default());
        let (sx, sy) = self.projection.screen(sun);
        let sun_visible = sun.z > SUN_RADIUS;
        let halo_radius = if sun_visible {
            (self.projection.focal * SUN_RADIUS / sun.z).max(1.0)
        } else {
            1.0
        };
        for y in 0..self.buffer.height {
            let h = y as f32 / self.buffer.height as f32;
            let band = (1.0 - (h * 2.0 - 0.85).abs()).max(0.0);
            let row_color = rgb(4, 7, 17).lerp(rgb(12, 20, 38), band * 0.63);
            let halo_y = ((y as f32 - sy) / halo_radius).powi(2);
            let row = y * self.buffer.width;
            for x in 0..self.buffer.width {
                let color = if sun_visible {
                    let d2 = ((x as f32 - sx) / halo_radius).powi(2) + halo_y;
                    let glow = (1.0 / (1.0 + d2 * d2 * 0.38) - 0.007).max(0.0);
                    row_color + rgb(214, 92, 31) * (0.42 * glow)
                } else {
                    row_color
                };
                self.buffer.pixels[row + x] = pack(color);
            }
        }
        for &(dir, strength) in &world.stars {
            let p = self.camera.vector(dir);
            if p.z <= NEAR {
                continue;
            }
            let (x, y) = self.projection.screen(p);
            let (x, y) = (x.round() as i32, y.round() as i32);
            let tint = rgb(112, 153, 212).lerp(rgb(249, 222, 195), strength);
            let radius = if strength > 0.96 { 1 } else { 0 };
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (px, py) = (x + dx, y + dy);
                    if px >= 0
                        && py >= 0
                        && px < self.buffer.width as i32
                        && py < self.buffer.height as i32
                    {
                        let falloff = if dx == 0 && dy == 0 { 1.0 } else { 0.22 };
                        self.buffer.pixels[py as usize * self.buffer.width + px as usize] =
                            pack(tint * ((0.25 + strength * strength * 0.70) * falloff));
                    }
                }
            }
        }
    }
}

pub fn draw_world(
    buffer: &mut Framebuffer,
    world: &World,
    view: &View,
    time: f32,
    options: &Options,
) -> Stats {
    let projection = Projection::new(buffer.width, buffer.height);
    let camera = view.camera;
    let mut frame = Frame {
        buffer,
        normal_map: &world.normal_map,
        projection,
        camera,
        options,
        stats: Stats::default(),
        vertices: Vec::new(),
    };
    frame.sky(world);
    let material = |shader, basis, seed| Material {
        shader,
        basis,
        camera: camera.eye,
        time,
        seed,
    };
    frame.draw(Draw {
        mesh: &world.ship,
        position: view.ship,
        scale: SHIP_SCALE,
        material: material(Shader::Model, view.basis, 0),
    });
    frame.draw(Draw {
        mesh: &world.sun,
        position: Vec3::default(),
        scale: 1.0,
        material: material(Shader::Sun, Basis::IDENTITY, 0),
    });
    for (i, body) in world::bodies(time).into_iter().enumerate() {
        let planet = PLANETS[i];
        let distance = (camera.eye - body.center).length();
        let projected_radius = projection.focal * planet.radius / distance.max(0.1);
        let lod = if !options.lod || projected_radius >= 92.0 {
            2
        } else if projected_radius >= 39.0 {
            1
        } else {
            0
        };
        if options.culling
            && projection.outside(
                camera.point(body.center),
                if i == 1 {
                    world.ring.radius
                } else {
                    world.planets[i][2].radius + 1.8
                },
            )
        {
            frame.stats.culled += 1 + (i == 1) as u32;
            if projected_radius > 32.0 {
                frame.stats.culled += world.instances[i].len() as u32;
            }
            continue;
        }
        frame.stats.lod[lod] += 1;
        frame.draw(Draw {
            mesh: &world.planets[i][lod],
            position: body.center,
            scale: 1.0,
            material: material(planet.shader, body.basis, world::seed_for(world.seed, i)),
        });
        if i == 1 {
            frame.draw(Draw {
                mesh: &world.ring,
                position: body.center,
                scale: 1.0,
                material: material(Shader::Ring, body.basis, 0),
            });
        }
        if projected_radius > 32.0 {
            for instance in &world.instances[i] {
                let position = body.center + body.basis.apply(instance.local_position);
                let basis = body.basis.compose(instance.local_basis);
                frame.draw(Draw {
                    mesh: &world.props[instance.model],
                    position,
                    scale: instance.scale,
                    material: material(Shader::Model, basis, 0),
                });
            }
        }
    }
    frame.stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_clipping_interpolates_attributes_instead_of_dropping_visible_faces() {
        let vertex = |x, z| Vertex {
            view: Vec3::new(x, 0.0, z),
            surface: Vec3::new(x * 2.0, 0.0, z * 3.0),
            ..Vertex::default()
        };
        let (polygon, len) = clip_near([vertex(0.0, 0.0), vertex(-1.0, 1.0), vertex(1.0, 1.0)]);
        assert_eq!(len, 4);
        for point in &polygon[..len] {
            assert!(point.view.z >= NEAR);
            assert!((point.surface.z - point.view.z * 3.0).abs() < 1e-6);
        }
        assert_eq!(clip_near([vertex(0.0, -1.0); 3]).1, 0);
    }
}
