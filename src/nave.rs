// Fisica y control de la nave: vuelo libre con inercia y modo orbita cerca de un planeta.
use crate::config::NaveCfg;
use crate::math::{amortiguar, v3, Vec3};
use crate::planeta::Planeta;

/// Entrada ya traducida desde el teclado/raton (main.rs). Valores entre -1 y 1.
#[derive(Default, Clone, Copy, Debug)]
pub struct Entrada {
    pub empuje: f32,
    pub giro: f32,
    pub cabeceo: f32,
    pub alabeo: f32,
    pub turbo: bool,
    pub freno: bool,
    pub raton: (f32, f32),
    pub mirar: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Modo {
    Libre,
    Orbita { planeta: usize },
}

pub struct Nave {
    pub pos: Vec3,
    pub vel: Vec3,
    pub adelante: Vec3,
    pub arriba: Vec3,
    /// velocidad angular actual (guinada, cabeceo, alabeo) en rad/s
    vel_ang: Vec3,
    /// 0..1, para el brillo de los motores
    pub empuje: f32,
    pub modo: Modo,
    // estado del modo orbita
    rumbo: Vec3,
    vel_superficie: f32,
    altitud: f32,
    altitud_obj: f32,
    pub radio: f32,
}

pub struct Cuerpo {
    pub centro: Vec3,
    pub masa: f32,
    pub radio: f32,
}

impl Nave {
    pub fn new(cfg: &NaveCfg) -> Nave {
        Nave {
            pos: Vec3::from_arr(cfg.posicion_inicial),
            vel: Vec3::ZERO,
            adelante: -Vec3::Z,
            arriba: Vec3::Y,
            vel_ang: Vec3::ZERO,
            empuje: 0.0,
            modo: Modo::Libre,
            rumbo: -Vec3::Z,
            vel_superficie: 0.0,
            altitud: 1.0,
            altitud_obj: 1.0,
            radio: cfg.escala * 0.9,
        }
    }

    pub fn derecha(&self) -> Vec3 {
        self.adelante.cross(self.arriba).norm()
    }

    pub fn rapidez(&self) -> f32 {
        self.vel.len()
    }

    /// Velocidad angular que se acerca poco a poco a la pedida: giro progresivo.
    fn girar_suave(&mut self, objetivo: Vec3, cfg: &NaveCfg, dt: f32) {
        let k = amortiguar(cfg.respuesta_giro, dt);
        self.vel_ang = self.vel_ang.lerp(objetivo, k);
    }

    pub fn entrar_orbita(&mut self, idx: usize, p: &Planeta) {
        let rel = self.pos - p.centro;
        let arriba = rel.norm();
        let mut rumbo = self.adelante - arriba * self.adelante.dot(arriba);
        if rumbo.len2() < 1e-4 {
            rumbo = arriba.perpendicular();
        }
        self.rumbo = rumbo.norm();
        let v_rel = self.vel - p.velocidad;
        self.vel_superficie = v_rel.dot(self.rumbo);
        self.altitud = (rel.len() - p.radio_en(self.pos)).max(0.05);
        self.altitud_obj = self.altitud;
        self.modo = Modo::Orbita { planeta: idx };
    }

    pub fn salir_orbita(&mut self) {
        self.modo = Modo::Libre;
    }

    pub fn paso(&mut self, dt: f32, e: &Entrada, cfg: &NaveCfg, planetas: &[Planeta], sol: &Cuerpo, gravedad: bool) {
        match self.modo {
            Modo::Libre => self.paso_libre(dt, e, cfg, planetas, sol, gravedad),
            Modo::Orbita { planeta } => {
                if let Some(p) = planetas.get(planeta) {
                    self.paso_orbita(dt, e, cfg, p)
                } else {
                    self.modo = Modo::Libre;
                }
            }
        }
        let meta = if e.turbo && e.empuje > 0.0 { 1.0 } else { e.empuje.abs() * 0.7 };
        self.empuje += (meta - self.empuje) * amortiguar(6.0, dt);
    }

