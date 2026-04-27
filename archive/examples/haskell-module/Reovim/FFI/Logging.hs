{-# LANGUAGE ForeignFunctionInterface #-}

-- |
-- Module      : Reovim.FFI.Logging
-- Description : Logging functions via kernel services
--
-- This module provides safe Haskell wrappers around the reovim kernel's
-- logging services. All functions handle NULL gracefully.

module Reovim.FFI.Logging
    ( -- * Logging Functions (String)
      logInfo
    , logWarn
    , logError
    , logDebug
      -- * Logging Functions (ByteString)
    , logInfoBS
    , logWarnBS
    , logErrorBS
    , logDebugBS
      -- * Low-level C Imports
    , c_reovim_log_info
    , c_reovim_log_warn
    , c_reovim_log_error
    , c_reovim_log_debug
    ) where

import Foreign.C.String (CString, withCString)
import qualified Data.ByteString.Char8 as BS8
import Data.ByteString (ByteString)

-- | Foreign imports for kernel logging services
-- All functions handle NULL msg gracefully (no-op)
foreign import ccall unsafe "reovim_log_info"
    c_reovim_log_info :: CString -> IO ()

foreign import ccall unsafe "reovim_log_warn"
    c_reovim_log_warn :: CString -> IO ()

foreign import ccall unsafe "reovim_log_error"
    c_reovim_log_error :: CString -> IO ()

foreign import ccall unsafe "reovim_log_debug"
    c_reovim_log_debug :: CString -> IO ()

-- | Log an info-level message
logInfo :: String -> IO ()
logInfo msg = withCString msg c_reovim_log_info

-- | Log a warning-level message
logWarn :: String -> IO ()
logWarn msg = withCString msg c_reovim_log_warn

-- | Log an error-level message
logError :: String -> IO ()
logError msg = withCString msg c_reovim_log_error

-- | Log a debug-level message
logDebug :: String -> IO ()
logDebug msg = withCString msg c_reovim_log_debug

-- | ByteString variants for efficiency
logInfoBS :: ByteString -> IO ()
logInfoBS msg = BS8.useAsCString msg c_reovim_log_info

logWarnBS :: ByteString -> IO ()
logWarnBS msg = BS8.useAsCString msg c_reovim_log_warn

logErrorBS :: ByteString -> IO ()
logErrorBS msg = BS8.useAsCString msg c_reovim_log_error

logDebugBS :: ByteString -> IO ()
logDebugBS msg = BS8.useAsCString msg c_reovim_log_debug
