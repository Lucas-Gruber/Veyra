use super::*;

const PROXY_TEST_DIRECTORY: &str = "VEYRA_TRAY_PROXY_TEST_DIRECTORY";

struct ProxyFixture {
    directory: std::path::PathBuf,
    children: Vec<std::process::Child>,
}

impl Drop for ProxyFixture {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

async fn wait_file(path: &std::path::Path) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Timed out waiting for isolated tray fixture");
}

#[test]
#[ignore = "Child fixture used only by tray_proxy_roundtrip"]
fn tray_proxy_client() {
    let directory = std::path::PathBuf::from(
        std::env::var_os(PROXY_TEST_DIRECTORY).expect("Proxy fixture only"),
    );
    let _tray = tray_icon::TrayIconBuilder::new()
        .with_id("veyra-flatpak-proxy-test")
        .with_icon(tray_icon::Icon::from_rgba([0, 0, 0, 255].repeat(4), 2, 2).unwrap())
        .build()
        .expect("Flatpak tray must register without owning a well-known name");
    std::fs::write(directory.join("client-ready"), b"ready").unwrap();
    let started = std::time::Instant::now();
    loop {
        if let Ok(tray_icon::TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            ..
        }) = tray_icon::TrayIconEvent::receiver().try_recv()
        {
            std::fs::write(directory.join("activated"), b"activated").unwrap();
        }
        if directory.join("host-done").exists() {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(12),
            "Host did not complete proxy roundtrip"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Run with dbus-run-session and xdg-dbus-proxy; never use a real desktop bus"]
async fn tray_proxy_roundtrip() {
    let directory = std::env::temp_dir().join(format!("veyra-tray-proxy-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let mut fixture = ProxyFixture {
        directory,
        children: Vec::new(),
    };
    let items = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    struct Watcher(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    #[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
    impl Watcher {
        fn register_status_notifier_item(&self, service: &str) {
            self.0.lock().unwrap().push(service.to_owned());
        }
        #[zbus(property)]
        fn is_status_notifier_host_registered(&self) -> bool {
            true
        }
    }
    // This host stays OUTSIDE the proxy. Only the child tray client is filtered.
    let watcher = zbus::connection::Builder::session()
        .unwrap()
        .name("org.kde.StatusNotifierWatcher")
        .unwrap()
        .serve_at("/StatusNotifierWatcher", Watcher(items.clone()))
        .unwrap()
        .build()
        .await
        .unwrap();
    let socket = fixture.directory.join("bus");
    fixture.children.push(
        std::process::Command::new("xdg-dbus-proxy")
            .arg(std::env::var_os("DBUS_SESSION_BUS_ADDRESS").expect("Private D-Bus required"))
            .arg(&socket)
            .args(["--filter", "--talk=org.kde.StatusNotifierWatcher"])
            .spawn()
            .unwrap(),
    );
    wait_file(&socket).await;
    fixture.children.push(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "tray::tests::tray_proxy_client"])
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", socket.display()),
            )
            .env("FLATPAK_ID", "io.github.LucasGruber.Veyra")
            .env(PROXY_TEST_DIRECTORY, &fixture.directory)
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .spawn()
            .unwrap(),
    );
    wait_file(&fixture.directory.join("client-ready")).await;
    let name = items
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("Tray must register through the proxy");
    assert!(
        name.starts_with(':'),
        "Sandboxed tray must use a unique connection name"
    );
    let icon = zbus::Proxy::new(
        &watcher,
        name,
        "/StatusNotifierItem",
        "org.kde.StatusNotifierItem",
    )
    .await
    .unwrap();
    let pixels: Vec<(i32, i32, Vec<u8>)> = icon.get_property("IconPixmap").await.unwrap();
    assert_eq!((pixels[0].0, pixels[0].1, pixels[0].2.len()), (2, 2, 16));
    let menu_path: zbus::zvariant::OwnedObjectPath = icon.get_property("Menu").await.unwrap();
    let menu = zbus::Proxy::new(
        &watcher,
        icon.destination().clone(),
        menu_path,
        "com.canonical.dbusmenu",
    )
    .await
    .unwrap();
    let version: u32 = menu.get_property("Version").await.unwrap();
    assert_eq!(version, 3);
    type Layout = (
        u32,
        (
            i32,
            std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            Vec<zbus::zvariant::OwnedValue>,
        ),
    );
    let (_revision, layout): Layout = menu
        .call("GetLayout", &(0_i32, -1_i32, Vec::<String>::new()))
        .await
        .unwrap();
    assert_eq!(layout.0, 0);
    let _: () = icon.call("Activate", &(0_i32, 0_i32)).await.unwrap();
    wait_file(&fixture.directory.join("activated")).await;
    std::fs::write(fixture.directory.join("host-done"), b"done").unwrap();
    assert!(fixture.children[1].wait().unwrap().success());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Run on a private bus: dbus-run-session -- cargo test tray_host_status -- --ignored"]
async fn tray_host_status_distinguishes_missing_and_registered_hosts() {
    assert!(!host_available().await);
    let registered = std::sync::Arc::new(AtomicBool::new(true));
    let items = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    struct Watcher {
        registered: std::sync::Arc<AtomicBool>,
        items: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }
    #[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
    impl Watcher {
        fn register_status_notifier_item(&self, service: &str) {
            self.items.lock().unwrap().push(service.to_owned());
        }

        #[zbus(property)]
        fn is_status_notifier_host_registered(&self) -> bool {
            self.registered.load(Ordering::Acquire)
        }
    }
    let _watcher = zbus::connection::Builder::session()
        .unwrap()
        .name("org.kde.StatusNotifierWatcher")
        .unwrap()
        .serve_at(
            "/StatusNotifierWatcher",
            Watcher {
                registered: registered.clone(),
                items: items.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    assert!(host_available().await);

    // Exercise the actual selected Linux backend, without GTK initialization
    // or any real desktop bus/display. This also checks the exported icon.
    let _tray = tray_icon::TrayIconBuilder::new()
        .with_id("veyra-isolated-tray-test")
        .with_icon(tray_icon::Icon::from_rgba([0, 0, 0, 255].repeat(4), 2, 2).unwrap())
        .build()
        .unwrap();
    let item_name = items
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("KSNI must register its tray item");
    assert!(item_name.starts_with("org.kde.StatusNotifierItem-"));
    let connection = zbus::Connection::session().await.unwrap();
    let icon = zbus::Proxy::new(
        &connection,
        item_name,
        "/StatusNotifierItem",
        "org.kde.StatusNotifierItem",
    )
    .await
    .unwrap();
    let pixels: Vec<(i32, i32, Vec<u8>)> = icon.get_property("IconPixmap").await.unwrap();
    assert_eq!(pixels.len(), 1);
    assert_eq!((pixels[0].0, pixels[0].1, pixels[0].2.len()), (2, 2, 16));
    registered.store(false, Ordering::Release);
    assert!(!host_available().await);
}
