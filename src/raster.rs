// Rasterizador por software: transforma vertices, recorta contra el plano cercano,
// descarta caras traseras y llena un G-buffer con prueba de profundidad.
// El sombreado (fragment shaders) se hace despues, una sola vez por pixel visible.
use crate::malla::Malla;
use crate::math::{Mat4, Vec3, Vec4};

/// Cantidad de "varyings" que se interpolan: pos mundo(3) normal(3) color(3) pos local(3) dato(1)
const NV: usize = 13;

#[derive(Clone, Copy)]
struct VClip {
    c: Vec4,
    v: [f32; NV],
}

impl VClip {
    fn lerp(&self, o: &VClip, t: f32) -> VClip {
        let mut v = [0.0; NV];
        for (k, x) in v.iter_mut().enumerate() {
            *x = self.v[k] + (o.v[k] - self.v[k]) * t;
        }
        VClip { c: self.c.lerp(o.c, t), v }
    }
}

pub struct GBuffer {
    pub ancho: usize,
    pub alto: usize,
    /// 1/w del fragmento mas cercano; 0 significa que no hay nada (fondo)
    pub prof: Vec<f32>,
    pub pos: Vec<Vec3>,
    pub normal: Vec<Vec3>,
    pub color: Vec<Vec3>,
    pub local: Vec<Vec3>,
    pub dato: Vec<f32>,
    pub obj: Vec<u16>,
}

impl GBuffer {
    pub fn new(ancho: usize, alto: usize) -> GBuffer {
        let n = ancho * alto;
        GBuffer {
            ancho,
            alto,
            prof: vec![0.0; n],
            pos: vec![Vec3::ZERO; n],
            normal: vec![Vec3::ZERO; n],
            color: vec![Vec3::ZERO; n],
            local: vec![Vec3::ZERO; n],
            dato: vec![0.0; n],
            obj: vec![0; n],
        }
    }

