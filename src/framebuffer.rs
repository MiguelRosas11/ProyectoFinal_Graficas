use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
    pub depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
            depth: vec![0.0; width * height],
        }
    }

    pub fn rgb(&self, bytes: &mut Vec<u8>) {
        bytes.clear();
        for &p in &self.pixels {
            bytes.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
    }

    pub fn rgba(&self, bytes: &mut Vec<u8>) {
        bytes.clear();
        for &p in &self.pixels {
            bytes.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
    }

    pub fn save_png(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = BufWriter::new(File::create(path)?);
        let mut encoder = png::Encoder::new(file, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut bytes = Vec::with_capacity(self.pixels.len() * 3);
        self.rgb(&mut bytes);
        encoder.write_header()?.write_image_data(&bytes)?;
        Ok(())
    }

    pub fn write_rgb(&self, out: &mut impl Write, bytes: &mut Vec<u8>) -> io::Result<()> {
        self.rgb(bytes);
        out.write_all(bytes)
    }
}
