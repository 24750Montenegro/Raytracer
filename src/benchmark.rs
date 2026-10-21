// Medicion y comparacion de los algoritmos de rasterizacion, sin ventana.
//
//   cargo run --release -- --benchmark-triangles   tiempos por algoritmo
//   cargo run --release -- --compare-triangles     posiciones de cada relleno
use raylib::prelude::*;
use std::collections::HashSet;
use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::fragment::Fragment;
use crate::primitive::primitive_assembly;
use crate::rasterizer::{rasterize, TriangleMode};
use crate::shaders::vertex_shader;
use crate::triangle::{triangle_barycentric, triangle_scanline};
use crate::vertex::Vertex;
use crate::Uniforms;

/// pasadas por medicion, y cuantas mediciones se toman para sacar la mediana
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

/// casos de prueba de la guia, en coordenadas de pantalla
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

/// un vertice blanco ya en coordenadas de pantalla
fn screen_vertex((x, y): (f32, f32)) -> Vertex {
    let position = Vector3::new(x, y, 0.0);
    let mut vertex = Vertex::new_with_color(position, Vector3::new(1.0, 1.0, 1.0));
    vertex.transformed_position = position;
    vertex
}

fn positions(fragments: Vec<Fragment>) -> HashSet<(i32, i32)> {
    fragments
        .iter()
        .map(|fragment| (fragment.position.x as i32, fragment.position.y as i32))
        .collect()
}
