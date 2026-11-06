// Ventana, teclado y HUD con raylib. Todo el 3D lo hace el renderizador propio (src/raster.rs);
// aqui solo se sube el framebuffer como textura y se dibuja escalado.
use raylib::prelude::*;
use space_travel::config::{self, Config};
use space_travel::escena::{Escena, InfoHud};
use space_travel::modelos::Modelos;
use space_travel::nave::Entrada;
use space_travel::render::Renderer;
use std::path::Path;
use std::time::SystemTime;

fn fecha(ruta: &Path) -> Option<SystemTime> {
    std::fs::metadata(ruta).and_then(|m| m.modified()).ok()
}

fn eje(rl: &RaylibHandle, pos: KeyboardKey, neg: KeyboardKey) -> f32 {
    (rl.is_key_down(pos) as i32 - rl.is_key_down(neg) as i32) as f32
}

fn leer_entrada(rl: &RaylibHandle) -> Entrada {
    use KeyboardKey::*;
    let mirar = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT);
    let raton = rl.get_mouse_delta();
    Entrada {
        empuje: eje(rl, KEY_W, KEY_S),
        giro: eje(rl, KEY_A, KEY_D),
        cabeceo: eje(rl, KEY_UP, KEY_DOWN),
        alabeo: eje(rl, KEY_E, KEY_Q),
        turbo: rl.is_key_down(KEY_LEFT_SHIFT) || rl.is_key_down(KEY_RIGHT_SHIFT),
        freno: rl.is_key_down(KEY_SPACE),
        raton: (raton.x, raton.y),
        mirar,
    }
}

fn nueva_textura(rl: &mut RaylibHandle, hilo: &RaylibThread, cfg: &Config) -> Texture2D {
    let [w, h] = cfg.render.resolucion;
    let img = Image::gen_image_color(w as i32, h as i32, Color::BLACK);
    let tex = rl.load_texture_from_image(hilo, &img).expect("no se pudo crear la textura");
    let filtro = if cfg.render.filtro_suave {
        TextureFilter::TEXTURE_FILTER_BILINEAR
    } else {
        TextureFilter::TEXTURE_FILTER_POINT
    };
    tex.set_texture_filter(hilo, filtro);
    tex
}

