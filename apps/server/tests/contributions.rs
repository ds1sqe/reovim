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
    let mut runtime =
        Runtime::new(111, Limits::default(), reovim_kernel_core::create)
            .unwrap();
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
fn contribution_paths_cannot_be_each_others_parents_in_either_order() {
    for paths in [["module", "module/parse"], ["module/parse", "module"]] {
        let mut runtime =
            Runtime::new(112, Limits::default(), reovim_kernel_core::create)
                .unwrap();
        let values = paths
            .map(|path| Contribution::Resource {
                path: path.into(),
                provider: reovim_resource_local::memory(b"kept"),
            })
            .into();
        let Err((error, _)) = runtime.register_contributions(values) else {
            panic!("a provider cannot also be a directory");
        };
        assert_eq!(error, Error::INVALID);
        assert_eq!(runtime.resource("module"), Err(Error::UNAVAILABLE));
    }
}

#[test]
fn batch_capacity_counts_shared_directories_once() {
    let limits = Limits { resources: 7, ..Limits::default() };
    let mut runtime =
        Runtime::new(113, limits, reovim_kernel_core::create).unwrap();
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
    let mut runtime =
        Runtime::new(114, limits, reovim_kernel_core::create).unwrap();
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
    let mut retry =
        Runtime::new(115, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    retry.register_contributions(values).map_err(|(e, _)| e).unwrap();
    assert!(retry.resource("first").is_ok());
    assert!(retry.resource("second").is_ok());
    retry.close().unwrap();
}
