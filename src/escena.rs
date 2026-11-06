// La escena completa: sol, planetas, nave, camara y lo que se manda a dibujar cada cuadro.
use crate::camara::{Camara, ModoCamara};
use crate::config::Config;
use crate::malla::{icosfera, Malla};
use crate::math::{suave, v3, Mat4, Vec3};
use crate::modelos::{Modelo, Modelos};
use crate::nave::{Cuerpo, Entrada, Modo, Nave};
use crate::planeta::Planeta;
use crate::raster::{Estadisticas, Frustum};
use crate::render::Renderer;
use crate::ruido::{Perlin, Rng};
use crate::shaders::{Globales, MapaNormales, Material, Uniforme};

/// Paso fijo de la fisica: la nave se comporta igual a 30 o a 200 FPS.
const PASO_FISICA: f32 = 1.0 / 120.0;
const CAJA_POLVO: f32 = 14.0;

pub struct Opciones {
    pub descarte: bool,
    pub lod: bool,
    pub gravedad: bool,
    pub orbitas: bool,
    pub mapa_normales: bool,
    pub hud: bool,
    pub ayuda: bool,
    pub pausa: bool,
}

pub struct Escena {
    pub cfg: Config,
    modelos: Modelos,
    pub planetas: Vec<Planeta>,
    malla_sol: Malla,
    pub nave: Nave,
    pub camara: Camara,
    pub tiempo: f32,
    pub opciones: Opciones,
    /// 0 = sol, 1..n = planetas
    pub objetivo: Option<usize>,
    ruido: Perlin,
    mapa: MapaNormales,
    estrellas: Vec<(Vec3, f32)>,
    polvo: Vec<Vec3>,
    acumulador: f32,
    visibilidad_sol: f32,
    pub stats: Estadisticas,
    pub mensaje: Option<(String, f32)>,
    ultima_vp: Mat4,
}

pub struct InfoHud {
    pub rapidez: f32,
    pub modo: String,
    pub camara: &'static str,
    pub cercano: Option<(String, f32)>,
    pub puede_orbitar: bool,
    pub objetivo: Option<(String, f32, Option<(f32, f32)>)>,
    pub stats: Estadisticas,
    pub semilla: u64,
    pub mensaje: Option<String>,
}

impl Escena {
    pub fn new(cfg: Config, modelos: Modelos) -> Escena {
        let nave = Nave::new(&cfg.nave);
        let camara = Camara::new(&nave);
        let ruido = Perlin::new(cfg.general.semilla);
        let mapa = MapaNormales::generar(&ruido, 512, 256);
        let mut e = Escena {
            planetas: Vec::new(),
            malla_sol: icosfera(4),
            nave,
            camara,
            tiempo: 0.0,
            opciones: Opciones {
                descarte: true,
                lod: true,
                gravedad: true,
                orbitas: true,
                mapa_normales: true,
                hud: true,
                ayuda: false,
                pausa: false,
            },
            objetivo: Some(2),
            ruido,
            mapa,
            estrellas: Vec::new(),
            polvo: Vec::new(),
            acumulador: 0.0,
            visibilidad_sol: 1.0,
            stats: Estadisticas::default(),
            mensaje: None,
            ultima_vp: Mat4::identidad(),
            modelos,
            cfg,
        };
        e.construir();
        e.actualizar_planetas(0.0);
        e
    }

    /// (Re)genera planetas, objetos, estrellas y polvo a partir de la config actual.
    fn construir(&mut self) {
        let g = &self.cfg.general;
        self.planetas = self
            .cfg
            .planetas
            .iter()
            .enumerate()
            .map(|(i, p)| Planeta::new(p, g.semilla, i, g.escala_objetos))
            .collect();
        let mut rng = Rng::new(77);
        self.estrellas = (0..self.cfg.render.estrellas).map(|_| (rng.direccion(), rng.rango(0.15, 0.9).powi(2))).collect();
        self.polvo = (0..220)
            .map(|_| v3(rng.rango(0.0, CAJA_POLVO), rng.rango(0.0, CAJA_POLVO), rng.rango(0.0, CAJA_POLVO)))
            .collect();
    }

