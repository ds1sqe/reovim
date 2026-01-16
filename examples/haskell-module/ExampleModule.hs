{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE OverloadedStrings #-}

-- |
-- Module      : ExampleModule
-- Description : Example reovim module in Haskell
--
-- This module demonstrates how to implement a reovim module in Haskell.
-- It exports all 6 required entry points using the C calling convention.
--
-- = Building
--
-- @
-- make
-- @
--
-- = Entry Points
--
-- The module exports these symbols:
--
-- * @REOVIM_MODULE_API_VERSION@ - Static API version
-- * @reovim_module_probe@ - Return module metadata
-- * @reovim_module_entry@ - Create module instance
-- * @reovim_module_init@ - Initialize module
-- * @reovim_module_exit@ - Cleanup module
-- * @reovim_module_destroy@ - Free module memory

module ExampleModule where

import Control.Exception (SomeException, catch)
import Data.Int (Int32)
import Data.IORef (IORef, newIORef, readIORef, writeIORef)
import Foreign.Ptr (Ptr, nullPtr)
import Foreign.StablePtr
    ( StablePtr
    , newStablePtr
    , freeStablePtr
    , deRefStablePtr
    , castStablePtrToPtr
    , castPtrToStablePtr
    )
import Foreign.Marshal.Alloc (malloc, free)
import Foreign.Storable (poke)

import System.IO.Unsafe (unsafePerformIO)

import Reovim.FFI
import Reovim.Module

-- | Module state
data ExampleState = ExampleState
    { stateInitialized :: !(IORef Bool)
    , stateCounter :: !(IORef Int)
    }

-- | Create initial module state
newExampleState :: IO ExampleState
newExampleState = ExampleState
    <$> newIORef False
    <*> newIORef 0

-- | Module metadata
exampleProbe :: ReovimModuleProbe
exampleProbe = buildProbe $
    newProbe
        & withId "example-haskell"
        & withName "Example Haskell Module"
        & withVersion 1 0 0
        & withRustcVersion "GHC"
  where
    (&) = flip ($)

instance ReovimModule ExampleState where
    moduleProbe _ = exampleProbe

    moduleInit state _ctx = do
        -- Fun greeting sequence
        logInfo "==================================="
        logInfo "  Hello, Reovim!"
        logInfo "  I am a Haskell module!"
        logInfo "  Functional programming FTW!"
        logInfo "==================================="

        -- Check ABI compatibility
        compatible <- checkKernelCompatibility
        if not compatible
            then do
                logError "ABI version mismatch"
                return returnFailed
            else do
                writeIORef (stateInitialized state) True
                writeIORef (stateCounter state) 42
                logDebug "Counter initialized to 42"
                logInfo "Module ready!"
                return returnSuccess

    moduleExit state = do
        logInfo "Example Haskell module exiting"
        writeIORef (stateInitialized state) False
        writeIORef (stateCounter state) 0
        return returnSuccess

-- ============================================================================
-- FFI Entry Points
-- ============================================================================

-- | Static API version symbol
--
-- The loader reads this before calling any functions.
-- We export a pointer to a statically allocated version struct.
foreign export ccall "reovim_module_api_version_ptr"
    apiVersionPtr :: IO (Ptr ReovimVersion)

{-# NOINLINE apiVersionStorage #-}
apiVersionStorage :: IORef (Maybe (Ptr ReovimVersion))
apiVersionStorage = unsafePerformIO $ newIORef Nothing
  where
    -- Safe use of unsafePerformIO for one-time initialization
    {-# NOINLINE unsafePerformIO #-}
    unsafePerformIO = System.IO.Unsafe.unsafePerformIO

apiVersionPtr :: IO (Ptr ReovimVersion)
apiVersionPtr = do
    existing <- readIORef apiVersionStorage
    case existing of
        Just ptr -> return ptr
        Nothing -> do
            ptr <- malloc
            poke ptr apiVersion
            writeIORef apiVersionStorage (Just ptr)
            return ptr

-- Note: REOVIM_MODULE_API_VERSION as a static data export is complex in Haskell.
-- The loader should also support reading via reovim_module_api_version_ptr().

-- | Return module metadata via pointer
--
-- The C ABI expects probe returned by value, but GHC can't do that.
-- We allocate and fill a buffer instead. The caller is responsible
-- for providing the buffer (or we malloc one).
foreign export ccall "reovim_module_probe"
    hsModuleProbe :: Ptr ReovimModuleProbe -> IO ()

hsModuleProbe :: Ptr ReovimModuleProbe -> IO ()
hsModuleProbe ptr = poke ptr exampleProbe

-- | Create module instance
--
-- Allocates state and wraps in StablePtr to prevent GC.
foreign export ccall "reovim_module_entry"
    hsModuleEntry :: IO (Ptr ())

hsModuleEntry :: IO (Ptr ())
hsModuleEntry = safeFFI' $ do
    logDebug "Creating Haskell module instance"
    state <- newExampleState
    newModulePtr state
  where
    safeFFI' action = action `catch` \(_ :: SomeException) -> return nullPtr

-- | Initialize module
foreign export ccall "reovim_module_init"
    hsModuleInit :: Ptr () -> Ptr () -> IO Int32

hsModuleInit :: Ptr () -> Ptr () -> IO Int32
hsModuleInit modulePtr ctxPtr = safeFFI $ do
    result <- withModulePtr modulePtr $ \(state :: ExampleState) ->
        moduleInit state ctxPtr
    case result of
        Just r -> return r
        Nothing -> do
            logError "Module init called with null pointer"
            return returnFailed

-- | Cleanup module
foreign export ccall "reovim_module_exit"
    hsModuleExit :: Ptr () -> IO Int32

hsModuleExit :: Ptr () -> IO Int32
hsModuleExit modulePtr = safeFFI $ do
    result <- withModulePtr modulePtr $ \(state :: ExampleState) ->
        moduleExit state
    case result of
        Just r -> return r
        Nothing -> do
            logError "Module exit called with null pointer"
            return returnFailed

-- | Free module memory
foreign export ccall "reovim_module_destroy"
    hsModuleDestroy :: Ptr () -> IO ()

hsModuleDestroy :: Ptr () -> IO ()
hsModuleDestroy modulePtr = safeFFIVoid $ do
    logDebug "Destroying Haskell module instance"
    freeModulePtr modulePtr

-- | Check if hot reload is supported
foreign export ccall "reovim_module_supports_hot_reload"
    hsModuleSupportsHotReload :: Ptr () -> IO Int32

hsModuleSupportsHotReload :: Ptr () -> IO Int32
hsModuleSupportsHotReload _ = return 0  -- Not supported

-- | Save state for hot reload (not supported)
foreign export ccall "reovim_module_save_state"
    hsModuleSaveState :: Ptr () -> Ptr (Ptr ()) -> Ptr Int -> IO Int32

hsModuleSaveState :: Ptr () -> Ptr (Ptr ()) -> Ptr Int -> IO Int32
hsModuleSaveState _ _ _ = return 1  -- No state to save

-- | Restore state after hot reload (not supported)
foreign export ccall "reovim_module_restore_state"
    hsModuleRestoreState :: Ptr () -> Ptr () -> Int -> IO Int32

hsModuleRestoreState :: Ptr () -> Ptr () -> Int -> IO Int32
hsModuleRestoreState _ _ _ = return returnFailed  -- Not supported

-- | Free state buffer (not supported)
foreign export ccall "reovim_module_free_state"
    hsModuleFreeState :: Ptr () -> Int -> IO ()

hsModuleFreeState :: Ptr () -> Int -> IO ()
hsModuleFreeState _ _ = return ()  -- Nothing to free
