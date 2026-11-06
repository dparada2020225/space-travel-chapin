# Space Travel — Sistema Solar Chapín

Proyecto final de Gráficas por Computadora (UVG, 2026). Un sistema solar pequeño con temática guatemalteca
que se explora en una nave con forma de quetzal. Todo el 3D se dibuja con un **renderizador por software
propio** escrito en Rust: raylib solo se usa para abrir la ventana, leer el teclado y mostrar el framebuffer
como una textura 2D.

| Cuerpo | Idea | Shader |
|---|---|---|
| **Sol** | plasma que se mueve | fBm con *domain warping* (animado) |
| **Fuego** | volcán de Fuego + Acatenango, cráter y ríos de lava | ruido de crestas que fluye (animado) + grietas con ruido celular |
| **Antigua** | calles empedradas, casas coloniales, Arco de Santa Catalina, volcán de Agua | ruido celular (Worley) para los adoquines |
| **Atitlán** | lago con tres volcanes y pueblitos en la orilla | olas con normales perturbadas + especular (animado) |
| **Cafetal** | colinas con surcos y cafetos con granos rojos | franjas por altura + surcos |
| **Tikal** | selva con templos mayas y ceibas | mapeo de normales |

## Cómo ejecutar

Requisitos: Rust estable, CMake y un compilador de C (los mismos que pide `raylib-rs`).

```bash
cargo run --release
```

Se debe correr desde la carpeta del proyecto (ahí están `config.toml` y `assets/`). Sin `--release` también
funciona porque el perfil de desarrollo usa `opt-level = 2`.

**Windows:** si la carpeta del proyecto tiene una ruta larga, la compilación de raylib (CMake + MSBuild) falla con
*"supera el límite máximo para la ruta de acceso"*. Se arregla usando una carpeta de compilación corta:

```powershell
setx CARGO_TARGET_DIR "C:\cargo-target"   # una sola vez; luego abrir una terminal nueva
cargo run --release
```

## Controles

| Tecla | Acción |
|---|---|
| W / S | acelerar / reversa |
| A / D | girar (guiñada) |
| ↑ / ↓ | nariz arriba / abajo — en modo órbita: subir / bajar |
| Q / E | alabeo |
| Shift | turbo |
| Espacio | frenar |
| F | entrar / salir del **modo órbita** (cuando el HUD dice que está disponible) |
| C | cambiar cámara (persecución, lejana, cabina) |
| Clic derecho + ratón | mirar alrededor |
| 0 – 5 | elegir objetivo (0 = Sol); aparece un marcador en pantalla |
| F1 | descarte por frustum on/off |
| F2 | niveles de detalle on/off |
| G / O / N | gravedad / líneas de órbita / mapa de normales |
| P | pausar el tiempo |
| R | recargar `config.toml` (también se recarga solo al guardarlo) |
| H | ayuda en pantalla · F3 ocultar HUD · F11 pantalla completa |

**Recorrido sugerido (el del video):** salir del punto inicial con turbo hacia Antigua (tecla 2 para marcarla),
frenar al acercarse, presionar F para entrar en órbita, dar la vuelta al planeta con W y A/D bajando con ↓
para ver las casas y el arco, subir con ↑, F para salir y alejarse con turbo.

## Características que solicito evaluar

| Característica | Pts | Dónde se ve / dónde está |
|---|---|---|
| 5 planetas distinguibles que orbitan y rotan | 50 | `planeta.rs` (`actualizar`), datos en `config.toml` |
| Tipos de modelo propios sobre la superficie: casa colonial, arco, cafeto, ceiba, templo maya, roca volcánica | 20 | `assets/modelos/*.obj` |
| Distribución procedural reproducible (semilla en `config.toml`) | 10 | `planeta.rs` → `generar_instancias` y `densidad` (Perlin + RNG con semilla) |
| Objetos orientados sobre la superficie curva mientras el planeta rota | 5 | `planeta.rs` → `matriz_sobre_superficie`; se multiplica por la matriz del planeta cada cuadro |
| Shaders propios distintos (sol, lava, agua, adoquín, terreno, selva, nave, objetos toon) | 24 | `shaders.rs` |
| Shader animado (sol, lava y agua) | 8 | `shaders.rs` usan `tiempo` |
| Mapeo de normales en el renderizador | 8 | Tikal; `MapaNormales` + base TBN en `shaders.rs::selva` (tecla N para comparar) |
| Deformación de la geometría con ruido | 10 | `planeta.rs` → `altura_cruda` |
| Generación/subdivisión procedural de geometría antes de rasterizar | 10 | `malla.rs` → `icosfera` (icosaedro subdividido, cada nivel ×4 triángulos) |
| Modo de control local para rodear un planeta | 8 | tecla F, `nave.rs` → `paso_orbita` |
| Aceleración, frenado y giro progresivos, iguales en cualquier PC | 8 | `nave.rs` con paso fijo de física de 1/120 s (`escena.rs`) |
| Atracción gravitatoria | 10 | `nave.rs` → `paso_libre` (tecla G para apagarla) |
| Colisión básica | 4 | `nave.rs` → `colisiones` (usa la altura real del terreno) |
| Descarte de objetos fuera de la vista | 7 | `raster.rs` → `Frustum`; F1 y contadores del HUD |
| Niveles de detalle por distancia | 8 | 4 LOD por planeta (320 a 20 480 triángulos) + objetos solo de cerca; F2 y contadores |
| Presentación (coherencia visual y claridad) | 10 | temática, HUD, marcador de objetivo, ayuda con H |

