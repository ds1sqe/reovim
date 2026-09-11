"""One server per test, bounded commands, and retained process evidence."""

from dataclasses import dataclass
from pathlib import Path
import subprocess
import socket
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class Reply:
    contract: str
    version: int
    data: bytes
    resources: tuple[str, ...]

    @classmethod
    def parse(cls, output):
        fields = output.removesuffix("\n").split("\t")
        if len(fields) != 5 or fields[0] != "ok":
            raise ValueError(f"expected a complete Message reply: {output!r}")
        _, contract, version, data, resources = fields
        return cls(contract, int(version), bytes.fromhex(data),
                   tuple(resources.split(",")) if resources else ())


class Scenario(unittest.TestCase):
    """Each test owns a fresh server and scratch space."""

    binaries = ROOT / "product/target/debug"
    artifacts = None

    def setUp(self):
        self.evidence = self.artifacts / self.id()
        self.evidence.mkdir(parents=True)
        self.work = Path(self.enterContext(tempfile.TemporaryDirectory(
            prefix="reovim-e2e-work-")))
        self.transcript = self.enterContext(
            (self.evidence / "commands.log").open("w"))
        output = self.enterContext(
            (self.evidence / "server.stdout").open("w"))
        errors = self.enterContext(
            (self.evidence / "server.stderr").open("w"))
        self.server = subprocess.Popen(
            [str(self.binaries / "reovim-server"), "127.0.0.1:0"],
            stdout=output, stderr=errors,
        )
        self.addCleanup(self.stop_server)
        self.address = self.wait_for_address()
        self.client = self.new_client()

    def new_client(self):
        result = self.command([str(self.binaries / "reovim-cli"),
                               self.address, "--connect"])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        return result.stdout.strip()

    def stop_server(self):
        self.stop_process(self.server)

    @staticmethod
    def stop_process(process):
        if process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)

    def wait_for_address(self):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            with (self.evidence / "server.stdout").open() as output:
                line = output.readline(1024)
            if line.endswith("\n"):
                self.assertTrue(line.startswith("listening "), line)
                return line.strip().removeprefix("listening ")
            if self.server.poll() is not None:
                self.fail(f"server exited with {self.server.returncode}")
            time.sleep(0.01)
        self.fail("server did not announce its address within 5 seconds")

    def command(self, argv, *, input=None, timeout=10):
        self.transcript.write(f"$ {argv!r}\n")
        if input is not None:
            self.transcript.write(f"stdin: {input!r}\n")
        self.transcript.flush()
        try:
            result = subprocess.run(argv, input=input, text=True,
                                    capture_output=True, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            self.transcript.write(
                f"timeout after {timeout}s\nstdout: {error.stdout!r}\n"
                f"stderr: {error.stderr!r}\n")
            self.transcript.flush()
            raise
        self.transcript.write(
            f"exit: {result.returncode}\nstdout:\n{result.stdout}"
            f"\nstderr:\n{result.stderr}\n")
        self.transcript.flush()
        return result

    def cli(self, *args, input=None, success=True, client=None):
        result = self.command(
            [str(self.binaries / "reovim-cli"), self.address, "--client",
             client or self.client, *args],
            input=input,
        )
        self.assertEqual(result.returncode == 0, success,
                         result.stdout + result.stderr)
        if success:
            self.assertEqual(result.stderr, "", result.stderr)
        return result.stdout

    def call(self, *args, success=True, retry_busy=False):
        self.assertTrue(not retry_busy or args[0] == "read",
                        "only passive reads may retry Busy")
        for _ in range(16 if retry_busy else 1):
            request = self.wire("reserve").split("\t")[1]
            output = self.wire("call", request, *args)
            self.assertEqual(self.wire("release", request), "ok")
            if not retry_busy or not output.startswith("error\tfailed\t8\t"):
                break
        self.assertEqual(output.startswith("ok\t"), success, output)
        return Reply.parse(output) if success else None

    def load_command(self, name, mount, *, imports=""):
        library = self.build_module("reovim-module-" + name, name)
        return self.load(library, mount, imports=imports)

    def build_c(self, source, name):
        library = self.work / f"{name}.so"
        result = self.command([
            "cc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            "-I", str(ROOT / "host/reference/include"),
            str(ROOT / source), "-o", str(library),
        ], timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return library

    def build_module(self, package, example):
        target = self.work / package
        result = self.command([
            "cargo", "build", "--offline", "--locked", "--manifest-path",
            str(ROOT / "product/Cargo.toml"), "-p", package,
            "--example", example, "--target-dir", str(target),
        ], timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return target / f"debug/examples/lib{example}.so"

    def read_final(self, reader, timeout=5):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            frame = reader.readline(65537)
            if frame != "pending\n":
                return frame
        self.fail("response did not finish within its scenario deadline")

    def wire(self, *fields, client=None, timeout=5):
        """Inspect a Resource through the public service during bootstrap."""
        host, port = self.address.rsplit(":", 1)
        frame = "v1\t" + (client or self.client) + "\t" + "\t".join(fields) + "\n"
        with socket.create_connection((host, int(port)), timeout=timeout) as stream:
            stream.sendall(frame.encode())
            with stream.makefile("r") as reader:
                reply = self.read_final(reader, timeout)
        self.transcript.write(f"RPC {frame!r}\n{reply}")
        self.transcript.flush()
        self.assertTrue(reply.endswith("\n"), "incomplete service reply")
        return reply.rstrip("\n")

    def rpc(self, *fields, client=None):
        reserved = self.wire("reserve", client=client)
        self.assertTrue(reserved.startswith("ok\t"), reserved)
        request = reserved.split("\t")[1]
        output = self.wire("call", request, *fields, client=client)
        self.assertEqual(self.wire("release", request, client=client), "ok")
        return Reply.parse(output)

    def load(self, library, mount, *, imports=None, success=True):
        payload = (str(library) + "\0" + mount).encode().hex()
        contract = "host.module.load" if imports is None else (
            "host.module.load-with")
        references = () if imports is None else (imports,)
        result = self.call("write", "kernel/modules/load", contract, "1",
                           payload, *references, success=success)
        if result:
            self.assertEqual(result.data, b"ready")
        return result.resources[0] if result else None

    def activate(self, name):
        reply = self.call("write", f"modules/catalog/{name}",
                          "kernel.module.activate", "1")
        return reply.resources[0]

    def unload(self, instance, *, success=True):
        self.call("write", instance, "kernel.module.unload", "1",
                  success=success)
