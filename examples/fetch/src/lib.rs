//! `use_future` + `<Suspense>`: fetch a URL, parse it, and show the answer.
//!
//! The request is the weather from [Open-Meteo](https://open-meteo.com), which
//! needs no key and answers a browser as readily as a server. The city is the
//! deps of `use_future`, so picking another one is a new request, and the
//! `<Suspense>` around the panel draws the spinner while it is in flight —
//! the component itself never draws a pending state of its own.
//!
//! The same code runs natively (`ehttp` uses ureq on a thread of its own) and
//! in a browser (`ehttp` uses the fetch API on the browser's event loop).

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;
use serde::Deserialize;

pub const META: Meta = Meta {
    name: "fetch",
    summary: "`use_future` runs the request; the nearest `<Suspense>` draws the spinner.",
    hooks: &["use_state", "use_future"],
    elements: &[
        "View",
        "Text",
        "Button",
        "ScrollArea",
        "Separator",
        "Suspense",
    ],
    source: include_str!("lib.rs"),
    plain: None,
};

/// The cities the buttons offer, with the coordinates the API wants.
pub const CITIES: [(&str, f32, f32); 4] = [
    ("Tokyo", 35.68, 139.69),
    ("Paris", 48.85, 2.35),
    ("Reykjavík", 64.15, -21.94),
    ("Sydney", -33.87, 151.21),
];

/// What the app asks Open-Meteo for: the weather now, and three days of it.
pub fn url(latitude: f32, longitude: f32) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}\
         &current=temperature_2m,wind_speed_10m,weather_code\
         &daily=weather_code,temperature_2m_max,temperature_2m_min&forecast_days=3\
         &timezone=auto"
    )
}

/// The answer, as much of it as the screen shows.
///
/// `serde` reads the fields it is asked for and ignores the rest, so this is
/// the shape of the screen rather than the shape of the API.
#[derive(Clone, Debug, Deserialize)]
pub struct Forecast {
    pub timezone: String,
    pub current: Current,
    pub daily: Daily,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Current {
    pub temperature_2m: f32,
    pub wind_speed_10m: f32,
    pub weather_code: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Daily {
    pub time: Vec<String>,
    pub weather_code: Vec<u8>,
    pub temperature_2m_max: Vec<f32>,
    pub temperature_2m_min: Vec<f32>,
}

/// WMO weather code 0..99, in words. The codes the API can answer with are
/// grouped the way a forecast reads them out.
pub fn sky(code: u8) -> &'static str {
    match code {
        0 => "clear",
        1..=2 => "mostly clear",
        3 => "overcast",
        45..=48 => "fog",
        51..=57 => "drizzle",
        61..=67 => "rain",
        71..=77 => "snow",
        80..=82 => "showers",
        85..=86 => "snow showers",
        95..=99 => "thunderstorm",
        _ => "weather",
    }
}

#[component]
pub fn App(cx: &mut Cx) {
    let mut city = use_state(cx, || 0usize);
    let mut attempt = use_state(cx, || 0u32);
    let picked = *city;
    let (_, latitude, longitude) = CITIES[picked];

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0} w="100%">
            // `wrap` on the row, so a narrow window puts the buttons on two
            // lines instead of running them off the right edge.
            <View direction="row" wrap gap={8} align="center" w="100%">
                for (i, (name, _, _)) in CITIES.iter().enumerate() {
                    <City key={name} name={name} picked={i == picked} on_click={|| *city = i}/>
                }
                // `attempt` is part of the deps below, so bumping it asks for
                // the same city again.
                <Button on_click={|| *attempt += 1}>"refresh"</Button>
            </View>

            // What is being asked for. `wrap` needs a width to wrap inside,
            // which is what `w="100%"` gives it.
            <Text w="100%" wrap size={11.0}>{url(latitude, longitude)}</Text>

            <Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
                <Weather city={picked} attempt={*attempt}/>
            </Suspense>
        </View>
    }
}

/// One city button: on when it is the one being shown.
///
/// `egui-react-elements` has no toggle, so this is the escape hatch one leaf
/// deep — the same shape every component in these examples has: take a
/// `style`, draw one thing, report the click.
#[component]
fn City(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    name: &str,
    picked: bool,
    #[event] on_click: (),
) {
    let clicked = cx.leaf(&style.shrink(0.0), |ui| {
        // A hand-written leaf sets the wrap mode itself: measured in a
        // zero-width `Ui` on its first draw, a wrapping widget would report
        // one character wide and stay that way.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.selectable_label(picked, name).clicked()
    });
    if clicked {
        on_click.emit(());
    }
}

