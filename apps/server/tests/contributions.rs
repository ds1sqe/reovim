use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};

fn contributions() -> Vec<Contribution> {
    vec![
        Contribution::Resource {
            path: "resource/notes".into(),
            provider: reovim_resource_local::memory(b"# a\n\xffkept"),
        },
        Contribution::Model {
            path: "models/heading".into(),
            provider: reovim_representation_basic::heading(),
        },
    ]
}

#[test]
fn rejected_module_contributions_remain_owned_and_retry_together() {
    let mut runtime = Runtime::new(111, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    let mut values = contributions();
    let Contribution::Model { provider, .. } = &mut values[1] else {
        panic!("expected model");
    };
    provider.edit = Contract::new("kernel.buffer.close", 1);
    let Err((error, mut values)) = runtime.register_contributions(values)
    else {
        panic!("invalid model must reject the whole publication");
    };
    assert_eq!(error, Error::INVALID);
    assert_eq!(runtime.resource("resource"), Err(Error::UNAVAILABLE));
    assert_eq!(runtime.resource("models"), Err(Error::UNAVAILABLE));
    assert_eq!(
        client.resolve("resource/notes").into_inner(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    let Contribution::Model { provider, .. } = &mut values[1] else {
        panic!("rejection must retain the model");
    };
    provider.edit = Contract::new("heading.replace", 1);
    runtime.register_contributions(values).map_err(|(e, _)| e).unwrap();
    let result = run(reovim_server::client::heading(&client), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap()
    .unwrap();
    assert_eq!(result.value.data.as_ref(), b"# b\n\xffkept");
    runtime.close().unwrap();
}

#[test]
fn callable_parents_keep_children_in_either_publication_order() {
    for paths in [["module", "module/info"], ["module/info", "module"]] {
        let mut runtime = Runtime::new(112, Limits::default()).unwrap();
        runtime.start().unwrap();
        let values = paths
            .map(|path| Contribution::Resource {
                path: path.into(),
                provider: reovim_resource_local::memory(path.as_bytes()),
            })
            .into();
        let installed = runtime
            .register_contributions(values)
            .map_err(|(error, _)| error)
            .unwrap();
        let root = runtime.resource("").unwrap();
        let client = runtime.connect(&[(root, Permissions::READ)]).unwrap();
        let parent = client.resolve("module").into_inner().unwrap();
        let info = client.resolve_at(parent, "info").into_inner().unwrap();
        let entries = client.children(parent).into_inner().unwrap();
        assert!(entries.iter().any(|entry| {
            entry.name.as_ref() == "info" && entry.resource == info
        }));
        for (subject, value) in [(parent, "module"), (info, "module/info")] {
            let reply = run(
                client.read(
                    subject,
                    &Message::new(Contract::new("resource.bytes", 1), &[]),
                ),
                |context| runtime.advance(context).map(|_| ()),
            )
            .unwrap()
            .unwrap();
            assert_eq!(reply.data.as_ref(), value.as_bytes());
        }
        runtime.unregister_contributions(&installed).unwrap();
        assert_eq!(runtime.resource("module"), Err(Error::UNAVAILABLE));
        assert_eq!(runtime.resource("module/info"), Err(Error::UNAVAILABLE));
        runtime.close().unwrap();
    }
}

#[test]
fn child_grant_allows_traversal_without_calling_its_parent() {
    let mut runtime = Runtime::new(116, Limits::default()).unwrap();
    runtime.start().unwrap();
    let parent = runtime
        .register("module", reovim_resource_local::memory(b"private"))
        .map_err(|(error, _)| error)
        .unwrap();
    let child = runtime
        .register("module/info", reovim_resource_local::memory(b"help"))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(child, Permissions::READ)]).unwrap();
    let info = client.resolve("module/info").into_inner().unwrap();
    let input = Message::new(Contract::new("resource.bytes", 1), &[]);
    let reply = run(client.read(info, &input), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"help");
    let subject = client.resolve("module").into_inner().unwrap();
    let denied = run(client.read(subject, &input), |_| Ok(())).unwrap();
    assert_eq!(denied, Err(RequestError::Rejected(Error::DENIED)));
    assert_eq!(runtime.unregister(parent), Err(Error::BUSY));
    assert_eq!(runtime.resource("module/info"), Ok(child));
    runtime.unregister_contributions(&[parent, child]).unwrap();
    runtime.close().unwrap();
}

#[test]
fn grouped_removal_retires_implicit_descendants_and_preserves_other_owners() {
    let mut runtime = Runtime::new(117, Limits::default()).unwrap();
    let values =
        ["module", "module/nested/info"].map(|path| Contribution::Resource {
            path: path.into(),
            provider: reovim_resource_local::memory(b"owned"),
        });
    let installed = runtime
        .register_contributions(values.into())
        .map_err(|(error, _)| error)
        .unwrap();
    let foreign = runtime
        .register("module/other", reovim_resource_local::memory(b"other"))
        .map_err(|(error, _)| error)
        .unwrap();
    assert_eq!(runtime.unregister_contributions(&installed), Err(Error::BUSY));
    assert_eq!(runtime.resource("module/other"), Ok(foreign));
    assert_eq!(runtime.resource("module"), Ok(installed[0]));
    runtime.unregister(foreign).unwrap();
    runtime.unregister_contributions(&installed).unwrap();
    assert_eq!(runtime.resource("module/nested"), Err(Error::UNAVAILABLE));
    let replacement = runtime
        .register("module", reovim_resource_local::memory(b"replacement"))
        .map_err(|(error, _)| error)
        .unwrap();
    assert_ne!(replacement, installed[0]);
    runtime.close().unwrap();
}

#[test]
fn duplicate_exports_reject_the_whole_batch() {
    let mut runtime = Runtime::new(118, Limits::default()).unwrap();
    let values = ["module/info", "module", "module"].map(|path| {
        Contribution::Resource {
            path: path.into(),
            provider: reovim_resource_local::memory(b"unpublished"),
        }
    });
    let Err((error, _)) = runtime.register_contributions(values.into()) else {
        panic!("duplicate exports must reject atomically");
    };
    assert_eq!(error, Error::INVALID);
    assert_eq!(runtime.resource("module"), Err(Error::UNAVAILABLE));
    runtime.close().unwrap();
}

#[test]
fn batch_capacity_counts_shared_directories_once() {
    let limits = Limits { resources: 7, ..Limits::default() };
    let mut runtime = Runtime::new(113, limits).unwrap();
    runtime.start().unwrap();
    let values = ["module/a", "module/b"].map(|path| Contribution::Resource {
        path: path.into(),
        provider: reovim_resource_local::memory(path.as_bytes()),
    });
    let installed = runtime
        .register_contributions(values.into())
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(installed[0], Permissions::READ)]).unwrap();
    let subject = client.resolve("module/a").into_inner().unwrap();
    let reply = run(
        client.read(
            subject,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"module/a");
    runtime.close().unwrap();
}

#[test]
fn capacity_failure_does_not_publish_an_earlier_contribution() {
    // Three built-in directories leave room for only one of two exports.
    let limits = Limits { resources: 4, ..Limits::default() };
    let mut runtime = Runtime::new(114, limits).unwrap();
    let values = ["first", "second"].map(|path| Contribution::Resource {
        path: path.into(),
        provider: reovim_resource_local::memory(path.as_bytes()),
    });
    let Err((error, values)) = runtime.register_contributions(values.into())
    else {
        panic!("capacity must be reserved for the complete publication");
    };
    assert_eq!(error, Error::PRESSURE);
    assert_eq!(runtime.resource("first"), Err(Error::UNAVAILABLE));
    assert_eq!(runtime.resource("second"), Err(Error::UNAVAILABLE));
    let mut retry = Runtime::new(115, Limits::default()).unwrap();
    retry.register_contributions(values).map_err(|(e, _)| e).unwrap();
    assert!(retry.resource("first").is_ok());
    assert!(retry.resource("second").is_ok());
    retry.close().unwrap();
}
