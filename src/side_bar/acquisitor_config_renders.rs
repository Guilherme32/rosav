use sycamore::{ rt, prelude::* };
use sycamore::futures::spawn_local_scoped;

use crate::api::*;
use acquisitors::*;


#[component]
pub fn RenderFileReaderConfig<G:Html>(cx: Scope) -> View<G> {
    let config = create_signal(cx, empty_file_reader_config());

    spawn_local_scoped(cx, async move {                // Get old config
        let _config = get_acquisitor_config().await;
        if let AcquisitorConfig::FileReaderConfig(_config) = _config {
            config.set(_config);
        }
    });

    let update_watcher_path = move |event: rt::Event| {
        event.prevent_default();
        spawn_local_scoped(cx, async move {
            match pick_folder().await {
                None => (),
                Some(path) => (*config.modify()).watcher_path = path
            }
        });
    };

    let watcher_path = create_memo(cx, || {
        format!("{}", (*config.get()).watcher_path.display())
    });

    create_effect(cx, move || {                    // Apply config when it is updated
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_file_reader_config() {
                apply_acquisitor_config(
                    AcquisitorConfig::FileReaderConfig((*config.get()).clone())
                ).await;
            }
        });
    });

    let do_nothing = |event: rt::Event| event.prevent_default();

    view! { cx, 
        form(on:submit=do_nothing) {
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
pub fn RenderImonConfig<G:Html>(cx: Scope) -> View<G> {
    let config = create_signal(cx, empty_imon_config());

    let multisampling = create_signal(cx, String::new());
    let exposure = create_signal(cx, String::new());
    let read_delay = create_signal(cx, String::new());


    spawn_local_scoped(cx, async move {                // Get old config
        let _config = get_acquisitor_config().await;

        if let AcquisitorConfig::ImonConfig(_config) = _config {
            multisampling.set(_config.multisampling.to_string());
            exposure.set(_config.exposure_ms.to_string());
            read_delay.set(_config.read_delay_ms.to_string());

            config.set(_config);
        }
    });

    create_effect(cx, move || {                    // Apply config when it is updated
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_imon_config() {
                apply_acquisitor_config(
                    AcquisitorConfig::ImonConfig((*config.get()).clone())
                ).await;
            }
        });
    });

    let update_config = |event: rt::Event| {
        event.prevent_default();

        let mut config = config.modify();

        match (*multisampling.get()).parse::<u64>() {
            Ok(value) if value >= 1 =>
                config.multisampling = value,

            _ => multisampling.set(config.multisampling.to_string())
        }

        match (*exposure.get()).parse::<u64>() {
            Ok(value) if 10 <= value && value <= 60_000 =>
                config.exposure_ms = value,

            _ => exposure.set(config.exposure_ms.to_string())
        }

        match (*read_delay.get()).parse::<u64>() {
            Ok(value) => config.read_delay_ms = value,
            Err(_) => read_delay.set(config.read_delay_ms.to_string())
        }
    };

    view! { cx, 
        form(on:submit=update_config) {
            input(type="submit", style="display: none;")

            p(class="mini-title") {
                p { "Aquisitor" }
                p { "(Ibsen IMON) "}
            }

            div(class="element") {
                p { "Multisampling: " }
                input(
                    bind:value=multisampling,
                    type="number",
                    on:focusout=update_config
                ) {}
            }

            div(class="element") {
                p { "Exposição: " }
                input(
                    bind:value=exposure,
                    type="number",
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
        }
    }
}


