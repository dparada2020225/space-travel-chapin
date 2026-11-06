// Mallas de triangulos: cargador de OBJ/MTL y generacion de esferas por subdivision.
use crate::math::{v3, Vec3};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vertice {
    pub pos: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
    /// Dato libre por vertice. En planetas: altura/radio. En objetos: emision (>0) o "se tiñe" (-1).
    pub dato: f32,
}

#[derive(Clone, Default)]
pub struct Malla {
    pub vertices: Vec<Vertice>,
    pub triangulos: Vec<[u32; 3]>,
    /// Esfera que envuelve la malla (para el descarte por frustum).
    pub centro: Vec3,
    pub radio: f32,
}

impl Malla {
    pub fn calcular_limites(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &self.vertices {
            min = v3(min.x.min(v.pos.x), min.y.min(v.pos.y), min.z.min(v.pos.z));
            max = v3(max.x.max(v.pos.x), max.y.max(v.pos.y), max.z.max(v.pos.z));
        }
        self.centro = (min + max) * 0.5;
        self.radio = self
            .vertices
            .iter()
            .map(|v| (v.pos - self.centro).len())
            .fold(0.0, f32::max);
    }

    /// Normales suaves: promedio de las normales de las caras que tocan cada vertice.
    pub fn recalcular_normales(&mut self) {
        for v in &mut self.vertices {
            v.normal = Vec3::ZERO;
        }
        for t in &self.triangulos {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (self.vertices[b].pos - self.vertices[a].pos)
                .cross(self.vertices[c].pos - self.vertices[a].pos);
            self.vertices[a].normal += n;
            self.vertices[b].normal += n;
            self.vertices[c].normal += n;
        }
        for v in &mut self.vertices {
            v.normal = v.normal.norm();
        }
    }
}

/// Etapa de generacion de geometria: icosaedro subdividido n veces (cada paso x4 triangulos).
pub fn icosfera(subdivisiones: u32) -> Malla {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let mut pos: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| v3(x, y, z).norm())
    .collect();

    let mut caras: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    for _ in 0..subdivisiones {
        // cache de puntos medios para no duplicar vertices en aristas compartidas
        let mut medios: HashMap<(u32, u32), u32> = HashMap::new();
        let mut medio = |a: u32, b: u32, pos: &mut Vec<Vec3>| -> u32 {
            let clave = (a.min(b), a.max(b));
            *medios.entry(clave).or_insert_with(|| {
                let m = ((pos[a as usize] + pos[b as usize]) * 0.5).norm();
                pos.push(m);
                (pos.len() - 1) as u32
            })
        };
        let mut nuevas = Vec::with_capacity(caras.len() * 4);
        for &[a, b, c] in &caras {
            let ab = medio(a, b, &mut pos);
            let bc = medio(b, c, &mut pos);
            let ca = medio(c, a, &mut pos);
            nuevas.push([a, ab, ca]);
            nuevas.push([b, bc, ab]);
            nuevas.push([c, ca, bc]);
            nuevas.push([ab, bc, ca]);
        }
        caras = nuevas;
    }

    let vertices = pos
        .into_iter()
        .map(|p| Vertice { pos: p, normal: p, color: Vec3::splat(1.0), dato: 0.0 })
        .collect();
    let mut m = Malla { vertices, triangulos: caras, ..Default::default() };
    m.calcular_limites();
    m
}

struct Material {
    kd: Vec3,
    ke: Vec3,
}

fn leer_mtl(ruta: &Path) -> HashMap<String, Material> {
    let mut mats = HashMap::new();
    let Ok(texto) = fs::read_to_string(ruta) else {
        eprintln!("aviso: no se pudo leer {}", ruta.display());
        return mats;
    };
    let mut actual: Option<String> = None;
    for linea in texto.lines() {
        let mut partes = linea.split_whitespace();
        match partes.next() {
            Some("newmtl") => {
                let nombre = partes.next().unwrap_or("").to_string();
                mats.insert(nombre.clone(), Material { kd: Vec3::splat(0.8), ke: Vec3::ZERO });
                actual = Some(nombre);
            }
            Some(k @ ("Kd" | "Ke")) => {
                let vals: Vec<f32> = partes.filter_map(|s| s.parse().ok()).collect();
                if let (Some(n), true) = (&actual, vals.len() >= 3) {
                    let c = v3(vals[0], vals[1], vals[2]);
                    let m = mats.get_mut(n).unwrap();
                    if k == "Kd" {
                        m.kd = c;
                    } else {
                        m.ke = c;
                    }
                }
            }
            _ => {}
        }
    }
    mats
}

