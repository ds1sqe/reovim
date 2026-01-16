{-# LANGUAGE ForeignFunctionInterface #-}

-- |
-- Module      : Reovim.FFI.Types
-- Description : FFI-safe types matching reovim C ABI
--
-- This module defines Haskell types that match the C structs defined in
-- reovim.h. All types have Storable instances for FFI interop.
--
-- ABI Version: 1.0.0
-- API Version: 0.2.0

module Reovim.FFI.Types
    ( -- * Version
      ReovimVersion(..)
    , versionToTuple
    , versionFromTuple
      -- * Module Probe
    , ReovimModuleProbe(..)
    , emptyProbe
      -- * Constants
    , maxIdLength
    , maxNameLength
    , maxRustcVersionLength
    , maxDeps
    , maxDepIdLength
    , probeSize
    ) where

import Data.Word (Word8, Word32)
import Foreign.Storable (Storable(..))
import Foreign.Ptr (Ptr, plusPtr)
import Foreign.Marshal.Array (pokeArray, peekArray)
import Data.ByteString (ByteString)
import qualified Data.ByteString as BS

-- | Buffer size constants matching reovim.h
maxIdLength :: Int
maxIdLength = 64

maxNameLength :: Int
maxNameLength = 128

maxRustcVersionLength :: Int
maxRustcVersionLength = 64

maxDeps :: Int
maxDeps = 8

maxDepIdLength :: Int
maxDepIdLength = 64

-- | Total size of ReovimModuleProbe struct (1308 bytes)
probeSize :: Int
probeSize = 1308

-- | Semantic version representation.
--
-- Size: 12 bytes
-- Alignment: 4 bytes
data ReovimVersion = ReovimVersion
    { versionMajor :: !Word32
    , versionMinor :: !Word32
    , versionPatch :: !Word32
    } deriving (Eq, Show)

versionToTuple :: ReovimVersion -> (Word32, Word32, Word32)
versionToTuple (ReovimVersion maj min_ pat) = (maj, min_, pat)

versionFromTuple :: (Word32, Word32, Word32) -> ReovimVersion
versionFromTuple (maj, min_, pat) = ReovimVersion maj min_ pat

instance Storable ReovimVersion where
    sizeOf _ = 12
    alignment _ = 4

    peek ptr = do
        maj <- peekByteOff ptr 0
        min_ <- peekByteOff ptr 4
        pat <- peekByteOff ptr 8
        return $ ReovimVersion maj min_ pat

    poke ptr (ReovimVersion maj min_ pat) = do
        pokeByteOff ptr 0 maj
        pokeByteOff ptr 4 min_
        pokeByteOff ptr 8 pat

-- | FFI-safe module metadata for discovery.
--
-- Size: 1308 bytes
-- Alignment: 4 bytes
--
-- Layout:
--   id[64]                    offset 0
--   name[128]                 offset 64
--   version (12 bytes)        offset 192
--   api_version (12 bytes)    offset 204
--   rustc_version[64]         offset 216
--   required_deps_count (1)   offset 280
--   required_deps[8][64]      offset 281
--   optional_deps_count (1)   offset 793
--   optional_deps[8][64]      offset 794
--   Total: 1306 bytes + 2 padding = 1308 bytes
data ReovimModuleProbe = ReovimModuleProbe
    { probeId :: !ByteString           -- ^ Module ID (max 63 chars + nul)
    , probeName :: !ByteString         -- ^ Display name (max 127 chars + nul)
    , probeVersion :: !ReovimVersion   -- ^ Module version
    , probeApiVersion :: !ReovimVersion -- ^ Required API version
    , probeRustcVersion :: !ByteString -- ^ Compiler version (max 63 chars + nul)
    , probeRequiredDeps :: ![ByteString] -- ^ Required dependency IDs (max 8)
    , probeOptionalDeps :: ![ByteString] -- ^ Optional dependency IDs (max 8)
    } deriving (Eq, Show)

-- | Empty probe with default values
emptyProbe :: ReovimModuleProbe
emptyProbe = ReovimModuleProbe
    { probeId = BS.empty
    , probeName = BS.empty
    , probeVersion = ReovimVersion 0 0 0
    , probeApiVersion = ReovimVersion 0 2 0
    , probeRustcVersion = BS.empty
    , probeRequiredDeps = []
    , probeOptionalDeps = []
    }

-- | Helper to write a ByteString to a fixed-size buffer with null termination
pokeFixedString :: Ptr Word8 -> Int -> ByteString -> IO ()
pokeFixedString ptr maxLen bs = do
    let truncated = BS.take (maxLen - 1) bs
        padded = truncated <> BS.replicate (maxLen - BS.length truncated) 0
    pokeArray ptr (BS.unpack padded)

-- | Helper to read a null-terminated string from a fixed-size buffer
peekFixedString :: Ptr Word8 -> Int -> IO ByteString
peekFixedString ptr maxLen = do
    bytes <- peekArray maxLen ptr
    return $ BS.pack $ takeWhile (/= 0) bytes

-- | Helper to write dependency array
pokeDepsArray :: Ptr Word8 -> Int -> Int -> [ByteString] -> IO ()
pokeDepsArray ptr maxDepsCount depSize deps = do
    let truncatedDeps = take maxDepsCount deps
        paddedDeps = truncatedDeps ++ replicate (maxDepsCount - length truncatedDeps) BS.empty
    mapM_ (\(i, dep) -> pokeFixedString (ptr `plusPtr` (i * depSize)) depSize dep)
          (zip [0..] paddedDeps)

-- | Helper to read dependency array
peekDepsArray :: Ptr Word8 -> Int -> Int -> Int -> IO [ByteString]
peekDepsArray ptr count maxDepsCount depSize = do
    let actualCount = min count maxDepsCount
    mapM (\i -> peekFixedString (ptr `plusPtr` (i * depSize)) depSize) [0..actualCount-1]

instance Storable ReovimModuleProbe where
    sizeOf _ = probeSize
    alignment _ = 4

    peek ptr = do
        -- id[64] at offset 0
        id_ <- peekFixedString (plusPtr ptr 0) maxIdLength
        -- name[128] at offset 64
        name <- peekFixedString (plusPtr ptr 64) maxNameLength
        -- version at offset 192
        version <- peek (plusPtr ptr 192)
        -- api_version at offset 204
        apiVersion <- peek (plusPtr ptr 204)
        -- rustc_version[64] at offset 216
        rustcVersion <- peekFixedString (plusPtr ptr 216) maxRustcVersionLength
        -- required_deps_count at offset 280
        reqCount <- peekByteOff ptr 280 :: IO Word8
        -- required_deps[8][64] at offset 281
        reqDeps <- peekDepsArray (plusPtr ptr 281) (fromIntegral reqCount) maxDeps maxDepIdLength
        -- optional_deps_count at offset 793
        optCount <- peekByteOff ptr 793 :: IO Word8
        -- optional_deps[8][64] at offset 794
        optDeps <- peekDepsArray (plusPtr ptr 794) (fromIntegral optCount) maxDeps maxDepIdLength

        return $ ReovimModuleProbe id_ name version apiVersion rustcVersion reqDeps optDeps

    poke ptr probe = do
        -- Zero the entire struct first
        pokeArray (plusPtr ptr 0 :: Ptr Word8) (replicate probeSize 0)

        -- id[64] at offset 0
        pokeFixedString (plusPtr ptr 0) maxIdLength (probeId probe)
        -- name[128] at offset 64
        pokeFixedString (plusPtr ptr 64) maxNameLength (probeName probe)
        -- version at offset 192
        poke (plusPtr ptr 192) (probeVersion probe)
        -- api_version at offset 204
        poke (plusPtr ptr 204) (probeApiVersion probe)
        -- rustc_version[64] at offset 216
        pokeFixedString (plusPtr ptr 216) maxRustcVersionLength (probeRustcVersion probe)
        -- required_deps_count at offset 280
        pokeByteOff ptr 280 (fromIntegral (length (probeRequiredDeps probe)) :: Word8)
        -- required_deps[8][64] at offset 281
        pokeDepsArray (plusPtr ptr 281) maxDeps maxDepIdLength (probeRequiredDeps probe)
        -- optional_deps_count at offset 793
        pokeByteOff ptr 793 (fromIntegral (length (probeOptionalDeps probe)) :: Word8)
        -- optional_deps[8][64] at offset 794
        pokeDepsArray (plusPtr ptr 794) maxDeps maxDepIdLength (probeOptionalDeps probe)
