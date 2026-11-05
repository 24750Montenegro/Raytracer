# Laboratorio: planetas y estrellas con shaders

Rasterizador por software escrito en Rust: todo se dibuja en CPU. La misma
esfera (`assets/models/sphere.obj`, 192 triángulos) se pinta con distintos
fragment shaders, y cada shader es un cuerpo celeste. No se usan texturas ni
se modifican los vértices: todo el aspecto sale de funciones matemáticas y
ruido.

> El laboratorio anterior (la nave, los rellenos y sus mediciones) está en la
> rama `feat/obj-rendering`.

## Cómo ejecutarlo

```sh
cargo run --release
```

La primera compilación construye raylib desde C, así que necesita CMake y un
compilador de C (en Windows, las Build Tools de Visual Studio).

Para guardar una captura de cada shader en `assets/capturas/` sin abrir la
ventana (el número es el instante, en segundos):

```sh
cargo run --release -- --capturas 2.0
```

## Controles

| Tecla | Qué hace |
|---|---|
| **Tab** / **Shift+Tab** | siguiente / anterior shader |
| **1**–**4** | elegir un shader directamente |
| **Flechas** | orbitar la cámara alrededor de la esfera |
| **Z** / **X** | alejar / acercar |
| **Q** / **E** | mover el sol alrededor de la esfera |
| **Espacio** | pausar el tiempo |
| **R** | volver al inicio |
| **P** | guardar una captura del shader actual |

Arriba a la izquierda se ve el nombre del cuerpo celeste y su tipo. Ese
texto no entra en las capturas.

## Cómo está armado

```
vértices → vertex shader → ensamblaje → rasterización → z-buffer → fragment shader → framebuffer
```

- **Vertex shader** (`src/shaders/mod.rs`): la matriz de modelo lleva la
  esfera a radio 1 en el origen, la de vista la mira desde la cámara orbital
  y la de viewport la pasa a píxeles (proyección ortográfica).
- **Rasterización** (`src/triangle.rs`): relleno baricéntrico que interpola
  la profundidad, la posición en el modelo y la posición en el mundo.
- **Z-buffer**: queda el fragmento más cercano de cada píxel, y el fragment
  shader corre una sola vez por píxel visible, repartido entre todos los
  núcleos (`Framebuffer::shade`).
- **Fragment shader**: arma un `Frag` con todo lo que se sabe del punto y
  llama al shader elegido con Tab.

La esfera no gira: lo que se mueve es la cámara. El sol queda fijo respecto
a la esfera, así que al orbitar se ven el lado de día y el de noche. La
normal se calcula desde la posición (en una esfera es la dirección desde el
centro), por eso la luz se ve suave aunque la malla tenga pocos triángulos.

## Cómo escribir un shader

Un shader es una función `fn shade(f: &Frag) -> Vector3` en
`src/shaders/`. Recibe el punto que se está pintando y devuelve su color
como `Vector3`: x = rojo, y = verde, z = azul, de 0 a 1. Lo que pase de 1 se
comprime hacia el blanco, útil para que una estrella brille.

### Lo que trae `Frag`

| | Qué es |
|---|---|
| `f.p` | punto de la esfera de radio 1, en coordenadas del modelo. Es la entrada de todos los patrones |
| `f.normal` | normal en el mundo |
| `f.to_sun` | dirección hacia el sol |
| `f.to_viewer` | dirección hacia la cámara |
| `f.light` | intensidad del sol en ese punto: 0 de noche, 1 de frente |
| `f.time` | segundos desde que arrancó (Espacio lo pausa) |
| `f.lit(color)` | el color con luz difusa básica (ambiente + sol) |
| `f.shade(color, &material, normal)` | Lambert + Phong + emisión con un `Material` y la normal que se quiera |

### Herramientas (`src/shaders/kit.rs`)

