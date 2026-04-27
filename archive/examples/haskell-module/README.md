# Haskell Module Example

This directory contains an example reovim module written in Haskell, demonstrating how to use the FFI bindings to extend reovim.

## Prerequisites

- **GHC 9.4+** with the `base` and `bytestring` packages
- **cabal-install 3.0+**
- **reovim** with FFI support (requires #290)

### Installing GHC

```bash
# Using ghcup (recommended)
curl --proto '=https' --tlsv1.2 -sSf https://get-ghcup.haskell.org | sh
ghcup install ghc 9.4
ghcup set ghc 9.4

# Verify installation
ghc --version
cabal --version
```

## Building

```bash
# Build the shared library
make

# Or using cabal directly
cabal build all
```

## Testing

```bash
# Run FFI safety tests
make test

# Verify exported symbols
make check
```

## Project Structure

```
examples/haskell-module/
  Reovim/
    FFI.hs              # Re-exports all FFI modules
    FFI/
      Types.hs          # ReovimVersion, ReovimModuleProbe (Storable)
      Version.hs        # Version constants (ABI 1.0.0, API 0.2.0)
      Probe.hs          # ModuleProbe builder pattern
      Logging.hs        # reovim_log_* foreign imports
    Module.hs           # ReovimModule typeclass, exception-safe wrappers
  ExampleModule.hs      # Working example with all entry points
  TestFFISafety.hs      # FFI safety tests
  example-module.cabal  # Package definition
  Makefile              # Build instructions
  README.md             # This file
```

## Writing Your Own Module

### 1. Define Module State

```haskell
data MyModuleState = MyModuleState
    { stateConfig :: IORef Config
    , stateCache :: IORef (Map String Value)
    }
```

### 2. Create Module Probe

```haskell
import Reovim.FFI

myProbe :: ReovimModuleProbe
myProbe = buildProbe $
    newProbe
        & withId "my-module"
        & withName "My Haskell Module"
        & withVersion 1 0 0
  where
    (&) = flip ($)
```

### 3. Implement ReovimModule Typeclass

```haskell
import Reovim.Module

instance ReovimModule MyModuleState where
    moduleProbe _ = myProbe

    moduleInit state _ctx = do
        logInfo "My module initializing"
        -- Initialize your state here
        return returnSuccess

    moduleExit state = do
        logInfo "My module exiting"
        -- Cleanup here
        return returnSuccess
```

### 4. Export FFI Entry Points

```haskell
foreign export ccall "reovim_module_probe"
    hsModuleProbe :: IO ReovimModuleProbe

hsModuleProbe :: IO ReovimModuleProbe
hsModuleProbe = return myProbe

foreign export ccall "reovim_module_entry"
    hsModuleEntry :: IO (Ptr ())

hsModuleEntry :: IO (Ptr ())
hsModuleEntry = do
    state <- newMyModuleState
    newModulePtr state

foreign export ccall "reovim_module_init"
    hsModuleInit :: Ptr () -> Ptr () -> IO Int32

hsModuleInit modulePtr ctxPtr = safeFFI $ do
    result <- withModulePtr modulePtr $ \(state :: MyModuleState) ->
        moduleInit state ctxPtr
    return $ fromMaybe returnFailed result

-- ... similarly for exit and destroy
```

## Entry Points Reference

| Symbol | Signature | Purpose |
|--------|-----------|---------|
| `reovim_module_probe` | `() -> ReovimModuleProbe` | Return module metadata |
| `reovim_module_entry` | `() -> Ptr ()` | Create module instance |
| `reovim_module_init` | `Ptr () -> Ptr () -> Int32` | Initialize module |
| `reovim_module_exit` | `Ptr () -> Int32` | Cleanup module |
| `reovim_module_destroy` | `Ptr () -> ()` | Free module memory |

### Return Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Defer (try again later) |
| `-1` | Failed |
| `-2` | Panic (exception occurred) |

## Exception Handling

**Critical**: Haskell exceptions must never escape to C code. Use the `safeFFI` wrapper:

```haskell
import Reovim.Module (safeFFI, returnPanic)

-- Exceptions are caught and converted to -2
hsModuleInit ptr ctx = safeFFI $ do
    -- Your code here, exceptions are safe
    return returnSuccess
```

## Memory Management

### StablePtr Usage

Module state must be wrapped in `StablePtr` to prevent GC:

```haskell
import Reovim.Module (newModulePtr, freeModulePtr, withModulePtr)

-- In entry():
ptr <- newModulePtr myState

-- In init/exit:
withModulePtr ptr $ \state -> do
    -- Use state
    return result

-- In destroy():
freeModulePtr ptr
```

### Memory Ownership Rules

1. **Haskell-allocated**: Use `StablePtr`, free with `freeStablePtr`
2. **Strings to C**: Use `withCString` (temporary) or `newCString` (caller frees)
3. **Never store ctx pointer**: The context in `init()` is only valid during that call

## Debugging

### Enable Debug Logging

```haskell
import Reovim.FFI.Logging

moduleInit state ctx = do
    logDebug "Entering init"
    -- ...
    logDebug "Init complete"
    return returnSuccess
```

### Check Exported Symbols

```bash
# List all exported symbols
nm -D dist-newstyle/.../libexample-haskell.so | grep reovim

# Verify specific symbols
make check
```

## Known Limitations

1. **Static data export**: `REOVIM_MODULE_API_VERSION` as a static symbol is complex in Haskell. Use `reovim_module_api_version_ptr()` instead.

2. **Hot reload**: Not supported in this example. Implementing `save_state`/`restore_state` requires serialization.

3. **GHC RTS overhead**: Each Haskell module loads its own GHC runtime (~10MB memory).

4. **Callback latency**: GHC's garbage collector may pause briefly. Keep callbacks fast (<1ms).

## Troubleshooting

### "undefined symbol: reovim_log_info"

The module is being tested standalone. These symbols are provided by reovim at load time.

### "cannot find -lHSrts"

GHC libraries not found. Ensure GHC is properly installed:

```bash
ghc-pkg list rts
```

### Module loads but init fails

Check ABI compatibility:

```haskell
compatible <- checkKernelCompatibility
unless compatible $ logError "ABI mismatch"
```

## Related

- [FFI Overview](../../docs/architecture/ffi/overview.md)
- [C Module Example](../c-module/)
- Issue: #294
