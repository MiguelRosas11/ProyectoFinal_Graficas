use crate::{
    math::Vec3,
    mesh::{Basis, Y, Z},
    noise::{mix, smooth},
    render::Camera,
    world::{self, Body, PLANETS, SUN_RADIUS},
};
use std::f32::consts::TAU;

pub const STEP: f32 = 1.0 / 120.0;
pub const SHIP_SCALE: f32 = 0.26;
pub const DEMO_LENGTH: f32 = 27.0;

#[derive(Default)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub lift: f32,
    pub turn: f32,
    pub pitch: f32,
    pub boost: bool,
    pub brake: bool,
}

pub struct Flight {
    pub time: f32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub target: usize,
    pub local: bool,
    pub gravity: bool,
    pub contact: f32,
    yaw: f32,
    pitch: f32,
    yaw_speed: f32,
    pitch_speed: f32,
    roll: f32,
}

fn angles(forward: Vec3) -> (f32, f32) {
    (
        forward.x.atan2(-forward.z),
        forward.y.clamp(-1.0, 1.0).asin(),
    )
}

fn direction(yaw: f32, pitch: f32) -> Vec3 {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    Vec3::new(sy * cp, sp, -cy * cp)
}

fn orbit_axes(radial: Vec3) -> (Vec3, Vec3) {
    let reference = if radial.y.abs() > 0.97 { Z } else { Y };
    let tangent = reference.cross(radial).normalized();
    (tangent, radial.cross(tangent))
}

fn damp(rate: f32) -> f32 {
    (-rate * STEP).exp()
}

impl Flight {
    pub fn new() -> Self {
        let eden = world::bodies(0.0)[0].center;
        let position = eden + Vec3::new(9.0, 7.0, -32.0);
        let (yaw, pitch) = angles((eden - position).normalized());
        Self {
            time: 0.0,
            position,
            velocity: Vec3::default(),
            target: 0,
            local: false,
            gravity: true,
            contact: 0.0,
            yaw,
            pitch,
            yaw_speed: 0.0,
            pitch_speed: 0.0,
            roll: 0.0,
        }
    }

    pub fn select_next(&mut self) {
        self.leave_local();
        self.target = (self.target + 1) % PLANETS.len();
    }

    pub fn aim_at_target(&mut self) {
        let center = world::bodies(self.time)[self.target].center;
        (self.yaw, self.pitch) = if self.local {
            (0.0, 0.0)
        } else {
            angles((center - self.position).normalized())
        };
        self.yaw_speed = 0.0;
        self.pitch_speed = 0.0;
    }

    pub fn toggle_local(&mut self) {
        if self.local {
            self.leave_local();
            return;
        }
        let bodies = world::bodies(self.time);
        let clearance = |i: usize| (self.position - bodies[i].center).length() - PLANETS[i].radius;
        if clearance(self.target) > 18.0 {
            let Some(nearest) = (0..PLANETS.len())
                .filter(|&i| clearance(i) <= 18.0)
                .min_by(|&a, &b| clearance(a).total_cmp(&clearance(b)))
            else {
                return;
            };
            self.target = nearest;
        }
        self.local = true;
        // Conservar la vista al cambiar al marco de referencia del planeta.
        let (base_yaw, base_pitch) =
            angles((bodies[self.target].center - self.position).normalized());
        self.yaw = (self.yaw - base_yaw + TAU * 0.5).rem_euclid(TAU) - TAU * 0.5;
        self.pitch -= base_pitch;
    }

    fn leave_local(&mut self) {
        if self.local {
            let body = world::bodies(self.time)[self.target];
            (self.yaw, self.pitch) = angles(self.forward(body));
            self.local = false;
        }
    }

    fn forward(&self, body: Body) -> Vec3 {
        let (yaw, pitch) = if self.local {
            let (yaw, pitch) = angles((body.center - self.position).normalized());
            (yaw + self.yaw, pitch + self.pitch)
        } else {
            (self.yaw, self.pitch)
        };
        direction(yaw, pitch.clamp(-1.48, 1.48))
    }