| Función | Qué hace |
|---|---|
| `Vector3::new(1.0, 0.5, 0.0)`, `gray(v)` | colores (naranja, gris) |
| `mix(a, b, t)` | `a` cuando t = 0, `b` cuando t = 1 |
| `ramp(&[(0.0, a), (0.5, b), (1.0, c)], t)` | degradado por paradas |
| `smoothstep(e0, e1, x)`, `step(borde, x)` | convertir un valor en máscara, suave o dura |
| `wave(x)` | seno pasado a 0..1 |
| `fract(x)` | 0..1 que se repite |
| `noise(p)` | ruido OpenSimplex2, 0..1 |
| `fbm(p, octavas)` | varias octavas de ruido: más detalle |
| `ridged(p, octavas)` | como `fbm` pero con crestas finas y nítidas: venas, relámpagos |
| `cellular(p)` | distancia al centro de la celda (Voronoi) |
| `cell_edge(p)` | distancia al borde entre celdas: 0 sobre el borde |
| `cell_value(p)` | un valor al azar por celda |
| `latitude(p)`, `longitude(p)` | -1..1 de polo a polo, 0..1 alrededor |
| `rotate_y(p, ángulo)`, `rotate(p, eje, ángulo)` | girar el patrón |
| `rim(f, potencia)` | 0 de frente, 1 en el borde: halos, coronas, atmósferas |
| `bump(f.normal, p, h, fuerza, altura)` | normal con relieve a partir de un campo de altura |
| `Material::matte()`, `polished()`, `lacquer()`, `metal()`, `lamp(emisión)` | cómo responde a la luz |
| `Material::new(Albedo::new(difuso, especular), brillo)` | un material a medida |

Con vectores: `f.p * 4.0` sube la frecuencia de un patrón, `f.p + f.time * 0.3`
lo desplaza con el tiempo y `f.p * Vector3::new(1.0, 8.0, 1.0)` lo estira.

### Capas

Una capa es una máscara de 0 a 1, un color y una forma de combinarlos:

```rust
use super::kit::*;

pub fn shade(f: &Frag) -> Vector3 {
    let mut color = Vector3::new(0.11, 0.14, 0.25);

    // encima: mix
    let franjas = wave(f.p.y * 12.0 + f.time);
    color = mix(color, Vector3::new(0.36, 0.48, 0.84), franjas);

    // sumar: brillos, emisión
    let calor = fbm(f.p * 4.0, 4);
    color = color + Vector3::new(1.0, 0.7, 0.28) * calor * 0.3;

    // multiplicar: oscurecer, sombras
    color = color * (0.6 + 0.4 * cellular(f.p * 6.0));

    // la luz al final (una estrella puede saltársela)
    f.lit(color)
}
```

Para ver solo un patrón, devolverlo en gris: `return gray(franjas);`.

`src/shaders/sandbox.rs` (**pruebas** en Tab) es un shader para jugar sin
tocar los tres cuerpos.

### Agregar otro shader

1. Crear `src/shaders/mi_shader.rs` con `use super::kit::*;` y
   `pub fn shade(f: &Frag) -> Vector3`.
2. En `src/shaders/mod.rs`, agregar `mod mi_shader;` y una línea en
   `SHADERS`: `name` es el tipo (y el nombre de la captura) y `body` el
   nombre que se muestra en pantalla. Tab lo recorre solo.

### Dónde engancha cada efecto

| Efecto | Qué usar |
|---|---|
| Material animado | `f.time` dentro del patrón |
| Día y noche | `f.light` como máscara (por ejemplo, algo visible solo donde es casi 0) |
| Respuestas distintas a la luz | `f.shade` con un `Material` distinto según la región |
| Nubes | otra máscara de ruido sobre `rotate_y(f.p, f.time * velocidad)` |
| Tormenta o vórtice | girar `p` alrededor de un punto, más cerca más giro, antes de calcular las bandas |
| Bump mapping | una normal nueva a partir de cómo cambia un campo de altura, pasada a `f.shade` |
| Parallax | mover `p` según `f.to_viewer` y la altura antes de muestrear |
| Sombras de nubes | la máscara de nubes evaluada en `p` corrido hacia `f.to_sun` |

## Cuerpos celestes

### Estrella: Lunan't

![Estrella](assets/capturas/estrella.png)

"Luna" + "n't": no es una luna, porque brilla sola. Es una estrella morada
con venas de plasma lavanda que fluyen rápido sobre un fondo gris oscuro,
y late.

**Patrones y cómo se combinan.**

