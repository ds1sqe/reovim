{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE ScopedTypeVariables #-}

-- |
-- Module      : Reovim.Module
-- Description : High-level module implementation helpers
--
-- This module provides the ReovimModule typeclass and utilities for
-- implementing reovim modules in Haskell with proper exception handling
-- and StablePtr management.
--
-- = Exception Safety
--
-- All exported FFI functions are wrapped in exception handlers to prevent
-- Haskell exceptions from escaping to C (which would be undefined behavior).
-- Exceptions are caught and converted to the -2 (panic) return code.
--
-- = Memory Management
--
-- Module state is wrapped in a StablePtr to prevent GC collection while
-- the C code holds a reference. The StablePtr must be freed in destroy().

module Reovim.Module
    ( -- * Module Typeclass
      ReovimModule(..)
      -- * Exception-Safe Wrappers
    , safeFFI
    , safeFFIVoid
      -- * StablePtr Utilities
    , newModulePtr
    , freeModulePtr
    , withModulePtr
      -- * Return Codes
    , returnSuccess
    , returnDefer
    , returnFailed
    , returnPanic
      -- * GHC RTS Management
    , initRTS
    , exitRTS
    ) where

import Control.Exception (SomeException, catch)
import Data.Int (Int32)
import Foreign.Ptr (Ptr, nullPtr)
import Foreign.StablePtr
    ( StablePtr
    , newStablePtr
    , freeStablePtr
    , deRefStablePtr
    , castStablePtrToPtr
    , castPtrToStablePtr
    )
import Foreign.C.String (CString)

import Reovim.FFI.Types (ReovimModuleProbe)
import Reovim.FFI.Logging (logError)

-- | Return codes for module lifecycle functions
returnSuccess, returnDefer, returnFailed, returnPanic :: Int32
returnSuccess = 0   -- ^ Operation succeeded
returnDefer   = 1   -- ^ Defer, try again later (like Linux EPROBE_DEFER)
returnFailed  = -1  -- ^ Operation failed
returnPanic   = -2  -- ^ Panic/exception occurred

-- | Typeclass for reovim modules
--
-- Implement this typeclass to define your module's behavior.
-- The default implementations handle common cases.
class ReovimModule a where
    -- | Return module metadata for discovery
    moduleProbe :: a -> ReovimModuleProbe

    -- | Initialize the module
    --
    -- Called after entry(). The context pointer is currently opaque.
    -- Return 0 for success, 1 to defer, -1 for failure.
    moduleInit :: a -> Ptr () -> IO Int32
    moduleInit _ _ = return returnSuccess

    -- | Cleanup before unload
    --
    -- Called before destroy(). Release any resources here.
    -- Return 0 for success, -1 for error.
    moduleExit :: a -> IO Int32
    moduleExit _ = return returnSuccess

    -- | Check if hot reload is supported
    moduleSupportsHotReload :: a -> Bool
    moduleSupportsHotReload _ = False

    -- | Save state for hot reload
    --
    -- Return Nothing if no state to save.
    moduleSaveState :: a -> IO (Maybe (Ptr (), Int))
    moduleSaveState _ = return Nothing

    -- | Restore state after hot reload
    --
    -- Return True on success, False on failure.
    moduleRestoreState :: a -> Ptr () -> Int -> IO Bool
    moduleRestoreState _ _ _ = return False

-- | Wrap an IO action that returns Int32, catching all exceptions
--
-- Exceptions are logged and converted to returnPanic (-2).
safeFFI :: IO Int32 -> IO Int32
safeFFI action = action `catch` handleException
  where
    handleException :: SomeException -> IO Int32
    handleException e = do
        logError $ "Haskell exception: " ++ show e
        return returnPanic

-- | Wrap a void IO action, catching all exceptions
--
-- Exceptions are logged but cannot affect return value.
safeFFIVoid :: IO () -> IO ()
safeFFIVoid action = action `catch` handleException
  where
    handleException :: SomeException -> IO ()
    handleException e = logError $ "Haskell exception: " ++ show e

-- | Create a new StablePtr for module state and return as opaque Ptr
newModulePtr :: a -> IO (Ptr ())
newModulePtr state = do
    stablePtr <- newStablePtr state
    return $ castStablePtrToPtr stablePtr

-- | Free a module's StablePtr
freeModulePtr :: Ptr () -> IO ()
freeModulePtr ptr
    | ptr == nullPtr = return ()
    | otherwise = freeStablePtr (castPtrToStablePtr ptr :: StablePtr ())

-- | Safely access module state from an opaque Ptr
--
-- Returns Nothing if the pointer is null.
withModulePtr :: Ptr () -> (a -> IO b) -> IO (Maybe b)
withModulePtr ptr action
    | ptr == nullPtr = return Nothing
    | otherwise = do
        let stablePtr = castPtrToStablePtr ptr :: StablePtr a
        state <- deRefStablePtr stablePtr
        result <- action state
        return (Just result)

-- | Initialize the GHC runtime system
--
-- This must be called before any Haskell code runs.
-- Safe to call multiple times (GHC tracks init count).
foreign import ccall "hs_init"
    c_hs_init :: Ptr (Ptr CString) -> Ptr (Ptr CString) -> IO ()

-- | Shutdown the GHC runtime system
--
-- Must be called after all Haskell code is done.
-- Must balance hs_init calls.
foreign import ccall "hs_exit"
    c_hs_exit :: IO ()

-- | Initialize the GHC RTS (safe wrapper)
initRTS :: IO ()
initRTS = c_hs_init nullPtr nullPtr

-- | Exit the GHC RTS (safe wrapper)
exitRTS :: IO ()
exitRTS = c_hs_exit