fn main() {
    let ruta_cfg = config::buscar("config.toml");
    let cfg = config::cargar(&ruta_cfg).unwrap_or_else(|e| {
        eprintln!("{e}\nse usan los valores por defecto");
        Config::default()
    });
    let modelos = Modelos::cargar().unwrap_or_else(|e| {
        eprintln!("error cargando modelos: {e}");
        std::process::exit(1);
    });

    let [vw, vh] = cfg.general.ventana;
    let (mut rl, hilo) = raylib::init()
        .size(vw, vh)
        .title("Space Travel - Sistema Solar Chapin")
        .resizable()
        .build();
    rl.set_target_fps(cfg.general.fps_max);

    let [rw, rh] = cfg.render.resolucion;
    let mut renderer = Renderer::new(rw, rh, cfg.render.cerca);
    let mut textura = nueva_textura(&mut rl, &hilo, &cfg);
    let mut escena = Escena::new(cfg, modelos);
    let mut fecha_cfg = fecha(&ruta_cfg);
    let mut reloj_cfg = 0.0;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time();

        // --- recarga en vivo de config.toml (al guardar el archivo o con R)
        reloj_cfg += dt;
        let mut recargar = rl.is_key_pressed(KeyboardKey::KEY_R);
        if reloj_cfg > 0.5 {
            reloj_cfg = 0.0;
            let f = fecha(&ruta_cfg);
            if f != fecha_cfg {
                fecha_cfg = f;
                recargar = true;
            }
        }
        if recargar {
            match config::cargar(&ruta_cfg) {
                Ok(c) => {
                    let cambio_res = c.render.resolucion != escena.cfg.render.resolucion
                        || c.render.filtro_suave != escena.cfg.render.filtro_suave;
                    rl.set_target_fps(c.general.fps_max);
                    escena.recargar(c);
                    if cambio_res {
                        let [w, h] = escena.cfg.render.resolucion;
                        renderer = Renderer::new(w, h, escena.cfg.render.cerca);
                        textura = nueva_textura(&mut rl, &hilo, &escena.cfg);
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    escena.avisar(&e);
                }
            }
        }

        // --- teclas de una sola pulsacion
        use KeyboardKey::*;
        let o = &mut escena.opciones;
        if rl.is_key_pressed(KEY_F1) {
            o.descarte = !o.descarte;
        }
        if rl.is_key_pressed(KEY_F2) {
            o.lod = !o.lod;
        }
        if rl.is_key_pressed(KEY_F3) {
            o.hud = !o.hud;
        }
        if rl.is_key_pressed(KEY_G) {
            o.gravedad = !o.gravedad;
        }
        if rl.is_key_pressed(KEY_O) {
            o.orbitas = !o.orbitas;
        }
        if rl.is_key_pressed(KEY_N) {
            o.mapa_normales = !o.mapa_normales;
        }
        if rl.is_key_pressed(KEY_P) {
            o.pausa = !o.pausa;
        }
        if rl.is_key_pressed(KEY_H) {
            o.ayuda = !o.ayuda;
        }
        if rl.is_key_pressed(KEY_F) {
            escena.alternar_orbita();
        }
        if rl.is_key_pressed(KEY_C) {
            escena.cambiar_camara();
        }
        if rl.is_key_pressed(KEY_F11) {
            rl.toggle_fullscreen();
        }
        let numeros = [KEY_ZERO, KEY_ONE, KEY_TWO, KEY_THREE, KEY_FOUR, KEY_FIVE, KEY_SIX, KEY_SEVEN, KEY_EIGHT, KEY_NINE];
        for (i, k) in numeros.iter().enumerate() {
            if rl.is_key_pressed(*k) && i <= escena.planetas.len() {
                escena.objetivo = Some(i);
            }
        }

        let entrada = leer_entrada(&rl);
        escena.actualizar(dt, &entrada);
        escena.dibujar(&mut renderer);
        if let Err(e) = textura.update_texture(&renderer.pixeles) {
            eprintln!("update_texture: {e}");
        }
        let hud = escena.hud(&renderer);
        let ayuda = escena.opciones.ayuda;
        let mostrar_hud = escena.opciones.hud;
        let estados = [
            ("F1 descarte", escena.opciones.descarte),
            ("F2 LOD", escena.opciones.lod),
            ("G gravedad", escena.opciones.gravedad),
            ("N mapa normales", escena.opciones.mapa_normales),
            ("P pausa", escena.opciones.pausa),
        ];

        let mut d = rl.begin_drawing(&hilo);
        d.clear_background(Color::BLACK);
        let (sw, sh) = (d.get_screen_width() as f32, d.get_screen_height() as f32);
        let (tw, th) = (renderer.ancho as f32, renderer.alto as f32);
        let esc = (sw / tw).min(sh / th);
        let (dw, dh) = (tw * esc, th * esc);
        let (ox, oy) = ((sw - dw) * 0.5, (sh - dh) * 0.5);
        d.draw_texture_pro(
            &textura,
            Rectangle::new(0.0, 0.0, tw, th),
            Rectangle::new(ox, oy, dw, dh),
            Vector2::zero(),
            0.0,
            Color::WHITE,
        );
        if mostrar_hud {
            dibujar_hud(&mut d, &hud, &estados, ayuda, (ox, oy, dw, dh));
        }
    }
}

fn panel(d: &mut RaylibDrawHandle, x: i32, y: i32, w: i32, h: i32) {
    d.draw_rectangle(x, y, w, h, Color::new(5, 10, 25, 170));
    d.draw_rectangle_lines(x, y, w, h, Color::new(80, 200, 230, 120));
}

