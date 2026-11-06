// Fragment shaders. Cada uno recibe lo que quedo en el G-buffer para ese pixel
// y devuelve un color. Todos usan la posicion LOCAL del planeta para los patrones,
// asi la textura gira con el planeta en vez de "resbalarse".
use crate::math::{mezcla, suave, v3, Mat4, Vec3};
use crate::ruido::{celular, id_a_f32, Perlin};
use std::f32::consts::PI;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Material {
    Sol,
    Lava,
    Agua,
    Adoquin,
    Terreno,
    Selva,
    Nave,
    Objeto,
}

#[derive(Clone, Debug)]
pub struct Uniforme {
    pub material: Material,
    /// Rotacion del objeto, para pasar normales de espacio local a mundo.
    pub rot: Mat4,
    /// colores a, b, c y color de atmosfera
    pub colores: [Vec3; 4],
    pub intensidad: f32,
    pub tinte: Vec3,
    /// Centro del planeta al que pertenece (para saber si es de dia o de noche)
    pub centro: Vec3,
    /// 1 = iluminado, 0 = en la sombra de un planeta
    pub sombra: f32,
}

impl Default for Uniforme {
    fn default() -> Self {
        Uniforme {
            material: Material::Objeto,
            rot: Mat4::identidad(),
            colores: [Vec3::splat(1.0); 4],
            intensidad: 1.0,
            tinte: Vec3::splat(1.0),
            centro: Vec3::ZERO,
            sombra: 1.0,
        }
    }
}

pub struct Frag {
    pub pos: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
    pub local: Vec3,
    pub dato: f32,
}

pub struct Globales<'a> {
    pub tiempo: f32,
    pub luz: Vec3,
    pub cam: Vec3,
    pub ambiente: f32,
    pub ruido: &'a Perlin,
    pub mapa: &'a MapaNormales,
    pub usar_mapa_normales: bool,
}

pub fn sombrear(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    match u.material {
        Material::Sol => sol(f, u, g),
        Material::Lava => lava(f, u, g),
        Material::Agua => agua(f, u, g),
        Material::Adoquin => adoquin(f, u, g),
        Material::Terreno => terreno(f, u, g),
        Material::Selva => selva(f, u, g),
        Material::Nave => nave(f, u, g),
        Material::Objeto => objeto(f, u, g),
    }
}

fn lambert(n: Vec3, l: Vec3) -> f32 {
    n.dot(l).max(0.0)
}

/// Borde azulado que simula atmosfera (mas fuerte en el contorno y del lado iluminado).
fn atmosfera(n: Vec3, v: Vec3, l: Vec3, color: Vec3) -> Vec3 {
    let borde = (1.0 - n.dot(v).max(0.0)).powi(3);
    let lado_dia = suave(-0.3, 0.6, n.dot(l));
    color * (borde * lado_dia * 0.9)
}

fn base_iluminacion(f: &Frag, g: &Globales) -> (Vec3, Vec3, Vec3) {
    let n = f.normal.norm();
    let l = (g.luz - f.pos).norm();
    let v = (g.cam - f.pos).norm();
    (n, l, v)
}

// ------------------------------------------------------------------ SOL (animado)
// fBm con "domain warping": se deforma el espacio con otro ruido antes de muestrear.
fn sol(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let p = f.local.norm() * 2.6;
    let t = g.tiempo * 0.22;
    let r = g.ruido;
    let q = v3(
        r.fbm(p + v3(0.0, t, 0.0), 3),
        r.fbm(p + v3(5.2, 1.3 - t, 2.8), 3),
        r.fbm(p + v3(1.7, 9.2, t * 0.7), 3),
    );
    let n = r.fbm(p * 1.6 + q * 2.2, 4) * 0.5 + 0.5;
    let [a, b, _, _] = u.colores;
    let mut col = mezcla(b, a, n * 1.5 - 0.1);
    col += v3(1.0, 0.95, 0.8) * (suave(0.62, 0.8, n) * 0.6);
    let v = (g.cam - f.pos).norm();
    let mu = f.normal.norm().dot(v).max(0.0);
    col = col * (0.5 + 0.5 * mu.sqrt()); // oscurecimiento del borde
    col * (1.25 * u.intensidad)
}

