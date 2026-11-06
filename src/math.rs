// Vectores y matrices propios (mano derecha, Y arriba, la camara mira hacia -Z).
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);
    pub const X: Vec3 = v3(1.0, 0.0, 0.0);
    pub const Y: Vec3 = v3(0.0, 1.0, 0.0);
    pub const Z: Vec3 = v3(0.0, 0.0, 1.0);

    pub fn splat(v: f32) -> Vec3 {
        v3(v, v, v)
    }
    pub fn from_arr(a: [f32; 3]) -> Vec3 {
        v3(a[0], a[1], a[2])
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len2(self) -> f32 {
        self.dot(self)
    }
    pub fn len(self) -> f32 {
        self.len2().sqrt()
    }
    pub fn norm(self) -> Vec3 {
        let l = self.len();
        if l > 1e-8 {
            self * (1.0 / l)
        } else {
            Vec3::ZERO
        }
    }
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
    pub fn mul_v(self, o: Vec3) -> Vec3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    pub fn max_c(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }
    pub fn clamp01(self) -> Vec3 {
        v3(self.x.clamp(0.0, 1.0), self.y.clamp(0.0, 1.0), self.z.clamp(0.0, 1.0))
    }
    /// Rota alrededor de un eje unitario (Rodrigues).
    pub fn rotar(self, eje: Vec3, ang: f32) -> Vec3 {
        let (s, c) = ang.sin_cos();
        self * c + eje.cross(self) * s + eje * (eje.dot(self) * (1.0 - c))
    }
    /// Un vector cualquiera perpendicular a este.
    pub fn perpendicular(self) -> Vec3 {
        let a = if self.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        a.cross(self).norm()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        self * (1.0 / s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        v3(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub fn xyz(self) -> Vec3 {
        v3(self.x, self.y, self.z)
    }
    pub fn lerp(self, o: Vec4, t: f32) -> Vec4 {
        Vec4 {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
            z: self.z + (o.z - self.z) * t,
            w: self.w + (o.w - self.w) * t,
        }
    }
}

/// Matriz 4x4 en filas: m[fila][columna], se multiplica por vectores columna.
#[derive(Clone, Copy, Debug)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub fn identidad() -> Mat4 {
        let mut m = [[0.0; 4]; 4];
        for (i, fila) in m.iter_mut().enumerate() {
            fila[i] = 1.0;
        }
        Mat4 { m }
    }

    pub fn traslacion(t: Vec3) -> Mat4 {
        let mut r = Mat4::identidad();
        r.m[0][3] = t.x;
        r.m[1][3] = t.y;
        r.m[2][3] = t.z;
        r
    }

    pub fn escala(s: f32) -> Mat4 {
        let mut r = Mat4::identidad();
        r.m[0][0] = s;
        r.m[1][1] = s;
        r.m[2][2] = s;
        r
    }

    pub fn rot_y(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::identidad();
        r.m[0][0] = c;
        r.m[0][2] = s;
        r.m[2][0] = -s;
        r.m[2][2] = c;
        r
    }

    pub fn rot_z(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::identidad();
        r.m[0][0] = c;
        r.m[0][1] = -s;
        r.m[1][0] = s;
        r.m[1][1] = c;
        r
    }

    pub fn rot_x(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::identidad();
        r.m[1][1] = c;
        r.m[1][2] = -s;
        r.m[2][1] = s;
        r.m[2][2] = c;
        r
    }

    /// Matriz cuyas columnas son los ejes x, y, z y la posicion.
    pub fn desde_base(x: Vec3, y: Vec3, z: Vec3, pos: Vec3) -> Mat4 {
        Mat4 {
            m: [
                [x.x, y.x, z.x, pos.x],
                [x.y, y.y, z.y, pos.y],
                [x.z, y.z, z.z, pos.z],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut r = [[0.0; 4]; 4];
        for (i, fila) in r.iter_mut().enumerate() {
            for (j, celda) in fila.iter_mut().enumerate() {
                *celda = (0..4).map(|k| self.m[i][k] * o.m[k][j]).sum();
            }
        }
        Mat4 { m: r }
    }

    pub fn punto4(&self, p: Vec3) -> Vec4 {
        let m = &self.m;
        Vec4 {
            x: m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
            y: m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
            z: m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
            w: m[3][0] * p.x + m[3][1] * p.y + m[3][2] * p.z + m[3][3],
        }
    }

    pub fn punto(&self, p: Vec3) -> Vec3 {
        self.punto4(p).xyz()
    }

    pub fn direccion(&self, d: Vec3) -> Vec3 {
        let m = &self.m;
        v3(
            m[0][0] * d.x + m[0][1] * d.y + m[0][2] * d.z,
            m[1][0] * d.x + m[1][1] * d.y + m[1][2] * d.z,
            m[2][0] * d.x + m[2][1] * d.y + m[2][2] * d.z,
        )
    }

    pub fn transpuesta(&self) -> Mat4 {
        let mut r = [[0.0; 4]; 4];
        for (i, fila) in r.iter_mut().enumerate() {
            for (j, celda) in fila.iter_mut().enumerate() {
                *celda = self.m[j][i];
            }
        }
        Mat4 { m: r }
    }

    /// Proyeccion en perspectiva estilo OpenGL (z de -1 a 1, w = -z de vista).
    pub fn perspectiva(fov_y: f32, aspecto: f32, cerca: f32, lejos: f32) -> Mat4 {
        let f = 1.0 / (fov_y * 0.5).tan();
        let mut r = Mat4 { m: [[0.0; 4]; 4] };
        r.m[0][0] = f / aspecto;
        r.m[1][1] = f;
        r.m[2][2] = (lejos + cerca) / (cerca - lejos);
        r.m[2][3] = 2.0 * lejos * cerca / (cerca - lejos);
        r.m[3][2] = -1.0;
        r
    }

    /// Matriz de vista: lleva el mundo al espacio de la camara.
    pub fn mirar(ojo: Vec3, objetivo: Vec3, arriba: Vec3) -> Mat4 {
        let f = (objetivo - ojo).norm();
        let s = f.cross(arriba).norm();
        let u = s.cross(f);
        Mat4 {
            m: [
                [s.x, s.y, s.z, -s.dot(ojo)],
                [u.x, u.y, u.z, -u.dot(ojo)],
                [-f.x, -f.y, -f.z, f.dot(ojo)],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

pub fn suave(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn mezcla(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a.lerp(b, t.clamp(0.0, 1.0))
}

/// Factor de suavizado exponencial que no depende de los FPS.
pub fn amortiguar(rapidez: f32, dt: f32) -> f32 {
    1.0 - (-rapidez * dt).exp()
}
