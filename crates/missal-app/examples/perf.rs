use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::test::TestWindowExt;
use gpui_kit::{Bounds, HeadlessAppContext, WindowBounds, WindowOptions, point, px, size};
use missal_app::db::Db;
use missal_app::launch::{init_app, open_shell};
use missal_app::prefs::{Column, Preferences};
use missal_app::seed::seed_firs;
use missal_app::state::Models;
use missal_store::Store;

fn stats(name: &str, mut samples: Vec<Duration>) {
    samples.sort();
    let n = samples.len();
    let mean = samples.iter().sum::<Duration>() / n as u32;
    println!(
        "{name}: n={n} mean={:.2}ms p50={:.2}ms p95={:.2}ms max={:.2}ms",
        mean.as_secs_f64() * 1e3,
        samples[n / 2].as_secs_f64() * 1e3,
        samples[(n * 95 / 100).min(n - 1)].as_secs_f64() * 1e3,
        samples[n - 1].as_secs_f64() * 1e3
    );
}

fn main() {
    let count: usize = std::env::args().nth(1).and_then(|value| value.parse().ok()).unwrap_or(50_000);
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("missal.sqlite");
    let started = Instant::now();
    seed_firs(&database, count).unwrap();
    println!("seeding {count} FIRs: {:.2}s", started.elapsed().as_secs_f64());
    drop(Store::open(&database).unwrap());

    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new("sans-serif"));
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.allow_parking();
    let launch = Instant::now();
    let (handle, models, shell) = cx.update(|cx| {
        let models = init_app(cx, Db::spawn(database.clone()), None, Preferences::default());
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(1280.), px(800.)),
            })),
            ..gpui_kit::component::TitleBar::window_options()
        };
        let (handle, shell) = open_shell(&models, options, cx).unwrap();
        (handle, models, shell)
    });
    let draw = |cx: &mut HeadlessAppContext| {
        cx.update_window(handle, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .unwrap();
    };
    loop {
        cx.run_until_parked();
        let ready = cx.update(|cx| models.firs.read(cx).load.is_ready());
        if ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    draw(&mut cx);
    cx.run_until_parked();
    draw(&mut cx);
    println!("launch to first frame with the list visible (CPU frame, no GPU submit): {:.0}ms", launch.elapsed().as_secs_f64() * 1e3);
    let rows = cx.update(|cx| models.firs.read(cx).rows.len());
    println!("rows loaded: {rows}");
    let list = cx.update(|cx| shell.read(cx).fir_list().unwrap());

    let mut idle = Vec::new();
    for _ in 0..60 {
        let t = Instant::now();
        draw(&mut cx);
        idle.push(t.elapsed());
    }
    stats("frame, unchanged list", idle);

    let mut scroll = Vec::new();
    for step in 0..300usize {
        cx.update(|cx| list.update(cx, |list, cx| list.scroll_to_row(step * 37 % rows.saturating_sub(40), cx)));
        let t = Instant::now();
        cx.run_until_parked();
        draw(&mut cx);
        scroll.push(t.elapsed());
    }
    stats("frame while scrolling (new rows each frame)", scroll);

    let mut sort = Vec::new();
    for column in [Column::FirNo, Column::Date, Column::Offence, Column::Status] {
        let t = Instant::now();
        cx.update(|cx| list.update(cx, |list, cx| list.toggle_sort(column, cx)));
        cx.run_until_parked();
        draw(&mut cx);
        sort.push(t.elapsed());
    }
    stats("sort change + frame", sort);

    let search = cx.update(|cx| list.read(cx).search_input(cx));
    cx.update_window(handle, |_, window, cx| search.update(cx, |input, cx| input.focus(window, cx))).unwrap();
    let mut typing = Vec::new();
    for character in "چوری 379".chars() {
        let t = Instant::now();
        cx.update_window(handle, |_, window, cx| window.input(&character.to_string(), cx)).unwrap();
        cx.run_until_parked();
        draw(&mut cx);
        typing.push(t.elapsed());
    }
    stats("search keystroke (filter 50k + frame)", typing);
    println!("matches after typing: {}", cx.update(|cx| list.read(cx).visible_count()));
    let _ = Models::get;
}