// ------------------------------------------------------------------ LAVA (animado)
// Roca oscura + rios de lava con ruido de crestas que se desplaza con el tiempo.
fn lava(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (n, l, v) = base_iluminacion(f, g);
    let dir = f.local.norm();
    let h = f.dato;
    let r = g.ruido;
    let [roca, lava_a, lava_b, atm] = u.colores;

    let grano = r.fbm(f.local * 9.0, 3) * 0.5 + 0.5;
    let mut col_roca = mezcla(roca * 0.55, roca * 1.2, grano);
    // ceniza clara en las partes altas
    col_roca = mezcla(col_roca, v3(0.42, 0.40, 0.40), suave(0.20, 0.36, h) * 0.6);

    let t = g.tiempo * 0.12;
    let flujo = r.crestas(f.local * 3.2 + v3(t * 0.5, -t * 1.6, t * 0.3), 3);
    let cerca_crater = suave(0.55, 0.95, dir.y);
    let rios = suave(0.56, 0.66, flujo) * cerca_crater;
    let crater = suave(0.975, 0.99, dir.y);
    // grietas brillantes en las llanuras
    let (f1, f2, _) = celular(f.local * 5.0);
    let zonas = suave(0.15, 0.35, r.fbm(f.local * 1.8 + v3(3.0, 0.0, 1.0), 2));
    let grietas = (1.0 - suave(0.0, 0.035, f2 - f1)) * (1.0 - suave(0.02, 0.10, h)) * zonas * 0.7;

    let mascara = (rios + crater + grietas).min(1.0);
    let pulso = 0.75 + 0.25 * (g.tiempo * 2.5 + flujo * 12.0).sin();
    let col_lava = mezcla(lava_a, lava_b, flujo * pulso) * (1.6 * u.intensidad);

    let luz = g.ambiente + lambert(n, l) * (1.0 - g.ambiente);
    let col = col_roca * luz;
    mezcla(col, col_lava, mascara) + atmosfera(n, v, l, atm)
}

// ------------------------------------------------------------------ AGUA (animado)
// Olas como suma de senos que perturban la normal + reflejo especular del sol.
fn agua(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (n_geo, l, v) = base_iluminacion(f, g);
    let h = f.dato;
    let dir = f.local.norm();
    let [agua_clara, agua_honda, tierra, atm] = u.colores;

    if h <= 0.0005 {
        let hondo = suave(0.0, 0.05, -h);
        let mut base = mezcla(agua_clara, agua_honda, hondo);
        let t = g.tiempo * (0.6 + 0.6 * u.intensidad);
        let p = dir * 60.0;
        let dx = (p.x * 1.0 + p.y * 0.3 + t * 1.3).cos() * 0.5 + ((p.x + p.z) * 0.7 - t * 0.9).cos() * 0.35;
        let dz = (p.z * 1.1 - p.y * 0.2 - t * 1.1).cos() * 0.5 + ((p.z - p.x) * 0.6 + t * 0.7).cos() * 0.35;
        let tg = dir.perpendicular();
        let bt = dir.cross(tg);
        let n_local = (dir + (tg * dx + bt * dz) * (0.07 * u.intensidad)).norm();
        let n = u.rot.direccion(n_local).norm();

        let difusa = lambert(n, l);
        let medio = (l + v).norm();
        let brillo = n.dot(medio).max(0.0).powf(90.0) * 1.4;
        let fresnel = (1.0 - n.dot(v).max(0.0)).powi(4);
        // espuma en la orilla que va y viene
        let espuma = suave(0.6, 0.9, ((-h) * 900.0 - g.tiempo * 3.0).sin()) * (1.0 - suave(0.0, 0.006, -h));
        base = mezcla(base, v3(0.9, 0.95, 1.0), espuma * 0.7);
        let lado_dia = suave(-0.1, 0.2, n_geo.dot(l));
        return base * (g.ambiente + difusa * 0.9) + v3(1.0, 0.95, 0.85) * (brillo * lado_dia) + atm * (fresnel * 0.35 * lado_dia);
    }

    let arena = v3(0.82, 0.74, 0.52);
    let roca = v3(0.38, 0.36, 0.35);
    let mut col = mezcla(arena, tierra, suave(0.003, 0.012, h));
    col = mezcla(col, roca, suave(0.10, 0.18, h));
    col = mezcla(col, v3(0.85, 0.85, 0.88), suave(0.22, 0.25, h)); // un poco de nieve/nube en la punta
    let luz = g.ambiente + lambert(n_geo, l) * (1.0 - g.ambiente);
    col * luz + atmosfera(n_geo, v, l, atm)
}

