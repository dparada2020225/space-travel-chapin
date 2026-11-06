// Framebuffer final: pasada de sombreado (en paralelo por filas) y efectos 2D encima
// (estrellas, polvo, lineas de orbita, resplandor del sol).
use crate::math::{mezcla, v3, Mat4, Vec3};
use crate::raster::Rasterizador;
use crate::shaders::{sombrear, Frag, Globales, Uniforme};
use rayon::prelude::*;

pub struct Renderer {
    pub ancho: usize,
    pub alto: usize,
    pub rast: Rasterizador,
    /// RGBA8, lo que se sube a la textura de raylib
    pub pixeles: Vec<u8>,
    pub uniformes: Vec<Uniforme>,
}

fn a_u8(c: f32) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

impl Renderer {
    pub fn new(ancho: usize, alto: usize, cerca: f32) -> Renderer {
        Renderer {
            ancho,
            alto,
            rast: Rasterizador::new(ancho, alto, cerca),
            pixeles: vec![0; ancho * alto * 4],
            uniformes: Vec::new(),
        }
    }

    pub fn comenzar(&mut self, cerca: f32) {
        self.rast.gb.limpiar();
        self.rast.stats = Default::default();
        self.rast.cerca = cerca;
        self.uniformes.clear();
    }

    pub fn uniforme(&mut self, u: Uniforme) -> u16 {
        self.uniformes.push(u);
        (self.uniformes.len() - 1) as u16
    }

    /// Corre el fragment shader de cada pixel ocupado. Los pixeles vacios quedan con el fondo.
    pub fn sombrear(&mut self, g: &Globales) {
        let gb = &self.rast.gb;
        let ancho = self.ancho;
        let alto = self.alto as f32;
        let uniformes = &self.uniformes;
        self.pixeles.par_chunks_mut(ancho * 4).enumerate().for_each(|(y, fila)| {
            let fondo = mezcla(v3(0.004, 0.006, 0.02), v3(0.02, 0.012, 0.035), y as f32 / alto);
            for x in 0..ancho {
                let i = y * ancho + x;
                let c = if gb.prof[i] == 0.0 {
                    fondo
                } else {
                    let f = Frag {
                        pos: gb.pos[i],
                        normal: gb.normal[i],
                        color: gb.color[i],
                        local: gb.local[i],
                        dato: gb.dato[i],
                    };
                    sombrear(&f, &uniformes[gb.obj[i] as usize], g)
                };
                let o = x * 4;
                fila[o] = a_u8(c.x);
                fila[o + 1] = a_u8(c.y);
                fila[o + 2] = a_u8(c.z);
                fila[o + 3] = 255;
            }
        });
    }

    /// Proyecta un punto del mundo: (x, y en pixeles, 1/w). None si queda detras de la camara.
    pub fn proyectar(&self, vp: &Mat4, p: Vec3) -> Option<(f32, f32, f32)> {
        let c = vp.punto4(p);
        if c.w <= self.rast.cerca {
            return None;
        }
        let iw = 1.0 / c.w;
        Some((
            (c.x * iw * 0.5 + 0.5) * self.ancho as f32,
            (0.5 - c.y * iw * 0.5) * self.alto as f32,
            iw,
        ))
    }

    fn mezclar(&mut self, x: i32, y: i32, color: Vec3, alfa: f32) {
        if x < 0 || y < 0 || x >= self.ancho as i32 || y >= self.alto as i32 {
            return;
        }
        let o = (y as usize * self.ancho + x as usize) * 4;
        for k in 0..3 {
            let actual = self.pixeles[o + k] as f32 / 255.0;
            let nuevo = [color.x, color.y, color.z][k];
            self.pixeles[o + k] = a_u8(actual + (nuevo - actual) * alfa);
        }
    }

    fn sumar(&mut self, x: i32, y: i32, color: Vec3) {
        if x < 0 || y < 0 || x >= self.ancho as i32 || y >= self.alto as i32 {
            return;
        }
        let o = (y as usize * self.ancho + x as usize) * 4;
        for (k, c) in [color.x, color.y, color.z].iter().enumerate() {
            self.pixeles[o + k] = a_u8(self.pixeles[o + k] as f32 / 255.0 + c);
        }
    }

    fn vacio(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.ancho as i32 && y < self.alto as i32 && self.rast.gb.prof[y as usize * self.ancho + x as usize] == 0.0
    }