    fn paso_libre(&mut self, dt: f32, e: &Entrada, cfg: &NaveCfg, planetas: &[Planeta], sol: &Cuerpo, gravedad: bool) {
        let g = cfg.giro_max;
        self.girar_suave(v3(e.giro * g, e.cabeceo * g * 0.8, e.alabeo * g * 1.4), cfg, dt);

        // guinada (alrededor de arriba), cabeceo (alrededor de derecha), alabeo (alrededor de adelante)
        self.adelante = self.adelante.rotar(self.arriba, self.vel_ang.x * dt);
        let der = self.derecha();
        self.adelante = self.adelante.rotar(der, self.vel_ang.y * dt);
        self.arriba = self.arriba.rotar(der, self.vel_ang.y * dt);
        self.arriba = self.arriba.rotar(self.adelante, self.vel_ang.z * dt);
        // reortonormalizar para que no se acumule error numerico
        self.adelante = self.adelante.norm();
        let der = self.derecha();
        self.arriba = der.cross(self.adelante).norm();

        // limite de velocidad: cerca de un planeta se vuela mas lento para poder apreciarlo
        let mut cercania: f32 = 1.0;
        for p in planetas {
            let alt = (self.pos - p.centro).len() - p.cfg.radio;
            cercania = cercania.min((alt / (p.cfg.radio * 2.0)).clamp(0.2, 1.0));
        }
        let turbo = if e.turbo { cfg.multiplicador_turbo } else { 1.0 };
        let vmax = cfg.velocidad_max * turbo * cercania;

        self.vel += self.adelante * (e.empuje * cfg.aceleracion * turbo * dt);

        if gravedad {
            let mut tirar = |c: Vec3, masa: f32, radio: f32| {
                let d = c - self.pos;
                let r2 = d.len2().max(radio * radio);
                self.vel += d.norm() * (cfg.gravedad * masa / r2 * dt);
            };
            tirar(sol.centro, sol.masa, sol.radio);
            for p in planetas {
                tirar(p.centro, p.cfg.masa, p.cfg.radio);
            }
        }

        if e.freno {
            self.vel *= (-cfg.frenado * dt).exp();
        }
        self.vel *= (-0.12 * dt).exp(); // un poquito de friccion para que sea controlable
        let rap = self.vel.len();
        if rap > vmax {
            self.vel = self.vel.lerp(self.vel.norm() * vmax, amortiguar(3.0, dt));
        }

        self.pos += self.vel * dt;
        self.colisiones(planetas, sol);
    }

    /// Colision basica: si la nave queda dentro de la superficie se empuja hacia afuera
    /// y se quita la parte de la velocidad que iba hacia adentro.
    fn colisiones(&mut self, planetas: &[Planeta], sol: &Cuerpo) {
        for p in planetas {
            let d = self.pos - p.centro;
            if d.len() > p.radio_max() + self.radio * 2.0 {
                continue;
            }
            let limite = p.radio_en(self.pos) + self.radio;
            if d.len() < limite {
                let n = d.norm();
                self.pos = p.centro + n * limite;
                let v_rel = self.vel - p.velocidad;
                let vn = v_rel.dot(n);
                if vn < 0.0 {
                    self.vel -= n * (vn * 1.2);
                }
            }
        }
        let d = self.pos - sol.centro;
        let limite = sol.radio + self.radio;
        if d.len() < limite {
            let n = d.norm();
            self.pos = sol.centro + n * limite;
            let vn = self.vel.dot(n);
            if vn < 0.0 {
                self.vel -= n * (vn * 1.5);
            }
        }
    }

    /// Modo orbita: la nave se mueve sobre circulos maximos del planeta, sigue su traslacion
    /// y mantiene una altitud sobre el terreno. W/S avanza, A/D cambia el rumbo, flechas suben/bajan.
    fn paso_orbita(&mut self, dt: f32, e: &Entrada, cfg: &NaveCfg, p: &Planeta) {
        let g = cfg.giro_max;
        self.girar_suave(v3(e.giro * g * 0.8, 0.0, 0.0), cfg, dt);

        let mut arriba = (self.pos - p.centro).norm();
        self.rumbo = self.rumbo.rotar(arriba, self.vel_ang.x * dt);
        self.rumbo = (self.rumbo - arriba * self.rumbo.dot(arriba)).norm();

        let turbo = if e.turbo { cfg.multiplicador_turbo } else { 1.0 };
        let vmax = cfg.velocidad_max * 0.35 * turbo * (p.cfg.radio / 5.0).sqrt();
        self.vel_superficie += e.empuje * cfg.aceleracion * 0.5 * dt;
        if e.freno {
            self.vel_superficie *= (-cfg.frenado * dt).exp();
        }
        self.vel_superficie *= (-0.3 * dt).exp();
        self.vel_superficie = self.vel_superficie.clamp(-vmax * 0.5, vmax);

        let alt_max = cfg.alcance_orbita * p.cfg.radio;
        self.altitud_obj = (self.altitud_obj + e.cabeceo * p.cfg.radio * 0.5 * dt).clamp(self.radio * 2.5, alt_max);
        self.altitud += (self.altitud_obj - self.altitud) * amortiguar(3.0, dt);

        let r = (self.pos - p.centro).len().max(0.01);
        let eje = arriba.cross(self.rumbo).norm();
        let ang = self.vel_superficie * dt / r;
        arriba = arriba.rotar(eje, ang).norm();
        self.rumbo = self.rumbo.rotar(eje, ang).norm();

        let superficie = p.radio_en(p.centro + arriba);
        let anterior = self.pos;
        self.pos = p.centro + arriba * (superficie + self.altitud);

        // orientacion: mirar hacia el rumbo con un poco de inclinacion al girar
        self.adelante = self.rumbo;
        self.arriba = arriba.rotar(self.rumbo, self.vel_ang.x * 0.25);
        // velocidad "real" en el mundo, para que al salir del modo no haya salto
        self.vel = if dt > 0.0 { (self.pos - anterior) / dt } else { Vec3::ZERO };
    }

    /// Planeta mas cercano y la altitud sobre su superficie.
    pub fn planeta_cercano(&self, planetas: &[Planeta]) -> Option<(usize, f32)> {
        planetas
            .iter()
            .enumerate()
            .map(|(i, p)| (i, (self.pos - p.centro).len() - p.radio_en(self.pos)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    }
}
