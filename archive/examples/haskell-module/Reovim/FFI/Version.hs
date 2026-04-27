{-# LANGUAGE ForeignFunctionInterface #-}

-- |
-- Module      : Reovim.FFI.Version
-- Description : Version constants and compatibility checking
--
-- This module provides the ABI and API version constants that must match
-- the reovim kernel's expectations, along with compatibility checking.

module Reovim.FFI.Version
    ( -- * ABI Version (binary compatibility)
      abiVersion
    , abiVersionMajor
    , abiVersionMinor
    , abiVersionPatch
      -- * API Version (semantic compatibility)
    , apiVersion
    , apiVersionMajor
    , apiVersionMinor
    , apiVersionPatch
      -- * Version Creation
    , makeVersion
      -- * Compatibility Checking
    , isCompatible
      -- * Kernel Services
    , getKernelAbiVersion
    , checkKernelCompatibility
    ) where

import Data.Word (Word32)
import Foreign.Ptr (Ptr)
import Foreign.Marshal.Alloc (alloca)
import Foreign.Storable (peek, poke)

import Reovim.FFI.Types (ReovimVersion(..))

-- | ABI version constants (binary compatibility)
-- Must match REOVIM_ABI_VERSION_* in reovim.h
abiVersionMajor, abiVersionMinor, abiVersionPatch :: Word32
abiVersionMajor = 1
abiVersionMinor = 0
abiVersionPatch = 0

-- | The ABI version this module is compiled against
abiVersion :: ReovimVersion
abiVersion = ReovimVersion abiVersionMajor abiVersionMinor abiVersionPatch

-- | API version constants (semantic compatibility)
-- Must match REOVIM_API_VERSION_* in reovim.h
apiVersionMajor, apiVersionMinor, apiVersionPatch :: Word32
apiVersionMajor = 0
apiVersionMinor = 2
apiVersionPatch = 0

-- | The API version this module requires
apiVersion :: ReovimVersion
apiVersion = ReovimVersion apiVersionMajor apiVersionMinor apiVersionPatch

-- | Create a version from components
makeVersion :: Word32 -> Word32 -> Word32 -> ReovimVersion
makeVersion = ReovimVersion

-- | Check if two versions are compatible.
--
-- Compatibility rules (same as reovim_abi_is_compatible):
-- - Major version must match exactly
-- - Required minor must be <= provided minor
-- - Patch version is ignored
isCompatible :: ReovimVersion  -- ^ Required version
             -> ReovimVersion  -- ^ Provided version
             -> Bool
isCompatible required provided =
    versionMajor required == versionMajor provided &&
    versionMinor required <= versionMinor provided

-- | Foreign import for kernel's ABI version function (via pointer)
--
-- The C function returns struct by value, but GHC FFI requires pointer.
-- We use a wrapper that fills a pre-allocated buffer.
foreign import ccall unsafe "reovim_abi_version_ptr"
    c_reovim_abi_version_ptr :: Ptr ReovimVersion -> IO ()

-- | Get the kernel's ABI version
--
-- Note: This requires the reovim kernel to provide reovim_abi_version_ptr,
-- or you can skip this check since the loader verifies compatibility.
getKernelAbiVersion :: IO ReovimVersion
getKernelAbiVersion = alloca $ \ptr -> do
    -- Initialize with known version in case the call fails
    poke ptr abiVersion
    c_reovim_abi_version_ptr ptr
    peek ptr

-- | Check if the kernel's ABI is compatible with our requirements
--
-- Note: The module loader already checks API version compatibility
-- before calling init(), so this is optional but recommended for
-- additional safety.
checkKernelCompatibility :: IO Bool
checkKernelCompatibility = do
    kernelVersion <- getKernelAbiVersion
    return $ isCompatible abiVersion kernelVersion

-- | Alternative: Skip runtime check, trust loader's version verification
--
-- The loader reads REOVIM_MODULE_API_VERSION before loading, so if
-- we reach init(), we know versions are compatible.
checkKernelCompatibilitySimple :: IO Bool
checkKernelCompatibilitySimple = return True