    pub fn step(&mut self, input: &Input, seed: u32) {
        let body = world::bodies(self.time)[self.target];
        self.yaw_speed = mix(input.turn * 1.68, self.yaw_speed, damp(10.0));
        self.pitch_speed = mix(input.pitch * 1.32, self.pitch_speed, damp(10.0));
        self.yaw = (self.yaw + self.yaw_speed * STEP + TAU * 0.5).rem_euclid(TAU) - TAU * 0.5;
        let limit = if self.local { 2.96 } else { 1.42 };
        self.pitch = (self.pitch + self.pitch_speed * STEP).clamp(-limit, limit);
        self.roll = mix(
            input.turn * 0.34 - input.strafe * 0.16,
            self.roll,
            damp(6.5),
        );
        let forward = self.forward(body);
        let radial = (self.position - body.center).normalized();
        let (tangent, orbit_up) = orbit_axes(radial);
        let (desired, frame_velocity, response) = if self.local {
            let radial_speed =
                if (self.position - body.center).length() < PLANETS[self.target].radius + 4.0 {
                    2.2
                } else {
                    5.0
                };
            (
                (radial * (-input.forward * radial_speed)
                    + tangent * (input.strafe * 4.8)
                    + orbit_up * (input.lift * 3.2)),
                body.velocity,
                3.5,
            )
        } else {
            let right = forward.cross(Y).normalized();
            let speed = if input.boost { 31.0 } else { 14.0 };
            (
                (forward * input.forward + right * input.strafe + Y * input.lift).normalized()
                    * speed,
                Vec3::default(),
                1.8,
            )
        };
        let desired = if input.brake {
            Vec3::default()
        } else {
            desired
        };
        let alpha = 1.0 - damp(if input.brake { 7.0 } else { response });
        self.velocity = self.velocity.lerp(frame_velocity + desired, alpha);

        if self.gravity {
            for (i, body) in world::bodies(self.time).iter().enumerate() {
                let toward = body.center - self.position;
                let d2 = toward.dot(toward);
                let radius = PLANETS[i].radius;
                let acceleration = radius * radius * 3.4 / d2.max(radius * radius);
                self.velocity = self.velocity + toward.normalized() * (acceleration * STEP);
            }
        }

        self.position = self.position + self.velocity * STEP;
        self.time += STEP;
        self.contact = (self.contact - STEP).max(0.0);
        for (i, body) in world::bodies(self.time).iter().enumerate() {
            let radial = (self.position - body.center).normalized();
            let local_dir = Vec3::new(
                radial.dot(body.basis.x),
                radial.dot(body.basis.y),
                radial.dot(body.basis.z),
            );
            let ground = world::terrain(PLANETS[i], local_dir, world::seed_for(seed, i));
            self.collide(body.center, body.velocity, ground + 1.18);
        }
        self.collide(Vec3::default(), Vec3::default(), SUN_RADIUS + 1.3);
        if self.local && (self.position - body.center).length() > PLANETS[self.target].radius + 24.0
        {
            self.leave_local();
        }
    }

    fn collide(&mut self, center: Vec3, center_velocity: Vec3, radius: f32) {
        let delta = self.position - center;
        let distance = delta.length();
        if distance < radius {
            let normal = if distance > 0.001 {
                delta * (1.0 / distance)
            } else {
                Y
            };
            self.position = center + normal * radius;
            let inward = (self.velocity - center_velocity).dot(normal).min(0.0);
            self.velocity = self.velocity - normal * inward;
            self.contact = 0.7;
        }
    }

    pub fn view(&self) -> View {
        let body = world::bodies(self.time)[self.target];
        let forward = self.forward(body);
        make_view(
            self.position,
            forward,
            forward,
            self.roll,
            self.time,
            self.target,
        )
    }
}

pub struct View {
    pub camera: Camera,
    pub ship: Vec3,
    pub basis: Basis,
    pub distance: f32,
    pub stage: Option<(&'static str, &'static str)>,
}

fn make_view(ship: Vec3, look: Vec3, nose: Vec3, roll: f32, time: f32, target: usize) -> View {
    let right = look.cross(Y).normalized();
    let up = right.cross(look);
    let offset = look * -4.25 + up * 1.16;
    let bodies = world::bodies(time);
    // Si la cámara va a meterse en un cuerpo, acortamos el brazo de seguimiento.
    let mut arm: f32 = 1.0;
    for (center, radius) in bodies
        .iter()
        .zip(PLANETS)
        .map(|(body, p)| (body.center, p.radius + p.relief + 0.12))
        .chain([(Vec3::default(), SUN_RADIUS + 0.12)])
    {
        let start = ship - center;
        let a = offset.dot(offset);
        let b = start.dot(offset);
        let c = start.dot(start) - radius * radius;
        let discriminant = b * b - a * c;
        if c > 0.0 && b < 0.0 && discriminant > 0.0 {
            arm = arm.min(((-b - discriminant.sqrt()) / a - 0.045).clamp(0.11, 1.0));
        }
    }
    let camera = Camera::new(ship + offset * arm, (look - up * 0.047).normalized());
    View {
        camera,
        ship,
        basis: Basis::ship(nose, roll),
        stage: None,
        distance: ((ship - bodies[target].center).length() - PLANETS[target].radius).max(0.0),
    }
}

pub fn demo(time: f32) -> View {
    let center = world::bodies(time)[0].center;
    let approach = smooth((time - 2.0) / 6.0);
    let loop_t = ((time - 8.0) / 13.8).clamp(0.0, 1.0);
    let departure = smooth((time - 21.8) / 5.0);
    let angle = 1.44 + TAU * smooth(loop_t) - 0.75 * departure;
    let (s, c) = angle.sin_cos();
    let radius = mix(37.0, 8.65, approach) + departure * 33.0;
    let altitude = mix(0.32, 0.30 + (TAU * loop_t).sin() * 0.18, approach) + departure * 0.15;
    let offset = Vec3::new(c, altitude, -s).normalized() * radius;
    let ship = center + offset;
    let inward = (center - ship).normalized();
    let tangent = Vec3::new(-s, 0.0, -c);
    let look = (inward + tangent * (0.15 - 0.28 * departure)).normalized();
    let nose = (look + tangent * (0.35 * approach * (1.0 - departure))).normalized();
    let mut view = make_view(ship, look, nose, -0.17 * approach, time, 0);
    view.stage = Some(if time < 2.0 {
        ("01 / VUELO ESPACIAL", "SISTEMA ESTELAR HELIOS")
    } else if time < 8.0 {
        ("02 / ACERCAMIENTO", "DESTINO EDEN")
    } else if time < 21.8 {
        ("03 / VUELTA COMPLETA", "BOSQUES Y FAROS EN SUPERFICIE")
    } else {
        ("04 / ALEJAMIENTO", "REGRESO A ESPACIO ABIERTO")
    });
    view
}

pub fn inspection(ship: Vec3, look: Vec3, time: f32, target: usize) -> View {
    make_view(ship, look, look, 0.12, time, target)
}
