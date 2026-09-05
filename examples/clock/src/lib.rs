//! A clock and a stopwatch, and what `use_effect`'s cleanup is for.
//!
//! Time comes from `ui.input(|i| i.time)` — seconds since the app started, as
//! egui counts them. `std::time::Instant` is not used anywhere, because it
//! panics on `wasm32-unknown-unknown`; the wall clock goes through `web_time`
//! for the same reason.
//!
//! Repainting is explicit. egui only draws when something asks it to, so a
//! stopped clock asks for one frame a second and a running stopwatch asks for
//! the next frame immediately. Nothing else in the app has to know.
//!
//! The `show ticker` checkbox mounts and unmounts a child. Its `use_effect`
//! returns a closure, which is the cleanup: it runs when the child goes away.
//! The cleanup is stored, so it is `'static` and cannot borrow the log — that
//! is what `Dispatch` is for.

use std::time::Duration;

use example_meta::Meta;
use react_egui::prelude::*;
use react_egui_elements::prelude::*;

pub const META: Meta = Meta {
    name: "clock",
    summary: "A stopwatch that asks for its own repaints, and an effect that cleans up after itself.",
    hooks: &["use_state", "use_memo", "use_effect", "use_reducer"],
    elements: &["View", "Text", "Button", "Checkbox", "Separator"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// The stopwatch. `started_at` is a reading of `i.time`, not a wall-clock
/// instant, so it survives being suspended and works the same in a browser.
#[derive(Default)]
pub struct Stopwatch {
    started_at: Option<f64>,
    accumulated: f64,
    laps: Vec<f64>,
}

impl Stopwatch {
    pub fn running(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn elapsed(&self, now: f64) -> f64 {
        self.accumulated + self.started_at.map_or(0.0, |start| now - start)
    }

    pub fn toggle(&mut self, now: f64) {
        match self.started_at.take() {
            Some(start) => self.accumulated += now - start,
            None => self.started_at = Some(now),
        }
    }

    pub fn lap(&mut self, now: f64) {
        self.laps.push(self.elapsed(now));
    }
}

/// Seconds since midnight UTC, from the system clock.
///
/// `web_time::SystemTime` rather than `std::time::SystemTime`: the standard one
/// panics on `wasm32-unknown-unknown`. There is no time zone here because that
/// would mean a time zone database; UTC is the honest label.
pub fn utc_now() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |since_epoch| since_epoch.as_secs() % 86_400)
}

/// `HH:MM:SS`, by hand. A date library would be a big dependency for one line.
pub fn clock_text(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

/// `MM:SS.CC`.
pub fn stopwatch_text(elapsed: f64) -> String {
    let cs = (elapsed.max(0.0) * 100.0) as u64;
    format!("{:02}:{:02}.{:02}", cs / 6000, (cs / 100) % 60, cs % 100)
}

/// `now` is the wall clock to show, in seconds since midnight UTC. `None`, the
/// default, reads the system clock; a test passes a fixed value so its picture
/// does not change every second.
#[component]
pub fn App(cx: &mut Cx, now: Option<u64>) {
    let mut watch = use_state(cx, Stopwatch::default);
    let mut ticker = use_state(cx, || true);
    // The log lives in a reducer because the effect's cleanup writes to it, and
    // a cleanup is `'static`: it can hold a `Dispatch` but not a borrow.
    let (log, dispatch) = use_reducer(
        cx,
        |lines: &mut Vec<String>, line: String| lines.push(line),
        Vec::new,
    );

    let time = cx.ui().input(|i| i.time);
    let running = watch.running();
    let elapsed = watch.elapsed(time);

    // Ask for exactly the frames that are needed: every one while the
    // stopwatch runs, one a second otherwise so the clock stays right.
    let ctx = cx.ctx().clone();
    if running {
        ctx.request_repaint();
    } else {
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    // Formatting every lap on every frame would be wasted work at 60fps, and
    // the list only changes when a lap is added or the watch is reset.
    let laps = use_memo(cx, watch.laps.len(), || {
        let total = watch.laps.len();
        watch
            .laps
            .iter()
            .enumerate()
            .rev()
            .map(|(i, lap)| format!("lap {}  {}", i + 1, stopwatch_text(*lap)))
            .take(total)
            .collect::<Vec<String>>()
    });
    let lines: Vec<(usize, String)> = log.iter().cloned().enumerate().rev().take(4).collect();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"clock"</Text>
            <Text size={32.0} strong>{clock_text(now.unwrap_or_else(utc_now))}</Text>
            <Text>"UTC"</Text>

            <Separator/>

            <Text size={28.0}>{stopwatch_text(elapsed)}</Text>
            <View direction="row" gap={8}>
                <Button on_click={|| watch.toggle(time)}>
                    {if running { "stop" } else { "start" }}
                </Button>
                <Button enabled={running} on_click={|| watch.lap(time)}>"lap"</Button>
                <Button on_click={|| *watch = Stopwatch::default()}>"reset"</Button>
            </View>
            for (i, lap) in laps.iter().enumerate() {
                <Text key={i}>{lap.as_str()}</Text>
            }

            <Separator/>

            <Checkbox bind={ticker.bind()} label="show ticker"/>
            if *ticker {
                <Ticker log={dispatch.clone()}/>
            }
            for (i, line) in lines.iter() {
                <Text key={i}>{line.as_str()}</Text>
            }
        </View>
    }
}

/// Mounted and unmounted by the checkbox above, and says so both times.
///
/// The effect's deps are `()`, so the body runs once when this component first
/// appears. What it returns is the cleanup, which runs when the component stops
/// being drawn and the pass-end sweep notices. The message lands on the next
/// visit to the reducer, so "unmounted" appears one frame later.
#[component]
fn Ticker(cx: &mut Cx, log: Dispatch<String>) {
    let mounted = log.clone();
    let unmounted = log.clone();
    use_effect(cx, (), move || {
        mounted.send(String::from("ticker mounted"));
        move || unmounted.send(String::from("ticker unmounted"))
    });

    rsx! { <Text>"ticker: mounted"</Text> }
}