fn dibujar_hud(d: &mut RaylibDrawHandle, h: &InfoHud, estados: &[(&str, bool)], ayuda: bool, area: (f32, f32, f32, f32)) {
    let (ox, oy, dw, dh) = area;
    let celeste = Color::new(120, 220, 255, 255);
    let blanco = Color::new(235, 240, 245, 255);
    let gris = Color::new(160, 170, 185, 255);

    // --- marcador del objetivo
    if let Some((nombre, dist, pantalla)) = &h.objetivo {
        match pantalla {
            Some((u, v)) if (0.0..=1.0).contains(u) && (0.0..=1.0).contains(v) => {
                let (x, y) = ((ox + u * dw) as i32, (oy + v * dh) as i32);
                d.draw_circle_lines(x, y, 14.0, Color::new(255, 210, 90, 220));
                d.draw_text(&format!("{} {:.0}", nombre, dist), x + 18, y - 6, 16, Color::new(255, 210, 90, 255));
            }
            _ => {
                let txt = format!("objetivo {} fuera de vista ({:.0} u)", nombre, dist);
                d.draw_text(&txt, (ox + dw * 0.5) as i32 - 140, (oy + dh) as i32 - 60, 16, Color::new(255, 210, 90, 255));
            }
        }
    }

    // --- panel de vuelo
    let x0 = ox as i32 + 12;
    let y0 = oy as i32 + 12;
    panel(d, x0, y0, 330, 118);
    d.draw_text("SISTEMA SOLAR CHAPIN", x0 + 10, y0 + 8, 18, celeste);
    d.draw_text(&format!("{}   camara: {}", h.modo, h.camara), x0 + 10, y0 + 32, 16, blanco);
    d.draw_text(&format!("velocidad: {:.1} u/s", h.rapidez), x0 + 10, y0 + 52, 16, blanco);
    if let Some((nombre, alt)) = &h.cercano {
        d.draw_text(&format!("cerca de {}  altitud {:.1}", nombre, alt.max(0.0)), x0 + 10, y0 + 72, 16, blanco);
    }
    let pista = if h.puede_orbitar { "[F] modo orbita disponible" } else { "[H] ayuda   [0-5] objetivo" };
    d.draw_text(pista, x0 + 10, y0 + 94, 16, if h.puede_orbitar { Color::new(140, 255, 160, 255) } else { gris });

    // --- panel del renderizador
    let s = &h.stats;
    let ancho_p = 270;
    let xr = (ox + dw) as i32 - ancho_p - 12;
    panel(d, xr, y0, ancho_p, 200);
    d.draw_fps(xr + 10, y0 + 8);
    let lineas = [
        format!("tri enviados:   {}", s.tri_enviados),
        format!("tri dibujados:  {}", s.tri_dibujados),
        format!("tri traseros:   {}", s.tri_traseros),
        format!("tri fuera:      {}", s.tri_fuera),
        format!("objetos: {} / descartados {}", s.objetos_dibujados, s.objetos_descartados),
        format!("semilla: {}", h.semilla),
    ];
    for (i, l) in lineas.iter().enumerate() {
        d.draw_text(l, xr + 10, y0 + 32 + i as i32 * 18, 15, blanco);
    }
    for (i, (nombre, on)) in estados.iter().enumerate() {
        let col = if *on { Color::new(140, 255, 160, 255) } else { Color::new(255, 120, 110, 255) };
        let (cx, cy) = (xr + 10 + (i as i32 % 2) * 130, y0 + 144 + (i as i32 / 2) * 18);
        d.draw_text(&format!("{} {}", nombre, if *on { "ON" } else { "OFF" }), cx, cy, 14, col);
    }

    if let Some(m) = &h.mensaje {
        let w = d.measure_text(m, 20);
        d.draw_text(m, (ox + dw * 0.5) as i32 - w / 2, (oy + dh * 0.18) as i32, 20, Color::new(255, 230, 140, 255));
    }

    if ayuda {
        let controles = [
            "W / S      acelerar / reversa",
            "A / D      girar izquierda / derecha",
            "Flechas    nariz arriba / abajo (en orbita: subir / bajar)",
            "Q / E      alabeo",
            "Shift      turbo        Espacio  frenar",
            "F          modo orbita cerca de un planeta",
            "C          cambiar camara   clic der + raton: mirar",
            "0 - 5      elegir objetivo (0 = Sol)",
            "F1 / F2    descarte / niveles de detalle",
            "G  O  N    gravedad / orbitas / mapa de normales",
            "P  pausa   R  recargar config.toml   F3 ocultar HUD",
        ];
        let (px, py) = ((ox + dw * 0.5) as i32 - 260, (oy + dh * 0.5) as i32 - 120);
        panel(d, px, py, 520, 24 + controles.len() as i32 * 20);
        for (i, c) in controles.iter().enumerate() {
            d.draw_text(c, px + 14, py + 12 + i as i32 * 20, 16, blanco);
        }
    }
}
