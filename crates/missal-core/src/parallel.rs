use std::num::NonZeroUsize;
use std::thread;

pub fn workers() -> usize {
    thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(8)
}

pub fn map_chunks<T, R>(items: &[T], min_chunk: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    let workers = workers();
    let chunk = items.len().div_ceil(workers).max(min_chunk.max(1));
    if workers < 2 || chunk >= items.len() {
        return items.iter().map(f).collect();
    }
    let f = &f;
    thread::scope(|scope| {
        let parts: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(f).collect::<Vec<R>>()))
            .collect();
        parts
            .into_iter()
            .flat_map(|part| part.join().expect("parallel worker"))
            .collect()
    })
}