    pub fn estrellas(&mut self, vp: &Mat4, cam: Vec3, estrellas: &[(Vec3, f32)], tiempo: f32, lejos: f32) {
        for (i, &(dir, brillo)) in estrellas.iter().enumerate() {
            if let Some((x, y, _)) = self.proyectar(vp, cam + dir * (lejos * 0.5)) {
                let (xi, yi) = (x as i32, y as i32);
                if self.vacio(xi, yi) {
                    let titileo = 0.75 + 0.25 * (tiempo * 2.0 + i as f32 * 1.7).sin();
                    let c = brillo * titileo;
                    let tono = if i % 7 == 0 { v3(1.0, 0.8, 0.6) } else if i % 5 == 0 { v3(0.7, 0.8, 1.0) } else { Vec3::splat(1.0) };
                    self.sumar(xi, yi, tono * c);
                }
            }
        }
    }

    /// Particulas de polvo alrededor de la camara para que se sienta la velocidad.
    pub fn polvo(&mut self, vp: &Mat4, cam: Vec3, puntos: &[Vec3], caja: f32) {
        for &p in puntos {
            let rel = p - cam;
            let envolver = |v: f32| (v + caja * 0.5).rem_euclid(caja) - caja * 0.5;
            let q = cam + v3(envolver(rel.x), envolver(rel.y), envolver(rel.z));
            let dist = (q - cam).len();
            if let Some((x, y, iw)) = self.proyectar(vp, q) {
                let (xi, yi) = (x as i32, y as i32);
                if xi < 0 || yi < 0 || xi >= self.ancho as i32 || yi >= self.alto as i32 {
                    continue;
                }
                if iw > self.rast.gb.prof[yi as usize * self.ancho + xi as usize] {
                    let a = (1.0 - dist / (caja * 0.5)).clamp(0.0, 1.0) * 0.45;
                    self.mezclar(xi, yi, v3(0.75, 0.8, 0.9), a);
                }
            }
        }
    }

    /// Linea 3D con prueba de profundidad (para las orbitas).
    pub fn linea(&mut self, vp: &Mat4, a: Vec3, b: Vec3, color: Vec3, alfa: f32) {
        let cerca = self.rast.cerca;
        let (mut ca, mut cb) = (vp.punto4(a), vp.punto4(b));
        if ca.w < cerca && cb.w < cerca {
            return;
        }
        if ca.w < cerca {
            ca = ca.lerp(cb, (cerca - ca.w) / (cb.w - ca.w));
        } else if cb.w < cerca {
            cb = cb.lerp(ca, (cerca - cb.w) / (ca.w - cb.w));
        }
        let pant = |c: crate::math::Vec4| {
            let iw = 1.0 / c.w;
            ((c.x * iw * 0.5 + 0.5) * self.ancho as f32, (0.5 - c.y * iw * 0.5) * self.alto as f32, iw)
        };
        let (x0, y0, iw0) = pant(ca);
        let (x1, y1, iw1) = pant(cb);
        let pasos = (x1 - x0).abs().max((y1 - y0).abs()).ceil().min(4000.0) as i32;
        if pasos <= 0 {
            return;
        }
        for s in 0..=pasos {
            let t = s as f32 / pasos as f32;
            let (x, y) = ((x0 + (x1 - x0) * t) as i32, (y0 + (y1 - y0) * t) as i32);
            if x < 0 || y < 0 || x >= self.ancho as i32 || y >= self.alto as i32 {
                continue;
            }
            let iw = iw0 + (iw1 - iw0) * t;
            if iw > self.rast.gb.prof[y as usize * self.ancho + x as usize] {
                self.mezclar(x, y, color, alfa);
            }
        }
    }

    /// Resplandor radial alrededor del sol (se suma encima de todo).
    pub fn resplandor(&mut self, cx: f32, cy: f32, radio: f32, color: Vec3, fuerza: f32) {
        if fuerza <= 0.01 {
            return;
        }
        let r = radio.max(4.0);
        let (x0, x1) = ((cx - r).floor().max(0.0) as i32, (cx + r).ceil().min(self.ancho as f32 - 1.0) as i32);
        let (y0, y1) = ((cy - r).floor().max(0.0) as i32, (cy + r).ceil().min(self.alto as f32 - 1.0) as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / r;
                if d < 1.0 {
                    let k = (1.0 - d).powi(3) * fuerza;
                    self.sumar(x, y, color * k);
                }
            }
        }
    }

    pub fn obj_en(&self, x: f32, y: f32) -> Option<u16> {
        let (xi, yi) = (x as i32, y as i32);
        if xi < 0 || yi < 0 || xi >= self.ancho as i32 || yi >= self.alto as i32 {
            return None;
        }
        let i = yi as usize * self.ancho + xi as usize;
        if self.rast.gb.prof[i] == 0.0 {
            None
        } else {
            Some(self.rast.gb.obj[i])
        }
    }
}
