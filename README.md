# Rasterizador de modelos .obj

Un rasterizador por software, escrito en Rust, que carga un modelo `.obj`
(`assets/models/nave-juanmontenegro-24750.obj`) y lo dibuja pasando por un
pipeline propio:

```
vértices → vertex shader → ensamblaje de triángulos → rasterización → fragment shader → framebuffer
```

La única dependencia es `raylib`, que se usa para abrir la ventana, subir el
cuadro a la pantalla y prestar sus tipos de vector y matriz.

> El trazador de rayos anterior (las islas de bloques) sigue en la rama
> `main`. De él se conservó la lógica de luces y sombras (`light.rs`,
> `shading.rs`, `material.rs`, `color.rs`).

La guía para construir el pipeline paso a paso está en
[`docs/guia-rasterizador.md`](docs/guia-rasterizador.md).

## Cómo ejecutarlo

```sh
cargo run --release
```

Los controles y los argumentos de abajo existen una vez completada la guía.

## Controles

| Tecla | Qué hace |
|---|---|
| **Tab** | cambia entre contorno y relleno |
| **F** | cambia el algoritmo de relleno (baricéntrico / líneas horizontales) |
| **Flechas** | trasladar |
| **W/S**, **A/D**, **Q/E** | rotar en X, Y y Z |
| **Z** / **X** | achicar / agrandar |
| **R** | volver a la posición inicial |
| **P** | guardar una captura del modo actual |

## Laboratorio: rellenar triángulos

### Capturas

<!-- Pendiente: una imagen de cada modo (tecla P). -->

### Mediciones

```sh
cargo run --release -- --benchmark-triangles
cargo run --release -- --compare-triangles
```

<!-- Pendiente: tabla con fragmentos y mediana de cada algoritmo. -->

### Perfil

<!-- Pendiente: flamegraph.svg o captura del perfilador. -->

### Diferencias encontradas

<!-- Pendiente: explicación breve. -->
