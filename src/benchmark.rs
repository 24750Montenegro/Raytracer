// Medicion y comparacion de los algoritmos de rasterizacion, sin ventana.
//
//   cargo run --release -- --benchmark-triangles   tiempos por algoritmo
//   cargo run --release -- --compare-triangles     posiciones de cada relleno
//   cargo run --release -- --profile-stages        tiempo de cada etapa
use raylib::prelude::*;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::hint::black_box;
use std::time::{Duration, Instant};

use crate::fragment::Fragment;
use crate::framebuffer::Framebuffer;
use crate::primitive::primitive_assembly;
use crate::rasterizer::{rasterize, TriangleMode};
use crate::shaders::vertex_shader;
use crate::triangle::{triangle_barycentric, triangle_scanline};
use crate::vertex::Vertex;
use crate::{render, Uniforms};

/// pasadas por medicion, y cuantas mediciones se toman para sacar la mediana
const RUNS: u32 = 20;
const REPEATS: usize = 7;

/// cuadros completos que se dibujan para sacar la mediana de cada etapa
const FRAMES: usize = 7;

/// triangulos al azar por cada tipo de coordenada
const RANDOM_TRIANGLES: usize = 20_000;

const MODES: [TriangleMode; 3] = [
    TriangleMode::Wireframe,
    TriangleMode::FilledBarycentric,
    TriangleMode::FilledScanline,
];

const STAGES: [&str; 4] = [
    "vertex shader",
    "ensamblaje",
    "rasterizacion",
    "fragmentos (z-buffer + shader)",
];

/// Mide solo la rasterizacion. Las dos primeras etapas se hacen una vez
/// afuera: no son lo que se esta comparando.
pub fn run(vertex_array: &[Vertex], uniforms: &Uniforms) {
    let triangles = assemble(vertex_array, uniforms);

    println!("{RUNS} pasadas por medicion, mediana de {REPEATS} mediciones\n");

    for mode in MODES {
        // cuantos fragmentos genera: un algoritmo que se salta parte del
        // triangulo puede parecer mas rapido solo por hacer menos trabajo
        let fragments = rasterize(&triangles, mode).len();

        let samples: Vec<Duration> = (0..REPEATS)
            .map(|_| {
                let start = Instant::now();
                for _ in 0..RUNS {
                    let fragments = rasterize(black_box(&triangles), mode);
                    black_box(fragments);
                }
                start.elapsed() / RUNS
            })
            .collect();

        println!(
            "{:<22} {:>10} fragmentos   mediana {:>10.2?} por pasada",
            mode.name(),
            fragments,
            median(samples),
        );
    }
}

/// Mide cada etapa del pipeline en cuadros completos, dibujados igual que en
/// la ventana pero sin subir la textura ni esperar entre cuadros.
pub fn profile_stages(
    framebuffer: &mut Framebuffer,
    vertex_array: &[Vertex],
    uniforms: &mut Uniforms,
    views: &[(&str, Matrix)],
) {
    println!("mediana de {FRAMES} cuadros\n");

    for (label, matrix) in views {
        uniforms.model_matrix = *matrix;

        // por modo: la mediana de cada etapa, la del total y los fragmentos
        let mut results = Vec::new();
        for mode in MODES {
            uniforms.lighting = mode.is_filled();

            let mut stages: [Vec<Duration>; 4] = Default::default();
            let mut totals = Vec::new();
            let mut fragments = 0;
            for _ in 0..FRAMES {
                framebuffer.clear();
                let stats = render(framebuffer, uniforms, vertex_array, mode);
                for (samples, time) in stages.iter_mut().zip(stats.stages) {
                    samples.push(time);
                }
                totals.push(stats.stages.iter().sum());
                fragments = stats.fragments;
            }
            results.push((stages.map(median), median(totals), fragments));
        }

        let ms = |time: Duration| format!("{:.0} ms", time.as_secs_f64() * 1000.0);
        println!("nave {label}");
        print_row("etapa", MODES.map(|mode| mode.name().to_string()));
        for (stage, name) in STAGES.iter().enumerate() {
            print_row(name, results.iter().map(|(stages, _, _)| ms(stages[stage])));
        }
        print_row("total", results.iter().map(|(_, total, _)| ms(*total)));
        print_row(
            "fragmentos generados",
            results.iter().map(|(_, _, fragments)| fragments.to_string()),
        );
        println!();
    }
}

