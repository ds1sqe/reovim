use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
    time::Duration,
};

use {
    reovim_client_cli::GrpcClient,
    reovim_client_tui::{TuiAppError, TuiHandle, connect_headless},
    reovim_protocol::v2::{ListModulesRequest, module_service_client::ModuleServiceClient},
    reovim_subsys_module_registry::{InstalledModule, InstalledModules, ModuleSource},
    reovim_testing::TestServerHarness,
    tempfile::TempDir,
};

const SAMPLE_MARKER: &str = "sample-count:1";
const PANEL_MARKER: &str = "sample-panel";
const TIMEOUT: Duration = Duration::from_secs(5);

async fn headless_tui(addr: &str, width: u16, height: u16) -> Result<TuiHandle, TuiAppError> {
    let (mut app, handle) =
        connect_headless(addr, width, height, None, None, &std::collections::HashSet::new())
            .await?;
    tokio::spawn(async move { app.run().await });
    Ok(handle)
}

async fn connect_with_retry(addr: &str) -> GrpcClient {
    for _ in 0..20 {
        match GrpcClient::connect(addr).await {
            Ok(client) => return client,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }

    panic!("failed to connect to test server at {addr}");
}

async fn connect_module_service_with_retry(
    addr: &str,
) -> ModuleServiceClient<tonic::transport::Channel> {
    let url = format!("http://{addr}");
    for _ in 0..20 {
        match ModuleServiceClient::connect(url.clone()).await {
            Ok(client) => return client,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }

    panic!("failed to connect module-service client to {addr}");
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("clients/tui manifest should have workspace root")
        .to_path_buf()
}

fn sample_server_so() -> Option<PathBuf> {
    let root = workspace_root();
    let path = root.join("target/debug/libreovim_module_sample.so");
    path.exists().then_some(path)
}

fn sample_client_so() -> Option<PathBuf> {
    let root = workspace_root();
    let path = root.join("target/debug/libreovim_client_module_sample.so");
    path.exists().then_some(path)
}

fn prepare_xdg_layout(temp: &TempDir, server_so: &Path, client_so: &Path) -> PathBuf {
    let xdg_root = temp.path();
    let modules_dir = xdg_root.join("reovim/modules");
    let client_modules_dir = xdg_root.join("reovim/client-modules");
    std::fs::create_dir_all(&modules_dir).expect("should create modules dir");
    std::fs::create_dir_all(&client_modules_dir).expect("should create client modules dir");

    let install_dir = modules_dir.join("sample");
    std::fs::create_dir_all(&install_dir).expect("should create sample install dir");
    let installed_server_so = install_dir.join(
        server_so
            .file_name()
            .expect("sample server .so should have filename"),
    );
    std::fs::copy(server_so, &installed_server_so).expect("should copy server fixture .so");

    let installed_client_so = client_modules_dir.join(
        client_so
            .file_name()
            .expect("sample client .so should have filename"),
    );
    std::fs::copy(client_so, &installed_client_so).expect("should copy client fixture .so");

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "sample".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path(install_dir.to_string_lossy().into_owned()),
        install_path: install_dir,
        library_path: Some(installed_server_so),
    });
    installed
        .save(&modules_dir.join("installed.json"))
        .expect("should write installed.json");

    client_modules_dir
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ClientEnvGuard {
    _lock: MutexGuard<'static, ()>,
    old_path: Option<OsString>,
    old_flag: Option<OsString>,
}

impl Drop for ClientEnvGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            if let Some(val) = self.old_path.take() {
                std::env::set_var("REOVIM_CLIENT_MODULE_PATH", val);
            } else {
                std::env::remove_var("REOVIM_CLIENT_MODULE_PATH");
            }

            if let Some(val) = self.old_flag.take() {
                std::env::set_var("REOVIM_LOAD_DYNAMIC_CLIENT_MODULES", val);
            } else {
                std::env::remove_var("REOVIM_LOAD_DYNAMIC_CLIENT_MODULES");
            }
        }
    }
}

