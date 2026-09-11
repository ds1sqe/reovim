"""Process configuration selects native contracts and private client instances."""

import shutil
import socket

from .fixture import ROOT, Scenario


class FailedClientBoot(Scenario):
    def boot_arguments(self):
        return ["--client-start", "missing", "1"]

    def new_client(self):
        host, port = self.address.rsplit(":", 1)
        with socket.create_connection((host, int(port)), timeout=5) as stream:
            stream.sendall(b"v1\topen\n")
            with stream.makefile("r") as reader:
                reply = self.read_final(reader).rstrip("\n").split("\t")
        self.assertEqual(reply[:2], ["error", "boot"])
        self.assertEqual(reply[4], "failed")
        self.receipt = reply[3]
        return reply[2]

    def test_failed_startup_keeps_receipt_and_cleanup_owner(self):
        self.assertEqual(self.wire("status", self.receipt), "ok\tfinished")
        self.assertTrue(self.wire("result", self.receipt).startswith("error\tboot\t"))
        self.assertIn("client busy", self.wire("disconnect"))
        self.assertEqual(self.wire("release", self.receipt), "ok")
        self.disconnect(self.client)


class Boot(Scenario):
    def boot_arguments(self):
        env = self.build_module("reovim-module-env", "env")
        return [*super().boot_arguments(),
                "--client-module", "environment", str(env), "",
                "--grant", "", "all", "--client-start", "env", "1"]

    def test_retained_client_capacity_includes_private_boot_scopes(self):
        clients = [self.client, *(self.new_client() for _ in range(15))]
        for index, client in enumerate(clients):
            self.rpc("write", "env/CWD", "resource.bytes", "1",
                     str(index).encode().hex(), client=client)
        for index, client in enumerate(clients):
            self.assertEqual(self.rpc("read", "env/CWD", "resource.bytes", "1",
                                      client=client).data, str(index).encode())
        result = self.command([str(self.binaries / "reovim-cli"),
                               self.address, "--connect"])
        self.assertNotEqual(result.returncode, 0)
        for client in clients:
            self.disconnect(client)

    def test_startup_env_is_private_and_teardown_releases_catalogs(self):
        second = self.new_client()
        self.assertEqual(self.cli("--requests"), "ok\t\n")
        self.rpc("write", "module/env/create", "env.create", "1",
                 b"custom\0first".hex())
        self.rpc("write", "module/env/create", "env.create", "1",
                 b"custom\0second".hex(), client=second)
        self.assertEqual(self.rpc("read", "env/custom", "resource.bytes", "1").data,
                         b"first")
        self.assertEqual(self.rpc("read", "env/custom", "resource.bytes", "1",
                                  client=second).data, b"second")
        self.disconnect(self.client)
        self.assertEqual(self.rpc("read", "env/custom", "resource.bytes", "1",
                                  client=second).data, b"second")
        self.disconnect(second)
        for _ in range(18):
            client = self.new_client()
            self.assertEqual(self.rpc("read", "env/CWD", "resource.bytes", "1",
                                      client=client).data, b"")
            self.disconnect(client)


class InstalledBoot(Scenario):
    def boot_arguments(self):
        # Installation supplies executables and the public header. The extension
        # is compiled later from local C source, with no checkout include paths.
        installed = self.work / "installed"
        installed.mkdir()
        for name in ("reovim-server", "reovim-cli"):
            shutil.copy2(self.binaries / name, installed / name)
        self.binaries = installed
        shutil.copy2(ROOT / "host/reference/include/reovim.h", installed)
        shutil.copy2(ROOT / "apps/server/tests/native/metadata.c",
                     installed / "extension.c")
        library = installed / "extension.so"
        result = self.command([
            "cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared",
            "-fPIC", "-DLEAF", "-I", str(installed),
            str(installed / "extension.c"), "-o", str(library),
        ])
        self.assertEqual(result.returncode, 0, result.stderr)
        return ["--module", "extension", str(library), "custom",
                "--start", "example.feature", "1"]

    def test_installed_server_admits_independently_compiled_extension(self):
        result = self.rpc("read", "modules/custom", "example.constructions", "1")
        self.assertEqual(int.from_bytes(result.data, "little"), 1)
        instance = self.rpc("write", "modules/catalog/extension",
                            "kernel.module.activate", "1")
        self.assertEqual(self.rpc("read", "modules/custom",
                                  "example.constructions", "1").data, result.data)
        self.unload(instance.resources[0])
        self.cli("--disconnect")

    def test_missing_incompatible_and_cyclic_boot_fail_before_listening(self):
        library = self.binaries / "extension.so"
        declaration = ["--module", "extension", str(library), "custom"]
        for contract, schema in (("absent", "1"), ("example.feature", "2")):
            result = self.command([str(self.binaries / "reovim-server"),
                                   "127.0.0.1:0", *declaration,
                                   "--start", contract, schema])
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("listening", result.stdout)
        cycle = self.binaries / "cycle.so"
        result = self.command([
            "cc", "-std=c11", "-shared", "-fPIC",
            '-DREQUIRED_FIRST="example.feature"', "-I", str(self.binaries),
            str(self.binaries / "extension.c"), "-o", str(cycle),
        ])
        self.assertEqual(result.returncode, 0, result.stderr)
        result = self.command([
            str(self.binaries / "reovim-server"), "127.0.0.1:0",
            "--module", "cycle", str(cycle), "cycle",
            "--start", "example.feature", "1",
        ])
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("listening", result.stdout)
        result = self.command([
            str(self.binaries / "reovim-server"), "127.0.0.1:0",
            *declaration, "--module", "duplicate", str(library), "second",
            "--start", "example.feature", "1",
        ])
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("listening", result.stdout)


class BootDependencies(Scenario):
    def boot_arguments(self):
        args = []
        for name in ("feature", "storage", "index"):
            library = self.work / f"{name}.so"
            result = self.command([
                "cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared",
                "-fPIC", "-I", str(ROOT / "host/reference/include"),
                f'-DMODULE_NAME="example.{name}"',
                "-DREQUIRE_STORAGE" if name == "feature" else "-DLEAF",
                str(ROOT / "apps/server/tests/native/metadata.c"),
                "-o", str(library),
            ])
            self.assertEqual(result.returncode, 0, result.stderr)
            args.extend(["--module", name, str(library), f"services/{name}",
                         "--grant", "", "all"])
        return [*args, "--start", "example.feature", "1"]

    def test_dependency_publication_precedes_dependent_construction(self):
        for name in ("feature", "storage", "index"):
            reply = self.rpc("read", f"modules/services/{name}",
                             "example.constructions", "1")
            self.assertEqual(int.from_bytes(reply.data, "little"), 1)
        feature = self.activate("feature")
        storage = self.activate("storage")
        self.unload(storage, success=False)
        self.unload(feature)
        self.unload(storage)