1. `ridged(p, 5)`: un `fbm` de crestas. Cada octava es `1 - |ruido|`, así
   que donde el ruido cruza por cero queda una cresta, y al elevarla al
   cuadrado se angosta. Eso da venas con borde nítido en vez de las manchas
   difuminadas de un `fbm` común. p gira (`SPIN`) y se desplaza con el
   tiempo (`FLOW_SPEED`), así que las venas fluyen.
2. `smoothstep(VEINS)` convierte las crestas en calor de 0 a 1, y un `ramp`
   lo lleva de gris oscuro a morado y a lavanda. Los morados están en
   escala 0–255, muy por encima de 1: la pantalla los comprime sin cambiar
   el tono, así que apenas hay calor el color salta a morado saturado y el
   gris solo queda donde el calor es 0.
3. El borde de la esfera se oscurece un poco (`0.65 + 0.35 · √(normal · vista)`).
4. Pulso: `1 + PULSE · sin(1.2 · t)`. Las venas lo reciben completo; el
   fondo solo recibe la mitad que oscurece, así que en el pico no se aclara.
5. No usa la luz del sol: todo es emisión.

**Parámetros** (`src/shaders/star.rs`):

| Parámetro | Qué cambia |
|---|---|
| `SPIN` | velocidad de giro de la superficie (rad/s) |
| `FLOW_SPEED` | qué tan rápido fluyen las venas |
| `SCALE` | tamaño del plasma: más alto, venas más chicas y juntas |
| `VEINS` | dónde empieza y termina el paso a morado: más cerrado, más fondo gris |
| `BRIGHTNESS` | brillo total |
| `PULSE` | cuánto late |
| `DARK`, `PURPLE`, `PURPLE_HOT` | colores del fondo, del plasma y de las venas |

### Planeta rocoso: Bocchi

![Planeta rocoso](assets/capturas/planeta-rocoso.png)

Un planeta *rock* rosado: casi todo el suelo es rosa, con algunos lagos
celestes y picos y casquetes polares de plata. Las nubes lo tapan a ratos y de noche
se le encienden grietas celestes.

**Patrones y cómo se combinan.**

1. `terrain`: `fbm` de 4 octavas, la altura del suelo. Debajo de
   `SEA_LEVEL` hay agua (celeste, más oscura cuanto más honda, con espuma en
   la costa); arriba, roca que va de rosa claro a malva según la altura.
2. Plata: sobre `PEAK_LEVEL` y cerca de los polos (`ICE_LATITUDE`,
   desplazada por la altura para que el borde del casquete no sea recto).
3. Relieve: `bump` inclina la normal según la pendiente de `terrain`. La
   altura se corta en `SEA_LEVEL`, así que el mar queda plano.
4. Cada región responde distinto a la luz: agua con brillo de Phong
   cerrado, roca mate y plata metálica (el brillo sale plateado).
5. Nubes: otro `fbm`, estirado en vertical para que salgan alargadas y
   girando a `CLOUD_SPEED`, distinto del giro del suelo (`SPIN`). Su sombra
   es la misma máscara evaluada en el punto corrido hacia el sol.
6. Noche: `cell_edge` dibuja grietas que solo brillan donde `f.light` es
   casi 0.
7. Un halo rosado en el borde con `rim`, más fuerte del lado del día.

**Parámetros** (`src/shaders/rocky.rs`):

| Parámetro | Qué cambia |
|---|---|
| `SPIN` | velocidad de giro del suelo |
| `TERRAIN_SCALE` | tamaño de los continentes |
| `SEA_LEVEL` | cuánto mar hay |
| `PEAK_LEVEL` | desde qué altura los picos son de plata |
| `ICE_LATITUDE` | hasta dónde bajan los casquetes |
| `BUMP` | fuerza del relieve |
| `CLOUD_SCALE`, `CLOUD_COVER` | tamaño de las nubes y cuánto cubren |
| `CLOUD_SPEED` | velocidad de las nubes |
| `CLOUD_HEIGHT` | qué tan corrida queda la sombra de las nubes |
| `CRACK_SCALE`, `NIGHT_GLOW` | tamaño e intensidad de las grietas de noche |
| `CELESTE`, `SEA_DEEP`, `PINK`, `HOT_PINK`, `SILVER`, ... | paleta |

### Gigante gaseoso: JupyterNotebook

