use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use gpui_kit::Global;
use missal_core::RepositoryError;
use missal_store::Store;

pub type StoreResult<T> = Result<T, RepositoryError>;

struct Slot<T> {
    value: Option<T>,
    waker: Option<Waker>,
    closed: bool,
}

struct Completer<T>(Arc<Mutex<Slot<T>>>);

pub struct Reply<T>(Arc<Mutex<Slot<T>>>);

fn reply<T>() -> (Completer<T>, Reply<T>) {
    let slot = Arc::new(Mutex::new(Slot {
        value: None,
        waker: None,
        closed: false,
    }));
    (Completer(slot.clone()), Reply(slot))
}

impl<T> Completer<T> {
    fn complete(self, value: T) {
        let waker = {
            let mut slot = self.0.lock().unwrap_or_else(|error| error.into_inner());
            slot.value = Some(value);
            slot.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Drop for Completer<T> {
    fn drop(&mut self) {
        let waker = {
            let mut slot = self.0.lock().unwrap_or_else(|error| error.into_inner());
            slot.closed = true;
            slot.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T> Future for Reply<StoreResult<T>> {
    type Output = StoreResult<T>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut slot = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(value) = slot.value.take() {
            return Poll::Ready(value);
        }
        if slot.closed {
            return Poll::Ready(Err(unavailable()));
        }
        slot.waker = Some(context.waker().clone());
        Poll::Pending
    }
}

fn unavailable() -> RepositoryError {
    RepositoryError::storage("database", "The database is not available")
}

type Opened = StoreResult<Store>;
type Job = Box<dyn FnOnce(&mut Opened) + Send>;

enum Mode {
    Thread(Mutex<Sender<Job>>),
    Inline(Mutex<Opened>),
}

struct Inner {
    mode: Mode,
    pending: AtomicUsize,
}

#[derive(Clone)]
pub struct Db(Arc<Inner>);

impl Global for Db {}

impl Db {
    pub fn spawn(path: PathBuf) -> Db {
        let (sender, receiver) = channel::<Job>();
        let started = std::thread::Builder::new()
            .name("missal-database".into())
            .spawn(move || {
                let mut opened = Store::open(&path);
                while let Ok(job) = receiver.recv() {
                    job(&mut opened);
                }
            });
        if let Err(error) = started {
            eprintln!("missal: could not start the database thread: {error}");
        }
        Db(Arc::new(Inner {
            mode: Mode::Thread(Mutex::new(sender)),
            pending: AtomicUsize::new(0),
        }))
    }

    pub fn inline(store: Store) -> Db {
        Db(Arc::new(Inner {
            mode: Mode::Inline(Mutex::new(Ok(store))),
            pending: AtomicUsize::new(0),
        }))
    }

    pub fn pending(&self) -> usize {
        self.0.pending.load(Ordering::SeqCst)
    }

    pub fn run<T, F>(&self, operation: F) -> Reply<StoreResult<T>>
    where
        T: Send + 'static,
        F: FnOnce(&mut Store) -> StoreResult<T> + Send + 'static,
    {
        let (completer, reply) = reply();
        let inner = self.0.clone();
        inner.pending.fetch_add(1, Ordering::SeqCst);
        let finish = {
            let inner = inner.clone();
            move |opened: &mut Opened, completer: Completer<StoreResult<T>>| {
                let result = match opened {
                    Ok(store) => operation(store),
                    Err(error) => Err(error.clone()),
                };
                inner.pending.fetch_sub(1, Ordering::SeqCst);
                completer.complete(result);
            }
        };
        match &self.0.mode {
            Mode::Inline(store) => {
                let mut store = store.lock().unwrap_or_else(|error| error.into_inner());
                finish(&mut store, completer);
            }
            Mode::Thread(sender) => {
                let sender = sender.lock().unwrap_or_else(|error| error.into_inner());
                let job: Job = Box::new(move |opened| finish(opened, completer));
                if sender.send(job).is_err() {
                    inner.pending.fetch_sub(1, Ordering::SeqCst);
                }
            }
        }
        reply
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::noop().clone();
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn jobs_run_on_the_database_thread_and_report_failures() {
        let directory = tempfile::tempdir().unwrap();
        let db = Db::spawn(directory.path().join("missal.sqlite"));
        let caller = std::thread::current().id();
        let ran_elsewhere = block_on(db.run(move |_| Ok(std::thread::current().id() != caller)));
        assert_eq!(ran_elsewhere, Ok(true));
        let count = block_on(db.run(|store| store.firs().map(|firs| firs.len())));
        assert_eq!(count, Ok(0));
        assert_eq!(db.pending(), 0);
        let failed = block_on(db.run(|_| -> StoreResult<()> {
            Err(RepositoryError::storage("test", "boom"))
        }));
        assert_eq!(failed.unwrap_err().message(), "boom");
    }

    #[test]
    fn an_unopenable_database_fails_every_job() {
        let directory = tempfile::tempdir().unwrap();
        let db = Db::spawn(directory.path().join("missing/dir/missal.sqlite"));
        let result = block_on(db.run(|store| store.firs().map(|firs| firs.len())));
        assert!(result.is_err());
    }
}
