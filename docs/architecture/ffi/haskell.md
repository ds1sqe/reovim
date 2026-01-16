# Haskell FFI Guide

This document covers Haskell-specific details for writing reovim modules.
For the general FFI interface, see [overview.md](overview.md).

## GHC Runtime System

The GHC Runtime System (RTS) must be initialized before any Haskell code
executes. This is critical because reovim's module loader calls probe
functions before the module's `entry()` function.

### The Problem

1. Loader calls `dlopen()` to load the shared library
2. Loader calls `reovim_module_api_version_ptr()` to check version
3. Loader calls `reovim_module_probe()` to get metadata
4. **At this point, GHC RTS is NOT initialized!**
5. Haskell code panics: "RTS is not initialised; call hs_init() first"

### The Solution: Constructor Attribute

Use GCC's `__attribute__((constructor))` to initialize the RTS when the
library is loaded, before any other function is called:

```c
// rts_shim.c
#include <stddef.h>
#include <HsFFI.h>

static void reovim_rts_init(void) __attribute__((constructor));
static void reovim_rts_init(void)
{
    static char *argv[] = { "reovim-haskell-module", NULL };
    static char **argv_ = argv;
    static int argc = 1;
    hs_init(&argc, &argv_);
}

static void reovim_rts_exit(void) __attribute__((destructor));
static void reovim_rts_exit(void)
{
    hs_exit();
}
```

The constructor runs automatically when `dlopen()` loads the library,
ensuring the RTS is ready before any Haskell functions are called.

### Limitations

- **No RTS reinit**: GHC does not support reinitializing the RTS after
  `hs_exit()`. Once a Haskell module is unloaded, it cannot be reloaded
  in the same process.
- **One RTS per process**: Multiple Haskell modules share the same RTS.

## FFI Signature Alternatives

GHC has limitations on FFI signatures that require alternative entry points.

### Return by Pointer

GHC cannot return structs by value from FFI functions. Instead of:

```c
ReovimModuleProbe reovim_module_probe(void);  // GHC cannot do this
```

Use a pointer-based signature:

```c
void reovim_module_probe(ReovimModuleProbe* out);  // GHC can do this
```

The reovim loader supports both signatures, trying return-by-value first,
then falling back to pointer-based.

### Static Symbol Export

GHC cannot export static data symbols. Instead of:

```c
const ReovimVersion REOVIM_MODULE_API_VERSION;  // GHC cannot do this
```

Use a function that writes to a pointer:

```c
void reovim_module_api_version_ptr(ReovimVersion* out);  // GHC can do this
```

## Type Mappings

| C Type | Haskell Type | Notes |
|--------|--------------|-------|
| `uint8_t` | `Word8` | |
| `uint32_t` | `Word32` | |
| `int32_t` | `Int32` | |
| `size_t` | `CSize` (Word) | |
| `const char*` | `CString` | Null-terminated |
| `void*` | `Ptr ()` | Opaque pointer |
| `uint8_t[64]` | `ByteString` | Fixed-size, stored inline |

### Storable Instances

All FFI structs need `Storable` instances for marshalling:

```haskell
instance Storable ReovimVersion where
    sizeOf _ = 12  -- 3 * 4 bytes
    alignment _ = 4
    peek ptr = ReovimVersion
        <$> peekByteOff ptr 0   -- major
        <*> peekByteOff ptr 4   -- minor
        <*> peekByteOff ptr 8   -- patch
    poke ptr (ReovimVersion maj min pat) = do
        pokeByteOff ptr 0 maj
        pokeByteOff ptr 4 min
        pokeByteOff ptr 8 pat
```

## Exception Safety

Haskell exceptions must NOT escape across the FFI boundary. All exported
functions must catch exceptions and convert them to return codes:

```haskell
safeFFI :: IO Int32 -> IO Int32
safeFFI action = action `catch` handleException
  where
    handleException :: SomeException -> IO Int32
    handleException e = do
        logError $ "Haskell exception: " ++ show e
        return returnPanic  -- -2
```

## Module Structure

A Haskell module should follow this structure:

```
my-module/
├── MyModule.hs           # Module implementation + FFI exports
├── Reovim/
│   ├── FFI.hs            # FFI bindings library
│   ├── FFI/
│   │   ├── Types.hs      # Core FFI types
│   │   ├── Version.hs    # Version constants
│   │   ├── Probe.hs      # ProbeBuilder
│   │   └── Logging.hs    # Kernel logging imports
│   └── Module.hs         # ReovimModule typeclass
├── rts_shim.c            # GHC RTS init/exit
├── my-module.cabal       # Cabal package
└── Makefile              # Build targets
```