// ------------------------------------------------------------------ ADOQUIN
// Ruido celular (Worley): F2-F1 da las juntas entre piedras de las calles de Antigua.
fn adoquin(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (n, l, v) = base_iluminacion(f, g);
    let dir = f.local.norm();
    let h = f.dato;
    let [piedra, pasto, volcan, atm] = u.colores;

    let (f1, f2, id) = celular(dir * (45.0 * u.intensidad.max(0.2)));
    let junta = suave(0.03, 0.10, f2 - f1);
    let tono = 0.75 + 0.5 * id_a_f32(id);
    let mut col = mezcla(piedra * 0.35, piedra * tono, junta);

    let manchas = g.ruido.fbm(dir * 5.0, 3);
    col = mezcla(col, pasto * (0.85 + 0.3 * id_a_f32(id / 7)), suave(-0.12, 0.04, manchas));
    col = mezcla(col, volcan, suave(0.03, 0.08, h));
    col = mezcla(col, v3(0.35, 0.32, 0.30), suave(0.22, 0.30, h));

    let luz = g.ambiente + lambert(n, l) * (1.0 - g.ambiente);
    col * luz + atmosfera(n, v, l, atm)
}

// ------------------------------------------------------------------ TERRENO (cafetal)
// Colores por franjas de altura con bordes ruidosos + surcos de siembra.
fn terreno(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (n, l, v) = base_iluminacion(f, g);
    let dir = f.local.norm();
    let [verde, verde_oscuro, tierra_roja, atm] = u.colores;
    let hh = f.dato + g.ruido.fbm(dir * 14.0, 2) * 0.02;

    let mut col = tierra_roja;
    col = mezcla(col, verde, suave(0.015, 0.03, hh));
    col = mezcla(col, verde_oscuro, suave(0.07, 0.09, hh));
    col = mezcla(col, v3(0.45, 0.43, 0.40), suave(0.12, 0.15, hh));

    let s = (dir.dot(v3(0.62, 0.28, 0.73)) * 420.0 * u.intensidad + g.ruido.ruido(dir * 3.0) * 8.0).sin();
    let surcos = suave(-0.3, 0.3, s);
    let en_siembra = suave(0.02, 0.035, hh) * (1.0 - suave(0.08, 0.1, hh));
    col = col * (1.0 - en_siembra * 0.15 * (1.0 - surcos));

    // luz con tres escalones suaves (un toque "cel")
    let d = lambert(n, l);
    let esc = suave(0.0, 0.08, d) * 0.45 + suave(0.25, 0.35, d) * 0.3 + suave(0.6, 0.7, d) * 0.25;
    col * (g.ambiente + esc * (1.0 - g.ambiente)) + atmosfera(n, v, l, atm)
}

// ------------------------------------------------------------------ SELVA (mapeo de normales)
// La normal sale de una textura (mapa de normales) en espacio tangente y se lleva a
// mundo con la base TBN de la esfera.
fn selva(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (n_geo, l, v) = base_iluminacion(f, g);
    let dir = f.local.norm();
    let [selva_a, selva_b, claro, atm] = u.colores;

    let uv_u = dir.z.atan2(dir.x) / (2.0 * PI) + 0.5;
    let uv_v = dir.y.clamp(-1.0, 1.0).acos() / PI;
    let (nts, altura) = g.mapa.muestrear(uv_u, uv_v);

    let n = if g.usar_mapa_normales {
        let mut t = dir.cross(Vec3::Y);
        if t.len2() < 1e-6 {
            t = Vec3::X;
        }
        let t = t.norm();
        let b = dir.cross(t);
        let fuerza = u.intensidad;
        let nt = v3(nts.x * fuerza, nts.y * fuerza, nts.z).norm();
        let n_local = (t * nt.x + b * nt.y + dir * nt.z).norm();
        u.rot.direccion(n_local).norm()
    } else {
        n_geo
    };

    let mut col = mezcla(selva_a, selva_b, altura);
    let claros = g.ruido.fbm(dir * 4.0, 3);
    col = mezcla(col, claro, suave(-0.22, -0.32, claros));
    let lado_dia = suave(-0.15, 0.1, n_geo.dot(l));
    let luz = g.ambiente + lambert(n, l) * (1.0 - g.ambiente) * lado_dia;
    col * luz + atmosfera(n_geo, v, l, atm)
}

// ------------------------------------------------------------------ NAVE
// Blinn-Phong + borde de Fresnel + motores que brillan segun el empuje.
fn nave(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (mut n, l, v) = base_iluminacion(f, g);
    if n.dot(v) < 0.0 {
        n = -n;
    }
    if f.dato > 0.5 {
        let parpadeo = 0.85 + 0.15 * (g.tiempo * 40.0).sin();
        return f.color * (0.6 + 1.4 * u.intensidad * parpadeo);
    }
    let luz = u.sombra;
    let d = lambert(n, l) * luz;
    let medio = (l + v).norm();
    let esp = n.dot(medio).max(0.0).powf(40.0) * 0.6 * luz;
    let fresnel = (1.0 - n.dot(v).max(0.0)).powi(3) * 0.5;
    f.color * (g.ambiente + 0.12 + d) + Vec3::splat(esp) + u.colores[0] * fresnel
}

