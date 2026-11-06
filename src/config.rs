// Todo lo que se puede cambiar en vivo vive en config.toml. Aqui solo estan los tipos y
// los valores por defecto (si una llave falta en el archivo se usa el de aqui).
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub sol: SolCfg,
    pub nave: NaveCfg,
    pub camara: CamaraCfg,
    pub render: RenderCfg,
    #[serde(rename = "planeta")]
    pub planetas: Vec<PlanetaCfg>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct General {
    pub semilla: u64,
    pub ventana: [i32; 2],
    pub fps_max: u32,
    pub velocidad_tiempo: f32,
    /// Escala global de los objetos de superficie (casas, arboles...)
    pub escala_objetos: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SolCfg {
    pub radio: f32,
    pub masa: f32,
    pub intensidad: f32,
    pub color_a: [f32; 3],
    pub color_b: [f32; 3],
    pub velocidad_rotacion: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct NaveCfg {
    pub escala: f32,
    pub aceleracion: f32,
    pub velocidad_max: f32,
    pub multiplicador_turbo: f32,
    pub frenado: f32,
    pub giro_max: f32,
    /// Que tan rapido responde el giro (mas alto = mas brusco)
    pub respuesta_giro: f32,
    pub gravedad: f32,
    pub posicion_inicial: [f32; 3],
    /// Distancia (en radios del planeta) donde se puede activar el modo orbita
    pub alcance_orbita: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct CamaraCfg {
    pub fov: f32,
    pub distancia: f32,
    pub altura: f32,
    pub suavizado: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct RenderCfg {
    pub resolucion: [usize; 2],
    pub filtro_suave: bool,
    pub cerca: f32,
    pub lejos: f32,
    pub estrellas: usize,
    pub luz_ambiente: f32,
    /// Distancias (en radios) para cambiar de nivel de detalle: [nivel 5, 4, 3]
    pub lod_distancias: [f32; 3],
    /// Mas alla de esta distancia (en radios) ya no se dibujan objetos de superficie
    pub distancia_objetos: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PlanetaCfg {
    pub nombre: String,
    /// volcan | ciudad | lago | cafetal | selva
    pub tipo: String,
    pub radio: f32,
    pub distancia_orbita: f32,
    pub velocidad_orbital: f32,
    pub velocidad_rotacion: f32,
    pub fase: f32,
    pub inclinacion_eje: f32,
    pub inclinacion_orbita: f32,
    pub masa: f32,
    pub deformacion: f32,
    pub intensidad_shader: f32,
    pub color_a: [f32; 3],
    pub color_b: [f32; 3],
    pub color_c: [f32; 3],
    pub atmosfera: [f32; 3],
    /// Se suma a la semilla general (cambiarla mueve los objetos de este planeta)
    pub semilla: u64,
    pub objetos: u32,
}

impl Default for General {
    fn default() -> Self {
        General { semilla: 2026, ventana: [1280, 720], fps_max: 144, velocidad_tiempo: 1.0, escala_objetos: 0.16 }
    }
}

impl Default for SolCfg {
    fn default() -> Self {
        SolCfg {
            radio: 9.0,
            masa: 420.0,
            intensidad: 1.0,
            color_a: [1.0, 0.85, 0.35],
            color_b: [0.95, 0.35, 0.05],
            velocidad_rotacion: 0.03,
        }
    }
}

impl Default for NaveCfg {
    fn default() -> Self {
        NaveCfg {
            escala: 0.22,
            aceleracion: 14.0,
            velocidad_max: 20.0,
            multiplicador_turbo: 2.5,
            frenado: 2.5,
            giro_max: 1.8,
            respuesta_giro: 5.0,
            gravedad: 1.0,
            posicion_inicial: [0.0, 6.0, 80.0],
            alcance_orbita: 3.0,
        }
    }
}

impl Default for CamaraCfg {
    fn default() -> Self {
        CamaraCfg { fov: 65.0, distancia: 1.6, altura: 0.45, suavizado: 8.0 }
    }
}

impl Default for RenderCfg {
    fn default() -> Self {
        RenderCfg {
            resolucion: [640, 360],
            filtro_suave: false,
            cerca: 0.02,
            lejos: 2000.0,
            estrellas: 1500,
            luz_ambiente: 0.07,
            lod_distancias: [3.0, 8.0, 25.0],
            distancia_objetos: 6.0,
        }
    }
}

impl Default for PlanetaCfg {
    fn default() -> Self {
        PlanetaCfg {
            nombre: "Planeta".into(),
            tipo: "cafetal".into(),
            radio: 4.0,
            distancia_orbita: 40.0,
            velocidad_orbital: 0.03,
            velocidad_rotacion: 0.1,
            fase: 0.0,
            inclinacion_eje: 0.2,
            inclinacion_orbita: 0.0,
            masa: 20.0,
            deformacion: 1.0,
            intensidad_shader: 1.0,
            color_a: [0.5, 0.5, 0.5],
            color_b: [0.3, 0.3, 0.3],
            color_c: [0.8, 0.8, 0.8],
            atmosfera: [0.4, 0.6, 1.0],
            semilla: 0,
            objetos: 40,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            general: General::default(),
            sol: SolCfg::default(),
            nave: NaveCfg::default(),
            camara: CamaraCfg::default(),
            render: RenderCfg::default(),
            planetas: vec![PlanetaCfg::default()],
        }
    }
}

/// Busca un archivo primero en la carpeta actual y luego en la del proyecto.
pub fn buscar(nombre: &str) -> PathBuf {
    let local = PathBuf::from(nombre);
    if local.exists() {
        return local;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(nombre)
}

pub fn cargar(ruta: &Path) -> Result<Config, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("no se pudo leer {}: {}", ruta.display(), e))?;
    let cfg: Config = toml::from_str(&texto).map_err(|e| format!("config.toml: {}", e.message()))?;
    if cfg.planetas.is_empty() {
        return Err("config.toml: tiene que haber al menos un [[planeta]]".into());
    }
    Ok(cfg)
}
