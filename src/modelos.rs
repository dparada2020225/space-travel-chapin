// Los modelos .obj del proyecto (assets/modelos).
use crate::config::buscar;
use crate::malla::{cargar_obj, Malla};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modelo {
    Nave,
    Casa,
    Arco,
    Cafeto,
    Ceiba,
    Piramide,
    Roca,
}

impl Modelo {
    pub const TODOS: [Modelo; 7] =
        [Modelo::Nave, Modelo::Casa, Modelo::Arco, Modelo::Cafeto, Modelo::Ceiba, Modelo::Piramide, Modelo::Roca];

    pub fn archivo(self) -> &'static str {
        match self {
            Modelo::Nave => "nave.obj",
            Modelo::Casa => "casa.obj",
            Modelo::Arco => "arco.obj",
            Modelo::Cafeto => "cafeto.obj",
            Modelo::Ceiba => "ceiba.obj",
            Modelo::Piramide => "piramide.obj",
            Modelo::Roca => "roca.obj",
        }
    }
}

pub struct Modelos {
    mallas: Vec<Malla>,
}

impl Modelos {
    pub fn cargar() -> Result<Modelos, String> {
        let carpeta = buscar("assets/modelos");
        let mut mallas = Vec::new();
        for m in Modelo::TODOS {
            mallas.push(cargar_obj(&carpeta.join(m.archivo()))?);
        }
        Ok(Modelos { mallas })
    }

    pub fn get(&self, m: Modelo) -> &Malla {
        &self.mallas[m as usize]
    }
}