// ------------------------------------------------------------------ OBJETOS de superficie
// Iluminacion por escalones (toon) y ventanas que se encienden en el lado nocturno.
fn objeto(f: &Frag, u: &Uniforme, g: &Globales) -> Vec3 {
    let (mut n, l, v) = base_iluminacion(f, g);
    if n.dot(v) < 0.0 {
        n = -n;
    }
    let arriba = (f.pos - u.centro).norm();
    let dia = suave(-0.12, 0.15, arriba.dot(l));
    let d = lambert(n, l) * dia;
    let escalon = if d > 0.55 {
        1.0
    } else if d > 0.2 {
        0.7
    } else if d > 0.02 {
        0.45
    } else {
        0.2
    };
    let mut base = f.color;
    if f.dato < -0.5 {
        base = base.mul_v(u.tinte);
    }
    let mut col = base * (g.ambiente + escalon * (1.0 - g.ambiente) * (0.35 + 0.65 * dia));
    if f.dato > 0.5 {
        let noche = 1.0 - dia;
        col = mezcla(col * 0.5, f.color * 1.3, 0.15 + 0.85 * noche);
    }
    col
}

// ------------------------------------------------------------------ textura de normales
pub struct MapaNormales {
    ancho: usize,
    alto: usize,
    normales: Vec<Vec3>,
    altura: Vec<f32>,
}

impl MapaNormales {
    /// Genera el mapa de normales a partir de un mapa de alturas hecho con ruido 3D sobre la esfera
    /// (asi no hay costura en u). La normal se saca por diferencias finitas.
    pub fn generar(ruido: &Perlin, ancho: usize, alto: usize) -> MapaNormales {
        let mut altura = vec![0.0; ancho * alto];
        for y in 0..alto {
            for x in 0..ancho {
                let u = (x as f32 + 0.5) / ancho as f32;
                let v = (y as f32 + 0.5) / alto as f32;
                let lon = (u - 0.5) * 2.0 * PI;
                let lat = v * PI;
                let dir = v3(lat.sin() * lon.cos(), lat.cos(), lat.sin() * lon.sin());
                // copas de arboles: celdas redondeadas + algo de fBm
                let (f1, _, _) = celular(dir * 70.0);
                let copa = 1.0 - (f1 * 1.4).min(1.0);
                altura[y * ancho + x] = (copa * 0.7 + (ruido.fbm(dir * 12.0, 4) * 0.5 + 0.5) * 0.3).clamp(0.0, 1.0);
            }
        }
        let mut normales = vec![Vec3::Z; ancho * alto];
        let fuerza = 6.0;
        for y in 0..alto {
            for x in 0..ancho {
                let h = |xx: i64, yy: i64| {
                    let xx = xx.rem_euclid(ancho as i64) as usize;
                    let yy = yy.clamp(0, alto as i64 - 1) as usize;
                    altura[yy * ancho + xx]
                };
                let (xi, yi) = (x as i64, y as i64);
                let dhdu = (h(xi + 1, yi) - h(xi - 1, yi)) * 0.5;
                let dhdv = (h(xi, yi + 1) - h(xi, yi - 1)) * 0.5;
                normales[y * ancho + x] = v3(-dhdu * fuerza, -dhdv * fuerza, 1.0).norm();
            }
        }
        MapaNormales { ancho, alto, normales, altura }
    }

    /// Muestreo bilineal. Devuelve (normal en espacio tangente, altura).
    pub fn muestrear(&self, u: f32, v: f32) -> (Vec3, f32) {
        let x = u * self.ancho as f32 - 0.5;
        let y = (v * self.alto as f32 - 0.5).clamp(0.0, self.alto as f32 - 1.0);
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let xi0 = (x0 as i64).rem_euclid(self.ancho as i64) as usize;
        let xi1 = (xi0 + 1) % self.ancho;
        let yi0 = y0 as usize;
        let yi1 = (yi0 + 1).min(self.alto - 1);
        let i = |xx: usize, yy: usize| yy * self.ancho + xx;
        let n = self.normales[i(xi0, yi0)] * ((1.0 - tx) * (1.0 - ty))
            + self.normales[i(xi1, yi0)] * (tx * (1.0 - ty))
            + self.normales[i(xi0, yi1)] * ((1.0 - tx) * ty)
            + self.normales[i(xi1, yi1)] * (tx * ty);
        let a = self.altura[i(xi0, yi0)] * ((1.0 - tx) * (1.0 - ty))
            + self.altura[i(xi1, yi0)] * (tx * (1.0 - ty))
            + self.altura[i(xi0, yi1)] * ((1.0 - tx) * ty)
            + self.altura[i(xi1, yi1)] * (tx * ty);
        (n.norm(), a)
    }
}
