mod about;
mod app;
mod config;
mod i18n;
mod menu;
mod options;
mod quick_connect;
mod session_manager;
mod status_bar;
mod tabs;
mod term;
mod toolbar;

use shagshell_core::check;

fn main() -> cosmic::iced::Result {
    match std::env::args().nth(1).as_deref() {
        Some("--version") => {
            println!("shagshell {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--check") => {
            let found = check::probe(&check::TOOLS, std::env::var_os("PATH").as_deref());
            for (name, ok) in &found {
                println!("{name:<8} {}", if *ok { "ok" } else { "MISSING" });
            }
            // ssh is the one we can't work without.
            let ssh = found.iter().any(|(name, ok)| name == "ssh" && *ok);
            std::process::exit(i32::from(!ssh));
        }
        _ => {}
    }
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .env()
        .init()
        .ok();
    let software = pick_renderer();
    i18n::init(&i18n_embed::DesktopLanguageRequester::requested_languages());
    let settings = cosmic::app::Settings::default()
        .size(cosmic::iced::Size::new(1200.0, 800.0))
        // softbuffer (tiny-skia) takes only 24-bit X11 visuals; a transparent window gets a 32-bit one.
        .transparent(!software);
    cosmic::app::run::<app::App>(settings, ())
}

// Adapted from shagoff-commander (crates/app/src/main.rs).
/// iced's wgpu shaders need `SHADER_F16_IN_F32`. An old or software OpenGL (X11 without DRI3, as
/// on Linux Lite in a VM) lacks it and the app panicked on its first frame. There: the software
/// renderer. `ICED_BACKEND` set by hand wins. Costs ~70 ms at startup. `true`: software.
fn pick_renderer() -> bool {
    use cosmic::iced::wgpu;
    use std::os::unix::process::CommandExt;
    if let Some(b) = std::env::var_os("ICED_BACKEND") {
        return b.to_string_lossy().starts_with("tiny-skia");
    }
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all()),
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }));
    let f16 = wgpu::DownlevelFlags::SHADER_F16_IN_F32;
    if !adapter.is_ok_and(|a| a.get_downlevel_capabilities().flags.contains(f16)) {
        log::warn!("GPU without SHADER_F16_IN_F32: software rendering (ICED_BACKEND=tiny-skia)");
        // Run ourselves again with it set: `set_var` here would race the GPU driver's threads
        // the probe just started. `exec` only returns on failure.
        let exe = std::env::current_exe().unwrap_or_else(|_| "shagshell".into());
        let e = std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .env("ICED_BACKEND", "tiny-skia")
            .exec();
        log::error!("re-exec with ICED_BACKEND=tiny-skia: {e}");
    }
    false
}
