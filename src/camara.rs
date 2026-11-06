// Camara en tercera persona que sigue a la nave, mas una vista de cabina.
use crate::config::CamaraCfg;
use crate::math::{amortiguar, Mat4, Vec3};
use crate::nave::{Entrada, Nave};
use crate::planeta::Planeta;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ModoCamara {
    Persecucion,
    Lejana,
    Cabina,
}

impl ModoCamara {
    pub fn siguiente(self) -> ModoCamara {
        match self {
            ModoCamara::Persecucion => ModoCamara::Lejana,
            ModoCamara::Lejana => ModoCamara::Cabina,
            ModoCamara::Cabina => ModoCamara::Persecucion,
        }
    }
    pub fn nombre(self) -> &'static str {
        match self {
            ModoCamara::Persecucion => "persecucion",
            ModoCamara::Lejana => "lejana",
            ModoCamara::Cabina => "cabina",
        }
    }
}

pub struct Camara {
    pub pos: Vec3,
    pub objetivo: Vec3,
    pub arriba: Vec3,
    pub modo: ModoCamara,
    /// desplazamiento respecto a la nave (se suaviza la direccion, no la traslacion)
    offset: Vec3,
    mirar_yaw: f32,
    mirar_pitch: f32,
}

impl Camara {
    pub fn new(nave: &Nave) -> Camara {
        let offset = -nave.adelante * 2.0 + nave.arriba * 0.5;
        Camara {
            pos: nave.pos + offset,
            objetivo: nave.pos,
            arriba: nave.arriba,
            modo: ModoCamara::Persecucion,
            offset,
            mirar_yaw: 0.0,
            mirar_pitch: 0.0,
        }
    }

    pub fn actualizar(&mut self, nave: &Nave, cfg: &CamaraCfg, escala_nave: f32, vmax: f32, e: &Entrada, dt: f32, planetas: &[Planeta]) {
        // mirar alrededor con el raton (clic derecho); al soltar regresa al centro
        if e.mirar {
            self.mirar_yaw = (self.mirar_yaw - e.raton.0 * 0.005).clamp(-2.8, 2.8);
            self.mirar_pitch = (self.mirar_pitch - e.raton.1 * 0.005).clamp(-1.2, 1.2);
        } else {
            let k = amortiguar(4.0, dt);
            self.mirar_yaw -= self.mirar_yaw * k;
            self.mirar_pitch -= self.mirar_pitch * k;
        }

        let esc = escala_nave / 0.22;
        if self.modo == ModoCamara::Cabina {
            self.pos = nave.pos + nave.adelante * (0.25 * esc) + nave.arriba * (0.07 * esc);
            let dir = nave.adelante.rotar(nave.arriba, self.mirar_yaw).rotar(nave.derecha(), self.mirar_pitch);
            self.objetivo = self.pos + dir;
            self.arriba = nave.arriba;
            self.offset = self.pos - nave.pos;
            return;
        }

        let lejos = if self.modo == ModoCamara::Lejana { 3.5 } else { 1.0 };
        let extra = 1.0 + (nave.rapidez() / vmax.max(1.0)).min(2.0) * 0.25;
        let dist = cfg.distancia * esc * lejos * extra;
        let atras = (-nave.adelante).rotar(nave.arriba, self.mirar_yaw).rotar(nave.derecha(), self.mirar_pitch);
        let deseado = atras * dist + nave.arriba * (cfg.altura * esc * lejos);
        self.offset = self.offset.lerp(deseado, amortiguar(cfg.suavizado, dt));
        self.pos = nave.pos + self.offset;
        self.objetivo = nave.pos + nave.adelante * (0.6 * esc * lejos);
        self.arriba = self.arriba.lerp(nave.arriba, amortiguar(cfg.suavizado * 0.7, dt)).norm();

        // que la camara no se meta dentro de un planeta
        for p in planetas {
            let d = self.pos - p.centro;
            if d.len() > p.radio_max() + 1.0 {
                continue;
            }
            let minimo = p.radio_en(self.pos) + 0.05 * esc;
            if d.len() < minimo {
                self.pos = p.centro + d.norm() * minimo;
            }
        }
    }

    pub fn vista(&self) -> Mat4 {
        Mat4::mirar(self.pos, self.objetivo, self.arriba)
    }
}
