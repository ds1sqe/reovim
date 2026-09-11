# Server and CLI scenarios

Build the server and CLI, then use the same entry point for all scenarios:

```sh
python3 scripts/e2e.py
python3 scripts/e2e.py --list
python3 scripts/e2e.py --scenario native_commands
python3 scripts/e2e.py --scenario custom_module --scenario native_commands
python3 scripts/e2e.py path/to/binaries --artifacts tmp/e2e
```

Each test starts a fresh server on an assigned loopback port. Scenarios build
native modules after startup, use real CLI processes, and release their owned
work. Server processes and temporary build files are cleaned up on failure too.
The runner prints the retained evidence directory: each test has raw command,
stdout and stderr logs. Failures name the test and exit nonzero.

Add a `test_*.py` file with a `Scenario` subclass and ordinary `test_*` methods.
The standard Python `unittest` loader discovers it; no registry changes or
third-party test dependencies are needed. Own each test's prerequisites so
scenario selection and execution order cannot change its result.

```python
from .fixture import Scenario


class Example(Scenario):
    def test_custom_command(self):
        library = self.build_module("reovim-module-example", "example")
        payload = (str(library) + "\0formatter").encode().hex()
        self.rpc("write", "kernel/modules/load", "host.module.load", "1",
                 payload)
        self.assertEqual(self.cli("/formatter", "Ada"), "Hello, Ada!\n")
```

`call` returns decoded Message fields (`contract`, `version`, `data`,
`resources`). Use `cli` for terminal output or `cli(input="...\n")` for a
shell session. `build_c` and `build_module` build native artifacts; `rpc`
provides public service access during bootstrap. The older scenario helpers
and journeys are still being migrated to the loaded-command syntax.
Keep semantic assertions in the scenario that owns them. Add shared helpers
only when another real scenario needs them; keep command dispatch and wire
changes in the fixture as the generic caller evolves.
