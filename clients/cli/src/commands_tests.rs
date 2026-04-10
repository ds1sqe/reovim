use {
    super::*,
    reovim_driver_module_registry::{InstalledModules, ModuleInfo as RegistryModuleInfo},
    reovim_protocol::v2::{ListModulesResponse, ModuleInfo},
    tempfile::TempDir,
};

fn sample_installed_module() -> InstalledModule {
    InstalledModule {
        id: "sample".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path("/tmp/sample"),
        install_path: std::path::PathBuf::from("/tmp/sample"),
        library_path: Some(std::path::PathBuf::from("/tmp/sample/libsample.so")),
    }
}

fn sample_registry_info() -> RegistryModuleInfo {
    RegistryModuleInfo {
        id: "sample".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path("/tmp/sample"),
        install_path: std::path::PathBuf::from("/tmp/sample"),
        library_exists: true,
        provides: vec!["cap-a".to_string()],
        requires: vec!["cap-b".to_string()],
    }
}

fn temp_registry_paths() -> (TempDir, RegistryPaths) {
    let temp = tempfile::tempdir().expect("tempdir should create");
    let paths = RegistryPaths::new(temp.path().join("reovim/modules"));
    (temp, paths)
}

fn save_installed(paths: &RegistryPaths, modules: Vec<InstalledModule>) {
    std::fs::create_dir_all(&paths.modules_dir).expect("registry dir should exist");
    let mut installed = InstalledModules::new();
    for module in modules {
        installed.insert(module);
    }
    installed
        .save(&paths.installed_json)
        .expect("installed.json should save");
}

#[test]
fn test_output_format_eq() {
    assert_eq!(OutputFormat::Plain, OutputFormat::Plain);
    assert_eq!(OutputFormat::Json, OutputFormat::Json);
    assert_ne!(OutputFormat::Plain, OutputFormat::Json);
}

#[test]
fn test_parse_module_source_git_https() {
    let source = parse_module_source("https://github.com/user/repo.git", None);
    assert!(source.is_git());
    assert!(!source.is_path());
}

#[test]
fn test_parse_module_source_git_with_rev() {
    let source = parse_module_source("git@github.com:user/repo.git", Some("v1.2.3"));
    match source {
        ModuleSource::Git { url, rev } => {
            assert_eq!(url, "git@github.com:user/repo.git");
            assert_eq!(rev.as_deref(), Some("v1.2.3"));
        }
        ModuleSource::Path { .. } => panic!("expected git source"),
    }
}

#[test]
fn test_parse_module_source_local_path() {
    let source = parse_module_source("./my-module", None);
    assert!(source.is_path());
    assert!(!source.is_git());
}

#[test]
fn test_collect_loaded_module_ids_uses_stable_id() {
    let response = ListModulesResponse {
        modules: vec![ModuleInfo {
            id: "sample".to_string(),
            name: "Human Readable Name".to_string(),
            version: "1.0.0".to_string(),
            path: "/tmp/sample.so".to_string(),
            loaded: true,
        }],
    };

    let ids = collect_loaded_module_ids(&response);
    assert!(ids.contains("sample"));
    assert!(!ids.contains("Human Readable Name"));
}

#[test]
fn test_format_module_list_plain_empty() {
    let result = format_module_list(&[], &HashSet::new(), OutputFormat::Plain);
    assert!(result.contains("No modules installed"));
}

#[test]
fn test_format_module_list_json_structure_and_loaded_flag() {
    let module = sample_installed_module();
    let loaded_ids = HashSet::from([module.id.clone()]);

    let result = format_module_list(&[module], &loaded_ids, OutputFormat::Json);
    let json: serde_json::Value = serde_json::from_str(&result).expect("json should parse");

    assert!(json["modules"].is_array());
    assert_eq!(json["modules"][0]["id"], "sample");
    assert_eq!(json["modules"][0]["loaded"], true);
}

#[test]
fn test_format_install_result_json() {
    let json = format_install_result(&sample_installed_module(), OutputFormat::Json);
    let value: serde_json::Value = serde_json::from_str(&json).expect("json should parse");
    assert_eq!(value["id"], "sample");
    assert_eq!(value["version"], "1.0.0");
}

#[test]
fn test_format_remove_result_json() {
    let json = format_remove_result("sample", OutputFormat::Json);
    let value: serde_json::Value = serde_json::from_str(&json).expect("json should parse");
    assert_eq!(value["id"], "sample");
    assert_eq!(value["removed"], true);
}

