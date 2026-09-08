//! The `Url` font source, as a real HTTP round trip: `ehttp` uses ureq on a
//! thread natively, so a `TcpListener` on the loopback interface serving one
//! font file exercises the same path a web server would. The chain reports
//! `Pending` after the first `apply`, `Loaded` once the callback has run, and
//! the definitions change exactly once.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use egui_react_app::fonts::{FontSource, Fonts, Outcome};
use epaint_default_fonts::{HACK_REGULAR, UBUNTU_LIGHT};

/// Serve `HACK_REGULAR` at `/hack.ttf` and a 404 for anything else, for
/// `requests` connections, on a thread. `None` when no port can be bound,
/// which is a sandbox's business and not this code's.
fn serve(requests: usize) -> Option<u16> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("cannot bind a loopback port ({err}); skipping the URL round trip");
            return None;
        }
    };
    let port = listener.local_addr().ok()?.port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(requests) {
            let Ok(mut stream) = stream else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            // Enough of the request to see the path; the client sends the
            // whole header block at once.
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);
            let path = request.split_whitespace().nth(1).unwrap_or("/");
            let (status, body): (&str, &[u8]) = if path == "/hack.ttf" {
                ("200 OK", HACK_REGULAR)
            } else {
                ("404 Not Found", b"no such font")
            };
            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Type: font/ttf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
            let _ = stream.flush();
        }
    });
    Some(port)
}

/// Poll the report until the first entry of the first stack is not
/// `Pending`, or give up after a while (a slow CI runner, not a failure).
fn wait_for_first_entry(fonts: &Fonts) -> Outcome {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let outcome = fonts.report()[0].entries[0].1.clone();
        if outcome != Outcome::Pending || Instant::now() > deadline {
            return outcome;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_url_is_pending_then_loaded_and_the_definitions_change_once() {
    let Some(port) = serve(1) else {
        return;
    };
    let fonts = Fonts::new().stack(
        "web",
        [
            FontSource::Url(format!("http://127.0.0.1:{port}/hack.ttf")),
            // A different font behind it: were it Hack too, the arrival
            // would resolve to the same key and change nothing.
            FontSource::Bundled(UBUNTU_LIGHT),
        ],
    );
    // A kittest harness rather than a bare `Context`: `set_fonts` only takes
    // effect at the start of a pass, and the harness is what runs one.
    let mut harness = egui_kittest::Harness::new_ui(|_ui| {});
    let ctx = harness.ctx.clone();
    // Nothing has been asked for yet.
    assert!(!fonts.pending());
    fonts.apply(&ctx);
    assert_eq!(fonts.report()[0].entries[0].1, Outcome::Pending);
    assert!(fonts.pending());
    assert_eq!(fonts.generation(), 1);

    let outcome = wait_for_first_entry(&fonts);
    // The one URL has arrived, so there is nothing left to wait for.
    assert!(!fonts.pending());
    assert_eq!(
        outcome,
        Outcome::Loaded {
            key: "Hack-Regular#0".into(),
            family: "Hack".into()
        }
    );
    // One change for the first apply, one for the arrival.
    assert_eq!(fonts.generation(), 2);

    // Applying again fetches nothing and changes nothing.
    fonts.apply(&ctx);
    assert_eq!(fonts.generation(), 2);

    // And egui got the definitions: after a frame the family is there.
    harness.run();
    let list =
        ctx.fonts(|f| f.definitions().families[&egui::FontFamily::Name("web".into())].clone());
    assert_eq!(list[0], "Hack-Regular#0");
}

#[test]
fn a_url_that_404s_is_reported_as_failed() {
    let Some(port) = serve(1) else {
        return;
    };
    let fonts = Fonts::new().stack(
        "web",
        [
            FontSource::Url(format!("http://127.0.0.1:{port}/missing.ttf")),
            FontSource::Bundled(UBUNTU_LIGHT),
        ],
    );
    let ctx = egui::Context::default();
    fonts.apply(&ctx);
    let outcome = wait_for_first_entry(&fonts);
    let Outcome::Failed(reason) = outcome else {
        panic!("expected Failed, got {outcome:?}");
    };
    assert!(reason.contains("404"), "{reason}");
    // A failure ends the wait the same way an arrival does.
    assert!(!fonts.pending());
    // The chain still draws with what is behind the failed entry.
    let report = fonts.report();
    assert!(matches!(report[0].entries[1].1, Outcome::Loaded { .. }));
}