/// Waits for one request. It never draws a pending state of its own: the
/// `let`-`else` hands that job to the `<Suspense>` above it.
#[component]
fn Weather(cx: &mut Cx, city: usize, attempt: u32) {
    let (name, latitude, longitude) = CITIES[city.min(CITIES.len() - 1)];
    let response = use_future(cx, (city, attempt), || {
        // Built here, where the coordinates are still borrowable; the future
        // itself owns the request and is `'static`.
        let request = ehttp::Request::get(url(latitude, longitude));
        async move { ehttp::fetch_async(request).await }
    });
    let Poll::Ready(response) = response else {
        return;
    };

    rsx! {
        <View direction="column" gap={8} w="100%" grow={1.0} min_h={0.0}>
            match parse(response) {
                Ok(forecast) => {
                    <Text size={22.0} strong>
                        {format!("{name}  {:.0}°C", forecast.current.temperature_2m)}
                    </Text>
                    <Text>
                        {format!(
                            "{}, wind {:.0} km/h, local time {}",
                            sky(forecast.current.weather_code),
                            forecast.current.wind_speed_10m,
                            forecast.timezone,
                        )}
                    </Text>
                    <Separator/>
                    <ScrollArea grow={1.0} h={0.0}>
                        <View direction="column" gap={4} w="100%">
                            for (i, day) in forecast.daily.time.iter().enumerate() {
                                <View key={day} direction="row" gap={8} w="100%">
                                    <Text w={100.0}>{day.as_str()}</Text>
                                    <Text w={110.0}>{sky(forecast.daily.weather_code[i])}</Text>
                                    <Text>
                                        {format!(
                                            "{:.0}° / {:.0}°",
                                            forecast.daily.temperature_2m_max[i],
                                            forecast.daily.temperature_2m_min[i],
                                        )}
                                    </Text>
                                </View>
                            }
                        </View>
                    </ScrollArea>
                }
                Err(err) => { <Text w="100%" wrap>{format!("{name}: {err}")}</Text> }
            }
        </View>
    }
}

/// The response as a [`Forecast`], or why it is not one.
///
/// Three ways to fail, and they are all the same to the screen: the request
/// never arrived, the server said no, or the body was not the JSON expected.
pub fn parse(response: &Result<ehttp::Response, String>) -> Result<Forecast, String> {
    let response = response.as_ref().map_err(String::clone)?;
    if !response.ok {
        return Err(format!("{} {}", response.status, response.status_text));
    }
    let body = response.text().ok_or("the body is not text")?;
    serde_json::from_str(body).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The URL carries the coordinates and everything the screen reads.
    #[test]
    fn the_url_asks_for_what_is_shown() {
        let url = url(35.68, 139.69);
        assert!(url.contains("latitude=35.68"), "{url}");
        assert!(url.contains("longitude=139.69"), "{url}");
        assert!(url.contains("current=temperature_2m"), "{url}");
        assert!(url.contains("forecast_days=3"), "{url}");
    }

    /// A body with more in it than the screen reads is still a `Forecast`.
    #[test]
    fn a_response_becomes_a_forecast() {
        let body = r#"{
            "latitude": 35.7, "longitude": 139.7, "timezone": "Asia/Tokyo",
            "current": {"time": "2026-09-09T12:00", "temperature_2m": 24.5,
                        "wind_speed_10m": 8.0, "weather_code": 61},
            "daily": {"time": ["2026-09-09", "2026-09-10"],
                      "weather_code": [61, 0],
                      "temperature_2m_max": [26.0, 28.0],
                      "temperature_2m_min": [19.0, 20.0]}
        }"#;
        let forecast: Forecast = serde_json::from_str(body).expect("the shape of the screen");
        assert_eq!(forecast.timezone, "Asia/Tokyo");
        assert_eq!(forecast.current.temperature_2m, 24.5);
        assert_eq!(sky(forecast.current.weather_code), "rain");
        assert_eq!(forecast.daily.time.len(), 2);
        assert_eq!(sky(forecast.daily.weather_code[1]), "clear");
    }

    /// Every way the request can fail reads as one line on screen.
    #[test]
    fn a_failure_is_a_message() {
        assert_eq!(
            parse(&Err(String::from("no network"))).unwrap_err(),
            "no network"
        );
    }
}
