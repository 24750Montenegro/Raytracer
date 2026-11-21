// pool.rs
// Hilos de trabajo que se crean una sola vez y viven todo el programa.
//
// Antes cada cuadro creaba y cerraba un hilo por nucleo. Con la ventana
// abierta, el driver de video reserva memoria para cada hilo que nace en el
// proceso y no la devuelve cuando el hilo termina: la memoria crecia unos
// 10 MB por segundo. Reusando siempre los mismos hilos eso pasa una sola vez.
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

pub struct Pool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

struct Shared {
    state: Mutex<State>,
    /// Despierta a los hilos cuando hay trabajo nuevo o hay que cerrar.
    work_ready: Condvar,
    /// Despierta a `run` cuando termina el ultimo hilo.
    work_done: Condvar,
}

struct State {
    /// El trabajo actual. Se guarda como 'static, pero solo es valido
    /// mientras dura `run`, que no vuelve hasta que todos terminan con el.
    task: Option<&'static (dyn Fn(usize) + Sync)>,
    /// Numero del trabajo actual, para que ningun hilo lo haga dos veces.
    generation: u64,
    /// Cuantos hilos faltan por terminar el trabajo actual.
    pending: usize,
    /// Si algun hilo entro en panico con el trabajo actual.
    panicked: bool,
    shutdown: bool,
}

impl Pool {
    pub fn new(threads: usize) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                task: None,
                generation: 0,
                pending: 0,
                panicked: false,
                shutdown: false,
            }),
            work_ready: Condvar::new(),
            work_done: Condvar::new(),
        });
        let workers = (0..threads.max(1))
            .map(|index| {
                let shared = Arc::clone(&shared);
                thread::spawn(move || work(index, &shared))
            })
            .collect();
        Pool { shared, workers }
    }

    pub fn threads(&self) -> usize {
        self.workers.len()
    }

    /// Corre `task(i)` en cada hilo `i`, de 0 a `threads() - 1`, y espera a
    /// que terminen todos. `task` puede usar datos prestados del que llama,
    /// igual que con `std::thread::scope`.
    pub fn run(&self, task: &(dyn Fn(usize) + Sync)) {
        // SAFETY: borrar la vida de la referencia es seguro porque esta
        // funcion no vuelve, ni siquiera por panico, hasta que `pending`
        // llega a cero, y despues de eso ningun hilo la vuelve a tocar.
        let task: &'static (dyn Fn(usize) + Sync) = unsafe { std::mem::transmute(task) };

        let mut state = self.shared.state.lock().unwrap();
        state.task = Some(task);
        state.generation += 1;
        state.pending = self.workers.len();
        state.panicked = false;
        self.shared.work_ready.notify_all();
        while state.pending > 0 {
            state = self.shared.work_done.wait(state).unwrap();
        }
        state.task = None;
        let panicked = state.panicked;
        drop(state);

        if panicked {
            panic!("un hilo de trabajo entro en panico");
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap().shutdown = true;
        self.shared.work_ready.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn work(index: usize, shared: &Shared) {
    let mut finished = 0;
    loop {
        let task = {
            let mut state = shared.state.lock().unwrap();
            while state.generation == finished && !state.shutdown {
                state = shared.work_ready.wait(state).unwrap();
            }
            if state.shutdown {
                return;
            }
            finished = state.generation;
            state.task.expect("un trabajo nuevo siempre trae su funcion")
        };

        // un panico no puede saltarse el aviso de abajo: `run` se quedaria
        // esperando para siempre
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| task(index)));

        let mut state = shared.state.lock().unwrap();
        state.panicked |= outcome.is_err();
        state.pending -= 1;
        if state.pending == 0 {
            shared.work_done.notify_one();
        }
    }
}
