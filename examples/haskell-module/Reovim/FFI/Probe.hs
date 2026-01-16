{-# LANGUAGE OverloadedStrings #-}

-- |
-- Module      : Reovim.FFI.Probe
-- Description : ModuleProbe builder pattern
--
-- This module provides a fluent builder API for constructing ReovimModuleProbe
-- structs with proper string truncation and validation.

module Reovim.FFI.Probe
    ( -- * Builder Type
      ProbeBuilder
      -- * Builder Functions
    , newProbe
    , withId
    , withName
    , withVersion
    , withApiVersion
    , withRustcVersion
    , withRequiredDep
    , withOptionalDep
    , buildProbe
      -- * Quick Builders
    , simpleProbe
    ) where

import Data.ByteString (ByteString)
import qualified Data.ByteString as BS
import qualified Data.ByteString.Char8 as BS8
import Data.Word (Word32)

import Reovim.FFI.Types
    ( ReovimModuleProbe(..)
    , ReovimVersion(..)
    , emptyProbe
    , maxIdLength
    , maxNameLength
    , maxRustcVersionLength
    , maxDeps
    , maxDepIdLength
    )
import Reovim.FFI.Version (apiVersion)

-- | Builder state for constructing a probe
newtype ProbeBuilder = ProbeBuilder { unBuilder :: ReovimModuleProbe }

-- | Start building a new probe
newProbe :: ProbeBuilder
newProbe = ProbeBuilder emptyProbe

-- | Set the module ID (truncated to 63 chars)
withId :: ByteString -> ProbeBuilder -> ProbeBuilder
withId id_ (ProbeBuilder p) = ProbeBuilder $ p
    { probeId = truncateString (maxIdLength - 1) id_ }

-- | Set the module display name (truncated to 127 chars)
withName :: ByteString -> ProbeBuilder -> ProbeBuilder
withName name (ProbeBuilder p) = ProbeBuilder $ p
    { probeName = truncateString (maxNameLength - 1) name }

-- | Set the module version
withVersion :: Word32 -> Word32 -> Word32 -> ProbeBuilder -> ProbeBuilder
withVersion major minor patch (ProbeBuilder p) = ProbeBuilder $ p
    { probeVersion = ReovimVersion major minor patch }

-- | Set the required API version (defaults to current API version)
withApiVersion :: Word32 -> Word32 -> Word32 -> ProbeBuilder -> ProbeBuilder
withApiVersion major minor patch (ProbeBuilder p) = ProbeBuilder $ p
    { probeApiVersion = ReovimVersion major minor patch }

-- | Set the compiler version string (truncated to 63 chars)
withRustcVersion :: ByteString -> ProbeBuilder -> ProbeBuilder
withRustcVersion ver (ProbeBuilder p) = ProbeBuilder $ p
    { probeRustcVersion = truncateString (maxRustcVersionLength - 1) ver }

-- | Add a required dependency (max 8, each truncated to 63 chars)
withRequiredDep :: ByteString -> ProbeBuilder -> ProbeBuilder
withRequiredDep dep (ProbeBuilder p) = ProbeBuilder $ p
    { probeRequiredDeps = take maxDeps $
        probeRequiredDeps p ++ [truncateString (maxDepIdLength - 1) dep]
    }

-- | Add an optional dependency (max 8, each truncated to 63 chars)
withOptionalDep :: ByteString -> ProbeBuilder -> ProbeBuilder
withOptionalDep dep (ProbeBuilder p) = ProbeBuilder $ p
    { probeOptionalDeps = take maxDeps $
        probeOptionalDeps p ++ [truncateString (maxDepIdLength - 1) dep]
    }

-- | Finalize and extract the probe
buildProbe :: ProbeBuilder -> ReovimModuleProbe
buildProbe (ProbeBuilder p) = p
    { probeApiVersion = if probeApiVersion p == ReovimVersion 0 0 0
                        then apiVersion
                        else probeApiVersion p
    }

-- | Create a simple probe with just ID, name, and version
simpleProbe :: ByteString  -- ^ Module ID
            -> ByteString  -- ^ Module name
            -> (Word32, Word32, Word32)  -- ^ Version (major, minor, patch)
            -> ReovimModuleProbe
simpleProbe id_ name (major, minor, patch) = buildProbe $
    newProbe
        & withId id_
        & withName name
        & withVersion major minor patch
  where
    -- Flip function application for builder pattern
    (&) = flip ($)

-- | Helper to truncate a ByteString to a maximum length
truncateString :: Int -> ByteString -> ByteString
truncateString maxLen = BS.take maxLen