    pub fn limpiar(&mut self) {
        self.prof.fill(0.0);
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Estadisticas {
    pub tri_enviados: u32,
    pub tri_dibujados: u32,
    pub tri_traseros: u32,
    pub tri_fuera: u32,
    pub objetos_dibujados: u32,
    pub objetos_descartados: u32,
    pub instancias: u32,
}

pub struct Rasterizador {
    pub gb: GBuffer,
    pub stats: Estadisticas,
    pub cerca: f32,
    temp: Vec<VClip>,
}

fn arista(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

impl Rasterizador {
    pub fn new(ancho: usize, alto: usize, cerca: f32) -> Rasterizador {
        Rasterizador { gb: GBuffer::new(ancho, alto), stats: Estadisticas::default(), cerca, temp: Vec::new() }
    }

    /// Dibuja una malla. `modelo` lleva a mundo, `rot` es la parte de rotacion (para normales),
    /// `vp` es vista*proyeccion. `obj` indica que uniforme/shader usa en el sombreado.
    pub fn dibujar(&mut self, malla: &Malla, modelo: &Mat4, rot: &Mat4, vp: &Mat4, obj: u16, doble_cara: bool) {
        // --- etapa de vertices
        self.temp.clear();
        for v in &malla.vertices {
            let w = modelo.punto(v.pos);
            let n = rot.direccion(v.normal);
            let c = vp.punto4(w);
            self.temp.push(VClip {
                c,
                v: [
                    w.x, w.y, w.z, n.x, n.y, n.z, v.color.x, v.color.y, v.color.z, v.pos.x, v.pos.y,
                    v.pos.z, v.dato,
                ],
            });
        }

        let temp = std::mem::take(&mut self.temp);
        for t in &malla.triangulos {
            self.stats.tri_enviados += 1;
            let a = temp[t[0] as usize];
            let b = temp[t[1] as usize];
            let c = temp[t[2] as usize];

            // descarte rapido: los tres vertices fuera del mismo plano del volumen de vista
            let fuera = |f: &dyn Fn(&Vec4) -> bool| f(&a.c) && f(&b.c) && f(&c.c);
            if fuera(&|p| p.x > p.w)
                || fuera(&|p| p.x < -p.w)
                || fuera(&|p| p.y > p.w)
                || fuera(&|p| p.y < -p.w)
                || fuera(&|p| p.w < self.cerca)
            {
                self.stats.tri_fuera += 1;
                continue;
            }

            if a.c.w >= self.cerca && b.c.w >= self.cerca && c.c.w >= self.cerca {
                self.triangulo(&a, &b, &c, obj, doble_cara);
            } else {
                // recorte contra el plano cercano (Sutherland-Hodgman con un solo plano)
                let entrada = [a, b, c];
                let mut poli: Vec<VClip> = Vec::with_capacity(4);
                for i in 0..3 {
                    let p = &entrada[i];
                    let q = &entrada[(i + 1) % 3];
                    let dp = p.c.w - self.cerca;
                    let dq = q.c.w - self.cerca;
                    if dp >= 0.0 {
                        poli.push(*p);
                    }
                    if (dp >= 0.0) != (dq >= 0.0) {
                        poli.push(p.lerp(q, dp / (dp - dq)));
                    }
                }
                for k in 1..poli.len().saturating_sub(1) {
                    self.triangulo(&poli[0], &poli[k], &poli[k + 1], obj, doble_cara);
                }
            }
        }
        self.temp = temp;
    }

    fn triangulo(&mut self, a: &VClip, b: &VClip, c: &VClip, obj: u16, doble_cara: bool) {
        let (w, h) = (self.gb.ancho as f32, self.gb.alto as f32);
        let pantalla = |v: &VClip| {
            let iw = 1.0 / v.c.w;
            ((v.c.x * iw * 0.5 + 0.5) * w, (0.5 - v.c.y * iw * 0.5) * h, iw)
        };
        let mut p = [pantalla(a), pantalla(b), pantalla(c)];
        let mut vs = [a, b, c];

        let mut area = arista(p[0].0, p[0].1, p[1].0, p[1].1, p[2].0, p[2].1);
        // con Y hacia abajo, un triangulo antihorario (de frente) da area negativa
        if area >= 0.0 {
            if !doble_cara {
                self.stats.tri_traseros += 1;
                return;
            }
        } else {
            p.swap(1, 2);
            vs.swap(1, 2);
            area = -area;
        }
        if area < 1e-7 {
            return;
        }
        self.stats.tri_dibujados += 1;

        let min_x = p[0].0.min(p[1].0).min(p[2].0).floor().max(0.0) as i32;
        let max_x = p[0].0.max(p[1].0).max(p[2].0).ceil().min(w - 1.0) as i32;
        let min_y = p[0].1.min(p[1].1).min(p[2].1).floor().max(0.0) as i32;
        let max_y = p[0].1.max(p[1].1).max(p[2].1).ceil().min(h - 1.0) as i32;
        if min_x > max_x || min_y > max_y {
            return;
        }

        // varyings premultiplicados por 1/w para interpolacion con correccion de perspectiva
        let mut pv = [[0.0f32; NV]; 3];
        for i in 0..3 {
            for k in 0..NV {
                pv[i][k] = vs[i].v[k] * p[i].2;
            }
        }
        let inv_area = 1.0 / area;
        let ancho = self.gb.ancho;

        // coeficientes de cada arista: e(x,y) = A*x + B*y + C
        let coef = |i: usize, j: usize| {
            let (ax, ay, bx, by) = (p[i].0, p[i].1, p[j].0, p[j].1);
            (-(by - ay), bx - ax, (by - ay) * ax - (bx - ax) * ay)
        };
        let e0 = coef(1, 2);
        let e1 = coef(2, 0);
        let e2 = coef(0, 1);

        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let px0 = min_x as f32 + 0.5;
            let mut w0 = e0.0 * px0 + e0.1 * py + e0.2;
            let mut w1 = e1.0 * px0 + e1.1 * py + e1.2;
            let mut w2 = e2.0 * px0 + e2.1 * py + e2.2;
            let fila = y as usize * ancho;
            for x in min_x..=max_x {
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let b0 = w0 * inv_area;
                    let b1 = w1 * inv_area;
                    let b2 = w2 * inv_area;
                    let iw = b0 * p[0].2 + b1 * p[1].2 + b2 * p[2].2;
                    let idx = fila + x as usize;
                    // prueba de profundidad: mayor 1/w = mas cerca
                    if iw > self.gb.prof[idx] {
                        self.gb.prof[idx] = iw;
                        let ww = 1.0 / iw;
                        let mut r = [0.0f32; NV];
                        for k in 0..NV {
                            r[k] = (b0 * pv[0][k] + b1 * pv[1][k] + b2 * pv[2][k]) * ww;
                        }
                        let g = &mut self.gb;
                        g.pos[idx] = Vec3 { x: r[0], y: r[1], z: r[2] };
                        g.normal[idx] = Vec3 { x: r[3], y: r[4], z: r[5] };
                        g.color[idx] = Vec3 { x: r[6], y: r[7], z: r[8] };
                        g.local[idx] = Vec3 { x: r[9], y: r[10], z: r[11] };
                        g.dato[idx] = r[12];
                        g.obj[idx] = obj;
                    }
                }
                w0 += e0.0;
                w1 += e1.0;
                w2 += e2.0;
            }
        }
    }
}

/// Planos del volumen de vista sacados de la matriz vista*proyeccion (Gribb/Hartmann).
pub struct Frustum {
    planos: [(Vec3, f32); 6],
}

impl Frustum {
    pub fn desde(vp: &Mat4) -> Frustum {
        let m = &vp.m;
        let fila = |i: usize| (m[i][0], m[i][1], m[i][2], m[i][3]);
        let r3 = fila(3);
        let mut planos = [(Vec3::ZERO, 0.0); 6];
        for (k, (i, signo)) in [(0, 1.0), (0, -1.0), (1, 1.0), (1, -1.0), (2, 1.0), (2, -1.0)].iter().enumerate() {
            let r = fila(*i);
            let n = Vec3 { x: r3.0 + signo * r.0, y: r3.1 + signo * r.1, z: r3.2 + signo * r.2 };
            let d = r3.3 + signo * r.3;
            let l = n.len();
            planos[k] = (n / l, d / l);
        }
        Frustum { planos }
    }

    pub fn esfera_visible(&self, c: Vec3, r: f32) -> bool {
        self.planos.iter().all(|(n, d)| n.dot(c) + d >= -r)
    }
}
