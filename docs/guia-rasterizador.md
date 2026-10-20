# Guía: rasterizador de modelos .obj

Esta guía parte de la base que quedó en la rama `feat/obj-rendering` y te
lleva paso a paso hasta un rasterizador por software que:

- carga la nave (`assets/models/nave-juanmontenegro-24750.obj`);
- la traslada, rota y escala con matrices que armas tú;
- la dibuja en **dos modos que se alternan con Tab**: contorno (los
  fragmentos de los vértices y sus aristas) y relleno de caras;
- rellena con **dos algoritmos** (coordenadas baricéntricas y líneas
  horizontales) que generan exactamente las mismas posiciones;
- usa la **lógica de luces que se conservó del raytracer** para sombrear las
  caras;
- trae un modo de medición y otro de comparación para el laboratorio.

Las secciones 1 a 11 siguen la guía del curso (fragmentos, uniforms, primer
triángulo, pipeline, vertex shader, ensamblaje, rasterización y fragment
shader). Las secciones 12 a 17 cubren la nave, el laboratorio de relleno, la
profundidad, las luces y las mediciones. Al final, en la
[sección 18](#18-archivos-completos), están **todos los archivos completos**
tal como quedan al terminar. Esos archivos se probaron: compilan sin
advertencias, la ventana abre y los dos rellenos coinciden píxel a píxel.

---

## 0. Punto de partida

### Qué quedó en la base

| Archivo | Qué hace | ¿Lo tocas? |
|---|---|---|
| `src/main.rs` | abre una ventana de 800×600 y muestra el framebuffer vacío | se reemplaza |
| `src/framebuffer.rs` | la imagen en RAM y cómo se sube a la ventana | se reemplaza |
| `src/color.rs` | `Rgb`, color en punto flotante para las cuentas de luz | no |
| `src/light.rs` | luces puntuales: posición, color, intensidad, alcance | no |
| `src/material.rs` | cómo responde una superficie a la luz (difuso, especular, brillo) | no |
| `src/shading.rs` | Lambert + Phong + emisión + prueba de sombra | no |
| `assets/models/nave-juanmontenegro-24750.obj` | el modelo: 159 490 vértices y 318 976 triángulos | no |

`shading.rs` ya no depende de cómo se encuentra la geometría. La función
`shade` recibe el punto, su normal, desde dónde se mira y una **función** que
dice si algo tapa cada luz. El raytracer pasaba ahí un rayo de sombra contra
el BVH. Tú vas a pasar `shading::no_shadows`, y más adelante podrías pasar
una consulta a un *shadow map*.

### Qué se eliminó

El terreno procedural, el ruido, los modelos y las texturas de Minecraft, el
lector de JSON, el cubo, el BVH, la escena, la cámara orbital y el trazado de
rayos (incluyendo reflexión y refracción, que necesitan rayos secundarios).
Todo sigue en la rama `main` si algún día lo necesitas.

### Qué vas a crear

| Archivo nuevo | Sección |
|---|---|
| `src/fragment.rs` | [2](#2-fragmentos) |
| `src/vertex.rs`, `src/line.rs`, `src/triangle.rs` | [3](#3-vértices-líneas-y-el-triángulo-en-contorno) |
| `src/matrix.rs` | [7](#7-matrices-traslación-escala-y-rotación) |
| `src/shaders.rs` | [8](#8-vertex-shader) y [11](#11-fragment-shader) |
| `src/primitive.rs` | [9](#9-ensamblaje-de-primitivas) |
| `src/rasterizer.rs` | [10](#10-rasterización) y [13](#13-laboratorio-rellenar-triángulos) |
| `src/obj.rs` | [12](#12-cargar-la-nave) |
| `src/benchmark.rs` | [16](#16-medir-y-perfilar) |

### Convenciones que se usan en toda la guía

- **Pantalla**: el origen está arriba a la izquierda, `x` crece a la derecha
  e `y` **hacia abajo**. Para que los ejes sigan la regla de la mano derecha,
  `z` apunta **hacia adentro** de la pantalla. Por eso un `z` más chico está
  más cerca del observador.
- **Colores**: `Vector3` con rojo, verde y azul entre `0.0` y `1.0` (el
  rasterizador). `Rgb` es lo mismo, pero lo usan las cuentas de luz.
  `Rgb::from_vector` y `Rgb::to_vector` hacen de puente.
- **Comentarios en el código**: sin tildes, como el resto del repo.

---

## 1. Framebuffer: colores como `Vector3`, `point` y `clear`

El framebuffer de la base recibía colores de raylib (`Color`, bytes de 0 a
255). La guía del curso trabaja con `Vector3` de 0 a 1 y necesita tres
métodos que la base no tiene:

- `clear()`: pinta todo con el color de fondo antes de cada cuadro;
- `point(x, y, color)`: escribe un color en un píxel;
- `init_texture(...)`: crea la textura de la ventana una sola vez.

Reemplaza `src/framebuffer.rs` por la versión de la
[sección 18](#srcframebufferrs). Trae además `depth_test`, que no se usa hasta
la [sección 14](#14-profundidad-e-iluminación).

Lo importante de `point` es que **descarta lo que cae fuera de la imagen**:

```rust
fn index(&self, x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
        return None;
    }
    Some(y as usize * self.width as usize + x as usize)
}

pub fn point(&mut self, x: i32, y: i32, color: Vector3) {
    let Some(index) = self.index(x, y) else { return };
    // ... escribe los 4 bytes del pixel
}
```

Con eso ya puedes contestar una de las pruebas de la guía: un fragmento en
`(-1, 80)` no aparece porque `index` devuelve `None` y `point` sale sin
escribir. Si escribes dos fragmentos en `(100, 80)`, queda el **último**,
porque `point` pisa lo que había.

> **Main de la base.** Tu `main.rs` actual llama
> `framebuffer.set_background_color(Color::BLACK)`. Con el framebuffer nuevo
> pasa a ser `Vector3::new(0.0, 0.0, 0.0)`. En la [sección 5](#5-primer-triángulo)
> reemplazas `main` entero, así que no te preocupes por eso todavía.

---

## 2. Fragmentos

Un fragmento es **un píxel candidato**: posición en la imagen, color y
profundidad. Todavía no es un píxel definitivo: puede caer fuera de la
imagen, o quedar detrás de algo que ya se dibujó.

Crea `src/fragment.rs` con la estructura de la guía del curso:

```rust
use raylib::math::{Vector2, Vector3};

pub struct Fragment {
    pub position: Vector2,
    pub color: Vector3,
    pub depth: f32,
}

impl Fragment {
    pub fn new(x: f32, y: f32, color: Vector3, depth: f32) -> Self {
        Self { position: Vector2::new(x, y), color, depth }
    }
}
```

Para probarlo, junta fragmentos en un `Vec<Fragment>` y recórrelos llamando a
`framebuffer.point(fragment.position.x as i32, fragment.position.y as i32,
fragment.color)`. Fíjate que `as i32` **trunca** (`100.9` pasa a `100`), no
redondea.

---

## 3. Vértices, líneas y el triángulo en contorno

La guía del curso da por hecho que ya tienes `Vertex`, `line` y `triangle`.
La base no los trae, así que hay que crearlos.

### `src/vertex.rs`

```rust
#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: Vector3,             // como viene del modelo
    pub normal: Vector3,
    pub tex_coords: Vector2,
    pub color: Vector3,
    pub transformed_position: Vector3, // la calcula el vertex shader
    pub transformed_normal: Vector3,
}
```

Tiene dos constructores: `new(position, normal, tex_coords)`, que usa el
lector del `.obj`, y `new_with_color(position, color)`, que usa la guía para
el primer triángulo. La diferencia entre `position` y `transformed_position`
es la clave del pipeline: **el modelo nunca se modifica**. Cada cuadro se
parte de las posiciones originales y se calculan las transformadas.

### `src/line.rs`: Bresenham

`line(a, b)` devuelve los fragmentos de la recta entre
`a.transformed_position` y `b.transformed_position`. El algoritmo de
Bresenham avanza un píxel por paso usando solo sumas de enteros: lleva un
`error` acumulado que dice si conviene moverse en `x`, en `y` o en los dos.
Cada fragmento lleva el color de `a`, y una profundidad repartida pareja
entre `a.z` y `b.z`.

### `src/triangle.rs`: contorno

```rust
pub fn triangle(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    fragments.extend(line(v1, v2));
    fragments.extend(line(v2, v3));
    fragments.extend(line(v3, v1));
    fragments
}
```

Este es el modo **contorno**. Como cada línea incluye sus dos extremos,
también dibuja los vértices. En la [sección 13](#13-laboratorio-rellenar-triángulos)
este mismo archivo recibe los dos rellenos.

---

## 4. Uniforms

Los **uniforms** son los parámetros que comparten todos los vértices y
fragmentos de un mismo dibujo. El caso típico es la matriz de modelo: si
cada vértice usara una matriz distinta, el modelo se deformaría en vez de
moverse como una unidad.

Por ahora, en `src/main.rs`, fuera de las funciones:

```rust
pub struct Uniforms {
    pub model_matrix: Matrix,
}
```

"Uniforme" no quiere decir "constante": la matriz cambia de un cuadro al
siguiente cuando mueves la nave. Lo que no cambia es su valor **durante** un
dibujo. En la [sección 14](#14-profundidad-e-iluminación) se le agregan las
luces, que también son iguales para todo el dibujo.

**Para clasificar** (pregunta de la guía): la posición original de cada
vértice es un *atributo por vértice*. La matriz de modelo y el número de
cuadro son *uniforms*. Para dibujar dos copias de la misma nave reutilizas el
mismo `vertex_array` y cambias solo `model_matrix`.

---

## 5. Primer triángulo

Antes de cargar la nave, comprueba que vértices → fragmentos → framebuffer
funciona con un triángulo puesto a mano en coordenadas de pantalla.

En `main.rs` declara los módulos nuevos. Deja las cuatro líneas de luces con
su `#[allow(dead_code)]` hasta la sección 14:

```rust
use raylib::prelude::*;

mod fragment;
mod framebuffer;
mod line;
mod triangle;
mod vertex;

#[allow(dead_code)]
mod color;
#[allow(dead_code)]
mod light;
#[allow(dead_code)]
mod material;
#[allow(dead_code)]
mod shading;

use framebuffer::Framebuffer;
use triangle::triangle;
use vertex::Vertex;

pub struct Uniforms {
    pub model_matrix: Matrix,
}

fn render(framebuffer: &mut Framebuffer, _uniforms: &Uniforms, vertex_array: &[Vertex]) {
    for vertices in vertex_array.chunks_exact(3) {
        let fragments = triangle(&vertices[0], &vertices[1], &vertices[2]);

        for fragment in fragments {
            framebuffer.point(
                fragment.position.x as i32,
                fragment.position.y as i32,
                fragment.color,
            );
        }
    }
}

fn main() {
    let (mut window, thread) = raylib::init()
        .size(800, 600)
        .title("Primer triangulo")
        .build();
    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(800, 600);
    framebuffer.set_background_color(Vector3::new(0.0, 0.0, 0.0));
    framebuffer.init_texture(&mut window, &thread);

    let positions = [
        Vector3::new(300.0, 200.0, 0.0),
        Vector3::new(400.0, 400.0, 0.0),
        Vector3::new(500.0, 200.0, 0.0),
    ];

    let vertex_array: Vec<Vertex> = positions
        .into_iter()
        .map(|position| {
            let mut vertex = Vertex::new_with_color(position, Vector3::new(1.0, 1.0, 1.0));
            // las posiciones ya estan en coordenadas de pantalla
            vertex.transformed_position = position;
            vertex
        })
        .collect();

    let uniforms = Uniforms { model_matrix: Matrix::identity() };

    while !window.window_should_close() {
        framebuffer.clear();
        render(&mut framebuffer, &uniforms, &vertex_array);
        framebuffer.swap_buffers(&mut window, &thread);
    }
}
```

Deberías ver el contorno blanco de un triángulo con la punta **hacia
abajo**, porque `y = 400` está debajo de `y = 200`. Es normal que el
compilador avise de campos y funciones sin usar (las normales, las luces):
desaparecen a medida que avanzas. Prueba a agregar tres
vértices más para dibujar un segundo triángulo en la misma llamada.

---

## 6. El pipeline en `render`

`render` se organiza en las cuatro etapas, en el orden en que viajan los
datos:

```rust
fn render(framebuffer: &mut Framebuffer, uniforms: &Uniforms, vertex_array: &[Vertex]) {
    // 1. Procesamiento de vertices     vertex_array + uniforms -> vertices transformados
    // 2. Ensamblaje de primitivas      de tres en tres -> triangulos
    // 3. Rasterizacion                 triangulos -> fragmentos
    // 4. Procesamiento de fragmentos   fragmentos -> framebuffer
}
```

Las secciones 8 a 11 llenan cada etapa. Al final, `main` pide un dibujo con
una sola llamada a `render`.

---

## 7. Matrices: traslación, escala y rotación

Aunque la guía del curso usa la matriz de modelo desde el vertex shader,
primero hace falta saber **construirla**. Crea `src/matrix.rs`.

### Coordenadas homogéneas

Un punto 3D se escribe con cuatro componentes, `(x, y, z, 1)`. Esa cuarta
componente `w = 1` es lo que permite que una matriz de 4×4 también
**traslade**: la última columna se suma multiplicada por `w`. Una dirección,
como una normal, lleva `w = 0` y por eso la traslación no la mueve.

### raylib guarda las matrices por columnas

Los campos de `Matrix` se llaman `m0` a `m15`, y `m0, m1, m2, m3` son la
**primera columna**. Para no pensar en eso cada vez, `matrix.rs` escribe
todo **por filas**, como en papel, y una función lo acomoda:

```rust
pub fn new_matrix4(rows: [[f32; 4]; 4]) -> Matrix {
    let [r0, r1, r2, r3] = rows;
    Matrix {
        m0: r0[0], m4: r0[1], m8: r0[2], m12: r0[3],
        m1: r1[0], m5: r1[1], m9: r1[2], m13: r1[3],
        m2: r2[0], m6: r2[1], m10: r2[2], m14: r2[3],
        m3: r3[0], m7: r3[1], m11: r3[2], m15: r3[3],
    }
}
```

### Las tres transformaciones

```text
Traslacion T(t)           Escala S(s)              Rotacion en Z  Rz(a)
| 1 0 0 tx |              | s 0 0 0 |              | cos -sin 0 0 |
| 0 1 0 ty |              | 0 s 0 0 |              | sin  cos 0 0 |
| 0 0 1 tz |              | 0 0 s 0 |              |  0    0  1 0 |
| 0 0 0 1  |              | 0 0 0 1 |              |  0    0  0 1 |
```

`Rx` y `Ry` son iguales, pero dejan fijo su propio eje (están en el archivo
completo).

### El orden importa

La matriz de modelo es `M = T · Rz · Ry · Rx · S`. Aplicada a un vértice,
actúa **de derecha a izquierda**: primero escala, después rota (X, luego Y,
luego Z) y por último traslada. Si trasladaras primero, la rotación haría
girar la nave alrededor de la esquina de la pantalla en vez de sobre sí
misma.

```rust
pub fn create_model_matrix(translation: Vector3, scale: f32, rotation: Vector3) -> Matrix {
    let rotation_matrix = multiply(
        &rotation_z(rotation.z),
        &multiply(&rotation_y(rotation.y), &rotation_x(rotation.x)),
    );
    multiply(&translation_matrix(translation), &multiply(&rotation_matrix, &scale_matrix(scale)))
}
```

> **Cuidado con el `*` de raylib.** raylib implementa `Matrix * Matrix`, pero
> en el orden contrario al de papel: `a * b` le aplica primero `a` y después
> `b`. Por eso `matrix.rs` tiene su propio `multiply(a, b)`, que sí es `a · b`
> como en clase. No mezcles los dos.

**Predicción antes de ejecutar**: con `rotation = (0, 0, PI/2)`, un vértice
en `(1, 0, 0)` del modelo termina en `(0, 1, 0)` antes de trasladar. En
pantalla, `y` crece hacia abajo, así que lo ves moverse **hacia abajo**, no
hacia arriba como en clase.

---

## 8. Vertex shader

Crea `src/shaders.rs`. El vertex shader recibe **un** vértice y los
uniforms, y devuelve el vértice con `transformed_position` calculada. Se
llama una vez por vértice, y todas las llamadas de un dibujo usan la misma
`model_matrix`.

```rust
fn multiply_matrix_vector4(matrix: &Matrix, vector: &Vector4) -> Vector4 {
    Vector4::new(
        matrix.m0 * vector.x + matrix.m4 * vector.y + matrix.m8 * vector.z + matrix.m12 * vector.w,
        matrix.m1 * vector.x + matrix.m5 * vector.y + matrix.m9 * vector.z + matrix.m13 * vector.w,
        matrix.m2 * vector.x + matrix.m6 * vector.y + matrix.m10 * vector.z + matrix.m14 * vector.w,
        matrix.m3 * vector.x + matrix.m7 * vector.y + matrix.m11 * vector.z + matrix.m15 * vector.w,
    )
}

pub fn vertex_shader(vertex: &Vertex, uniforms: &Uniforms) -> Vertex {
    let position = Vector4::new(vertex.position.x, vertex.position.y, vertex.position.z, 1.0);
    let transformed = multiply_matrix_vector4(&uniforms.model_matrix, &position);

    let mut result = vertex.clone();
    result.transformed_position = Vector3::new(transformed.x, transformed.y, transformed.z);
    result
}
```

Cada fila de la multiplicación usa `m0, m4, m8, m12`: es la primera **fila**
de la matriz, que raylib guarda salteada porque almacena por columnas.

En `main.rs` agrega `mod matrix;`, `mod shaders;` y
`use shaders::vertex_shader;`. Como primera etapa de `render`:

```rust
// 1. Procesamiento de vertices
let transformed_vertices: Vec<Vertex> = vertex_array
    .iter()
    .map(|vertex| vertex_shader(vertex, uniforms))
    .collect();
```

Comprueba que con `Matrix::identity()` el triángulo no se mueve. Después
prueba `create_model_matrix(Vector3::new(100.0, 0.0, 0.0), 1.0, Vector3::zero())`:
debería correrse 100 píxeles a la derecha.

**Versión final.** En la sección 14 el vertex shader también transforma la
normal (con `w = 0`) y, en el modo relleno, calcula el color del vértice con
las luces.

---

## 9. Ensamblaje de primitivas

Crea `src/primitive.rs` con `primitive_assembly`, tal como está en la guía
del curso: agrupa los vértices transformados de tres en tres en
`Vec<[Vertex; 3]>`. El `assert!` avisa si sobra algún vértice, en vez de
perderlo en silencio.

```rust
// 2. Ensamblaje de primitivas
let triangles = primitive_assembly(&transformed_vertices);
```

Con siete vértices, el `assert!` hace que el programa se detenga con
"La cantidad de vertices debe ser multiplo de 3". El lector del `.obj`
siempre entrega múltiplos de 3, porque parte cada cara en triángulos.

---

## 10. Rasterización

Crea `src/rasterizer.rs`. La primera versión (la de la guía del curso)
llama a `triangle` para cada triángulo y junta todo en un solo `Vec`:

```rust
pub fn rasterize(triangles: &[[Vertex; 3]]) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    for tri in triangles {
        fragments.extend(triangle(&tri[0], &tri[1], &tri[2]));
    }
    fragments
}
```

En la [sección 13](#13-laboratorio-rellenar-triángulos) recibe además un
`TriangleMode` para elegir el algoritmo. El ensamblaje y el procesamiento de
fragmentos no cambian.

---

## 11. Fragment shader

En `src/shaders.rs`:

```rust
pub fn fragment_shader(fragment: &Fragment) -> Vector3 {
    fragment.color
}
```

Es un *passthrough*: devuelve el color que el fragmento ya traía. La cuarta
etapa de `render` queda así:

```rust
// 4. Procesamiento de fragmentos
for fragment in fragments {
    let color = fragment_shader(&fragment);
    framebuffer.point(fragment.position.x as i32, fragment.position.y as i32, color);
}
```

Para comprobar que `render` usa lo que devuelve el shader, cambia
temporalmente el cuerpo por `Vector3::new(1.0, 0.0, 0.0)`: el contorno debe
salir rojo. Después vuelve a `fragment.color`.

En este rasterizador el color de las caras se calcula **por vértice** (en el
vertex shader) y no por fragmento, porque es mucho más barato: hay unos 957
mil vértices por cuadro contra varios millones de fragmentos al acercarse.
Por eso el fragment shader puede quedarse como passthrough.

---

## 12. Cargar la nave

### El formato `.obj`

Es texto, una instrucción por línea. Solo hacen falta cuatro:

```text
v  0.000000 -0.000000 0.416008     posicion de un vertice
vn 0.0307 0.9201 0.3906            una normal
vt 0.5 0.25                        una coordenada de textura
f  19316/1/1 19330/2/1 19315/3/1   una cara: posicion/textura/normal de cada esquina
```

Hay que tener en cuenta varias cosas:

- **Los índices empiezan en 1**, no en 0. Un índice negativo cuenta desde el
  último elemento leído (`-1` es el último).
- **Una esquina puede omitir campos**: `12`, `12/5`, `12//3` y `12/5/3` son
  todas válidas.
- **Una cara puede tener más de tres esquinas.** Se parte en **abanico**: una
  cara `[a, b, c, d]` da los triángulos `[a, b, c]` y `[a, c, d]`.

`src/obj.rs` guarda las listas y los triángulos como índices.
`get_vertex_array()` arma la secuencia de vértices de tres en tres, que es
exactamente lo que espera el pipeline. Un vértice que comparten varias caras
aparece una vez por cada una.

### Centrar el modelo

La nave no está centrada en el origen: en `z` va de −3,96 a 1,94. Si la
rotaras así, giraría alrededor de un punto que no es su centro.
`center_model` (en `main.rs`) calcula la caja que la contiene y le resta el
centro a cada `position` **una sola vez**, al cargar. También devuelve el
radio, que sirve para escalarla a la ventana:

```rust
scale = 0.42 * min(ancho, alto) / radio    // ~0.42 del lado corto
```

### Orientarla

Blender exporta con `y` hacia arriba, y en la pantalla `y` crece hacia
abajo. Si dibujas la nave sin rotar, se ve de cabeza y de canto. La rotación
inicial es `(-1.1, 0.5, PI)`:

- **−1.1 en X** (casi −90°): la acuesta para verla desde arriba, un poco
  inclinada hacia el observador;
- **0.5 en Y**: la muestra de tres cuartos;
- **π en Z**: le pone la punta hacia arriba.

Usar una rotación y no una escala negativa en `y` es a propósito. Una escala
negativa en un solo eje es un **espejo**: la nave quedaría reflejada como en
un espejo y con las caras al revés.

### Cuánto pesa

| | Cantidad |
|---|---|
| vértices en el archivo | 159 490 |
| triángulos | 318 976 |
| vértices en el `vertex_array` (3 por triángulo) | 956 928 |

---

## 13. Laboratorio: rellenar triángulos

### 13.1 Los modos

En `rasterizer.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangleMode {
    Wireframe,
    FilledBarycentric,
    FilledScanline,
}
```

`rasterize(triangles, mode)` hace un `match` sobre el modo y llama a
`triangle`, `triangle_barycentric` o `triangle_scanline`. Las tres reciben
los mismos tres vértices y devuelven `Vec<Fragment>`. Cambiar de algoritmo no
toca ni el ensamblaje ni el procesamiento de fragmentos.

### 13.2 Una regla común para los dos rellenos

Para que los dos algoritmos generen **las mismas posiciones**, tienen que
responder igual a la pregunta "¿este píxel está adentro?". La regla es esta:

1. **Se prueba el centro de cada píxel**, `(x + 0.5, y + 0.5)`.
2. **Un centro que cae justo sobre un borde** se dibuja solo si el borde es
   **izquierdo** (el interior queda a su derecha) o **superior** (es
   horizontal y el interior queda debajo). Es la regla *top-left* que usan
   las GPU.

¿Por qué la segunda regla? Dos triángulos que comparten una arista la tienen
como borde derecho en uno e izquierdo en el otro. Con la regla, el píxel que
cae justo encima se dibuja **una vez**: no queda hueco y no se dibuja dos
veces. La prueba del cuadrado partido en dos
([13.5](#135-comprobar-los-resultados)) lo verifica.

Las dos funciones auxiliares que lo traducen a píxeles:

```rust
fn first_pixel(value: f32) -> i32 { (value - 0.5).ceil() as i32 }  // primer centro >= value
fn last_pixel(value: f32) -> i32 { (value - 0.5).floor() as i32 }  // ultimo centro <= value
```

### 13.3 Algoritmo 1: coordenadas baricéntricas

**La función de borde.** Para un borde `a → b` y un punto `p`:

```text
E(a, b, p) = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
```

`E` es el **doble del área con signo** del triángulo `(a, b, p)`. Vale cero
sobre el borde, es positiva de un lado y negativa del otro. Un punto está
adentro del triángulo si está **del mismo lado de los tres bordes**.

**Los pasos:**

1. `area = E(a, b, c)`. Si `|area| < 1e-6`, los tres vértices están
   alineados (**caso degenerado**): se devuelve un `Vec` vacío sin dividir
   entre cero.
2. Si `area < 0`, los vértices vienen en el otro orden. Se intercambian `b`
   y `c`, y el área pasa a ser positiva. Así **invertir el orden no hace
   desaparecer el triángulo**.
3. Se calcula el rectángulo más chico que lo contiene, ya en píxeles, con
   `first_pixel` del mínimo y `last_pixel` del máximo.
4. Para cada píxel del rectángulo se calculan los tres pesos:
   `w_a = E(b, c, p)`, `w_b = E(c, a, p)` y `w_c = E(a, b, p)`. Cada peso es
   el área del subtriángulo **opuesto** a ese vértice.
5. Si los tres son positivos (o cero sobre un borde superior o izquierdo),
   el píxel está adentro.
6. Las **coordenadas baricéntricas** son los pesos divididos por el área, y
   suman 1. Sirven para interpolar cualquier atributo. Aquí se usan para la
   profundidad: `depth = (w_a·a.z + w_b·b.z + w_c·c.z) / area`.

Con el triángulo ya orientado, la regla top-left para un borde
`from → to` queda en una línea:

```rust
fn is_top_left(from: Vector3, to: Vector3) -> bool {
    to.y < from.y || (to.y == from.y && to.x > from.x)
}
```

La primera mitad detecta un borde izquierdo (sube mientras recorre el
triángulo). La segunda detecta un borde superior (horizontal y recorrido
hacia la derecha).

### 13.4 Algoritmo 2: líneas horizontales

Se avanza **una fila a la vez** y, en cada fila, se rellena el tramo entre
los dos bordes.

1. Mismo descarte de triángulos degenerados.
2. Se ordenan los vértices por `y`: `top`, `middle`, `bottom`.
3. **Filas**: las que tienen el centro en `[top.y, bottom.y)`. Se incluye la
   de arriba y no la de abajo, igual que un borde superior sí y uno inferior
   no.
4. En cada fila hay dos bordes:
   - el **largo**, de `top` a `bottom`, que cruza todas las filas;
   - el **corto**, que es `top → middle` si el centro de la fila está antes
     de `middle.y`, y `middle → bottom` desde ahí.
5. Se calcula dónde cruza la fila cada borde (y su profundidad). El de menor
   `x` es el izquierdo.
6. Se generan los píxeles cuyo centro está en `[izquierda, derecha)`: el
   borde izquierdo se incluye y el derecho no.

**Los casos delicados** (los que pide pensar la guía):

- **Aristas horizontales.** Nunca se usan para calcular un cruce. Si
  `top.y == middle.y`, todas las filas cumplen `centro >= middle.y` y usan
  `middle → bottom`. Si `middle.y == bottom.y`, ninguna fila llega a
  `bottom.y`. Así nunca se divide entre cero.
- **Un vértice justo en el centro de una fila.** En `y == middle.y` los dos
  bordes cortos dan la misma `x` (el propio `middle`), así que da igual cuál
  se use. En el vértice de arriba el tramo tiene ancho cero, y
  `[x, x)` está vacío: no se dibuja. En la regla top-left pasa lo mismo,
  porque ese punto está sobre un borde derecho.

Que las dos reglas coincidan en todos estos casos es lo que hace que los dos
rellenos den **las mismas posiciones**.

### 13.5 Comprobar los resultados

`--compare-triangles` (en `benchmark.rs`) corre los casos de la guía con los
dos algoritmos, compara los **conjuntos de posiciones** y repite con los
vértices en orden inverso. No abre ventana. El resultado que debería darte:

```text
caso                      baricentr.      lineas  distintas  orden inverso
pequeno                           10          10          0          igual
grande                        172000      172000          0          igual
borde horizontal               15000       15000          0          igual
borde vertical                 16000       16000          0          igual
parcialmente fuera             40645       40645          0          igual
colineales                         0           0          0          igual
bordes sobre centros              45          45          0          igual
vertices fraccionarios          2199        2199          0          igual

cuadrado partido en dos triangulos (deberia dar 10000 y 0):
  baricentrico   pixeles:  10000   repetidos: 0
  por lineas     pixeles:  10000   repetidos: 0
```

Esto es lo que muestra cada caso:

- **parcialmente fuera**: los dos generan también los fragmentos de fuera
  de la imagen, y es `framebuffer.point` el que los descarta (sección 1).
  Recortar el rectángulo a la ventana sería una buena optimización.
- **colineales**: cero fragmentos, porque un triángulo sin área no cubre
  ningún centro.
- **bordes sobre centros**: el triángulo `(0,0) (10,0) (0,10)` tiene la
  hipotenusa pasando exactamente por centros de píxel. Es donde se nota la
  regla top-left: sin ella los dos algoritmos podrían diferir justo ahí.

Si alguna vez te da `distintas > 0`, casi siempre es redondeo de `f32` en un
centro que cae casi exactamente sobre un borde. La función de borde y la
interpolación por filas hacen la misma cuenta en distinto orden, así que
pueden diferir en el último bit.

### 13.6 Tab y F

En `main`:

```rust
let mut mode = TriangleMode::Wireframe;
let mut fill = TriangleMode::FilledBarycentric;   // el relleno que usa Tab

if window.is_key_pressed(KeyboardKey::KEY_TAB) {
    mode = if mode.is_filled() { TriangleMode::Wireframe } else { fill };
}
if window.is_key_pressed(KeyboardKey::KEY_F) {
    fill = if fill == TriangleMode::FilledBarycentric {
        TriangleMode::FilledScanline
    } else {
        TriangleMode::FilledBarycentric
    };
    if mode.is_filled() { mode = fill; }
}
```

**Tab** alterna los dos modos que pediste (contorno y relleno). **F** elige
cuál de los dos algoritmos usa el relleno, para comparar los tres modos del
laboratorio sin un tercer estado en Tab. El título de la ventana muestra el
modo actual y cuánto tardó el último cuadro.

> **El contorno de la nave se ve como una mancha blanca.** No es un error:
> son 319 mil triángulos en unos 150 mil píxeles, así que las aristas tapan
> todo. Acércate con **X** (escala) y se ve la malla.

---

## 14. Profundidad e iluminación

Con las caras rellenas aparecen dos problemas que el contorno no tenía.

### Profundidad (z-buffer)

Los triángulos se dibujan en el orden del archivo, y sin más información un
ala de atrás puede pintarse **encima** de una de adelante. El framebuffer
guarda, por cada píxel, la profundidad de lo que ya tiene dibujado:

```rust
pub fn depth_test(&mut self, x: i32, y: i32, depth: f32) -> bool {
    let Some(index) = self.index(x, y) else { return false };
    if depth < self.depth[index] {     // mas cerca que lo que habia
        self.depth[index] = depth;
        true
    } else {
        false
    }
}
```

`clear()` lo vuelve a llenar con `f32::INFINITY` en cada cuadro. Cada
relleno interpola la profundidad dentro del triángulo (baricéntricas o por
filas), y la cuarta etapa la consulta **antes** de llamar al fragment shader:

```rust
if mode.is_filled() && !framebuffer.depth_test(x, y, fragment.depth) {
    continue;   // queda detras: ni se sombrea
}
```

En el contorno no se hace la prueba, a propósito: así se ven también las
aristas de atrás.

### Iluminación con las luces que se conservaron

Con un solo color, la nave rellena es una silueta plana. Aquí es donde
vuelven las luces del raytracer. Los uniforms crecen:

```rust
pub struct Uniforms {
    pub model_matrix: Matrix,
    pub lighting: bool,          // true en relleno, false en contorno
    pub lights: Vec<Light>,
    pub ambient: Rgb,
    pub material: Material,
}
```

**En el vertex shader** pasan tres cosas:

1. **La normal se transforma con `w = 0`**: la rotación la gira y la
   traslación no la mueve. Después se normaliza, porque la escala la estira.
   Esto vale solo con escala pareja: con una escala distinta en cada eje
   habría que usar la *inversa transpuesta* de la matriz.
2. **Se da vuelta la normal si mira hacia atrás.** El observador está en
   `-z` (`TO_VIEWER = (0, 0, -1)`). Si la normal no apunta hacia él, se
   invierte, y así una cara vista de espaldas no queda negra.
3. **Se llama a `shading::shade`**, la misma función del raytracer, con el
   punto y la normal ya transformados y `shading::no_shadows` como prueba de
   sombra. El resultado se guarda en `vertex.color`.

**En el relleno** cada cara toma un solo color, el promedio de sus tres
vértices (`flat_color` en `triangle.rs`). Es *flat shading*. Si en vez de
promediar interpolaras los colores con las coordenadas baricéntricas,
tendrías *Gouraud*; es el siguiente paso natural.

Las tres luces son las del raytracer (principal arriba a la izquierda,
relleno a la derecha, contraluz por detrás). Están **fijas en coordenadas de
pantalla**: cuando rotas la nave, gira la nave bajo las luces y no las luces
con ella.

**¿Y las sombras?** `shade` sigue sabiendo descartar una luz tapada. Lo que
falta es quién le conteste si algo la tapa. En un rasterizador eso se hace
con un **shadow map**: se rasteriza la escena desde la luz guardando solo la
profundidad, y al sombrear se compara contra ese mapa. Cuando lo tengas, lo
pasas en lugar de `no_shadows`.

---

## 15. Controles

| Tecla | Qué hace | Cómo |
|---|---|---|
| **Tab** | contorno ↔ relleno | cambia `mode` |
| **F** | baricéntrico ↔ por líneas | cambia `fill` |
| **Flechas** | trasladar | `translation.x/y ± 300·dt` |
| **W/S** | rotar en X | `rotation.x ∓ 1.5·dt` |
| **A/D** | rotar en Y | `rotation.y ∓ 1.5·dt` |
| **Q/E** | rotar en Z | `rotation.z ∓ 1.5·dt` |
| **Z / X** | escala | `scale ÷/× 1.8^dt` |
| **R** | volver al inicio | `transform = initial` |
| **P** | guardar captura | `captura-<modo>.png` |

Todo se multiplica por `dt`, el tiempo del último cuadro, así que la
velocidad no depende de cuántos cuadros por segundo saque la máquina. La
escala **multiplica** en vez de sumar, para que el zoom se sienta igual de
rápido de cerca que de lejos.

Con esta malla, un cuadro tarda entre 0,1 y 0,2 s. Por eso el ciclo
**redibuja solo si algo cambió**: guarda en `drawn` el `(Transform, modo)`
que tiene dibujado y lo compara en cada vuelta. Mientras no tocas nada, la
ventana responde al instante.

---

## 16. Medir y perfilar

### Medir solo la rasterización

`--benchmark-triangles` (en `benchmark.rs`) hace el vertex shader y el
ensamblaje **una vez, afuera**, y después mide cada modo:

- 20 pasadas por medición y 7 mediciones, de las que se toma la **mediana**;
- `black_box` en la entrada y en el resultado, para que el optimizador no
  elimine el trabajo;
- también informa **cuántos fragmentos** produce cada modo. Un algoritmo que
  se saltara parte del triángulo parecería más rápido solo por hacer menos.

```sh
cargo run --release -- --benchmark-triangles
```

Como referencia, en esta máquina dio:

```text
contorno                  2113900 fragmentos   mediana   107.59ms por pasada
relleno baricentrico       175740 fragmentos   mediana    20.08ms por pasada
relleno por lineas         175740 fragmentos   mediana    20.80ms por pasada
```

**Cómo leerlo:**

- **Los dos rellenos generan los mismos 175 740 fragmentos** y tardan casi
  lo mismo. Con la nave a su tamaño normal, casi todos los triángulos miden
  menos de un píxel: el rectángulo del baricéntrico tiene muy pocos píxeles
  y el scanline recorre una fila o ninguna, así que los dos hacen casi el
  mismo trabajo. Lo más probable es que pese más el costo fijo de cada
  triángulo (crear su `Vec` y copiarlo al grande) que el relleno en sí. El
  perfil sirve para confirmarlo o descartarlo.
- **El contorno genera 12 veces más fragmentos** que el relleno y es 5 veces
  más lento. Cada línea genera al menos un fragmento aunque mida menos de un
  píxel, mientras que un triángulo que no cubre ningún centro de píxel no
  genera ninguno. Además cada arista se dibuja dos veces, una por cada
  triángulo que la comparte.
- **Al acercarse, la cosa cambia.** Con la nave 6 veces más grande, el
  cuadro completo tardó unos 410 ms con baricéntricas y 330 ms por líneas.
  Con triángulos grandes, el baricéntrico prueba todo el rectángulo y
  descarta cerca de la mitad, mientras que el scanline solo visita píxeles de
  adentro.

Con esa misma vista ampliada, cada etapa del cuadro en relleno tardó:

| Etapa | Tiempo |
|---|---|
| vertex shader con luces | 82 ms (29 ms sin luces) |
| ensamblaje (clonar 957 mil vértices) | 20 ms |
| rasterización (7,7 millones de fragmentos) | 287 ms |

### Perfilar con flamegraph

Una medición dice **cuánto** tarda. Un perfilador dice **dónde**.

1. Para que el perfil muestre nombres de funciones, agrega símbolos de
   depuración a un perfil propio en `Cargo.toml`:

   ```toml
   [profile.profiling]
   inherits = "release"
   debug = true
   ```

2. Instala la herramienta. En Windows, `cargo flamegraph` usa el rastreo de
   eventos del sistema (ETW), así que **abre la terminal como
   administrador**:

   ```sh
   cargo install flamegraph
   cargo flamegraph --profile profiling -- --benchmark-triangles
   ```

   Si no logra arrancar, otra opción es
   [samply](https://github.com/mstange/samply):
   `cargo install samply`, `cargo build --profile profiling` y
   `samply record target/profiling/raytracer.exe --benchmark-triangles`.
   Abre el perfil en el navegador, con una vista de llamas equivalente.

3. Abre `flamegraph.svg` en el navegador y busca `triangle_barycentric`,
   `triangle_scanline`, `line` y `rasterize`.

**Qué mirar** (y qué contestar en el README):

- ¿Cuánto ancho se lleva el cálculo (`edge_function`, `cross_at`) frente a
  `push` y la reserva de memoria (`RawVec`, `reserve`, `alloc`)?
- ¿Cuánto pesa `extend` dentro de `rasterize`? Cada triángulo crea su propio
  `Vec` y después se copia al grande.
- El **ancho** de un bloque es la proporción de muestras en las que apareció
  esa función. No indica el orden en que se ejecutaron.

Si alguna función no aparece, es porque el compilador la *inlineó* dentro de
otra. Ponle `#[inline(never)]` solo mientras perfilas.

**Ideas de mejora para comentar** (no hace falta implementarlas): pasar a
cada `triangle_*` un `&mut Vec<Fragment>` para que escriba directo en el
grande, recortar el rectángulo y las filas al tamaño de la ventana, y no
clonar los vértices en el ensamblaje (`[&Vertex; 3]` en vez de
`[Vertex; 3]`).

---

## 17. Entrega

El README de la rama ya tiene las secciones de la entrega con comentarios
`Pendiente`:

- [ ] **Una imagen de cada modo**: abre la ventana, pulsa **P** en contorno,
  Tab y **P** en relleno, y **F** y **P** para el otro algoritmo. Se guardan
  como `captura-<modo>.png` en la carpeta del proyecto.
- [ ] **Mediciones**: la salida de `--benchmark-triangles` y de
  `--compare-triangles`.
- [ ] **Perfil**: `flamegraph.svg` o una captura de samply.
- [ ] **Explicación breve**: la regla que comparten los dos rellenos, por
  qué dan las mismas posiciones, por qué el contorno es el más lento con
  esta malla y qué cambia al acercarse.

---

## 18. Archivos completos

Así queda cada archivo al terminar. `color.rs`, `light.rs`, `material.rs` y
`shading.rs` **no cambian**: son los de la base. `Cargo.toml` tampoco, salvo
el perfil opcional `profiling` de la sección 16.

| Archivo | Estado |
|---|---|
| `src/main.rs` | se reemplaza |
| `src/framebuffer.rs` | se reemplaza |
| `src/fragment.rs` | nuevo |
| `src/vertex.rs` | nuevo |
| `src/line.rs` | nuevo |
| `src/triangle.rs` | nuevo |
| `src/matrix.rs` | nuevo |
| `src/shaders.rs` | nuevo |
| `src/primitive.rs` | nuevo |
| `src/rasterizer.rs` | nuevo |
| `src/obj.rs` | nuevo |
| `src/benchmark.rs` | nuevo |
| `src/color.rs`, `src/light.rs`, `src/material.rs`, `src/shading.rs` | sin cambios |

### `src/main.rs`

El pipeline en `render`, los uniforms, la carga y el ciclo de la ventana.

```rust
// main.rs
// Rasterizador por software: carga un .obj y lo dibuja pasando por el
// pipeline vertices -> triangulos -> fragmentos -> framebuffer.
use raylib::prelude::*;
use std::f32::consts::PI;
use std::time::Instant;

mod benchmark;
mod color;
mod fragment;
mod framebuffer;
mod light;
mod line;
mod material;
mod matrix;
mod obj;
mod primitive;
mod rasterizer;
mod shaders;
mod shading;
mod triangle;
mod vertex;

use color::Rgb;
use framebuffer::Framebuffer;
use light::Light;
use material::Material;
use matrix::create_model_matrix;
use obj::Obj;
use primitive::primitive_assembly;
use rasterizer::{rasterize, TriangleMode};
use shaders::{fragment_shader, vertex_shader};
use vertex::Vertex;

const WIDTH: i32 = 800;
const HEIGHT: i32 = 600;
const MODEL_PATH: &str = "assets/models/nave-juanmontenegro-24750.obj";

/// Cuanto de la ventana ocupa el modelo al arrancar (radio / lado corto).
const FIT: f32 = 0.42;
/// Velocidades de los controles, por segundo.
const MOVE_SPEED: f32 = 300.0;
const TURN_SPEED: f32 = 1.5;
const ZOOM_SPEED: f32 = 1.8;

/// Lo que comparten todos los vertices y fragmentos de un mismo dibujo.
pub struct Uniforms {
    pub model_matrix: Matrix,
    /// Si el vertex shader calcula el color con las luces. En el contorno
    /// no: las lineas quedan del color del vertice.
    pub lighting: bool,
    pub lights: Vec<Light>,
    pub ambient: Rgb,
    pub material: Material,
}

/// Donde esta el modelo, que tan grande y como esta girado.
#[derive(Clone, Copy, PartialEq)]
struct Transform {
    translation: Vector3,
    scale: f32,
    /// Angulos en radianes alrededor de X, Y y Z.
    rotation: Vector3,
}

impl Transform {
    fn matrix(&self) -> Matrix {
        create_model_matrix(self.translation, self.scale, self.rotation)
    }
}

fn render(
    framebuffer: &mut Framebuffer,
    uniforms: &Uniforms,
    vertex_array: &[Vertex],
    mode: TriangleMode,
) {
    // 1. Procesamiento de vertices
    let transformed_vertices: Vec<Vertex> = vertex_array
        .iter()
        .map(|vertex| vertex_shader(vertex, uniforms))
        .collect();

    // 2. Ensamblaje de primitivas
    let triangles = primitive_assembly(&transformed_vertices);

    // 3. Rasterizacion
    let fragments = rasterize(&triangles, mode);

    // 4. Procesamiento de fragmentos
    for fragment in fragments {
        let x = fragment.position.x as i32;
        let y = fragment.position.y as i32;

        // relleno: solo pasa lo que queda delante de lo ya dibujado. En el
        // contorno se ven todas las aristas, tambien las de atras
        if mode.is_filled() && !framebuffer.depth_test(x, y, fragment.depth) {
            continue;
        }

        let color = fragment_shader(&fragment);
        framebuffer.point(x, y, color);
    }
}

/// Mueve el modelo para que su centro quede en el origen, y devuelve su
/// radio. Asi rota sobre si mismo y se puede escalar a la ventana.
fn center_model(vertices: &mut [Vertex]) -> f32 {
    let mut min = Vector3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vector3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

    for vertex in vertices.iter() {
        let p = vertex.position;
        min = Vector3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
        max = Vector3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
    }

    let center = (min + max) * 0.5;
    let mut radius: f32 = 0.0;
    for vertex in vertices.iter_mut() {
        vertex.position = vertex.position - center;
        radius = radius.max(vertex.position.length());
    }

    radius.max(f32::EPSILON)
}

/// Las tres luces del raytracer: una principal arriba a la izquierda, una de
/// relleno a la derecha y una por detras. Estan en coordenadas de pantalla
/// (y hacia abajo, z hacia adentro), alrededor del centro de la ventana.
fn default_lights(center: Vector3, radius: f32) -> Vec<Light> {
    vec![
        Light::new(
            center + Vector3::new(-1.2 * radius, -2.2 * radius, -1.4 * radius),
            Rgb::new(1.0, 0.96, 0.88),
            0.9,
        ),
        Light::new(
            center + Vector3::new(1.8 * radius, -0.6 * radius, -1.1 * radius),
            Rgb::new(0.68, 0.80, 1.0),
            0.3,
        ),
        Light::new(
            center + Vector3::new(0.7 * radius, 2.0 * radius, 0.9 * radius),
            Rgb::new(0.74, 0.84, 1.0),
            0.34,
        ),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--compare-triangles") {
        benchmark::compare_algorithms();
        return;
    }

    let obj = match Obj::load(MODEL_PATH) {
        Ok(obj) => obj,
        Err(problem) => {
            eprintln!("no se pudo cargar el modelo: {problem}");
            return;
        }
    };
    let mut vertex_array = obj.get_vertex_array();
    let radius = center_model(&mut vertex_array);
    println!("triangulos: {}", vertex_array.len() / 3);

    let screen_center = Vector3::new(WIDTH as f32 / 2.0, HEIGHT as f32 / 2.0, 0.0);
    let fit_radius = FIT * WIDTH.min(HEIGHT) as f32;

    // En Blender y sube y en la pantalla baja. Girar casi -90 grados en X lo
    // deja visto desde arriba e inclinado hacia el observador, 180 en Z le
    // pone la punta hacia arriba y el giro en Y lo muestra de tres cuartos.
    let initial = Transform {
        translation: screen_center,
        scale: fit_radius / radius,
        rotation: Vector3::new(-1.1, 0.5, PI),
    };

    let mut uniforms = Uniforms {
        model_matrix: initial.matrix(),
        lighting: false,
        lights: default_lights(screen_center, fit_radius),
        ambient: Rgb::new(0.24, 0.27, 0.33),
        material: Material::polished(Color::new(196, 202, 214, 255)),
    };

    if args.iter().any(|arg| arg == "--benchmark-triangles") {
        benchmark::run(&vertex_array, &uniforms);
        return;
    }

    let (mut window, thread) = raylib::init()
        .size(WIDTH, HEIGHT)
        .title("Rasterizador")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();
    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(WIDTH as u32, HEIGHT as u32);
    framebuffer.set_background_color(Vector3::new(0.05, 0.06, 0.09));
    framebuffer.init_texture(&mut window, &thread);

    let mut transform = initial;
    let mut mode = TriangleMode::Wireframe;
    // el algoritmo de relleno que se usa al pasar a relleno con Tab
    let mut fill = TriangleMode::FilledBarycentric;
    // lo que tiene dibujado el framebuffer ahora mismo
    let mut drawn: Option<(Transform, TriangleMode)> = None;

    println!("Tab = contorno / relleno | F = algoritmo de relleno");
    println!("flechas = mover | W/S, A/D, Q/E = rotar en X, Y, Z | Z/X = escala");
    println!("R = volver al inicio | P = guardar captura");

    while !window.window_should_close() {
        let dt = window.get_frame_time();
        let held = |key| window.is_key_down(key);

        if window.is_key_pressed(KeyboardKey::KEY_TAB) {
            mode = if mode.is_filled() { TriangleMode::Wireframe } else { fill };
        }
        if window.is_key_pressed(KeyboardKey::KEY_F) {
            fill = if fill == TriangleMode::FilledBarycentric {
                TriangleMode::FilledScanline
            } else {
                TriangleMode::FilledBarycentric
            };
            if mode.is_filled() {
                mode = fill;
            }
        }
        if window.is_key_pressed(KeyboardKey::KEY_R) {
            transform = initial;
        }

        // traslacion
        let step = MOVE_SPEED * dt;
        if held(KeyboardKey::KEY_LEFT) { transform.translation.x -= step; }
        if held(KeyboardKey::KEY_RIGHT) { transform.translation.x += step; }
        if held(KeyboardKey::KEY_UP) { transform.translation.y -= step; }
        if held(KeyboardKey::KEY_DOWN) { transform.translation.y += step; }

        // rotacion
        let turn = TURN_SPEED * dt;
        if held(KeyboardKey::KEY_W) { transform.rotation.x -= turn; }
        if held(KeyboardKey::KEY_S) { transform.rotation.x += turn; }
        if held(KeyboardKey::KEY_A) { transform.rotation.y -= turn; }
        if held(KeyboardKey::KEY_D) { transform.rotation.y += turn; }
        if held(KeyboardKey::KEY_Q) { transform.rotation.z -= turn; }
        if held(KeyboardKey::KEY_E) { transform.rotation.z += turn; }

        // escala: multiplicar en vez de sumar hace que el zoom se sienta
        // igual de rapido de cerca que de lejos
        if held(KeyboardKey::KEY_Z) { transform.scale /= ZOOM_SPEED.powf(dt); }
        if held(KeyboardKey::KEY_X) { transform.scale *= ZOOM_SPEED.powf(dt); }

        // con cientos de miles de triangulos un cuadro tarda mas de 100 ms:
        // se vuelve a dibujar solo si algo cambio
        if drawn != Some((transform, mode)) {
            // los uniforms de este dibujo
            uniforms.model_matrix = transform.matrix();
            uniforms.lighting = mode.is_filled();

            let start = Instant::now();
            framebuffer.clear();
            render(&mut framebuffer, &uniforms, &vertex_array, mode);
            let elapsed = start.elapsed().as_secs_f32() * 1000.0;

            window.set_window_title(
                &thread,
                &format!("Rasterizador | {} | {elapsed:.0} ms", mode.name()),
            );
            drawn = Some((transform, mode));
        }

        if window.is_key_pressed(KeyboardKey::KEY_P) {
            let path = format!("captura-{}.png", mode.name().replace(' ', "-"));
            match framebuffer.save(&path) {
                Ok(()) => println!("captura guardada en {path}"),
                Err(problem) => eprintln!("{problem}"),
            }
        }

        framebuffer.swap_buffers(&mut window, &thread);
    }
}
```

### `src/framebuffer.rs`

Colores como `Vector3`, `clear`, `point`, `depth_test` y la textura de la ventana.

```rust
// framebuffer.rs
// La imagen en RAM, su profundidad y como se sube a la ventana.
use raylib::prelude::*;

pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// Pixeles en RAM, RGBA8 plano (4 bytes por pixel).
    pixels: Vec<u8>,
    /// Profundidad de lo que ya se dibujo en cada pixel (z-buffer).
    depth: Vec<f32>,
    background_color: Vector3,
    /// Textura en GPU reutilizada frame a frame; se crea una sola vez.
    texture: Option<Texture2D>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = width as usize * height as usize;
        Framebuffer {
            width,
            height,
            pixels: vec![0; size * 4],
            depth: vec![f32::INFINITY; size],
            background_color: Vector3::new(0.0, 0.0, 0.0),
            texture: None,
        }
    }

    pub fn set_background_color(&mut self, color: Vector3) {
        self.background_color = color;
    }

    /// Pinta todo con el fondo y olvida las profundidades.
    pub fn clear(&mut self) {
        let background = [
            to_byte(self.background_color.x),
            to_byte(self.background_color.y),
            to_byte(self.background_color.z),
            255,
        ];
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&background);
        }
        self.depth.fill(f32::INFINITY);
    }

    /// Indice del pixel, o None si cae fuera de la imagen.
    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(y as usize * self.width as usize + x as usize)
    }

    /// Escribe un color. Lo que cae fuera de la imagen se descarta.
    pub fn point(&mut self, x: i32, y: i32, color: Vector3) {
        let Some(index) = self.index(x, y) else { return };

        let pixel = &mut self.pixels[index * 4..index * 4 + 4];
        pixel[0] = to_byte(color.x);
        pixel[1] = to_byte(color.y);
        pixel[2] = to_byte(color.z);
        pixel[3] = 255;
    }

    /// Prueba de profundidad: true si en ese pixel no hay nada mas cerca, y
    /// en ese caso se queda con la nueva profundidad.
    pub fn depth_test(&mut self, x: i32, y: i32, depth: f32) -> bool {
        let Some(index) = self.index(x, y) else { return false };

        if depth < self.depth[index] {
            self.depth[index] = depth;
            true
        } else {
            false
        }
    }

    /// Crea la textura de GPU donde se sube cada cuadro.
    pub fn init_texture(&mut self, window: &mut RaylibHandle, thread: &RaylibThread) {
        let image = Image::gen_image_color(self.width as i32, self.height as i32, Color::BLACK);
        self.texture = window.load_texture_from_image(thread, &image).ok();
    }

    /// Manda el buffer a la ventana actualizando la textura existente.
    pub fn swap_buffers(&mut self, window: &mut RaylibHandle, thread: &RaylibThread) {
        if self.texture.is_none() {
            self.init_texture(window, thread);
        }

        // destructuramos para poder leer pixels mientras mutamos la textura
        let Framebuffer { pixels, texture, .. } = self;
        let Some(texture) = texture.as_mut() else { return };
        let _ = texture.update_texture(pixels);

        let mut renderer = window.begin_drawing(thread);
        renderer.clear_background(Color::BLACK);
        renderer.draw_texture(texture, 0, 0, Color::WHITE);
    }

    /// Guarda el cuadro como PNG.
    pub fn save(&self, path: &str) -> Result<(), String> {
        let mut image =
            Image::gen_image_color(self.width as i32, self.height as i32, Color::BLACK);

        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            let x = (index % self.width as usize) as i32;
            let y = (index / self.width as usize) as i32;
            image.draw_pixel(x, y, Color::new(pixel[0], pixel[1], pixel[2], pixel[3]));
        }

        image.export_image(path);
        if std::path::Path::new(path).exists() {
            return Ok(());
        }

        Err(format!("no se pudo escribir {path}"))
    }
}

/// De 0.0..1.0 a 0..255, recortando lo que se pase.
fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
```

### `src/fragment.rs`

Igual al de la guía del curso.

```rust
// fragment.rs
// Un fragmento: la informacion de un pixel candidato antes de escribirlo en
// el framebuffer.
use raylib::math::{Vector2, Vector3};

pub struct Fragment {
    /// Coordenadas en la imagen: origen arriba a la izquierda, x crece a la
    /// derecha e y hacia abajo.
    pub position: Vector2,
    /// Rojo, verde y azul entre 0.0 y 1.0.
    pub color: Vector3,
    /// Que tan lejos queda. Mas chico = mas cerca del observador.
    pub depth: f32,
}

impl Fragment {
    pub fn new(x: f32, y: f32, color: Vector3, depth: f32) -> Self {
        Self {
            position: Vector2::new(x, y),
            color,
            depth,
        }
    }
}
```

### `src/vertex.rs`

Posición y normal originales, color y las versiones transformadas.

```rust
// vertex.rs
// Un vertice: los datos originales del modelo y los que calcula el vertex
// shader.
use raylib::prelude::*;

#[derive(Debug, Clone)]
pub struct Vertex {
    /// Posicion original, tal como viene del modelo.
    pub position: Vector3,
    pub normal: Vector3,
    /// Coordenadas de textura. Todavia no se usan: quedan para cuando el
    /// modelo lleve textura.
    #[allow(dead_code)]
    pub tex_coords: Vector2,
    /// Rojo, verde y azul entre 0.0 y 1.0.
    pub color: Vector3,
    /// Posicion despues del vertex shader, ya en coordenadas de pantalla.
    pub transformed_position: Vector3,
    /// Normal despues del vertex shader.
    pub transformed_normal: Vector3,
}

impl Vertex {
    pub fn new(position: Vector3, normal: Vector3, tex_coords: Vector2) -> Self {
        Vertex {
            position,
            normal,
            tex_coords,
            color: Vector3::new(1.0, 1.0, 1.0),
            transformed_position: position,
            transformed_normal: normal,
        }
    }

    pub fn new_with_color(position: Vector3, color: Vector3) -> Self {
        Vertex {
            position,
            normal: Vector3::zero(),
            tex_coords: Vector2::new(0.0, 0.0),
            color,
            transformed_position: Vector3::zero(),
            transformed_normal: Vector3::zero(),
        }
    }
}
```

### `src/line.rs`

Bresenham sobre las posiciones transformadas.

```rust
// line.rs
// Bresenham: los fragmentos de una linea entre dos vertices ya transformados.
use crate::fragment::Fragment;
use crate::vertex::Vertex;

pub fn line(a: &Vertex, b: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();

    let start = a.transformed_position;
    let end = b.transformed_position;

    let (mut x, mut y) = (start.x as i32, start.y as i32);
    let (x_end, y_end) = (end.x as i32, end.y as i32);

    let dx = (x_end - x).abs();
    let dy = -(y_end - y).abs();
    let step_x = if x < x_end { 1 } else { -1 };
    let step_y = if y < y_end { 1 } else { -1 };
    let mut error = dx + dy;

    // la profundidad se reparte pareja a lo largo de la linea
    let steps = dx.max(-dy).max(1) as f32;
    let mut step = 0;

    loop {
        let depth = start.z + (end.z - start.z) * (step as f32 / steps);
        fragments.push(Fragment::new(x as f32, y as f32, a.color, depth));

        if x == x_end && y == y_end {
            break;
        }

        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += step_x;
        }
        if doubled <= dx {
            error += dx;
            y += step_y;
        }
        step += 1;
    }

    fragments
}
```

### `src/triangle.rs`

Contorno y los dos rellenos con la regla común.

```rust
// triangle.rs
// Los tres modos de convertir un triangulo en fragmentos: contorno, relleno
// con coordenadas baricentricas y relleno por lineas horizontales.
//
// Los dos rellenos usan la misma regla, asi que generan las mismas
// posiciones:
// - se prueba el centro de cada pixel, (x + 0.5, y + 0.5);
// - un centro que cae justo sobre un borde se dibuja solo si ese borde es
//   izquierdo o superior (regla "top-left"). Asi dos triangulos que
//   comparten una arista no dibujan dos veces el mismo pixel ni dejan un
//   hueco entre ellos.
use raylib::prelude::*;

use crate::fragment::Fragment;
use crate::line::line;
use crate::vertex::Vertex;

/// Por debajo de esta area (en pixeles cuadrados) el triangulo se trata como
/// degenerado: sus tres vertices estan practicamente sobre una misma linea.
const DEGENERATE_AREA: f32 = 1e-6;

/// Contorno: los tres bordes.
pub fn triangle(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();

    fragments.extend(line(v1, v2));
    fragments.extend(line(v2, v3));
    fragments.extend(line(v3, v1));

    fragments
}

/// Relleno con coordenadas baricentricas.
pub fn triangle_barycentric(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    let color = flat_color(v1, v2, v3);

    let a = v1.transformed_position;
    let mut b = v2.transformed_position;
    let mut c = v3.transformed_position;

    // el doble del area con signo; el signo depende del orden de los vertices
    let mut area = edge_function(a, b, c.x, c.y);
    if area.abs() < DEGENERATE_AREA {
        return fragments;
    }
    // con el orden invertido todos los pesos salen negativos: se da vuelta
    // el triangulo para que adentro siempre sea positivo
    if area < 0.0 {
        std::mem::swap(&mut b, &mut c);
        area = -area;
    }

    // el rectangulo mas chico que lo contiene, en pixeles
    let min_x = first_pixel(a.x.min(b.x).min(c.x));
    let max_x = last_pixel(a.x.max(b.x).max(c.x));
    let min_y = first_pixel(a.y.min(b.y).min(c.y));
    let max_y = last_pixel(a.y.max(b.y).max(c.y));

    // que hacer con un centro que cae justo sobre cada borde
    let keep_bc = is_top_left(b, c);
    let keep_ca = is_top_left(c, a);
    let keep_ab = is_top_left(a, b);

    for y in min_y..=max_y {
        let py = y as f32 + 0.5;

        for x in min_x..=max_x {
            let px = x as f32 + 0.5;

            // cada peso es el area del sub-triangulo opuesto a un vertice
            let w_a = edge_function(b, c, px, py);
            let w_b = edge_function(c, a, px, py);
            let w_c = edge_function(a, b, px, py);

            if covers(w_a, keep_bc) && covers(w_b, keep_ca) && covers(w_c, keep_ab) {
                // coordenadas baricentricas: los pesos divididos por el area
                let depth = (w_a * a.z + w_b * b.z + w_c * c.z) / area;
                fragments.push(Fragment::new(x as f32, y as f32, color, depth));
            }
        }
    }

    fragments
}

/// Relleno por lineas horizontales.
pub fn triangle_scanline(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    let color = flat_color(v1, v2, v3);

    let mut points = [
        v1.transformed_position,
        v2.transformed_position,
        v3.transformed_position,
    ];

    // tres puntos alineados no cubren ningun centro: no hay nada que rellenar
    if edge_function(points[0], points[1], points[2].x, points[2].y).abs() < DEGENERATE_AREA {
        return fragments;
    }

    // de arriba hacia abajo (y crece hacia abajo)
    points.sort_by(|p, q| p.y.total_cmp(&q.y));
    let [top, middle, bottom] = points;

    // filas cuyo centro cae en [top.y, bottom.y): la fila de arriba se
    // incluye y la de abajo no, igual que un borde superior y uno inferior
    let first_row = first_pixel(top.y);
    let last_row = first_pixel(bottom.y) - 1;

    for y in first_row..=last_row {
        let py = y as f32 + 0.5;

        // el borde largo va de top a bottom y cruza todas las filas; el otro
        // lado cambia de borde al pasar por el vertice del medio. Ninguno de
        // los dos puede ser horizontal aca: una fila nunca usa un borde que
        // empieza y termina a la misma altura
        let long_side = cross_at(top, bottom, py);
        let short_side = if py < middle.y {
            cross_at(top, middle, py)
        } else {
            cross_at(middle, bottom, py)
        };

        let (left, right) = if long_side.0 <= short_side.0 {
            (long_side, short_side)
        } else {
            (short_side, long_side)
        };

        // pixeles cuyo centro cae en [left, right): el borde izquierdo se
        // incluye y el derecho no
        let first_column = first_pixel(left.0);
        let last_column = first_pixel(right.0) - 1;
        let width = right.0 - left.0;

        for x in first_column..=last_column {
            let px = x as f32 + 0.5;
            let t = if width > 0.0 { (px - left.0) / width } else { 0.0 };
            let depth = left.1 + (right.1 - left.1) * t;
            fragments.push(Fragment::new(x as f32, y as f32, color, depth));
        }
    }

    fragments
}

/// Doble del area con signo del triangulo (a, b, p). Es positiva de un lado
/// del borde a -> b, negativa del otro y cero sobre el borde.
fn edge_function(a: Vector3, b: Vector3, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

/// Un centro sobre el borde (peso cero) cuenta solo si el borde es
/// izquierdo o superior.
fn covers(weight: f32, keep_edge: bool) -> bool {
    weight > 0.0 || (weight == 0.0 && keep_edge)
}

/// Si el borde from -> to es izquierdo o superior en un triangulo cuyo
/// interior da pesos positivos. Izquierdo: el interior queda a su derecha.
/// Superior: es horizontal y el interior queda debajo.
fn is_top_left(from: Vector3, to: Vector3) -> bool {
    to.y < from.y || (to.y == from.y && to.x > from.x)
}

/// Primer pixel cuyo centro queda en `value` o despues.
fn first_pixel(value: f32) -> i32 {
    (value - 0.5).ceil() as i32
}

/// Ultimo pixel cuyo centro queda en `value` o antes.
fn last_pixel(value: f32) -> i32 {
    (value - 0.5).floor() as i32
}

/// Donde cruza la fila `y` el borde from -> to: su x y su profundidad.
fn cross_at(from: Vector3, to: Vector3, y: f32) -> (f32, f32) {
    let t = (y - from.y) / (to.y - from.y);
    (from.x + (to.x - from.x) * t, from.z + (to.z - from.z) * t)
}

/// Un solo color para toda la cara: el promedio de los tres vertices.
fn flat_color(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Vector3 {
    (v1.color + v2.color + v3.color) * (1.0 / 3.0)
}
```

### `src/matrix.rs`

Traslación, escala, rotaciones y la matriz de modelo.

```rust
// matrix.rs
// Matrices de 4x4 para trasladar, escalar y rotar, armadas a mano.
//
// raylib guarda la matriz por columnas: m0, m1, m2, m3 son la primera
// columna. Para no pensar en eso cada vez, aca todo se escribe por filas,
// como en papel, y new_matrix4 se encarga de acomodarlo.
use raylib::prelude::*;

/// Una matriz a partir de sus cuatro filas.
pub fn new_matrix4(rows: [[f32; 4]; 4]) -> Matrix {
    let [r0, r1, r2, r3] = rows;
    Matrix {
        m0: r0[0], m4: r0[1], m8: r0[2], m12: r0[3],
        m1: r1[0], m5: r1[1], m9: r1[2], m13: r1[3],
        m2: r2[0], m6: r2[1], m10: r2[2], m14: r2[3],
        m3: r3[0], m7: r3[1], m11: r3[2], m15: r3[3],
    }
}

/// Las filas de una matriz, el camino inverso de new_matrix4.
fn rows(m: &Matrix) -> [[f32; 4]; 4] {
    [
        [m.m0, m.m4, m.m8, m.m12],
        [m.m1, m.m5, m.m9, m.m13],
        [m.m2, m.m6, m.m10, m.m14],
        [m.m3, m.m7, m.m11, m.m15],
    ]
}

/// Producto a * b como en papel: aplicado a un vector, primero actua b y
/// despues a.
///
/// No se usa el operador * de raylib a proposito: raylib multiplica en el
/// orden contrario (a * b le aplica primero a y despues b), y mezclar las dos
/// convenciones es la forma mas facil de terminar rotando alrededor de la
/// esquina de la pantalla.
pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let (a, b) = (rows(a), rows(b));
    let mut result = [[0.0; 4]; 4];

    for (row, result_row) in result.iter_mut().enumerate() {
        for (column, cell) in result_row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[row][k] * b[k][column]).sum();
        }
    }

    new_matrix4(result)
}

pub fn translation_matrix(offset: Vector3) -> Matrix {
    new_matrix4([
        [1.0, 0.0, 0.0, offset.x],
        [0.0, 1.0, 0.0, offset.y],
        [0.0, 0.0, 1.0, offset.z],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

pub fn scale_matrix(scale: f32) -> Matrix {
    new_matrix4([
        [scale, 0.0, 0.0, 0.0],
        [0.0, scale, 0.0, 0.0],
        [0.0, 0.0, scale, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Rotacion alrededor del eje X, en radianes.
pub fn rotation_x(angle: f32) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    new_matrix4([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cos, -sin, 0.0],
        [0.0, sin, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Rotacion alrededor del eje Y, en radianes.
pub fn rotation_y(angle: f32) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    new_matrix4([
        [cos, 0.0, sin, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sin, 0.0, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Rotacion alrededor del eje Z, en radianes.
pub fn rotation_z(angle: f32) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    new_matrix4([
        [cos, -sin, 0.0, 0.0],
        [sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Matriz de modelo: escala, despues rota (X, luego Y, luego Z) y por
/// ultimo traslada.
///
/// El orden importa. Si se trasladara primero, la rotacion haria girar al
/// modelo alrededor del origen de la pantalla en vez de sobre si mismo.
pub fn create_model_matrix(translation: Vector3, scale: f32, rotation: Vector3) -> Matrix {
    let rotation_matrix = multiply(
        &rotation_z(rotation.z),
        &multiply(&rotation_y(rotation.y), &rotation_x(rotation.x)),
    );

    multiply(
        &translation_matrix(translation),
        &multiply(&rotation_matrix, &scale_matrix(scale)),
    )
}
```

### `src/shaders.rs`

Vertex shader (posición, normal y luces) y fragment shader.

```rust
// shaders.rs
// Las dos etapas programables del pipeline: el vertex shader, que prepara
// cada vertice, y el fragment shader, que decide el color de cada fragmento.
use raylib::prelude::*;

use crate::color::Rgb;
use crate::fragment::Fragment;
use crate::shading::{self, SurfacePoint};
use crate::vertex::Vertex;
use crate::Uniforms;

/// Hacia donde esta el observador. No hay camara todavia: se mira la
/// pantalla de frente, y como x va a la derecha e y hacia abajo, el eje z
/// apunta hacia adentro de la pantalla. El observador queda en -z.
const TO_VIEWER: Vector3 = Vector3::new(0.0, 0.0, -1.0);

fn multiply_matrix_vector4(matrix: &Matrix, vector: &Vector4) -> Vector4 {
    Vector4::new(
        matrix.m0 * vector.x + matrix.m4 * vector.y + matrix.m8 * vector.z + matrix.m12 * vector.w,
        matrix.m1 * vector.x + matrix.m5 * vector.y + matrix.m9 * vector.z + matrix.m13 * vector.w,
        matrix.m2 * vector.x + matrix.m6 * vector.y + matrix.m10 * vector.z + matrix.m14 * vector.w,
        matrix.m3 * vector.x + matrix.m7 * vector.y + matrix.m11 * vector.z + matrix.m15 * vector.w,
    )
}

pub fn vertex_shader(vertex: &Vertex, uniforms: &Uniforms) -> Vertex {
    // w = 1: es un punto, asi que la traslacion lo mueve
    let position = Vector4::new(vertex.position.x, vertex.position.y, vertex.position.z, 1.0);
    let transformed = multiply_matrix_vector4(&uniforms.model_matrix, &position);

    // w = 0: es una direccion, asi que la traslacion no la mueve. Con escala
    // pareja alcanza con volver a normalizarla; con una escala distinta en
    // cada eje habria que usar la inversa transpuesta de la matriz
    let normal = Vector4::new(vertex.normal.x, vertex.normal.y, vertex.normal.z, 0.0);
    let normal = multiply_matrix_vector4(&uniforms.model_matrix, &normal);
    let normal = Vector3::new(normal.x, normal.y, normal.z);
    let length = normal.length();

    let mut result = vertex.clone();
    result.transformed_position = Vector3::new(transformed.x, transformed.y, transformed.z);
    result.transformed_normal = if length > 0.0 { normal / length } else { normal };

    if uniforms.lighting {
        result.color = light_vertex(&result, uniforms);
    }

    result
}

/// Color de un vertice con las luces de la escena (iluminacion por vertice).
fn light_vertex(vertex: &Vertex, uniforms: &Uniforms) -> Vector3 {
    // la normal del lado que se ve: asi una cara vista de espaldas no queda
    // negra
    let normal = if vertex.transformed_normal.dot(TO_VIEWER) < 0.0 {
        -vertex.transformed_normal
    } else {
        vertex.transformed_normal
    };

    let surface = SurfacePoint {
        point: vertex.transformed_position,
        normal,
        view_dir: TO_VIEWER,
    };
    let base = Rgb::from_color(uniforms.material.diffuse) * Rgb::from_vector(vertex.color);

    shading::shade(
        &uniforms.lights,
        uniforms.ambient,
        &uniforms.material,
        base,
        &surface,
        shading::no_shadows,
    )
    .to_vector()
}

pub fn fragment_shader(fragment: &Fragment) -> Vector3 {
    fragment.color
}
```

### `src/primitive.rs`

Igual al de la guía del curso.

```rust
// primitive.rs
// Ensamblaje de primitivas: de una secuencia de vertices a triangulos.
use crate::vertex::Vertex;

pub fn primitive_assembly(transformed_vertices: &[Vertex]) -> Vec<[Vertex; 3]> {
    assert!(
        transformed_vertices.len() % 3 == 0,
        "La cantidad de vertices debe ser multiplo de 3",
    );

    transformed_vertices
        .chunks_exact(3)
        .map(|group| [group[0].clone(), group[1].clone(), group[2].clone()])
        .collect()
}
```

### `src/rasterizer.rs`

`TriangleMode` y `rasterize`.

```rust
// rasterizer.rs
// Rasterizacion: de triangulos a fragmentos, con el algoritmo que pida el modo.
use crate::fragment::Fragment;
use crate::triangle::{triangle, triangle_barycentric, triangle_scanline};
use crate::vertex::Vertex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangleMode {
    /// Solo los tres bordes.
    Wireframe,
    /// Relleno probando cada pixel del rectangulo que lo contiene.
    FilledBarycentric,
    /// Relleno fila por fila.
    FilledScanline,
}

impl TriangleMode {
    pub fn name(self) -> &'static str {
        match self {
            TriangleMode::Wireframe => "contorno",
            TriangleMode::FilledBarycentric => "relleno baricentrico",
            TriangleMode::FilledScanline => "relleno por lineas",
        }
    }

    pub fn is_filled(self) -> bool {
        self != TriangleMode::Wireframe
    }
}

pub fn rasterize(triangles: &[[Vertex; 3]], mode: TriangleMode) -> Vec<Fragment> {
    let mut fragments = Vec::new();

    for tri in triangles {
        let generated = match mode {
            TriangleMode::Wireframe => triangle(&tri[0], &tri[1], &tri[2]),
            TriangleMode::FilledBarycentric => triangle_barycentric(&tri[0], &tri[1], &tri[2]),
            TriangleMode::FilledScanline => triangle_scanline(&tri[0], &tri[1], &tri[2]),
        };
        fragments.extend(generated);
    }

    fragments
}
```

### `src/obj.rs`

Lector de `.obj`.

```rust
// obj.rs
// Lector de modelos .obj: posiciones, normales, coordenadas de textura y
// caras. Lo demas (materiales, grupos, suavizado) se ignora.
use raylib::prelude::*;

use crate::vertex::Vertex;

/// Una esquina de una cara: que posicion, que coordenada de textura y que
/// normal usa, como indices dentro de las listas del modelo.
#[derive(Clone, Copy)]
struct Corner {
    position: usize,
    tex_coords: Option<usize>,
    normal: Option<usize>,
}

pub struct Obj {
    positions: Vec<Vector3>,
    normals: Vec<Vector3>,
    tex_coords: Vec<Vector2>,
    /// Cada cara ya partida en triangulos.
    triangles: Vec<[Corner; 3]>,
}

impl Obj {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;

        let mut obj = Obj {
            positions: Vec::new(),
            normals: Vec::new(),
            tex_coords: Vec::new(),
            triangles: Vec::new(),
        };

        for (index, line) in text.lines().enumerate() {
            let number = index + 1;
            let mut parts = line.split_whitespace();

            match parts.next() {
                Some("v") => {
                    let [x, y, z] = parse_numbers(&mut parts, number)?;
                    obj.positions.push(Vector3::new(x, y, z));
                }
                Some("vn") => {
                    let [x, y, z] = parse_numbers(&mut parts, number)?;
                    obj.normals.push(Vector3::new(x, y, z));
                }
                Some("vt") => {
                    let [u, v] = parse_numbers(&mut parts, number)?;
                    obj.tex_coords.push(Vector2::new(u, v));
                }
                Some("f") => {
                    let corners = parts
                        .map(|text| obj.parse_corner(text, number))
                        .collect::<Result<Vec<_>, _>>()?;

                    if corners.len() < 3 {
                        return Err(format!("linea {number}: una cara necesita 3 vertices"));
                    }

                    // una cara de n vertices se parte en abanico: n - 2
                    // triangulos que comparten el primer vertice
                    for i in 1..corners.len() - 1 {
                        obj.triangles.push([corners[0], corners[i], corners[i + 1]]);
                    }
                }
                _ => {}
            }
        }

        Ok(obj)
    }

    /// Los vertices de todos los triangulos, de tres en tres. Un vertice que
    /// comparten varias caras aparece una vez por cada una.
    pub fn get_vertex_array(&self) -> Vec<Vertex> {
        let mut vertices = Vec::with_capacity(self.triangles.len() * 3);

        for triangle in &self.triangles {
            for corner in triangle {
                let position = self.positions[corner.position];
                let normal = corner.normal.map_or(Vector3::zero(), |i| self.normals[i]);
                let tex_coords = corner
                    .tex_coords
                    .map_or(Vector2::new(0.0, 0.0), |i| self.tex_coords[i]);

                vertices.push(Vertex::new(position, normal, tex_coords));
            }
        }

        vertices
    }

    /// "12/5/3", "12//3", "12/5" o "12".
    fn parse_corner(&self, text: &str, number: usize) -> Result<Corner, String> {
        let mut fields = text.split('/');

        let position = resolve(fields.next(), self.positions.len(), number)?
            .ok_or_else(|| format!("linea {number}: falta la posicion en '{text}'"))?;
        let tex_coords = resolve(fields.next(), self.tex_coords.len(), number)?;
        let normal = resolve(fields.next(), self.normals.len(), number)?;

        Ok(Corner { position, tex_coords, normal })
    }
}

/// Pasa un indice del archivo a uno de Rust. En el .obj empiezan en 1, y los
/// negativos cuentan desde el ultimo que se leyo (-1 es el ultimo).
fn resolve(field: Option<&str>, count: usize, number: usize) -> Result<Option<usize>, String> {
    let Some(field) = field.filter(|field| !field.is_empty()) else {
        return Ok(None);
    };

    let index: i64 = field
        .parse()
        .map_err(|_| format!("linea {number}: indice invalido '{field}'"))?;
    let resolved = if index > 0 { index - 1 } else { count as i64 + index };

    if index == 0 || resolved < 0 || resolved >= count as i64 {
        return Err(format!("linea {number}: el indice {index} no existe"));
    }

    Ok(Some(resolved as usize))
}

/// Lee los siguientes N numeros de la linea.
fn parse_numbers<const N: usize>(
    parts: &mut std::str::SplitWhitespace,
    number: usize,
) -> Result<[f32; N], String> {
    let mut values = [0.0; N];

    for value in &mut values {
        let text = parts
            .next()
            .ok_or_else(|| format!("linea {number}: faltan valores"))?;
        *value = text
            .parse()
            .map_err(|_| format!("linea {number}: numero invalido '{text}'"))?;
    }

    Ok(values)
}
```

### `src/benchmark.rs`

`--benchmark-triangles` y `--compare-triangles`.

```rust
// benchmark.rs
// Medicion y comparacion de los algoritmos de rasterizacion, sin ventana.
//
//   cargo run --release -- --benchmark-triangles   tiempos por algoritmo
//   cargo run --release -- --compare-triangles     posiciones de cada relleno
use raylib::prelude::*;
use std::collections::HashSet;
use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::primitive::primitive_assembly;
use crate::rasterizer::{rasterize, TriangleMode};
use crate::shaders::vertex_shader;
use crate::triangle::{triangle_barycentric, triangle_scanline};
use crate::vertex::Vertex;
use crate::Uniforms;

/// Pasadas por medicion, y cuantas mediciones se toman para sacar la mediana.
const RUNS: u32 = 20;
const REPEATS: usize = 7;

const MODES: [TriangleMode; 3] = [
    TriangleMode::Wireframe,
    TriangleMode::FilledBarycentric,
    TriangleMode::FilledScanline,
];

/// Mide solo la rasterizacion. Las dos primeras etapas se hacen una vez
/// afuera: no son lo que se esta comparando.
pub fn run(vertex_array: &[Vertex], uniforms: &Uniforms) {
    let transformed: Vec<Vertex> = vertex_array
        .iter()
        .map(|vertex| vertex_shader(vertex, uniforms))
        .collect();
    let triangles = primitive_assembly(&transformed);

    println!("{RUNS} pasadas por medicion, mediana de {REPEATS} mediciones\n");

    for mode in MODES {
        // cuantos fragmentos genera: un algoritmo que se salta parte del
        // triangulo puede parecer mas rapido solo por hacer menos trabajo
        let fragments = rasterize(&triangles, mode).len();

        let mut samples: Vec<Duration> = (0..REPEATS)
            .map(|_| {
                let start = Instant::now();
                for _ in 0..RUNS {
                    let fragments = rasterize(black_box(&triangles), mode);
                    black_box(fragments);
                }
                start.elapsed() / RUNS
            })
            .collect();
        samples.sort();

        println!(
            "{:<22} {:>10} fragmentos   mediana {:>10.2?} por pasada",
            mode.name(),
            fragments,
            samples[REPEATS / 2],
        );
    }
}

/// Casos de prueba de la guia, en coordenadas de pantalla.
const CASES: [(&str, [(f32, f32); 3]); 8] = [
    ("pequeno", [(10.0, 10.0), (14.0, 11.0), (11.0, 15.0)]),
    ("grande", [(50.0, 40.0), (750.0, 120.0), (300.0, 560.0)]),
    ("borde horizontal", [(100.0, 100.0), (300.0, 100.0), (180.0, 250.0)]),
    ("borde vertical", [(100.0, 100.0), (100.0, 300.0), (260.0, 220.0)]),
    ("parcialmente fuera", [(-80.0, 50.0), (200.0, -60.0), (150.0, 250.0)]),
    ("colineales", [(10.0, 10.0), (100.0, 100.0), (200.0, 200.0)]),
    ("bordes sobre centros", [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]),
    ("vertices fraccionarios", [(10.3, 20.7), (90.6, 35.2), (40.1, 80.9)]),
];

/// Compara las posiciones que genera cada relleno, en los dos ordenes.
pub fn compare_algorithms() {
    println!(
        "{:<24} {:>11} {:>11} {:>10} {:>14}",
        "caso", "baricentr.", "lineas", "distintas", "orden inverso"
    );

    for (name, points) in CASES {
        let [a, b, c] = points.map(screen_vertex);

        let barycentric = positions(triangle_barycentric(&a, &b, &c));
        let scanline = positions(triangle_scanline(&a, &b, &c));
        let reversed_ok = barycentric == positions(triangle_barycentric(&c, &b, &a))
            && scanline == positions(triangle_scanline(&c, &b, &a));

        println!(
            "{:<24} {:>11} {:>11} {:>10} {:>14}",
            name,
            barycentric.len(),
            scanline.len(),
            barycentric.symmetric_difference(&scanline).count(),
            if reversed_ok { "igual" } else { "DISTINTO" },
        );
    }

    // dos triangulos que comparten la diagonal de un cuadrado de 100 x 100:
    // con la regla top-left juntos cubren 10000 pixeles sin repetir ninguno
    let corners = [(100.0, 100.0), (200.0, 100.0), (200.0, 200.0), (100.0, 200.0)].map(screen_vertex);
    let first = [&corners[0], &corners[1], &corners[3]];
    let second = [&corners[1], &corners[2], &corners[3]];

    println!("\ncuadrado partido en dos triangulos (deberia dar 10000 y 0):");
    for (label, fill) in [
        ("baricentrico", triangle_barycentric as fn(&Vertex, &Vertex, &Vertex) -> _),
        ("por lineas", triangle_scanline),
    ] {
        let one = positions(fill(first[0], first[1], first[2]));
        let two = positions(fill(second[0], second[1], second[2]));
        println!(
            "  {:<14} pixeles: {:>6}   repetidos: {}",
            label,
            one.union(&two).count(),
            one.intersection(&two).count(),
        );
    }
}

/// Un vertice blanco ya en coordenadas de pantalla.
fn screen_vertex((x, y): (f32, f32)) -> Vertex {
    let position = Vector3::new(x, y, 0.0);
    let mut vertex = Vertex::new_with_color(position, Vector3::new(1.0, 1.0, 1.0));
    vertex.transformed_position = position;
    vertex
}

fn positions(fragments: Vec<crate::fragment::Fragment>) -> HashSet<(i32, i32)> {
    fragments
        .iter()
        .map(|fragment| (fragment.position.x as i32, fragment.position.y as i32))
        .collect()
}
```
