use sycamore::futures::spawn_local_scoped;
use sycamore::{prelude::*, rt};

use crate::api::*;
use crate::side_bar::check_number_input;
use acquisitors::*;

use wasm_bindgen::prelude::wasm_bindgen;
#[wasm_bindgen(inline_js = "export function blur() { document.activeElement.blur(); }")]
extern "C" {
    fn blur();
}

fn form_blur(event: rt::Event) {
    event.prevent_default();
    blur();
}

#[component]
pub fn RenderFileReaderConfig<G: Html>(cx: Scope) -> View<G> {
    let config = create_signal(cx, empty_file_reader_config());

    spawn_local_scoped(cx, async move {
        // Get old config. Retries a few times
        for _ in 0..3 {
            let _config = get_acquisitor_config().await;

            if let AcquisitorConfig::FileReaderConfig(_config) = _config {
                config.set(_config);
                return;
            }
        }
    });

    let update_watcher_path = move |event: rt::Event| {
        event.prevent_default();
        spawn_local_scoped(cx, async move {
            match pick_folder().await {
                None => (),
                Some(path) => (config.modify()).watcher_path = path,
            }
        });
    };

    let watcher_path = create_memo(cx, || format!("{}", (config.get()).watcher_path.display()));

    create_effect(cx, move || {
        // Apply config when it is updated
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_file_reader_config() {
                apply_acquisitor_config(AcquisitorConfig::FileReaderConfig(
                    (*config.get()).clone(),
                ))
                .await;
            }
        });
    });

    view! { cx,
        form(on:submit=form_blur) {
            input(type="submit", style="display: none;")

            p(class="mini-title") {
                p { "Aquisitor" }
                p { "(Leitor de Arquivos) "}
            }

            div(class="element") {
                p { "Caminho para vigiar:" }
                p {
                    button(on:click=update_watcher_path) { " " }
                    (watcher_path.get())
                }
            }

        }
    }
}

#[component]
pub fn RenderImonConfig<G: Html>(cx: Scope) -> View<G> {
    let config = create_signal(cx, empty_imon_config());

    let exposure = create_signal(cx, String::new());
    let read_delay = create_signal(cx, String::new());
    let multisampling = create_signal(cx, String::new());

    spawn_local_scoped(cx, async move {
        // Get old config. Retries a few times
        for _ in 0..3 {
            let _config = get_acquisitor_config().await;

            if let AcquisitorConfig::ImonConfig(_config) = _config {
                exposure.set(_config.exposure_ms.to_string());
                read_delay.set(_config.read_delay_ms.to_string());
                multisampling.set(_config.multisampling.to_string());

                config.set(_config);
                return;
            }
        }
    });

    create_effect(cx, move || {
        // Apply config when it is updated
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_imon_config() {
                apply_acquisitor_config(AcquisitorConfig::ImonConfig((*config.get()).clone()))
                    .await;
            }
        });
    });

    let update_config = |event: rt::Event| {
        event.prevent_default();

        let mut config = config.modify();

        match (*exposure.get()).parse::<f64>() {
            Ok(value) if (0.001..=60_000.0).contains(&value) => config.exposure_ms = value,

            _ => exposure.set(config.exposure_ms.to_string()),
        }

        match (*read_delay.get()).parse::<u64>() {
            Ok(value) => config.read_delay_ms = value,
            Err(_) => read_delay.set(config.read_delay_ms.to_string()),
        }

        match (*multisampling.get()).parse::<u32>() {
            Ok(value) => config.multisampling = value,
            Err(_) => multisampling.set(config.multisampling.to_string()),
        }
    };

    view! { cx,
        form(on:submit=form_blur) {
            input(type="submit", style="display: none;")

            p(class="mini-title") {
                p { "Aquisitor" }
                p { "(Ibsen IMON) "}
            }

            div(class="element") {
                p { "Exposição: " }
                input(
                    bind:value=exposure,
                    on:input=|_| check_number_input(exposure),
                    on:focusout=update_config
                ) {}
                "ms"
            }

            div(class="element") {
                p { "Delay entre leituras: " }
                input(
                    bind:value=read_delay,
                    type="number",
                    on:focusout=update_config
                ) {}
                "ms"
            }

            div(class="element") {
                p { "multisampling: " }
                input(
                    bind:value=multisampling,
                    type="number",
                    on:focusout=update_config
                ) {}
            }
        }
    }
}