/// casos de prueba de la guia, en coordenadas de pantalla
const CASES: [(&str, [(f32, f32); 3]); 9] = [
    ("pequeno", [(10.0, 10.0), (14.0, 11.0), (11.0, 15.0)]),
    ("grande", [(50.0, 40.0), (750.0, 120.0), (300.0, 560.0)]),
    ("borde horizontal", [(100.0, 100.0), (300.0, 100.0), (180.0, 250.0)]),
    ("borde vertical", [(100.0, 100.0), (100.0, 300.0), (260.0, 220.0)]),
    ("parcialmente fuera", [(-80.0, 50.0), (200.0, -60.0), (150.0, 250.0)]),
    ("colineales", [(10.0, 10.0), (100.0, 100.0), (200.0, 200.0)]),
    ("bordes sobre centros", [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]),
    // el borde x + y = 12 pasa justo por centros como (4.5, 7.5)
    ("diagonal sobre centros", [(6.0, 9.0), (11.0, 1.0), (0.0, 12.0)]),
    ("vertices fraccionarios", [(10.3, 20.7), (90.6, 35.2), (40.1, 80.9)]),
];

/// Compara las posiciones que genera cada relleno: los casos de la guia en
/// los dos ordenes, triangulos al azar y la nave completa.
pub fn compare_algorithms(vertex_array: &[Vertex], uniforms: &mut Uniforms, views: &[(&str, Matrix)]) {
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

    // triangulos al azar. Con vertices enteros o de medio pixel muchos
    // centros caen justo sobre un borde, que es donde se nota si los dos
    // aplican la misma regla
    println!("\n{RANDOM_TRIANGLES} triangulos al azar por tipo de coordenada:");
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let kinds: [(&str, fn(&mut Random) -> f32); 3] = [
        ("enteras", |random| random.below(101) as f32 - 20.0),
        ("medio pixel", |random| random.below(101) as f32 - 19.5),
        ("reales", |random| random.below(200_000) as f32 / 1000.0 - 50.0),
    ];
    for (label, coordinate) in kinds {
        let (mut triangles, mut different) = (0, 0);
        for _ in 0..RANDOM_TRIANGLES {
            let [a, b, c] =
                [(); 3].map(|_| screen_vertex((coordinate(&mut random), coordinate(&mut random))));
            let barycentric = positions(triangle_barycentric(&a, &b, &c));
            let scanline = positions(triangle_scanline(&a, &b, &c));
            let count = barycentric.symmetric_difference(&scanline).count();
            if count > 0 {
                triangles += 1;
                different += count;
            }
        }
        println!(
            "  {:<12} triangulos con diferencias: {:>4}   posiciones distintas: {:>4}",
            label, triangles, different,
        );
    }

    // la nave completa. Se compara la lista entera, con repetidas, porque
    // varios triangulos pueden cubrir el mismo pixel
    println!("\nnave completa:");
    uniforms.lighting = false;
    for (label, matrix) in views {
        uniforms.model_matrix = *matrix;
        let triangles = assemble(vertex_array, uniforms);
        let barycentric = sorted_positions(rasterize(&triangles, TriangleMode::FilledBarycentric));
        let scanline = sorted_positions(rasterize(&triangles, TriangleMode::FilledScanline));
        println!(
            "  {:<4} baricentrico {:>9}   por lineas {:>9}   posiciones distintas: {}",
            label,
            barycentric.len(),
            scanline.len(),
            multiset_difference(&barycentric, &scanline),
        );
    }
}

/// las dos primeras etapas del pipeline: vertex shader y ensamblaje
fn assemble(vertex_array: &[Vertex], uniforms: &Uniforms) -> Vec<[Vertex; 3]> {
    let transformed: Vec<Vertex> = vertex_array
        .iter()
        .map(|vertex| vertex_shader(vertex, uniforms))
        .collect();
    primitive_assembly(&transformed)
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn print_row(name: &str, cells: impl IntoIterator<Item = String>) {
    print!("{name:<32}");
    for cell in cells {
        print!("{cell:>22}");
    }
    println!();
}

/// generador xorshift: da siempre la misma secuencia, asi la comparacion se
/// puede repetir
struct Random(u64);

impl Random {
    /// un entero en [0, limit)
    fn below(&mut self, limit: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % limit
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

fn sorted_positions(fragments: Vec<Fragment>) -> Vec<(i32, i32)> {
    let mut positions: Vec<(i32, i32)> = fragments
        .iter()
        .map(|fragment| (fragment.position.x as i32, fragment.position.y as i32))
        .collect();
    positions.sort_unstable();
    positions
}

/// cuantas posiciones estan en una lista ordenada y no en la otra, contando
/// las repetidas
fn multiset_difference(a: &[(i32, i32)], b: &[(i32, i32)]) -> usize {
    let (mut i, mut j, mut count) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Less => {
                count += 1;
                i += 1;
            }
            Ordering::Greater => {
                count += 1;
                j += 1;
            }
            Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    count + (a.len() - i) + (b.len() - j)
}