    /// Se llama cuando cambia config.toml: reconstruye todo pero deja la nave y el tiempo como estaban.
    pub fn recargar(&mut self, cfg: Config) {
        let cambio_semilla = cfg.general.semilla != self.cfg.general.semilla;
        self.cfg = cfg;
        if cambio_semilla {
            self.ruido = Perlin::new(self.cfg.general.semilla);
            self.mapa = MapaNormales::generar(&self.ruido, 512, 256);
        }
        self.construir();
        if let Modo::Orbita { planeta } = self.nave.modo {
            if planeta >= self.planetas.len() {
                self.nave.salir_orbita();
            }
        }
        self.nave.radio = self.cfg.nave.escala * 0.9;
        self.actualizar_planetas(0.0);
        self.avisar("config.toml recargado");
    }

    pub fn avisar(&mut self, texto: &str) {
        self.mensaje = Some((texto.to_string(), 3.0));
    }

    fn actualizar_planetas(&mut self, dt: f32) {
        for p in &mut self.planetas {
            p.actualizar(self.tiempo, dt);
        }
    }

    fn cuerpo_sol(&self) -> Cuerpo {
        Cuerpo { centro: Vec3::ZERO, masa: self.cfg.sol.masa, radio: self.cfg.sol.radio }
    }

    pub fn actualizar(&mut self, dt: f32, e: &Entrada) {
        let dt = dt.min(0.1);
        if let Some((_, t)) = &mut self.mensaje {
            *t -= dt;
            if *t <= 0.0 {
                self.mensaje = None;
            }
        }
        self.acumulador += dt;
        let sol = self.cuerpo_sol();
        while self.acumulador >= PASO_FISICA {
            self.acumulador -= PASO_FISICA;
            let paso_mundo = if self.opciones.pausa { 0.0 } else { PASO_FISICA * self.cfg.general.velocidad_tiempo };
            self.tiempo += paso_mundo;
            self.actualizar_planetas(PASO_FISICA);
            self.nave.paso(PASO_FISICA, e, &self.cfg.nave, &self.planetas, &sol, self.opciones.gravedad);
        }
        let vmax = self.cfg.nave.velocidad_max;
        self.camara.actualizar(&self.nave, &self.cfg.camara, self.cfg.nave.escala, vmax, e, dt, &self.planetas);
    }

    pub fn alternar_orbita(&mut self) {
        match self.nave.modo {
            Modo::Orbita { .. } => {
                self.nave.salir_orbita();
                self.avisar("modo vuelo libre");
            }
            Modo::Libre => {
                if let Some((i, alt)) = self.nave.planeta_cercano(&self.planetas) {
                    let p = &self.planetas[i];
                    if alt < self.cfg.nave.alcance_orbita * p.cfg.radio {
                        self.nave.entrar_orbita(i, p);
                        let nombre = p.cfg.nombre.clone();
                        self.avisar(&format!("modo orbita: {}", nombre));
                        return;
                    }
                }
                self.avisar("acercate mas a un planeta para orbitar");
            }
        }
    }

    pub fn cambiar_camara(&mut self) {
        self.camara.modo = self.camara.modo.siguiente();
    }

    fn nombre_cuerpo(&self, i: usize) -> String {
        if i == 0 {
            "Sol".into()
        } else {
            self.planetas.get(i - 1).map(|p| p.cfg.nombre.clone()).unwrap_or_default()
        }
    }

    fn centro_cuerpo(&self, i: usize) -> Option<Vec3> {
        if i == 0 {
            Some(Vec3::ZERO)
        } else {
            self.planetas.get(i - 1).map(|p| p.centro)
        }
    }

    /// Si la nave esta a la sombra de algun planeta (rayo nave -> sol contra esferas).
    fn sombra_en(&self, punto: Vec3) -> f32 {
        let hacia_sol = (Vec3::ZERO - punto).norm();
        for p in &self.planetas {
            let oc = p.centro - punto;
            let t = oc.dot(hacia_sol);
            if t <= 0.0 {
                continue;
            }
            let d2 = oc.len2() - t * t;
            let r = p.cfg.radio;
            if d2 < r * r {
                return 0.15 + 0.85 * suave(r * 0.85, r, d2.sqrt());
            }
        }
        1.0
    }

