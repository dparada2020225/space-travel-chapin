// Ruido de Perlin 3D (version mejorada de Ken Perlin), fBm, ruido celular y un RNG con semilla.
use crate::math::{v3, Vec3};

/// Generador pseudoaleatorio SplitMix64: misma semilla => misma secuencia.
#[derive(Clone)]
pub struct Rng {
    estado: u64,
}

impl Rng {
    pub fn new(semilla: u64) -> Rng {
        Rng { estado: semilla.wrapping_add(0x9E37_79B9_7F4A_7C15) }
    }
    pub fn siguiente(&mut self) -> u64 {
        self.estado = self.estado.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.estado;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Flotante en [0, 1)
    pub fn f32(&mut self) -> f32 {
        (self.siguiente() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn rango(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
    /// Direccion uniforme sobre la esfera unitaria.
    pub fn direccion(&mut self) -> Vec3 {
        let z = self.rango(-1.0, 1.0);
        let a = self.rango(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).sqrt();
        v3(r * a.cos(), z, r * a.sin())
    }
}

pub struct Perlin {
    perm: [u8; 512],
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn grad(h: u8, x: f32, y: f32, z: f32) -> f32 {
    // 12 gradientes hacia los bordes de un cubo
    let h = h & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Perlin {
    pub fn new(semilla: u64) -> Perlin {
        let mut p: Vec<u8> = (0..=255).collect();
        let mut rng = Rng::new(semilla);
        // Fisher-Yates
        for i in (1..256).rev() {
            let j = (rng.siguiente() % (i as u64 + 1)) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Perlin { perm }
    }

    /// Valor aproximadamente en [-1, 1]
    pub fn ruido(&self, p: Vec3) -> f32 {
        let (fx, fy, fz) = (p.x.floor(), p.y.floor(), p.z.floor());
        let xi = (fx as i32 & 255) as usize;
        let yi = (fy as i32 & 255) as usize;
        let zi = (fz as i32 & 255) as usize;
        let (x, y, z) = (p.x - fx, p.y - fy, p.z - fz);
        let (u, v, w) = (fade(x), fade(y), fade(z));
        let pm = &self.perm;
        let a = pm[xi] as usize + yi;
        let aa = pm[a] as usize + zi;
        let ab = pm[a + 1] as usize + zi;
        let b = pm[xi + 1] as usize + yi;
        let ba = pm[b] as usize + zi;
        let bb = pm[b + 1] as usize + zi;

        lerp(
            lerp(
                lerp(grad(pm[aa], x, y, z), grad(pm[ba], x - 1.0, y, z), u),
                lerp(grad(pm[ab], x, y - 1.0, z), grad(pm[bb], x - 1.0, y - 1.0, z), u),
                v,
            ),
            lerp(
                lerp(grad(pm[aa + 1], x, y, z - 1.0), grad(pm[ba + 1], x - 1.0, y, z - 1.0), u),
                lerp(
                    grad(pm[ab + 1], x, y - 1.0, z - 1.0),
                    grad(pm[bb + 1], x - 1.0, y - 1.0, z - 1.0),
                    u,
                ),
                v,
            ),
            w,
        )
    }

    /// Movimiento browniano fraccional: suma de octavas.
    pub fn fbm(&self, p: Vec3, octavas: u32) -> f32 {
        let mut suma = 0.0;
        let mut amp = 0.5;
        let mut q = p;
        for _ in 0..octavas {
            suma += amp * self.ruido(q);
            q = q * 2.03 + v3(17.1, 9.3, 3.7);
            amp *= 0.5;
        }
        suma
    }

    /// Ruido "ridged": crestas afiladas, sirve para rios de lava.
    pub fn crestas(&self, p: Vec3, octavas: u32) -> f32 {
        let mut suma = 0.0;
        let mut amp = 0.5;
        let mut q = p;
        for _ in 0..octavas {
            let n = 1.0 - self.ruido(q).abs();
            suma += amp * n * n;
            q = q * 2.1 + v3(5.2, 1.3, 7.7);
            amp *= 0.5;
        }
        suma
    }
}

fn hash3(x: i32, y: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

fn hash_f(h: u32, k: u32) -> f32 {
    let v = hash3(h as i32, k as i32, 0x51ed);
    (v & 0xffff) as f32 / 65535.0
}

/// Ruido celular (Worley): devuelve (F1, F2, id de la celda mas cercana).
pub fn celular(p: Vec3) -> (f32, f32, u32) {
    let (cx, cy, cz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let mut f1 = 9.0f32;
    let mut f2 = 9.0f32;
    let mut id = 0u32;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y, z) = (cx + dx, cy + dy, cz + dz);
                let h = hash3(x, y, z);
                let punto = v3(
                    x as f32 + hash_f(h, 1),
                    y as f32 + hash_f(h, 2),
                    z as f32 + hash_f(h, 3),
                );
                let d = (punto - p).len2();
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                    id = h;
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    (f1.sqrt(), f2.sqrt(), id)
}

pub fn id_a_f32(id: u32) -> f32 {
    (id % 1000) as f32 / 1000.0
}