## Cabal Configuration

Use `foreign-library` for the shared library component:

```cabal
foreign-library my-module
    type:             native-shared
    lib-version-info: 1:0:0

    other-modules:    MyModule
    c-sources:        rts_shim.c
    build-depends:
        base >= 4.16 && < 5,
        bytestring >= 0.11 && < 0.13,
        my-module-ffi-lib
    default-language: GHC2021
    ghc-options:      -fPIC
```

Key points:
- `type: native-shared` produces a `.so` file
- `c-sources: rts_shim.c` includes the RTS init shim
- `-fPIC` is required for shared libraries

## Entry Point Implementation

```haskell
{-# LANGUAGE ForeignFunctionInterface #-}

module MyModule where

import Foreign
import Foreign.C
import Reovim.FFI
import Reovim.Module

-- Module state (managed via StablePtr)
data MyModuleState = MyModuleState
    { msInitialized :: !Bool
    }

-- API version (pointer-based for GHC compatibility)
foreign export ccall "reovim_module_api_version_ptr"
    hsApiVersionPtr :: Ptr ReovimVersion -> IO ()

hsApiVersionPtr :: Ptr ReovimVersion -> IO ()
hsApiVersionPtr ptr = poke ptr apiVersion

-- Probe (pointer-based for GHC compatibility)
foreign export ccall "reovim_module_probe"
    hsModuleProbe :: Ptr ReovimModuleProbe -> IO ()

hsModuleProbe :: Ptr ReovimModuleProbe -> IO ()
hsModuleProbe ptr = poke ptr myProbe
  where
    myProbe = buildProbe $ do
        setId "my-module"
        setName "My Module"
        setVersion (1, 0, 0)
        setApiVersion (0, 2, 0)

-- Entry (creates StablePtr to module state)
foreign export ccall "reovim_module_entry"
    hsModuleEntry :: IO (Ptr ())

hsModuleEntry :: IO (Ptr ())
hsModuleEntry = do
    logDebug "Creating module instance"
    let state = MyModuleState { msInitialized = False }
    stablePtr <- newStablePtr state
    return $ castStablePtrToPtr stablePtr

-- Init
foreign export ccall "reovim_module_init"
    hsModuleInit :: Ptr () -> Ptr () -> IO Int32

hsModuleInit :: Ptr () -> Ptr () -> IO Int32
hsModuleInit modulePtr _ctxPtr = safeFFI $ do
    state <- deRefStablePtr (castPtrToStablePtr modulePtr)
    logInfo "Module initializing"
    -- Update state...
    return 0  -- Success

-- Exit
foreign export ccall "reovim_module_exit"
    hsModuleExit :: Ptr () -> IO Int32

hsModuleExit :: Ptr () -> IO Int32
hsModuleExit modulePtr = safeFFI $ do
    _state <- deRefStablePtr (castPtrToStablePtr modulePtr)
    logInfo "Module exiting"
    return 0  -- Success

-- Destroy (frees StablePtr)
foreign export ccall "reovim_module_destroy"
    hsModuleDestroy :: Ptr () -> IO ()

hsModuleDestroy :: Ptr () -> IO ()
hsModuleDestroy modulePtr = do
    freeStablePtr (castPtrToStablePtr modulePtr :: StablePtr MyModuleState)
```

## Building

```bash
# Build
cabal build flib:my-module

# Find the library
find dist-newstyle -name "*.so*" -type f

# Test loading
reovim cli load /path/to/libmy-module.so.1.0.0
```

## Logging

Kernel logging functions are imported via FFI:

```haskell
foreign import ccall unsafe "reovim_log_info"
    c_reovim_log_info :: CString -> IO ()

logInfo :: String -> IO ()
logInfo msg = withCString msg c_reovim_log_info
```

For standalone testing (without reovim), provide stub implementations in
`rts_shim.c`:

```c
void reovim_log_info(const char *msg) {
    fprintf(stderr, "[INFO] %s\n", msg);
}
```

## Example Module

See `examples/haskell-module/` for a complete working example.

## References

- [GHC User's Guide - Shared Libraries](https://downloads.haskell.org/ghc/latest/docs/users_guide/shared_libs.html)
- [GHC User's Guide - FFI](https://ghc.gitlab.haskell.org/ghc/doc/users_guide/exts/ffi.html)
- [Well-Typed Blog - Building Plugins as Haskell Shared Libs](https://well-typed.com/blog/2009/05/buildings-plugins-as-haskell-shared-libs/)
