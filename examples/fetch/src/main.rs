//! `use_future` + `<Suspense>`: fetch a URL and show the response.
//!
//! The same code runs natively (`ehttp` uses ureq on a thread of its own) and
//! in a browser (`ehttp` uses the fetch API on the browser's event loop).

use react_egui::prelude::*;
use react_egui_app::{Options, run};
use react_egui_elements::prelude::*;

#[component]
fn App(cx: &mut Cx) {
    let mut url = use_state(cx, || String::from("https://httpbin.org/get"));
    let mut attempt = use_state(cx, || 0u32);

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <View direction="row" gap={8} align="center">
                // `attempt` is part of the deps, so bumping it re-fetches the
                // same URL. Editing the URL re-fetches on its own.
                <TextEdit grow={1.0} bind={url.bind()} on_submit={|_: String| *attempt += 1}/>
                <Button on_click={|| *attempt += 1}>"fetch"</Button>
            </View>
            <Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
                <Response url={url.as_str()} attempt={*attempt}/>
            </Suspense>
        </View>
    }
}

/// Waits for one request. It never draws a pending state of its own: the
/// `let`-`else` hands that job to the `<Suspense>` above it.
#[component]
fn Response(cx: &mut Cx, url: &str, attempt: u32) {
    let response = use_future(cx, (url, attempt), || {
        // Built here, where `url` is still borrowable; the future itself owns
        // the request and is `'static`.
        let request = ehttp::Request::get(url);
        async move { ehttp::fetch_async(request).await }
    });
    let Poll::Ready(response) = response else {
        return;
    };

    rsx! {
        match response {
            Ok(response) => {
                <Text strong>{format!("{} {}", response.status, response.status_text)}</Text>
                <ScrollArea grow={1.0}>
                    <Text>{body_preview(response)}</Text>
                </ScrollArea>
            }
            Err(err) => { <Text>{format!("error: {err}")}</Text> }
        }
    }
}

/// The first 2000 characters of the body, or a note if it is not text.
fn body_preview(response: &ehttp::Response) -> String {
    response
        .text()
        .unwrap_or("(not text)")
        .chars()
        .take(2000)
        .collect()
}

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("react-egui: fetch"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
