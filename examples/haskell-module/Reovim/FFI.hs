{-# LANGUAGE ForeignFunctionInterface #-}

-- |
-- Module      : Reovim.FFI
-- Description : Re-exports all FFI types and functions
--
-- This module provides a convenient single import for all FFI bindings
-- needed to write reovim modules in Haskell.
--
-- = Usage
--
-- @
-- import Reovim.FFI
--
-- myProbe :: ReovimModuleProbe
-- myProbe = simpleProbe "my-module" "My Module" (1, 0, 0)
-- @
--
-- = ABI Compatibility
--
-- This module is compiled against:
--
-- * ABI Version: 1.0.0
-- * API Version: 0.2.0
--
-- Modules must check compatibility before using kernel services.

module Reovim.FFI
    ( -- * Types
      module Reovim.FFI.Types
      -- * Version
    , module Reovim.FFI.Version
      -- * Probe Builder
    , module Reovim.FFI.Probe
      -- * Logging
    , module Reovim.FFI.Logging
      -- * Kernel Services
    , reovimAbiIsCompatible
    ) where

import Reovim.FFI.Types
import Reovim.FFI.Version
import Reovim.FFI.Probe
import Reovim.FFI.Logging

-- | Check if two ABI versions are compatible
--
-- This is a pure Haskell implementation matching the kernel's logic.
-- We don't call the C function because GHC FFI can't pass structs by value.
reovimAbiIsCompatible :: ReovimVersion -> ReovimVersion -> Bool
reovimAbiIsCompatible = isCompatible
