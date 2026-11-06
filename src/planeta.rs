// Planetas: deformacion de la esfera con ruido, niveles de detalle, orbita/rotacion
// y la distribucion procedural de objetos sobre la superficie.
use crate::config::PlanetaCfg;
use crate::malla::{icosfera, Malla};
use crate::math::{suave, v3, Mat4, Vec3};
use crate::modelos::Modelo;
use crate::ruido::{Perlin, Rng};
use crate::shaders::Material;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tipo {
    Volcan,
    Ciudad,
    Lago,
    Cafetal,
    Selva,
}

impl Tipo {
    pub fn desde(s: &str) -> Tipo {
        match s.to_lowercase().as_str() {
            "volcan" | "volcán" => Tipo::Volcan,
            "ciudad" => Tipo::Ciudad,
            "lago" => Tipo::Lago,
            "selva" => Tipo::Selva,
            _ => Tipo::Cafetal,
        }
    }

    pub fn material(self) -> Material {
        match self {
            Tipo::Volcan => Material::Lava,
            Tipo::Ciudad => Material::Adoquin,
            Tipo::Lago => Material::Agua,
            Tipo::Cafetal => Material::Terreno,
            Tipo::Selva => Material::Selva,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Instancia {
    pub modelo: Modelo,
    /// Transformacion en el espacio local del planeta (ya orientada a la superficie)
    pub local: Mat4,
    pub dir: Vec3,
    pub tinte: Vec3,
    pub radio: f32,
}

pub struct Planeta {
    pub cfg: PlanetaCfg,
    pub tipo: Tipo,
    ruido: Perlin,
    /// lods[0] es el de menos detalle
    pub lods: Vec<Malla>,
    pub instancias: Vec<Instancia>,
    // estado que cambia cada cuadro
    pub centro: Vec3,
    pub velocidad: Vec3,
    pub angulo: f32,
}

/// Niveles de subdivision del icosaedro para cada LOD (320, 1280, 5120 y 20480 triangulos).
pub const NIVELES_LOD: [u32; 4] = [2, 3, 4, 5];

fn cono(dir: Vec3, eje: Vec3, ancho: f32) -> f32 {
    let ang = dir.dot(eje).clamp(-1.0, 1.0).acos();
    (1.0 - ang / ancho).max(0.0)
}

impl Planeta {
    pub fn new(cfg: &PlanetaCfg, semilla_general: u64, indice: usize, escala_objetos: f32) -> Planeta {
        let semilla = semilla_general
            .wrapping_mul(31)
            .wrapping_add(indice as u64 * 1013)
            .wrapping_add(cfg.semilla);
        let mut p = Planeta {
            cfg: cfg.clone(),
            tipo: Tipo::desde(&cfg.tipo),
            ruido: Perlin::new(semilla),
            lods: Vec::new(),
            instancias: Vec::new(),
            centro: Vec3::ZERO,
            velocidad: Vec3::ZERO,
            angulo: 0.0,
        };
        p.generar_lods();
        p.generar_instancias(semilla, escala_objetos);
        p
    }

    /// Altura de la superficie en una direccion local, relativa al radio (0 = radio base).
    /// Puede ser negativa (fondo del lago).
    pub fn altura_cruda(&self, dir: Vec3) -> f32 {
        let r = &self.ruido;
        let k = self.cfg.deformacion;
        let h = match self.tipo {
            Tipo::Volcan => {
                // Fuego + Acatenango: dos conos, el grande con crater
                let polo = Vec3::Y;
                let c1 = cono(dir, polo, 0.95).powf(1.7) * 0.42;
                let crater = cono(dir, polo, 0.12).powi(2) * 0.10;
                let c2 = cono(dir, v3(0.45, 0.88, 0.15).norm(), 0.45).powf(1.5) * 0.22;
                let rugoso = r.fbm(dir * 3.0, 5) * 0.06 + r.crestas(dir * 6.0, 3) * 0.03;
                c1.max(c2) - crater + rugoso
            }
            Tipo::Ciudad => {
                // valle plano con el volcan de Agua a un lado
                let agua = cono(dir, v3(-0.2, 0.75, 0.62).norm(), 0.6).powf(1.6) * 0.34;
                agua + r.fbm(dir * 2.0, 4) * 0.025
            }
            Tipo::Lago => {
                // lago con tres volcanes en la orilla (San Pedro, Toliman y Atitlan)
                let mut h = r.fbm(dir * 1.7, 5) * 0.16 - 0.01;
                for (eje, alto) in [
                    (v3(0.30, 0.55, 0.78), 0.24),
                    (v3(0.05, 0.75, 0.66), 0.27),
                    (v3(-0.25, 0.62, 0.74), 0.21),
                ] {
                    h = h.max(cono(dir, eje.norm(), 0.32).powf(1.4) * alto);
                }
                h
            }
            Tipo::Cafetal => {
                let colinas = (r.fbm(dir * 2.3, 5) * 0.5 + 0.5).powf(1.3);
                colinas * 0.13 + r.crestas(dir * 5.0, 2) * 0.02
            }
            Tipo::Selva => r.fbm(dir * 3.0, 4) * 0.03 + 0.01,
        };
        h * k
    }

    /// Altura de lo que realmente se ve/choca (el agua queda plana a nivel 0).
    pub fn altura(&self, dir: Vec3) -> f32 {
        let h = self.altura_cruda(dir);
        if self.tipo == Tipo::Lago {
            h.max(0.0)
        } else {
            h
        }
    }

    fn generar_lods(&mut self) {
        let radio = self.cfg.radio;
        self.lods = NIVELES_LOD
            .iter()
            .map(|&n| {
                let mut m = icosfera(n);
                for v in &mut m.vertices {
                    let dir = v.pos;
                    let cruda = self.altura_cruda(dir);
                    let h = if self.tipo == Tipo::Lago { cruda.max(0.0) } else { cruda };
                    v.pos = dir * (radio * (1.0 + h));
                    v.dato = cruda;
                }
                m.recalcular_normales();
                // en el agua la normal es la de la esfera, para que el reflejo se vea liso
                if self.tipo == Tipo::Lago {
                    for v in &mut m.vertices {
                        if v.dato <= 0.0 {
                            v.normal = v.pos.norm();
                        }
                    }
                }
                m.calcular_limites();
                m
            })
            .collect();
    }

    /// Reglas de cada planeta: que objetos van, cuantos y donde se permiten.
    fn generar_instancias(&mut self, semilla: u64, escala: f32) {
        let mut rng = Rng::new(semilla ^ 0xC0FFEE);
        let total = self.cfg.objetos as usize;
        let mut lista: Vec<Instancia> = Vec::new();

        // (modelo, cantidad, separacion minima en radianes, escala min, escala max)
        let reglas: Vec<(Modelo, usize, f32, f32, f32)> = match self.tipo {
            Tipo::Volcan => vec![(Modelo::Roca, total, 0.06, 0.6, 1.6)],
            Tipo::Ciudad => vec![
                (Modelo::Arco, 1, 0.0, 1.0, 1.0),
                (Modelo::Casa, total, 0.045, 0.85, 1.15),
                (Modelo::Cafeto, total / 3, 0.04, 0.9, 1.2),
            ],
            Tipo::Lago => vec![(Modelo::Casa, total / 2, 0.05, 0.8, 1.0), (Modelo::Roca, total / 2, 0.06, 0.5, 1.2)],
            Tipo::Cafetal => vec![(Modelo::Cafeto, total, 0.03, 0.8, 1.3), (Modelo::Casa, 6, 0.3, 1.0, 1.1)],
            Tipo::Selva => vec![(Modelo::Piramide, 4, 0.55, 1.0, 1.3), (Modelo::Ceiba, total, 0.05, 0.8, 1.3)],
        };

        let colores_casa = [
            v3(0.96, 0.80, 0.36),
            v3(0.92, 0.55, 0.30),
            v3(0.40, 0.55, 0.85),
            v3(0.95, 0.93, 0.88),
            v3(0.80, 0.30, 0.30),
            v3(0.55, 0.75, 0.45),
        ];

        for (modelo, cantidad, separacion, esc_min, esc_max) in reglas {
            let mut puestos = 0;
            let mut intentos = 0;
            // el arco va donde la "densidad de ciudad" es mayor (centro del pueblo)
            let mejor_centro = if modelo == Modelo::Arco {
                let mut mejor = (f32::MIN, Vec3::Y);
                for _ in 0..400 {
                    let d = rng.direccion();
                    let s = self.densidad(modelo, d);
                    if s > mejor.0 {
                        mejor = (s, d);
                    }
                }
                Some(mejor.1)
            } else {
                None
            };

            while puestos < cantidad && intentos < cantidad * 40 + 100 {
                intentos += 1;
                let dir = mejor_centro.unwrap_or_else(|| rng.direccion());
                if mejor_centro.is_none() && self.densidad(modelo, dir) < rng.f32() {
                    continue;
                }
                let sep = separacion.max(if modelo == Modelo::Arco { 0.0 } else { 0.02 });
                let choca = lista.iter().any(|o| {
                    let minimo = if o.modelo == Modelo::Arco || o.modelo == Modelo::Piramide { sep.max(0.12) } else { sep };
                    o.dir.dot(dir).clamp(-1.0, 1.0).acos() < minimo
                });
                if choca {
                    continue;
                }
                let s = rng.rango(esc_min, esc_max) * escala * (self.cfg.radio / 5.0).sqrt();
                let giro = rng.rango(0.0, std::f32::consts::TAU);
                let tinte = if modelo == Modelo::Casa {
                    colores_casa[(rng.siguiente() % colores_casa.len() as u64) as usize]
                } else {
                    Vec3::splat(1.0)
                };
                let base = match modelo {
                    Modelo::Arco => 2.2,
                    Modelo::Piramide => 2.0,
                    Modelo::Cafeto => 1.6,
                    Modelo::Ceiba => 1.3,
                    _ => 1.0,
                };
                lista.push(Instancia {
                    modelo,
                    local: self.matriz_sobre_superficie(dir, giro, s * base),
                    dir,
                    tinte,
                    radio: s * base * 2.6,
                });
                puestos += 1;
            }
        }
        self.instancias = lista;
    }

    /// Probabilidad (0..1) de aceptar un objeto en esa direccion.
    fn densidad(&self, modelo: Modelo, dir: Vec3) -> f32 {
        let h = self.altura_cruda(dir);
        let r = &self.ruido;
        let n = r.fbm(dir * 3.0 + v3(4.0, 1.0, 7.0), 3);
        match (self.tipo, modelo) {
            (Tipo::Volcan, _) => {
                let lejos_crater = 1.0 - suave(0.85, 0.95, dir.y);
                (0.4 + n) * lejos_crater
            }
            (Tipo::Ciudad, Modelo::Casa) | (Tipo::Ciudad, Modelo::Arco) => {
                // barrios donde el ruido es alto y el terreno es plano
                suave(-0.05, 0.15, n) * (1.0 - suave(0.02, 0.05, h))
            }
            (Tipo::Ciudad, _) => suave(0.0, -0.2, n) * (1.0 - suave(0.04, 0.09, h)),
            (Tipo::Lago, Modelo::Casa) => {
                if h > 0.004 && h < 0.04 {
                    0.9
                } else {
                    0.0
                }
            }
            (Tipo::Lago, _) => suave(0.06, 0.12, h),
            (Tipo::Cafetal, Modelo::Cafeto) => {
                // los cafetos crecen en las laderas medias
                suave(0.02, 0.04, h) * (1.0 - suave(0.09, 0.12, h)) * (0.5 + n)
            }
            (Tipo::Cafetal, _) => suave(0.03, 0.05, h) * (1.0 - suave(0.07, 0.09, h)),
            (Tipo::Selva, Modelo::Piramide) => 0.8,
            (Tipo::Selva, _) => (0.6 + n).clamp(0.0, 1.0),
        }
    }

    /// Matriz local que pone un objeto parado sobre la superficie curva:
    /// su eje Y apunta hacia afuera (la normal de la esfera) y gira `giro` sobre ese eje.
    fn matriz_sobre_superficie(&self, dir: Vec3, giro: f32, escala: f32) -> Mat4 {
        let arriba = dir.norm();
        let t = arriba.perpendicular().rotar(arriba, giro);
        let b = t.cross(arriba);
        let altura = self.cfg.radio * (1.0 + self.altura(dir)) - escala * 0.04;
        Mat4::desde_base(t * escala, arriba * escala, b * escala, arriba * altura)
    }

    /// Orbita y rotacion dependen del tiempo transcurrido, no de los cuadros.
    pub fn actualizar(&mut self, t: f32, dt: f32) {
        let ang = self.cfg.fase + t * self.cfg.velocidad_orbital;
        let d = self.cfg.distancia_orbita;
        let plano = v3(ang.cos() * d, 0.0, ang.sin() * d);
        let nuevo = Mat4::rot_x(self.cfg.inclinacion_orbita).punto(plano);
        if dt > 0.0 {
            self.velocidad = (nuevo - self.centro) / dt;
        }
        self.centro = nuevo;
        self.angulo = t * self.cfg.velocidad_rotacion;
    }

    pub fn rotacion(&self) -> Mat4 {
        Mat4::rot_z(self.cfg.inclinacion_eje).mul(&Mat4::rot_y(self.angulo))
    }

    pub fn matriz_modelo(&self) -> Mat4 {
        Mat4::traslacion(self.centro).mul(&self.rotacion())
    }

    /// Radio de la superficie justo debajo de un punto del mundo.
    pub fn radio_en(&self, punto: Vec3) -> f32 {
        let local = self.rotacion().transpuesta().direccion(punto - self.centro).norm();
        self.cfg.radio * (1.0 + self.altura(local))
    }

    /// Radio maximo aproximado (para descarte y colision gruesa)
    pub fn radio_max(&self) -> f32 {
        self.lods.last().map(|m| m.radio).unwrap_or(self.cfg.radio)
    }

    pub fn puntos_orbita(&self, n: usize) -> Vec<Vec3> {
        let rot = Mat4::rot_x(self.cfg.inclinacion_orbita);
        (0..=n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                rot.punto(v3(a.cos(), 0.0, a.sin()) * self.cfg.distancia_orbita)
            })
            .collect()
    }
}