#[test]
fn test_format_update_result_json() {
    let json = format_update_result(&sample_installed_module(), OutputFormat::Json);
    let value: serde_json::Value = serde_json::from_str(&json).expect("json should parse");
    assert_eq!(value["id"], "sample");
    assert_eq!(value["version"], "1.0.0");
}

#[test]
fn test_format_check_report_json_structure() {
    let mut report = CheckReport::default();
    report
        .broken
        .push(("sample".to_string(), "missing library".to_string()));
    let json = format_check_report(&report, OutputFormat::Json);
    let value: serde_json::Value = serde_json::from_str(&json).expect("json should parse");
    assert_eq!(value["clean"], false);
    assert_eq!(value["broken"][0]["id"], "sample");
}

#[test]
fn test_format_update_all_results_plain_mixed() {
    let results = vec![
        Ok(sample_installed_module()),
        Err(("broken".to_string(), "build failed".to_string())),
    ];

    let output = format_update_all_results(&results, OutputFormat::Plain);
    assert!(output.contains("Updated sample to v1.0.0"));
    assert!(output.contains("Failed broken: build failed"));
}

#[test]
fn test_format_module_info_plain() {
    let output = format_module_info(&sample_registry_info(), OutputFormat::Plain);
    assert!(output.contains("ID:        sample"));
    assert!(output.contains("Provides:  cap-a"));
    assert!(output.contains("Requires:  cap-b"));
}

#[test]
fn test_format_check_report_clean() {
    let report = CheckReport::default();
    let output = format_check_report(&report, OutputFormat::Plain);
    assert!(output.contains("OK"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_with_paths_list_loaded_requires_client() {
    let (_temp, paths) = temp_registry_paths();
    let result = module_with_paths(
        None,
        &ModuleSubcommand::List { loaded: true },
        OutputFormat::Plain,
        &paths,
    )
    .await;

    assert!(matches!(result, Err(GrpcClientError::ConnectionFailed(_))));
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_with_paths_list_local_reads_registry_without_grpc() {
    let (_temp, paths) = temp_registry_paths();
    save_installed(&paths, vec![sample_installed_module()]);

    let output = module_with_paths(
        None,
        &ModuleSubcommand::List { loaded: false },
        OutputFormat::Plain,
        &paths,
    )
    .await
    .expect("local list should succeed");

    assert!(output.contains("sample v1.0.0"));
    assert!(!output.contains("[loaded]"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_with_paths_missing_info_maps_to_operation_failed() {
    let (_temp, paths) = temp_registry_paths();
    let result = module_with_paths(
        None,
        &ModuleSubcommand::Info {
            id: "missing".to_string(),
        },
        OutputFormat::Plain,
        &paths,
    )
    .await;

    assert!(matches!(result, Err(GrpcClientError::OperationFailed(_))));
    assert!(!matches!(result, Err(GrpcClientError::InvalidArgument(_))));
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_with_paths_remove_updates_registry() {
    let (_temp, paths) = temp_registry_paths();
    save_installed(&paths, vec![sample_installed_module()]);

    let output = module_with_paths(
        None,
        &ModuleSubcommand::Remove {
            id: "sample".to_string(),
        },
        OutputFormat::Plain,
        &paths,
    )
    .await
    .expect("remove should succeed");

    assert!(output.contains("Removed sample"));
    let remaining = workflow::list(&paths).expect("list should succeed");
    assert!(remaining.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_with_paths_check_reports_broken_library() {
    let (_temp, paths) = temp_registry_paths();
    let mut module = sample_installed_module();
    module.library_path = Some(paths.modules_dir.join("missing.so"));
    save_installed(&paths, vec![module]);

    let output = module_with_paths(None, &ModuleSubcommand::Check, OutputFormat::Plain, &paths)
        .await
        .expect("check should succeed");

    assert!(output.contains("BROKEN: sample"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_with_paths_update_missing_maps_to_operation_failed() {
    let (_temp, paths) = temp_registry_paths();
    let result = module_with_paths(
        None,
        &ModuleSubcommand::Update {
            id: Some("missing".to_string()),
        },
        OutputFormat::Plain,
        &paths,
    )
    .await;

    assert!(matches!(result, Err(GrpcClientError::OperationFailed(_))));
}