Los contadores del HUD (triángulos enviados, dibujados, traseros, fuera de vista, objetos descartados)
sirven para demostrar el descarte y el LOD: al presionar F1/F2 los números suben.

## Cómo funciona el pipeline

1. **Vértices** (`raster.rs`): cada vértice se multiplica por la matriz de modelo y luego por vista × proyección.
2. **Recorte**: los triángulos completamente fuera de un plano se descartan; los que cruzan el plano cercano se
   recortan (Sutherland–Hodgman) para poder acercarse a los planetas sin errores.
3. **Rasterización**: funciones de arista con coordenadas baricéntricas, descarte de caras traseras e
   interpolación con corrección de perspectiva. La prueba de profundidad guarda `1/w` (más grande = más cerca).
4. **G-buffer**: por píxel se guarda posición, normal, color, posición local y un dato extra.
5. **Fragment shaders** (`shaders.rs`): corren una vez por píxel visible, en paralelo por filas (rayon).
6. **Extras 2D**: estrellas, polvo espacial, líneas de órbita con prueba de profundidad y resplandor del sol.

## Cambios en vivo (config.toml)

Todo lo ajustable está en `config.toml`. Con el programa abierto, se edita, se guarda y la escena se reconstruye
sola (o se presiona R). La nave se queda donde estaba.

| Lo que pidan | Qué cambiar | Qué pasa |
|---|---|---|
| Radio de un planeta | `radio` en su `[[planeta]]` | se regenera la esfera, el relieve y los objetos a la nueva escala |
| Velocidad orbital / de rotación | `velocidad_orbital`, `velocidad_rotacion` (rad/s) | la posición depende de `tiempo × velocidad`, así que cambia al instante |
| Distancia al sol | `distancia_orbita` | |
| Semilla de distribución | `semilla` en `[general]` (todos) o `semilla` de un planeta | otras posiciones para casas/árboles/rocas; misma semilla = mismas posiciones |
| Cantidad de objetos | `objetos` | |
| Intensidad de un shader | `intensidad_shader` (lava: brillo, agua: olas, adoquín: tamaño de piedra, cafetal: surcos, selva: fuerza del mapa de normales) · `intensidad` del sol | |
| Colores | `color_a`, `color_b`, `color_c`, `atmosfera` (RGB de 0 a 1) | |
| Relieve | `deformacion` (0 = esfera lisa) | |
| Todo más rápido | `velocidad_tiempo` en `[general]` | |
| Nave | `[nave]`: `velocidad_max`, `aceleracion`, `gravedad`, `escala`… | |
| Calidad | `[render] resolucion` (ej. `[960, 540]`) | |

## Estructura

```
src/
  main.rs      ventana raylib, teclado, HUD
  escena.rs    actualiza el mundo y arma lo que se dibuja cada cuadro (descarte y LOD)
  raster.rs    transformación, recorte, rasterización, z-buffer, frustum
  render.rs    pasada de sombreado y efectos 2D
  shaders.rs   fragment shaders y mapa de normales
  planeta.rs   deformación, LOD, órbita/rotación, objetos procedurales
  nave.rs      física y controles de la nave
  camara.rs    cámara de persecución y cabina
  malla.rs     cargador OBJ/MTL e icosfera
  ruido.rs     Perlin 3D, fBm, ruido celular, RNG con semilla
  math.rs      vectores y matrices
  config.rs    lectura de config.toml
assets/modelos/  nave.obj, casa.obj, arco.obj, cafeto.obj, ceiba.obj, piramide.obj, roca.obj
```

## Integrantes y contribuciones

- **Denil José Parada Cabrera — 24761**: proyecto individual (renderizador, shaders, planetas, nave, modelos y escena).

## Recursos externos y uso de IA

- Librerías: `raylib` (ventana/entrada/textura), `rayon` (hilos), `serde` + `toml` (leer la configuración).
- Ruido de Perlin basado en la versión mejorada de Ken Perlin (2002).
- Se usó asistencia de IA (Claude) como apoyo durante el desarrollo. El código fue revisado y puedo explicar cada parte.

## Video

Enlace al video del recorrido: _pendiente_
