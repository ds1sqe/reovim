import re
import socket
import time

from .fixture import Scenario


def events(data):
    pattern = r"(\d+) request=(\d+) parent=(?:Some\((\d+)\)|None) (\w+)"
    return [(int(sequence), int(request),
             int(parent) if parent else None, stage)
            for sequence, request, parent, stage in re.findall(pattern, data)]


class NativeForward(Scenario):
    def test_import_grants_cleanup_and_independent_execution(self):
        counter = self.activate("counter")
        self.call("write", "modules/counter/value", "external.counter", "1",
                  (9).to_bytes(8, "little").hex())
        library = self.build_c("modules/native-forward/forward.c", "forward")
        mount = "native/forward"
        # Exceed the 16 caller slots: failed construction must reclaim imports.
        for _ in range(18):
            self.load(library, mount, imports="kernel/log", success=False)
        module = self.load(library, mount, imports="modules/counter/value")
        baseline = self.call("read", "kernel/log", "kernel.diagnostics", "1")
        cursor = int(baseline.data.decode().split("next=")[-1])
        self.call("write", mount, "native.forward", "1",
                  (4).to_bytes(8, "little").hex())
        # The incomplete frame leaves transport waiting while Runtime runs.
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall(f"v1\t{self.client}\trequests".encode())
            time.sleep(0.05)
            stream.sendall(b"\n")
            self.assertTrue(stream.recv(65536).startswith(b"ok\t"))
        observed = self.call("read", "kernel/log", "kernel.diagnostics", "1",
                             cursor.to_bytes(8, "little").hex())
        self.assert_forwarding_finished_before_observation(
            observed.data.decode())
        forwarded = self.call("read", mount, "native.forward", "1",
                              retry_busy=True)
        self.assertEqual(int.from_bytes(forwarded.data, "little"), 13)
        value = self.call("read", "modules/counter/value",
                          "external.counter", "1")
        self.assertEqual(int.from_bytes(value.data, "little"), 13)
        self.unload(module)
        self.unload(counter)

    def assert_forwarding_finished_before_observation(self, log):
        trace = events(log)
        parents = {request: parent for _, request, parent, stage in trace
                   if stage == "Admitted"}
        leaves = [request for request, parent in parents.items()
                  if parent is not None
                  and request not in parents.values()]
        self.assertTrue(leaves, log)
        observation = max(sequence for sequence, _, parent, stage in trace
                          if parent is None and stage == "Admitted")
        finished = {request: sequence for sequence, request, _, stage in trace
                    if stage == "Finished"}
        for request in leaves:
            self.assertIn(request, finished, log)
            self.assertLess(finished[request], observation, log)
