//! A deliberately narrow desktop launcher. It never accepts vault data or secrets.
use std::ffi::OsString;
use tauri::Manager;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Launch {
    Main,
    QuickAccess,
}

#[derive(Debug, PartialEq, Eq)]
enum Arguments {
    Launch(Launch),
    Help,
    Version,
}

fn arguments(args: impl IntoIterator<Item = OsString>) -> Result<Arguments, &'static str> {
    let args: Vec<_> = args.into_iter().collect();
    match args.as_slice() {
        [] => Ok(Arguments::Launch(Launch::Main)),
        [arg] if arg == "--quick-access" => Ok(Arguments::Launch(Launch::QuickAccess)),
        [arg] if arg == "--help" || arg == "-h" => Ok(Arguments::Help),
        [arg] if arg == "--version" || arg == "-V" => Ok(Arguments::Version),
        _ => Err("Unsupported arguments. Run veyra --help for usage."),
    }
}

pub(crate) struct Startup {
    pub launch: Launch,
    #[cfg(target_os = "linux")]
    connection: zbus::blocking::Connection,
    #[cfg(target_os = "linux")]
    requests: std::sync::mpsc::Receiver<Launch>,
}

pub(crate) fn prepare() -> Result<Option<Startup>, &'static str> {
    let launch = match arguments(std::env::args_os().skip(1))? {
        Arguments::Help => {
            println!("Veyra — Your vault. One shortcut away.\n\nUsage: veyra [--quick-access | --help | --version]\n\n  --quick-access  Open Quick Access in the running session, or start it\n  --help, -h      Show this help\n  --version, -V   Show the installed version");
            return Ok(None);
        }
        Arguments::Version => {
            println!("Veyra {}", env!("CARGO_PKG_VERSION"));
            return Ok(None);
        }
        Arguments::Launch(launch) => launch,
    };
    #[cfg(target_os = "linux")]
    return linux::prepare(launch);
    #[cfg(not(target_os = "linux"))]
    Ok(Some(Startup { launch }))
}

pub(crate) fn show(app: &tauri::AppHandle, launch: Launch) {
    let app = app.clone();
    let handle = app.clone();
    let _ = handle.run_on_main_thread(move || match launch {
        Launch::QuickAccess => {
            if crate::show_quick_access(app).is_err() {
                eprintln!("The desktop could not show Veyra Quick Access.");
            }
        }
        Launch::Main => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    });
}

impl Startup {
    pub(crate) fn attach(self, app: &tauri::AppHandle) {
        show(app, self.launch);
        #[cfg(target_os = "linux")]
        {
            // Claim the launch name before GTK registers its unique application
            // ID. Keeping that ID enabled is necessary for dock icon association.
            app.manage(LaunchConnection(self.connection));
            let app = app.clone();
            std::thread::spawn(move || {
                for launch in self.requests {
                    show(&app, launch);
                }
            });
        }
    }
}

#[cfg(target_os = "linux")]
struct LaunchConnection(#[allow(dead_code)] zbus::blocking::Connection);

#[cfg(target_os = "linux")]
mod linux {
    use super::{Launch, Startup};
    use std::sync::mpsc::{sync_channel, SyncSender};
    use zbus::blocking::{connection::Builder, Connection};

    const NAME: &str = "io.github.LucasGruber.Veyra.Launcher";
    const PATH: &str = "/io/github/LucasGruber/Veyra/Launcher";
    const INTERFACE: &str = "io.github.LucasGruber.Veyra.Launcher";

    struct Launcher(SyncSender<Launch>);

    #[zbus::interface(name = "io.github.LucasGruber.Veyra.Launcher")]
    impl Launcher {
        fn launch(&self, target: &str) -> zbus::fdo::Result<()> {
            let target = match target {
                "main" => Launch::Main,
                "quick-access" => Launch::QuickAccess,
                _ => {
                    return Err(zbus::fdo::Error::InvalidArgs(
                        "Unknown launch target".into(),
                    ))
                }
            };
            self.0.try_send(target).map_err(|_| {
                zbus::fdo::Error::Failed("The desktop launch queue is unavailable".into())
            })
        }
    }

    pub(super) fn prepare(launch: Launch) -> Result<Option<Startup>, &'static str> {
        let (send, requests) = sync_channel(8);
        let builder = Builder::session()
            .map_err(|_| "A desktop D-Bus session is required to launch Veyra.")?
            .name(NAME)
            .map_err(|_| "The Veyra desktop launch service could not be configured.")?
            .replace_existing_names(false)
            .allow_name_replacements(false)
            .serve_at(PATH, Launcher(send))
            .map_err(|_| "The Veyra desktop launch service could not be configured.")?;
        match builder.build() {
            Ok(connection) => Ok(Some(Startup {
                launch,
                connection,
                requests,
            })),
            Err(zbus::Error::NameTaken) => {
                let connection = Connection::session()
                    .map_err(|_| "The running Veyra session could not be contacted.")?;
                connection
                    .call_method(
                        Some(NAME),
                        PATH,
                        Some(INTERFACE),
                        "Launch",
                        &(match launch {
                            Launch::Main => "main",
                            Launch::QuickAccess => "quick-access",
                        },),
                    )
                    .map_err(|_| "The running Veyra session did not accept the launch request.")?;
                Ok(None)
            }
            Err(_) => Err("The Veyra desktop launch service is unavailable."),
        }
    }

    #[test]
    fn launcher_accepts_only_window_targets_and_bounds_queued_requests() {
        let (send, receive) = sync_channel(1);
        let service = Launcher(send);
        assert!(service.launch("quick-access").is_ok());
        assert!(service.launch("main").is_err());
        assert_eq!(receive.recv().unwrap(), Launch::QuickAccess);
        assert!(service.launch("main").is_ok());
        assert_eq!(receive.recv().unwrap(), Launch::Main);
        assert!(service.launch("file:///tmp/untrusted").is_err());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Arguments, &'static str> {
        arguments(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_only_explicit_desktop_actions() {
        assert_eq!(parse(&[]), Ok(Arguments::Launch(Launch::Main)));
        assert_eq!(
            parse(&["--quick-access"]),
            Ok(Arguments::Launch(Launch::QuickAccess))
        );
        for option in ["--help", "-h"] {
            assert_eq!(parse(&[option]), Ok(Arguments::Help));
        }
        for option in ["--version", "-V"] {
            assert_eq!(parse(&[option]), Ok(Arguments::Version));
        }
    }

    #[test]
    fn rejects_unrecognized_or_combined_arguments_without_echoing_them() {
        for args in [
            vec!["--password=secret"],
            vec!["--quick-access", "--help"],
            vec!["https://example.org"],
        ] {
            let message = parse(&args).unwrap_err();
            assert!(!message.contains("secret"));
        }
    }
}