#[component]
pub fn RenderExampleConfig<G: Html>(cx: Scope) -> View<G> {
    let config = create_signal(cx, empty_example_config());

    let points = create_signal(cx, String::new());
    let amplitude = create_signal(cx, String::new());
    let phase_t_speed = create_signal(cx, String::new());
    let phase_x_speed = create_signal(cx, String::new());
    let update_delay_millis = create_signal(cx, String::new());

    spawn_local_scoped(cx, async move {
        // Get old config. Retries a few times
        for _ in 0..3 {
            let _config = get_acquisitor_config().await;

            if let AcquisitorConfig::ExampleConfig(_config) = _config {
                points.set(_config.points.to_string());
                amplitude.set(_config.amplitude.to_string());
                phase_t_speed.set(_config.phase_t_speed.to_string());
                phase_x_speed.set(_config.phase_x_speed.to_string());
                update_delay_millis.set(_config.update_delay_millis.to_string());

                config.set(_config);
                return;
            }
        }
    });

    create_effect(cx, move || {
        // Apply config when it is updated
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_example_config() {
                apply_acquisitor_config(AcquisitorConfig::ExampleConfig((*config.get()).clone()))
                    .await;
            }
        });
    });

    // Check if should send the config on input update
    let update_config = |event: rt::Event| {
        event.prevent_default();

        let mut config = config.modify();

        match (*points.get()).parse::<u64>() {
            Ok(value) => config.points = value,
            Err(_) => points.set(config.points.to_string()),
        }

        match (*amplitude.get()).parse::<f64>() {
            Ok(value) => config.amplitude = value,
            _ => amplitude.set(config.amplitude.to_string()),
        }

        match (*phase_t_speed.get()).parse::<f64>() {
            Ok(value) => config.phase_t_speed = value,
            _ => phase_t_speed.set(config.phase_t_speed.to_string()),
        }

        match (*phase_x_speed.get()).parse::<f64>() {
            Ok(value) => config.phase_x_speed = value,
            _ => phase_x_speed.set(config.phase_x_speed.to_string()),
        }

        match (*update_delay_millis.get()).parse::<u64>() {
            Ok(value) => config.update_delay_millis = value,
            Err(_) => update_delay_millis.set(config.update_delay_millis.to_string()),
        }
    };

    view! { cx,
        form(on:submit=form_blur) {
            input(type="submit", style="display: none;")

            p(class="mini-title") {
                p { "Aquisitor" }
                p { "(Example) "}
            }

            div(class="element") {
                p { "Densidade: " }
                input(
                    bind:value=points,
                    type="number",
                    on:focusout=update_config
                ) {}
                "pontos"
            }

            div(class="element") {
                p { "Amplitude: " }
                input(
                    bind:value=amplitude,
                    on:input=|_| check_number_input(amplitude),
                    on:focusout=update_config
                ) {}
            }

            div(class="element") {
                p { "Velocidade angular em t: " }
                input(
                    bind:value=phase_t_speed,
                    on:input=|_| check_number_input(phase_t_speed),
                    on:focusout=update_config
                ) {}
                "rad/s"
            }

            div(class="element") {
                p { "Velocidade angular em x: " }
                input(
                    bind:value=phase_x_speed,
                    on:input=|_| check_number_input(phase_x_speed),
                    on:focusout=update_config
                ) {}
                "rad/un"
            }

            div(class="element") {
                p { "Delay entre leituras: " }
                input(
                    bind:value=update_delay_millis,
                    type="number",
                    on:focusout=update_config
                ) {}
                "ms"
            }
        }
    }
}
