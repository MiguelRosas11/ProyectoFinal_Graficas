use crate::{color::Color, math::Vec3};
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug)]
pub struct Face {
    pub indices: [usize; 3],
    pub material: String,
}

pub struct Model {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
    pub materials: HashMap<String, Color>,
}

impl Model {
    pub fn load(path: &Path) -> Result<Self, String> {
        let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut model = Self::parse(&source)?;
        for line in source.lines() {
            let mut parts = line.split('#').next().unwrap_or("").split_whitespace();
            if parts.next() == Some("mtllib") {
                for name in parts {
                    let mtl_path = path.parent().unwrap_or(Path::new(".")).join(name);
                    let mtl = fs::read_to_string(&mtl_path)
                        .map_err(|e| format!("{}: {e}", mtl_path.display()))?;
                    model.read_materials(&mtl)?;
                }
            }
        }
        Ok(model)
    }

    pub fn parse(source: &str) -> Result<Self, String> {
        let mut model = Self {
            vertices: vec![],
            faces: vec![],
            materials: HashMap::new(),
        };
        let mut material = String::new();
        for (line_number, line) in source.lines().enumerate() {
            let parts: Vec<_> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            if parts.is_empty() {
                continue;
            }
            let error = |detail: &str| format!("OBJ linea {}: {detail}", line_number + 1);
            match parts[0] {
                "v" => {
                    if parts.len() < 4 {
                        return Err(error("vertice incompleto"));
                    }
                    let mut coords = [0.0; 3];
                    for i in 0..3 {
                        coords[i] = parts[i + 1]
                            .parse::<f32>()
                            .map_err(|_| error("coordenada invalida"))?;
                        if !coords[i].is_finite() {
                            return Err(error("coordenada no finita"));
                        }
                    }
                    model
                        .vertices
                        .push(Vec3::new(coords[0], coords[1], coords[2]));
                }
                "usemtl" => {
                    material = parts
                        .get(1)
                        .ok_or_else(|| error("falta material"))?
                        .to_string()
                }
                "f" => {
                    if parts.len() != 4 {
                        return Err(error("se requieren caras trianguladas de tres indices"));
                    }
                    let mut indices = [0; 3];
                    for i in 0..3 {
                        // OBJ empieza en 1. En v/vt/vn solo necesitamos el indice de v.
                        let value = parts[i + 1]
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .parse::<i64>()
                            .map_err(|_| error("indice invalido"))?;
                        let index = if value > 0 {
                            value - 1
                        } else {
                            model.vertices.len() as i64 + value
                        };
                        if value == 0 || index < 0 || index >= model.vertices.len() as i64 {
                            return Err(error("indice fuera del arreglo de vertices"));
                        }
                        indices[i] = index as usize;
                    }
                    model.faces.push(Face {
                        indices,
                        material: material.clone(),
                    });
                }
                _ => {} // Normales y UV no son necesarios para este ejercicio.
            }
        }
        if model.vertices.is_empty() || model.faces.is_empty() {
            return Err("El OBJ no contiene triangulos".into());
        }
        Ok(model)
    }

    fn read_materials(&mut self, source: &str) -> Result<(), String> {
        let mut name = String::new();
        for line in source.lines() {
            let parts: Vec<_> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            match parts.first().copied() {
                Some("newmtl") => name = parts.get(1).ok_or("MTL: falta nombre")?.to_string(),
                Some("Kd") => {
                    if parts.len() != 4 || name.is_empty() {
                        return Err("MTL: Kd invalido".into());
                    }
                    let mut rgb = [0; 3];
                    for i in 0..3 {
                        let v = parts[i + 1]
                            .parse::<f32>()
                            .map_err(|_| "MTL: color invalido")?;
                        if !v.is_finite() {
                            return Err("MTL: color no finito".into());
                        }
                        rgb[i] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                    self.materials
                        .insert(name.clone(), Color(rgb[0], rgb[1], rgb[2]));
                }
                _ => {}
            }
        }
        Ok(())
    }
}
