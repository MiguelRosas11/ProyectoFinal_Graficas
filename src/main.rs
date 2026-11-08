mod color;
mod flight;
mod framebuffer;
mod hud;
mod math;
mod mesh;
mod noise;
mod obj;
mod render;
mod shading;
mod world;

use flight::{DEMO_LENGTH, Flight, Input, STEP};
use framebuffer::Framebuffer;
use math::Vec3;
use obj::Model;
use raylib::prelude::{self as rl, RaylibDraw, RaylibTexture2D};
use render::Options;
use std::{
    error::Error,
    io::{BufWriter, Write},
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};
use world::{PLANETS, SEED, World};

const WIDTH: usize = 960;
const HEIGHT: usize = 540;
const VIDEO_FPS: u32 = 30;

struct App {
    world: World,
    flight: Flight,
    buffer: Framebuffer,
    options: Options,
    show_stats: bool,
}

impl App {
    fn new(seed: u32) -> Result<Self, Box<dyn Error>> {
        let model =
            Model::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("modelos/nave_aster.obj"))?;
        Ok(Self {
            world: World::new(&model, seed),
            flight: Flight::new(),
            buffer: Framebuffer::new(WIDTH, HEIGHT),
            options: Options::default(),
            show_stats: false,
        })
    }

    fn draw(&mut self, demo: Option<f32>) {
        let (view, time) = if let Some(time) = demo {
            (flight::demo(time), time)
        } else {
            (self.flight.view(), self.flight.time)
        };
        self.draw_view(&view, time, demo.is_some());
    }

    fn draw_view(&mut self, view: &flight::View, time: f32, demo: bool) {
        let stats = render::draw_world(&mut self.buffer, &self.world, view, time, &self.options);
        hud::draw(
            &mut self.buffer,
            view,
            hud::Telemetry {
                time,
                flight: &self.flight,
                options: &self.options,
                stats: &stats,
                show_stats: self.show_stats,
                demo,
            },
        );
    }

    fn window(&mut self, smoke_test: bool) -> Result<(), Box<dyn Error>> {
        use rl::KeyboardKey::*;
        let (mut window, thread) = raylib::init()
            .size(WIDTH as i32, HEIGHT as i32)
            .title("ASTER // DERIVA | Exploracion del sistema Helios")
            .build();
        window.set_target_fps(60);
        let image = unsafe {
            rl::Image::from_raw(raylib::ffi::GenImageColor(
                WIDTH as i32,
                HEIGHT as i32,
                rl::Color::BLACK,
            ))
        };
        let mut texture = window.load_texture_from_image(&thread, &image)?;
        let mut pixels = Vec::with_capacity(WIDTH * HEIGHT * 4);
        let mut previous = Instant::now();
        let mut accumulator = 0.0;
        let mut demo: Option<f32> = None;
        let mut frames = 0;
        while !window.window_should_close() {
            let now = Instant::now();
            let dt = now.duration_since(previous).as_secs_f32();
            previous = now;
            if window.is_key_pressed(KEY_P) {
                demo = if demo.is_some() { None } else { Some(0.0) };
            }
            if window.is_key_pressed(KEY_F1) {
                self.show_stats = !self.show_stats;
            }
            if window.is_key_pressed(KEY_C) {
                self.options.culling = !self.options.culling;
            }
            if window.is_key_pressed(KEY_L) {
                self.options.lod = !self.options.lod;
            }
            if window.is_key_pressed(KEY_R) {
                self.flight = Flight::new();
                demo = None;
                accumulator = 0.0;
            }
            if let Some(time) = &mut demo {
                *time += dt;
                if *time >= DEMO_LENGTH {
                    demo = None;
                }
            } else {
                if window.is_key_pressed(KEY_TAB) {
                    self.flight.select_next();
                }
                if window.is_key_pressed(KEY_T) {
                    self.flight.aim_at_target();
                }
                if window.is_key_pressed(KEY_F) {
                    self.flight.toggle_local();
                }
                if window.is_key_pressed(KEY_G) {
                    self.flight.gravity = !self.flight.gravity;
                }
                let axis = |pos, neg| {
                    window.is_key_down(pos) as u8 as f32 - window.is_key_down(neg) as u8 as f32
                };
                let input = Input {
                    forward: axis(KEY_W, KEY_S),
                    strafe: axis(KEY_D, KEY_A),
                    lift: axis(KEY_E, KEY_Q),
                    turn: axis(KEY_RIGHT, KEY_LEFT),
                    pitch: axis(KEY_UP, KEY_DOWN),
                    boost: window.is_key_down(KEY_LEFT_SHIFT)
                        || window.is_key_down(KEY_RIGHT_SHIFT),
                    brake: window.is_key_down(KEY_SPACE),
                };
                accumulator += dt;
                // Limitar el trabajo por cuadro sin descartar el tiempo acumulado.
                for _ in 0..30 {
                    if accumulator < STEP {
                        break;
                    }
                    self.flight.step(&input, self.world.seed);
                    accumulator -= STEP;
                }
            }
            self.draw(demo);
            if window.is_key_pressed(KEY_F12) {
                std::fs::create_dir_all("capturas")?;
                self.buffer.save_png(Path::new("capturas/actual.png"))?;
            }
            self.buffer.rgba(&mut pixels);
            texture.update_texture(&pixels)?;
            {
                let mut draw = window.begin_drawing(&thread);
                draw.draw_texture(&texture, 0, 0, rl::Color::WHITE);
            }
            frames += 1;
            if smoke_test && frames >= 3 {
                break;
            }
        }
        Ok(())
    }

    fn video(&mut self, path: &str, ffmpeg: &str) -> Result<(), Box<dyn Error>> {
        let dimensions = format!("{WIDTH}x{HEIGHT}");
        let frame_rate = VIDEO_FPS.to_string();
        let mut encoder = Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgb24",
                "-video_size",
                &dimensions,
                "-framerate",
                &frame_rate,
                "-i",
                "pipe:0",
                "-an",
                "-c:v",
                "libx264",
                "-crf",
                "19",
                "-preset",
                "fast",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
                path,
            ])
            .stdin(Stdio::piped())
            .spawn()?;
        let result = {
            let mut output =
                BufWriter::new(encoder.stdin.take().expect("stdin configurado como pipe"));
            let mut bytes = Vec::with_capacity(WIDTH * HEIGHT * 3);
            (|| -> std::io::Result<()> {
                for i in 0..(DEMO_LENGTH * VIDEO_FPS as f32) as u32 {
                    self.draw(Some(i as f32 / VIDEO_FPS as f32));
                    self.buffer.write_rgb(&mut output, &mut bytes)?;
                }
                output.flush()
            })()
        };
        let status = encoder.wait()?;
        result?;
        if !status.success() {
            return Err(format!("FFmpeg termino con {status}").into());
        }
        Ok(())
    }

    fn inspect(&mut self, target: usize, path: &Path) -> Result<(), Box<dyn Error>> {
        self.flight.target = target;
        let time = 4.0;
        let body = world::bodies(time)[target];
        let sunward = (Vec3::default() - body.center).normalized();
        let offset =
            (sunward + Vec3::new(0.22, 0.42, -0.36)).normalized() * (PLANETS[target].radius + 4.1);
        let ship = body.center + offset;
        let look = (body.center - ship).normalized();
        let view = flight::inspection(ship, look, time, target);
        self.draw_view(&view, time, false);
        self.buffer.save_png(path)
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let seed = if args.first().is_some_and(|s| s == "--seed") {
        if args.len() < 2 {
            return Err("Falta el entero de --seed".into());
        }
        let seed = args[1].parse()?;
        args.drain(..2);
        seed
    } else {
        SEED
    };
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    if matches!(args.as_slice(), ["--help"] | ["-h"]) {
        println!(
            "ASTER // DERIVA\n  [--seed N] [--smoke-test]\n  --shot SEGUNDOS FOTO.png\n  --inspect PLANETA_1_A_5 FOTO.png\n  --video SALIDA.mp4 [ruta-a-ffmpeg]"
        );
        return Ok(());
    }
    let mut app = App::new(seed)?;
    match args.as_slice() {
        [] => app.window(false),
        ["--smoke-test"] => app.window(true),
        ["--shot", time, path] => {
            let time: f32 = time.parse()?;
            if !time.is_finite() || !(0.0..=DEMO_LENGTH).contains(&time) {
                return Err(format!("Tiempo fuera del recorrido (0 a {DEMO_LENGTH} s)").into());
            }
            app.draw(Some(time));
            app.buffer.save_png(Path::new(path))
        }
        ["--inspect", planet, path] => {
            let number: usize = planet.parse()?;
            if !(1..=PLANETS.len()).contains(&number) {
                return Err("Planeta fuera del rango 1 a 5".into());
            }
            app.inspect(number - 1, Path::new(path))
        }
        ["--video", path] => app.video(path, "ffmpeg"),
        ["--video", path, ffmpeg] => app.video(path, ffmpeg),
        _ => Err("Argumentos invalidos. Usa --help".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
