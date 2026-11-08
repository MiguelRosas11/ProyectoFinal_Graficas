use crate::{
    flight::{self, Flight, Input, STEP},
    framebuffer::Framebuffer,
    math::Vec3,
    mesh,
    obj::Model,
    render::{self, Options},
    world::{self, PLANETS, SEED, World},
};

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("modelos/nave_aster.obj");
    let model = Model::load(&path).expect("El repositorio incluye OBJ y MTL");
    assert_eq!(model.faces.len(), 444);
    World::new(&model, SEED)
}

#[test]
fn subdivision_increases_geometry_before_terrain_displacement() {
    let flat = mesh::icosphere(2, |_| 5.0);
    let displaced = mesh::icosphere(2, |direction| 5.0 + 0.4 * direction.y);
    let finer = mesh::icosphere(3, |_| 5.0);
    assert_eq!(flat.faces.len(), 320);
    assert_eq!(flat.vertices.len(), 162);
    assert_eq!(displaced.faces.len(), flat.faces.len());
    assert_eq!(finer.faces.len(), flat.faces.len() * 4);
    assert!((flat.radius - 5.0).abs() < 0.0001);
    assert!(displaced.radius > 5.3);
}

#[test]
fn surface_instances_follow_planet_rotation_and_seed() {
    let first = world();
    let second = world();
    let body = world::bodies(7.0)[0];
    let mut visible_types = [false; 4];
    for (a, b) in first.instances.iter().zip(&second.instances) {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert_eq!(a.model, b.model);
            assert_eq!((a.local_position - b.local_position).length(), 0.0);
            let world_position = body.center + body.basis.apply(a.local_position);
            let radial = (world_position - body.center).normalized();
            let world_up = body.basis.compose(a.local_basis).y;
            assert!(world_up.dot(radial) > 0.999);
            visible_types[a.model] = true;
        }
    }
    assert!(visible_types.into_iter().all(|visible| visible));
    assert!((world::bodies(0.0)[0].center - body.center).length() > 1.0);
    assert!(world::bodies(0.0)[0].basis.x.dot(body.basis.x) < 0.5);
}

fn fly(flight: &mut Flight, input: Input, seconds: usize) {
    for _ in 0..(seconds as f32 / STEP).round() as usize {
        flight.step(&input, SEED);
    }
}

#[test]
fn pilot_can_approach_switch_to_local_fly_sideways_and_depart() {
    let mut flight = Flight::new();
    let start = flight.view().distance;
    fly(
        &mut flight,
        Input {
            forward: 1.0,
            ..Input::default()
        },
        2,
    );
    assert!(flight.view().distance < start - 9.0);
    flight.toggle_local();
    assert!(flight.local);
    flight.aim_at_target();
    let bearing = |flight: &Flight| {
        (flight.position - world::bodies(flight.time)[flight.target].center).normalized()
    };
    let before = bearing(&flight);
    fly(
        &mut flight,
        Input {
            strafe: 1.0,
            ..Input::default()
        },
        3,
    );
    assert!(flight.local);
    assert!(bearing(&flight).dot(before) < 0.91);
    let near = flight.view().distance;
    fly(
        &mut flight,
        Input {
            forward: -1.0,
            ..Input::default()
        },
        4,
    );
    assert!(flight.view().distance > near + 7.0);
}

#[test]
fn thrust_and_gravity_cannot_push_ship_through_terrain() {
    let mut flight = Flight::new();
    let center = world::bodies(0.0)[0].center;
    flight.position = center + Vec3::new(0.0, 0.0, -12.0);
    flight.aim_at_target();
    flight.toggle_local();
    fly(
        &mut flight,
        Input {
            forward: 1.0,
            ..Input::default()
        },
        5,
    );
    let body = world::bodies(flight.time)[0];
    let delta = flight.position - body.center;
    let dir = delta.normalized();
    let local_dir = Vec3::new(
        dir.dot(body.basis.x),
        dir.dot(body.basis.y),
        dir.dot(body.basis.z),
    );
    let ground = world::terrain(PLANETS[0], local_dir, world::seed_for(SEED, 0));
    assert!(delta.length() >= ground + 1.16);
    assert!(flight.contact > 0.0);
}

#[test]
fn acceleration_and_gravity_change_flight_at_fixed_time_step() {
    let mut gravity = Flight::new();
    let mut no_gravity = Flight::new();
    no_gravity.gravity = false;
    fly(&mut gravity, Input::default(), 2);
    fly(&mut no_gravity, Input::default(), 2);
    assert!((gravity.time - 2.0).abs() < 0.0001);
    assert!(gravity.velocity.length() > 0.02);
    assert_eq!(no_gravity.velocity.length(), 0.0);
    no_gravity.step(
        &Input {
            forward: 1.0,
            ..Input::default()
        },
        SEED,
    );
    let speed_after_one_step = no_gravity.velocity.length();
    assert!((0.01..1.0).contains(&speed_after_one_step));
    fly(
        &mut no_gravity,
        Input {
            forward: 1.0,
            ..Input::default()
        },
        1,
    );
    assert!(no_gravity.velocity.length() > speed_after_one_step);
    fly(
        &mut no_gravity,
        Input {
            brake: true,
            ..Input::default()
        },
        1,
    );
    assert!(no_gravity.velocity.length() < speed_after_one_step);
}

#[test]
fn culling_preserves_framebuffer_and_lod_reduces_submissions() {
    let world = world();
    let view = flight::demo(0.0);
    let mut buffer = Framebuffer::new(960, 540);
    let automatic = render::draw_world(&mut buffer, &world, &view, 0.0, &Options::default());
    assert!(automatic.rasterized > 0);
    assert!(automatic.culled > 0);
    let reference = buffer.pixels.clone();
    let uncull = render::draw_world(
        &mut buffer,
        &world,
        &view,
        0.0,
        &Options {
            culling: false,
            lod: true,
        },
    );
    assert_eq!(reference, buffer.pixels);
    assert_eq!(uncull.culled, 0);
    assert!(uncull.submitted > automatic.submitted);
    let high = render::draw_world(
        &mut buffer,
        &world,
        &view,
        0.0,
        &Options {
            culling: true,
            lod: false,
        },
    );
    assert!(high.submitted > automatic.submitted);
    println!(
        "Triángulos enviados: automático {}, sin descarte {}, LOD alto fijo {}",
        automatic.submitted, uncull.submitted, high.submitted
    );
}

#[test]
fn demo_covers_a_full_loop_and_returns_to_deep_space() {
    let mut swept = 0.0;
    let bearing = |t| (flight::demo(t).ship - world::bodies(t)[0].center).normalized();
    let mut last = bearing(8.0);
    for i in 1..=138 {
        let next = bearing(8.0 + i as f32 * 0.1);
        let (x1, z1) = (last.x, last.z);
        let (x2, z2) = (next.x, next.z);
        swept += (z1 * x2 - x1 * z2).atan2(x1 * x2 + z1 * z2);
        last = next;
    }
    assert!((swept - std::f32::consts::TAU).abs() < 0.001);
    assert!(flight::demo(0.0).distance > 25.0);
    assert!(flight::demo(14.0).distance < 3.0);
    assert!(flight::demo(27.0).distance > 30.0);
}