    pub fn dibujar(&mut self, r: &mut Renderer) {
        let rc = self.cfg.render.clone();
        let cerca = rc.cerca;
        r.comenzar(cerca);
        let aspecto = r.ancho as f32 / r.alto as f32;
        let proy = Mat4::perspectiva(self.cfg.camara.fov.to_radians(), aspecto, cerca, rc.lejos);
        let vista = self.camara.vista();
        let vp = proy.mul(&vista);
        self.ultima_vp = vp;
        let frustum = Frustum::desde(&vp);
        let cam = self.camara.pos;
        let descarte = self.opciones.descarte;
        let lod = self.opciones.lod;

        // ---- sol
        let s = self.cfg.sol.clone();
        let u_sol = r.uniforme(Uniforme {
            material: Material::Sol,
            colores: [Vec3::from_arr(s.color_a), Vec3::from_arr(s.color_b), Vec3::ZERO, Vec3::ZERO],
            intensidad: s.intensidad,
            ..Default::default()
        });
        if !descarte || frustum.esfera_visible(Vec3::ZERO, s.radio) {
            let rot = Mat4::rot_y(self.tiempo * s.velocidad_rotacion);
            let m = rot.mul(&Mat4::escala(s.radio));
            r.rast.dibujar(&self.malla_sol, &m, &rot, &vp, u_sol, false);
            r.rast.stats.objetos_dibujados += 1;
        } else {
            r.rast.stats.objetos_descartados += 1;
        }

        // ---- planetas y sus objetos
        let [d5, d4, d3] = rc.lod_distancias;
        for p in &self.planetas {
            let centro = p.centro;
            if descarte && !frustum.esfera_visible(centro, p.radio_max()) {
                r.rast.stats.objetos_descartados += 1 + p.instancias.len() as u32;
                continue;
            }
            let pc = &p.cfg;
            let dist = (cam - centro).len() / pc.radio;
            let nivel = if !lod {
                p.lods.len() - 1
            } else if dist < d5 {
                3
            } else if dist < d4 {
                2
            } else if dist < d3 {
                1
            } else {
                0
            }
            .min(p.lods.len() - 1);
            let rot = p.rotacion();
            let modelo = p.matriz_modelo();
            let u = r.uniforme(Uniforme {
                material: p.tipo.material(),
                rot,
                colores: [
                    Vec3::from_arr(pc.color_a),
                    Vec3::from_arr(pc.color_b),
                    Vec3::from_arr(pc.color_c),
                    Vec3::from_arr(pc.atmosfera),
                ],
                intensidad: pc.intensidad_shader,
                centro,
                ..Default::default()
            });
            r.rast.dibujar(&p.lods[nivel], &modelo, &rot, &vp, u, false);
            r.rast.stats.objetos_dibujados += 1;

            // objetos de superficie: solo cerca (LOD) y solo los que dan hacia la camara
            if lod && dist > rc.distancia_objetos {
                r.rast.stats.objetos_descartados += p.instancias.len() as u32;
                continue;
            }
            for inst in &p.instancias {
                let normal = rot.direccion(inst.dir);
                let pos = modelo.punto(inst.dir * pc.radio);
                if descarte {
                    let hacia_cam = (cam - pos).norm();
                    if normal.dot(hacia_cam) < -0.2 || !frustum.esfera_visible(pos, inst.radio) {
                        r.rast.stats.objetos_descartados += 1;
                        continue;
                    }
                }
                let m = modelo.mul(&inst.local);
                let ui = r.uniforme(Uniforme { material: Material::Objeto, tinte: inst.tinte, centro, ..Default::default() });
                r.rast.dibujar(self.modelos.get(inst.modelo), &m, &m, &vp, ui, false);
                r.rast.stats.instancias += 1;
                r.rast.stats.objetos_dibujados += 1;
            }
        }

        // ---- nave (en cabina no se dibuja)
        if self.camara.modo != ModoCamara::Cabina {
            let n = &self.nave;
            let e = self.cfg.nave.escala;
            let m = Mat4::desde_base(n.derecha() * e, n.arriba * e, -n.adelante * e, n.pos);
            let un = r.uniforme(Uniforme {
                material: Material::Nave,
                rot: m,
                colores: [v3(0.35, 0.85, 1.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO],
                intensidad: n.empuje,
                sombra: self.sombra_en(n.pos),
                ..Default::default()
            });
            r.rast.dibujar(self.modelos.get(Modelo::Nave), &m, &m, &vp, un, true);
            r.rast.stats.objetos_dibujados += 1;
        }

        // ---- sombreado (fragment shaders)
        let g = Globales {
            tiempo: self.tiempo,
            luz: Vec3::ZERO,
            cam,
            ambiente: rc.luz_ambiente,
            ruido: &self.ruido,
            mapa: &self.mapa,
            usar_mapa_normales: self.opciones.mapa_normales,
        };
        r.sombrear(&g);

        // ---- extras 2D
        r.estrellas(&vp, cam, &self.estrellas, self.tiempo, rc.lejos);
        if self.opciones.orbitas {
            for p in &self.planetas {
                let pts = p.puntos_orbita(160);
                let col = Vec3::from_arr(p.cfg.atmosfera);
                for w in pts.windows(2) {
                    // se desvanecen cerca de la camara para no tapar la vista al acercarse
                    let d = ((w[0] + w[1]) * 0.5 - cam).len();
                    let alfa = 0.28 * suave(8.0, 50.0, d);
                    if alfa > 0.01 {
                        r.linea(&vp, w[0], w[1], col, alfa);
                    }
                }
            }
        }
        r.polvo(&vp, cam, &self.polvo, CAJA_POLVO);

        if let Some((x, y, iw)) = r.proyectar(&vp, Vec3::ZERO) {
            let visible = match r.obj_en(x, y) {
                None => 1.0,
                Some(o) if o == u_sol => 1.0,
                _ => 0.0,
            };
            self.visibilidad_sol += (visible - self.visibilidad_sol) * 0.25;
            let f = 1.0 / (self.cfg.camara.fov.to_radians() * 0.5).tan();
            let radio_px = s.radio * iw * f * r.alto as f32 * 0.5;
            r.resplandor(x, y, radio_px * 3.2, Vec3::from_arr(s.color_a) * 0.55, self.visibilidad_sol * s.intensidad);
        }
        self.stats = r.rast.stats;
    }

    pub fn hud(&self, r: &Renderer) -> InfoHud {
        let n = &self.nave;
        let cercano = n.planeta_cercano(&self.planetas).map(|(i, alt)| (self.planetas[i].cfg.nombre.clone(), alt));
        let puede_orbitar = n
            .planeta_cercano(&self.planetas)
            .map(|(i, alt)| alt < self.cfg.nave.alcance_orbita * self.planetas[i].cfg.radio)
            .unwrap_or(false);
        let modo = match n.modo {
            Modo::Libre => "VUELO LIBRE".to_string(),
            Modo::Orbita { planeta } => format!("ORBITA ({})", self.planetas[planeta].cfg.nombre),
        };
        let objetivo = self.objetivo.and_then(|i| {
            let c = self.centro_cuerpo(i)?;
            let dist = (c - n.pos).len();
            let pantalla = r.proyectar(&self.ultima_vp, c).map(|(x, y, _)| (x / r.ancho as f32, y / r.alto as f32));
            Some((self.nombre_cuerpo(i), dist, pantalla))
        });
        InfoHud {
            rapidez: n.rapidez(),
            modo,
            camara: self.camara.modo.nombre(),
            cercano,
            puede_orbitar,
            objetivo,
            stats: self.stats,
            semilla: self.cfg.general.semilla,
            mensaje: self.mensaje.as_ref().map(|m| m.0.clone()),
        }
    }
}
