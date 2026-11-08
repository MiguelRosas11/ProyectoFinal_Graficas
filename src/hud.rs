use crate::{
    flight::{Flight, View},
    framebuffer::Framebuffer,
    render::{Options, Projection, Stats},
    world::{self, PLANETS},
};
use font8x8::{BASIC_FONTS, UnicodeFonts};

const WHITE: u32 = 0xe9_f5_f8;
const MUTE: u32 = 0x95_a9_bc;
const CYAN: u32 = 0x75_e8_d7;
const AMBER: u32 = 0xff_b6_76;
const LINE: u32 = 0x35_55_67;

struct Hud<'a>(&'a mut Framebuffer);

impl Hud<'_> {
    fn px(&mut self, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 && x < self.0.width as i32 && y < self.0.height as i32 {
            self.0.pixels[y as usize * self.0.width + x as usize] = color;
        }
    }

    fn line(&mut self, x: i32, y: i32, width: i32, color: u32) {
        for i in 0..width {
            self.px(x + i, y, color);
        }
    }

    fn panel(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for iy in y..y + h {
            for ix in x..x + w {
                let i = iy as usize * self.0.width + ix as usize;
                let base = (self.0.pixels[i] & 0xfc_fc_fc) >> 2;
                self.0.pixels[i] = base + 0x07_0d_15;
            }
        }
        self.line(x, y, w, LINE);
        self.line(x, y, 19, CYAN);
    }

    fn text(&mut self, x: i32, y: i32, text: &str, color: u32, scale: i32) {
        for (i, ch) in text.chars().enumerate() {
            let Some(glyph) = BASIC_FONTS.get(ch) else {
                continue;
            };
            for (row, bits) in glyph.into_iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) == 0 {
                        continue;
                    }
                    for dy in 0..scale {
                        for dx in 0..scale {
                            self.px(
                                x + (i as i32 * 8 + col) * scale + dx,
                                y + row as i32 * scale + dy,
                                color,
                            );
                        }
                    }
                }
            }
        }
    }

    fn reticle(&mut self, x: i32, y: i32, color: u32) {
        self.line(x - 16, y, 6, color);
        self.line(x + 11, y, 6, color);
        for offset in [(-16..-10), (11..17)] {
            for i in offset {
                self.px(x, y + i, color);
            }
        }
        self.px(x, y, color);
    }
}

pub struct Telemetry<'a> {
    pub time: f32,
    pub flight: &'a Flight,
    pub options: &'a Options,
    pub stats: &'a Stats,
    pub show_stats: bool,
    pub demo: bool,
}

pub fn draw(buffer: &mut Framebuffer, view: &View, telemetry: Telemetry<'_>) {
    let (w, h) = (buffer.width as i32, buffer.height as i32);
    let Telemetry {
        time,
        flight,
        options,
        stats,
        show_stats,
        demo,
    } = telemetry;
    let target = if demo { 0 } else { flight.target };
    let planet = PLANETS[target];
    let mut hud = Hud(buffer);
    hud.panel(20, 20, 304, 58);
    hud.text(32, 31, "HELIOS / REGISTRO 05", CYAN, 1);
    hud.text(30, 47, "ASTER // DERIVA", WHITE, 2);
    hud.panel(w - 228, 20, 208, 58);
    hud.text(
        w - 215,
        32,
        &format!("OBJETIVO  {:02} / 05", target + 1),
        CYAN,
        1,
    );
    hud.text(w - 215, 47, planet.name, WHITE, 2);
    hud.text(w - 215, 66, planet.description, MUTE, 1);

    if !demo {
        let projection = Projection::new(w as usize, h as usize);
        let body = world::bodies(time)[target];
        let (x, y, on_screen) = projection.marker(view.camera, body.center);
        if !on_screen || view.distance > 11.0 {
            let (x, y) = (x.round() as i32, y.round() as i32);
            hud.reticle(x, y, CYAN);
            let label = if on_screen {
                planet.name
            } else {
                "OBJETIVO / T"
            };
            let text_x = (x + 23).min(w - 8 - label.len() as i32 * 8);
            hud.text(text_x, y - 4, label, CYAN, 1);
        }
    }

    let bottom = h - 74;
    hud.panel(20, bottom, if demo { 414 } else { 590 }, 54);
    if let Some((stage, subtitle)) = view.stage {
        hud.text(32, bottom + 9, stage, WHITE, 2);
        hud.text(32, bottom + 33, subtitle, CYAN, 1);
    } else {
        hud.text(
            32,
            bottom + 9,
            "WASD  MOVER     FLECHAS  MIRAR    Q/E  BAJAR/SUBIR",
            WHITE,
            1,
        );
        hud.text(
            32,
            bottom + 23,
            "F  MODO LOCAL   TAB  OBJETIVO      T  APUNTAR",
            CYAN,
            1,
        );
        hud.text(
            32,
            bottom + 37,
            "SHIFT  IMPULSO  ESPACIO  FRENO    F1  DATOS   P  DEMO",
            MUTE,
            1,
        );
    }

    hud.panel(w - 228, bottom, 208, 54);
    let mode = if demo {
        "RECORRIDO GUIADO"
    } else if flight.contact > 0.0 {
        "LIMITE DE SUPERFICIE"
    } else if flight.local {
        "CONTROL LOCAL"
    } else {
        "VUELO LIBRE"
    };
    hud.text(
        w - 215,
        bottom + 9,
        mode,
        if flight.contact > 0.0 && !demo {
            AMBER
        } else {
            CYAN
        },
        1,
    );
    hud.text(
        w - 215,
        bottom + 25,
        &format!("{:04.1} U", view.distance),
        WHITE,
        2,
    );
    hud.text(w - 106, bottom + 31, "RADIAL", MUTE, 1);

    if show_stats {
        hud.panel(20, 92, 288, 113);
        let switch = |v| if v { "SI" } else { "NO" };
        hud.text(32, 103, "TELEMETRIA / RASTER CPU", CYAN, 1);
        hud.text(
            32,
            118,
            &format!("TRI ENVIADOS  {:6}", stats.submitted),
            WHITE,
            1,
        );
        hud.text(
            32,
            132,
            &format!("TRI EN PANTALLA {:4}", stats.rasterized),
            WHITE,
            1,
        );
        hud.text(
            32,
            146,
            &format!("OBJ DESCARTADOS {:4}", stats.culled),
            WHITE,
            1,
        );
        hud.text(
            32,
            160,
            &format!(
                "LOD BAJO/MEDIO/ALTO  {}/{}/{}",
                stats.lod[0], stats.lod[1], stats.lod[2]
            ),
            WHITE,
            1,
        );
        hud.text(
            32,
            174,
            &format!(
                "C DESC. {} | L LOD {} | G GRAV. {}",
                switch(options.culling),
                switch(options.lod),
                switch(flight.gravity)
            ),
            MUTE,
            1,
        );
        hud.text(32, 188, "F12 FOTO | R REINICIO | ESC SALIR", MUTE, 1);
    }
}