fn indice(s: &str, total: usize) -> Option<usize> {
    let i: i64 = s.parse().ok()?;
    if i > 0 {
        Some(i as usize - 1)
    } else if i < 0 {
        Some((total as i64 + i) as usize)
    } else {
        None
    }
}

/// Carga un .obj (v, vn, f, usemtl, mtllib). Las caras de mas de 3 lados se triangulan en abanico.
pub fn cargar_obj(ruta: &Path) -> Result<Malla, String> {
    let texto = fs::read_to_string(ruta).map_err(|e| format!("{}: {}", ruta.display(), e))?;
    let carpeta = ruta.parent().unwrap_or(Path::new("."));
    let mut posiciones: Vec<Vec3> = Vec::new();
    let mut normales: Vec<Vec3> = Vec::new();
    let mut mats: HashMap<String, Material> = HashMap::new();
    let mut color = Vec3::splat(0.8);
    let mut dato = 0.0;
    let mut malla = Malla::default();

    for linea in texto.lines() {
        let mut partes = linea.split_whitespace();
        match partes.next() {
            Some("v") | Some("vn") => {
                let es_v = linea.starts_with("v ");
                let c: Vec<f32> = partes.filter_map(|s| s.parse().ok()).collect();
                if c.len() < 3 {
                    continue;
                }
                if es_v {
                    posiciones.push(v3(c[0], c[1], c[2]));
                } else {
                    normales.push(v3(c[0], c[1], c[2]).norm());
                }
            }
            Some("mtllib") => {
                if let Some(n) = partes.next() {
                    mats = leer_mtl(&carpeta.join(n));
                }
            }
            Some("usemtl") => {
                let nombre = partes.next().unwrap_or("");
                if let Some(m) = mats.get(nombre) {
                    color = m.kd;
                    dato = if m.ke.max_c() > 0.0 { 1.0 } else { 0.0 };
                    if m.ke.max_c() > 0.0 {
                        color = m.ke;
                    }
                }
                // las paredes blancas se tiñen con el color de cada instancia
                if nombre == "pared" {
                    dato = -1.0;
                }
            }
            Some("f") => {
                let mut esquinas: Vec<(usize, Option<usize>)> = Vec::new();
                for tok in partes {
                    let mut it = tok.split('/');
                    let vi = it.next().and_then(|s| indice(s, posiciones.len()));
                    let _vt = it.next();
                    let ni = it.next().and_then(|s| indice(s, normales.len()));
                    if let Some(vi) = vi {
                        esquinas.push((vi, ni));
                    }
                }
                if esquinas.len() < 3 {
                    continue;
                }
                let p0 = posiciones[esquinas[0].0];
                let normal_cara = (posiciones[esquinas[1].0] - p0)
                    .cross(posiciones[esquinas[2].0] - p0)
                    .norm();
                for k in 1..esquinas.len() - 1 {
                    let base = malla.vertices.len() as u32;
                    for &(vi, ni) in &[esquinas[0], esquinas[k], esquinas[k + 1]] {
                        malla.vertices.push(Vertice {
                            pos: posiciones[vi],
                            normal: ni.map(|n| normales[n]).unwrap_or(normal_cara),
                            color,
                            dato,
                        });
                    }
                    malla.triangulos.push([base, base + 1, base + 2]);
                }
            }
            _ => {}
        }
    }
    if malla.triangulos.is_empty() {
        return Err(format!("{} no tiene caras", ruta.display()));
    }
    malla.calcular_limites();
    Ok(malla)
}