#[allow(unsafe_code)]
fn set_client_env(path: &Path) -> ClientEnvGuard {
    let lock = env_lock().lock().expect("env lock should not be poisoned");
    let old_path = std::env::var_os("REOVIM_CLIENT_MODULE_PATH");
    let old_flag = std::env::var_os("REOVIM_LOAD_DYNAMIC_CLIENT_MODULES");
    // SAFETY: guarded by a process-wide mutex in test code to avoid concurrent env mutation.
    unsafe {
        std::env::set_var("REOVIM_CLIENT_MODULE_PATH", path);
        std::env::set_var("REOVIM_LOAD_DYNAMIC_CLIENT_MODULES", "1");
    }
    ClientEnvGuard {
        _lock: lock,
        old_path,
        old_flag,
    }
}

async fn wait_for_sample_content(client: &mut GrpcClient) -> String {
    let start = std::time::Instant::now();
    while start.elapsed() < TIMEOUT {
        let response = client
            .get_buffer_content(None)
            .await
            .expect("get_buffer_content should succeed");
        let content = response.lines.join("\n");
        if content.contains(SAMPLE_MARKER) {
            return content;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let buffers = client
        .list_buffers()
        .await
        .expect("list_buffers should succeed after timeout");
    panic!(
        "active buffer did not contain '{SAMPLE_MARKER}' within timeout; visible buffers: {:?}",
        buffers
            .buffers
            .iter()
            .map(|buffer| format!("{}:{}", buffer.id, buffer.name))
            .collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_sample_module_e2e() {
    let Some(server_so) = sample_server_so() else {
        eprintln!(
            "SKIP: sample server fixture .so not found. Run: cargo build -p reovim-module-sample"
        );
        return;
    };
    let Some(client_so) = sample_client_so() else {
        eprintln!(
            "SKIP: sample client fixture .so not found. Run: cargo build -p reovim-client-module-sample"
        );
        return;
    };

    let temp = tempfile::tempdir().expect("should create tempdir");
    let client_module_dir = prepare_xdg_layout(&temp, &server_so, &client_so);
    let _env = set_client_env(&client_module_dir);

    let xdg_data_home = temp.path().to_string_lossy().into_owned();
    let harness = TestServerHarness::spawn_with_env(&[("XDG_DATA_HOME", &xdg_data_home)])
        .await
        .expect("should spawn test server with custom XDG data home");
    let addr = format!("127.0.0.1:{}", harness.port());

    let mut grpc = connect_with_retry(&addr).await;
    grpc.get_mode()
        .await
        .expect("gRPC client should join session");
    let modules = connect_module_service_with_retry(&addr)
        .await
        .list(ListModulesRequest {})
        .await
        .expect("module-service list should succeed")
        .into_inner();
    assert!(
        modules
            .modules
            .iter()
            .any(|module| module.name == "Sample Module"),
        "sample server module should be loaded via registry bootstrap; got: {:?}",
        modules
            .modules
            .iter()
            .map(|module| module.name.clone())
            .collect::<Vec<_>>()
    );

    let handle = headless_tui(&addr, 80, 24)
        .await
        .expect("headless TUI should connect");

    let frame = handle
        .wait_for(TIMEOUT, |frame| frame.contains(PANEL_MARKER))
        .await
        .expect("sample client module should render panel");
    assert!(frame.contains(PANEL_MARKER), "frame should contain sample panel marker");

    assert!(
        handle
            .send_keys(":sample-inc<CR>")
            .await
            .expect("sample-inc keys should send through TUI handle"),
        "sample-inc keys should be processed through TUI handle"
    );
    assert!(
        handle
            .send_keys(":sample-get<CR>")
            .await
            .expect("sample-get keys should send through TUI handle"),
        "sample-get keys should be processed through TUI handle"
    );

    let content = wait_for_sample_content(&mut grpc).await;
    drop(grpc);
    assert!(content.contains(SAMPLE_MARKER), "buffer should contain sample counter marker");

    handle.stop().await;
}
