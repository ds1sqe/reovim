from .fixture import Reply, Scenario


class ClientScopes(Scenario):
    def test_retained_clients_have_independent_module_instances(self):
        second = self.new_client()
        library = self.build_c("modules/native-counter/counter.c", "counter")
        payload = (str(library) + "\0state").encode().hex()
        forbidden = (str(library) + "\0modules/injected").encode().hex()
        self.cli("--contract", "host.module.load", "1", "--hex", forbidden,
                 "/kernel/modules/load", success=False)
        first_module = self.rpc("write", "kernel/modules/load",
                                "host.module.load", "1", payload)
        second_module = self.rpc("write", "kernel/modules/load",
                                 "host.module.load", "1", payload,
                                 client=second)
        changed = Reply.parse(self.cli("--contract", "native.counter", "1",
                                       "--hex", "07", "/state"))
        self.assertEqual(int.from_bytes(changed.data, "little"), 7)
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "state", "native.counter", "1", client=second).data,
            "little"), 0)
        self.cli("--contract", "kernel.module.unload", "1",
                 first_module.resources[0], client=second, success=False)
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "state", "native.counter", "1").data, "little"), 7)
        changed = Reply.parse(self.cli("--contract", "native.counter", "1",
                                       "--hex", "03", "/state", client=second))
        self.assertEqual(int.from_bytes(changed.data, "little"), 3)

        shared = self.rpc("write", "modules/catalog/counter",
                          "kernel.module.activate", "1")
        self.rpc("write", "modules/counter/value", "external.counter", "1",
                 (5).to_bytes(8, "little").hex())
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "modules/counter/value", "external.counter", "1",
            client=second).data, "little"), 5)
        self.cli("--contract", "kernel.module.unload", "1",
                 first_module.resources[0])
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "state", "native.counter", "1", client=second).data,
            "little"), 3)
        replacement = self.rpc("write", "kernel/modules/load",
                               "host.module.load", "1", payload)
        self.assertNotEqual(replacement.resources, first_module.resources)
        self.assertEqual(int.from_bytes(self.rpc(
            "read", "state", "native.counter", "1").data, "little"), 0)
        for instance in (replacement.resources[0], shared.resources[0]):
            self.cli("--contract", "kernel.module.unload", "1", instance)
        self.cli("--contract", "kernel.module.unload", "1",
                 second_module.resources[0], client=second)
