from .fixture import Scenario


class EnvNavigation(Scenario):
    def test_private_cwd_with_a_shared_session_and_absolute_repair(self):
        second = self.new_client()
        installed = {self.client: [], second: []}
        for name in ("env", "cd", "pwd", "ls"):
            library = self.build_module("reovim-module-" + name, name)
            mount = "" if name == "env" else name
            payload = (str(library) + "\0" + mount).encode().hex()
            for client in installed:
                result = self.rpc("write", "kernel/modules/load",
                                  "host.module.load-with", "1", payload, "",
                                  client=client)
                self.assertEqual(result.data, b"ready")
                installed[client].append(result.resources[0])
        created = self.rpc("write", "module/env/create", "env.create", "1",
                           b"MY_OPTION\0first".hex())
        peer_value = self.rpc("write", "module/env/create", "env.create", "1",
                              b"MY_OPTION\0second".hex(), client=second)
        self.assertEqual(self.rpc("read", "env/MY_OPTION", "resource.bytes", "1").data,
                         b"first")
        self.assertEqual(self.rpc("read", peer_value.resources[0], "resource.bytes", "1",
                                  client=second).data, b"second")
        self.rpc("write", created.resources[0], "resource.bytes", "1", b"changed".hex())
        self.assertEqual(self.rpc("read", "env/MY_OPTION", "resource.bytes", "1").data,
                         b"changed")
        for name in ("create", "info", "watch"):
            self.rpc("write", "module/env/create", "env.create", "1",
                     (name + "\0").encode().hex())
            self.assertEqual(self.rpc("read", "env/" + name, "resource.bytes", "1").data,
                             b"")
        self.cli("/module/env/remove", "MY_OPTION")
        self.cli("--contract", "resource.bytes", "1", "--hex", "00",
                 created.resources[0], success=False)
        replaced = self.rpc("write", "module/env/create", "env.create", "1",
                            b"MY_OPTION\0replacement".hex())
        self.assertNotEqual(replaced.resources, created.resources)
        self.cli("--contract", "resource.bytes", "1", "--hex", "ff",
                 created.resources[0], success=False)
        self.assertEqual(self.rpc("read", "env/MY_OPTION", "resource.bytes", "1").data,
                         b"replacement")
        self.assertEqual(self.rpc("read", "env/MY_OPTION", "resource.bytes", "1",
                                  client=second).data, b"second")
        self.cli("/cd", "/modules/catalog")
        self.cli("/cd", "/models", client=second)
        self.assertEqual(self.cli("/pwd"), "/modules/catalog\n")
        self.assertEqual(self.cli("/pwd", client=second), "/models\n")
        self.assertIn("counter\n", self.cli("/ls"))
        self.assertIn("session\n", self.cli("/ls", client=second))

        first = self.rpc("read", "session", "heading.source", "1")
        peer = self.rpc("read", "session", "heading.source", "1", client=second)
        self.assertEqual(first.data, peer.data)
        updated = self.rpc("write", "session", "heading.replace", "1",
                           b"Together".hex(), ",".join(first.resources))
        observed = self.rpc("read", "session", "heading.source", "1",
                            client=second)
        self.assertEqual(observed.data, b"# Together\n")
        self.assertEqual(updated.data, observed.data)
        self.assertEqual(self.cli("/pwd"), "/modules/catalog\n")
        self.assertEqual(self.cli("/pwd", client=second), "/models\n")
        for client, snapshots in ((self.client, (first, updated)),
                                  (second, (peer, observed))):
            for snapshot in snapshots:
                self.rpc("write", snapshot.resources[1],
                         "kernel.session.release", "1", client=client)

        self.cli("--contract", "resource.bytes", "1", "--hex", "ff", "/env/CWD")
        self.cli("pwd", success=False)
        self.cli("/cd", "/")
        self.assertEqual(self.cli("/pwd"), "/\n")
        self.cli("/env/CWD", "modules")
        self.assertEqual(self.cli("/pwd"), "/modules\n")
        self.cli("/env/CWD")  # Empty value is a complete write.
        self.assertEqual(self.rpc("read", "env/CWD", "resource.bytes", "1").data,
                         b"")
        self.assertEqual(self.cli("/pwd", client=second), "/models\n")
        for client, instances in installed.items():
            for instance in reversed(instances):
                self.cli("--contract", "kernel.module.unload", "1", instance,
                         client=client)