![Gigante gaseoso](assets/capturas/gigante-gaseoso.png)

Un Júpiter cibernético pintado con la paleta Dracula, un tema de editor de
código: franjas anchas de neón (verde, cyan, rosa y morado) separadas por
líneas finas casi negras, que son los fondos del tema. Las franjas bajan,
los colores van cambiando con el tiempo y unas vetas oscuras de nubes le dan
la textura de gas.

**Patrones y cómo se combinan.**

1. `franjas = wave(y · BANDS + t · FALL_SPEED + remolino)³`: un seno de la
   altura más el tiempo. Como el tiempo suma, cada valor queda cada vez más
   abajo: las franjas bajan. Al elevarlo al cubo, solo se acerca a 1 en una
   parte angosta de cada ciclo.
2. Remolino: un `fbm` estirado en vertical que se suma dentro del seno y
   ondula los bordes, para que parezcan gas.
3. Color: `mix(neón, oscuro, franjas)`. Como `franjas` casi siempre está
   cerca de 0, domina el neón y lo oscuro queda en líneas finas. Cada color
   sale de `cycle`, que recorre su lista en círculo con un índice que
   depende de la altura (cada franja tiene su color) y del tiempo
   (`COLOR_SPEED`). El neón y lo oscuro avanzan a ritmos distintos, así que
   las combinaciones no se repiten igual.
4. Vetas: otro `fbm` más fino multiplica el color entre 0.4 y 1.2, y deja
   nubes oscuras alargadas sobre el neón.
5. `Material::lamp(GLOW)`: además de la luz del sol, la superficie emite su
   propio color, así que el neón brilla también del lado de noche. Las
   líneas oscuras casi no emiten.

**Parámetros** (`src/shaders/gas_giant.rs`):

| Parámetro | Qué cambia |
|---|---|
| `BANDS` | cuántas franjas hay |
| `FALL_SPEED` | qué tan rápido bajan las franjas |
| `TURBULENCE` | qué tan ondulados son los bordes |
| `COLOR_SPEED` | qué tan rápido cambian los colores |
| `GLOW` | cuánto brilla solo: más alto, neón más vivo, pero las líneas oscuras se aclaran |
| `LIGHT`, `DARK` | colores del neón y de las líneas oscuras (paleta Dracula) |

## Efectos adicionales

| Efecto | Cuerpo | Archivo y función | Evidencia | Cómo funciona | Parámetro y qué cambia |
|---|---|---|---|---|---|
| Material animado | los tres | `star.rs`, `rocky.rs`, `gas_giant.rs` → `shade` | _video pendiente_ | `f.time` hace fluir el plasma y latir la estrella, gira el suelo y las nubes, baja las franjas y les cambia el color | `FLOW_SPEED`, `PULSE`, `SPIN`, `FALL_SPEED`, `COLOR_SPEED` |
| Día y noche | Bocchi | `rocky.rs` → `shade` | _video pendiente_ | grietas emisivas pesadas por `1 - smoothstep(0, 0.12, f.light)` | `NIGHT_GLOW`: intensidad; `CRACK_SCALE`: tamaño |
| Respuesta distinta a la luz por región | Bocchi | `rocky.rs` → `shade` | _video pendiente_ | agua con Phong cerrado, roca mate, plata metálica | `PEAK_LEVEL`, `SEA_LEVEL`: dónde empieza cada región |
| Nubes | Bocchi | `rocky.rs` → `clouds` | _video pendiente_ | `fbm` estirado que gira más rápido que el suelo | `CLOUD_SPEED`, `CLOUD_COVER` |
| Sombras de nubes | Bocchi | `rocky.rs` → `shade` | _video pendiente_ | la máscara de nubes evaluada en `p + to_sun · CLOUD_HEIGHT` oscurece el suelo | `CLOUD_HEIGHT`: cuánto se corre la sombra |
| Bump mapping | Bocchi | `kit.rs` → `bump`, `rocky.rs` → `terrain` | _video pendiente_ | diferencias finitas de la altura en dos tangentes inclinan la normal | `BUMP`: fuerza del relieve |

Los videos se pueden grabar con **Win + Alt + R** (Xbox Game Bar) con la
ventana enfocada.
