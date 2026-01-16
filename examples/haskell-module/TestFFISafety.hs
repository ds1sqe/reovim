{-# LANGUAGE ForeignFunctionInterface #-}
{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE ScopedTypeVariables #-}

-- |
-- Module      : TestFFISafety
-- Description : FFI safety tests for Haskell module bindings
--
-- This module provides tests for critical FFI boundary conditions:
--
-- * Exception handling at FFI boundary
-- * StablePtr round-trip correctness
-- * NULL pointer handling
-- * String truncation behavior
--
-- = Running Tests
--
-- @
-- make test
-- @

module Main where

import Control.Exception (SomeException, catch, throwIO, Exception)
import Control.Monad (unless, when)
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
import Foreign.Marshal.Alloc (malloc, free, mallocBytes)
import Foreign.Storable (peek, poke, sizeOf)
import System.Exit (exitFailure, exitSuccess)
import qualified Data.ByteString as BS
import qualified Data.ByteString.Char8 as BS8

import Reovim.FFI
import Reovim.Module

-- | Custom exception for testing
data TestException = TestException String
    deriving (Show)

instance Exception TestException

-- | Test result tracking
data TestResult = TestResult
    { testName :: String
    , testPassed :: Bool
    , testMessage :: String
    }

-- | Run a test and return result
runTest :: String -> IO Bool -> IO TestResult
runTest name action = do
    result <- action `catch` \(e :: SomeException) ->
        return False
    return $ TestResult name result (if result then "OK" else "FAILED")

-- | Assert a condition
assert :: String -> Bool -> IO ()
assert msg cond = unless cond $ do
    putStrLn $ "Assertion failed: " ++ msg
    throwIO $ TestException msg

-- ============================================================================
-- Exception Boundary Tests
-- ============================================================================

-- | Test that safeFFI catches exceptions and returns -2
testExceptionBoundary :: IO Bool
testExceptionBoundary = do
    let throwingAction :: IO Int32
        throwingAction = throwIO (TestException "test exception")

    result <- safeFFI throwingAction
    return $ result == returnPanic  -- Should be -2

-- | Test that safeFFIVoid catches exceptions without crashing
testExceptionBoundaryVoid :: IO Bool
testExceptionBoundaryVoid = do
    let throwingAction :: IO ()
        throwingAction = throwIO (TestException "test exception")

    -- Should not throw, just log
    safeFFIVoid throwingAction
    return True  -- If we got here, test passed

-- | Test that nested exceptions are handled
testNestedExceptions :: IO Bool
testNestedExceptions = do
    let nestedThrow :: IO Int32
        nestedThrow = do
            _ <- safeFFI (throwIO (TestException "inner"))
            throwIO (TestException "outer")

    result <- safeFFI nestedThrow
    return $ result == returnPanic

-- ============================================================================
-- StablePtr Tests
-- ============================================================================

-- | Test basic StablePtr round-trip
testStablePtrRoundTrip :: IO Bool
testStablePtrRoundTrip = do
    let testValue = 42 :: Int

    -- Create StablePtr
    stablePtr <- newStablePtr testValue

    -- Convert to Ptr ()
    let opaquePtr = castStablePtrToPtr stablePtr

    -- Convert back to StablePtr
    let recoveredStable = castPtrToStablePtr opaquePtr :: StablePtr Int

    -- Dereference
    recovered <- deRefStablePtr recoveredStable

    -- Cleanup
    freeStablePtr stablePtr

    return $ recovered == testValue

-- | Test StablePtr with complex data
testStablePtrComplex :: IO Bool
testStablePtrComplex = do
    -- Create complex state
    ref1 <- newIORef (100 :: Int)
    ref2 <- newIORef "hello"
    let state = (ref1, ref2)

    -- Wrap in StablePtr
    stablePtr <- newStablePtr state

    -- Convert through opaque pointer
    let opaquePtr = castStablePtrToPtr stablePtr
    let recovered = castPtrToStablePtr opaquePtr :: StablePtr (IORef Int, IORef String)

    -- Access state
    (r1, r2) <- deRefStablePtr recovered
    v1 <- readIORef r1
    v2 <- readIORef r2

    -- Modify and verify
    writeIORef r1 200
    v1' <- readIORef r1

    -- Cleanup
    freeStablePtr stablePtr

    return $ v1 == 100 && v2 == "hello" && v1' == 200

-- | Test newModulePtr and freeModulePtr
testModulePtrHelpers :: IO Bool
testModulePtrHelpers = do
    let testData = "module state" :: String

    -- Create module pointer
    ptr <- newModulePtr testData

    -- Access via withModulePtr
    result <- withModulePtr ptr $ \(s :: String) -> return s

    -- Cleanup
    freeModulePtr ptr

    return $ result == Just testData

-- | Test that freeModulePtr handles null safely
testFreeNullPtr :: IO Bool
testFreeNullPtr = do
    -- Should not crash
    freeModulePtr nullPtr
    return True

-- ============================================================================
-- NULL Pointer Handling Tests
-- ============================================================================

-- | Test withModulePtr with null pointer
testWithNullPtr :: IO Bool
testWithNullPtr = do
    result <- withModulePtr nullPtr $ \(_ :: Int) -> return 42
    return $ result == Nothing

-- ============================================================================
-- Type Tests
-- ============================================================================

-- | Test ReovimVersion Storable instance
testVersionStorable :: IO Bool
testVersionStorable = do
    let version = ReovimVersion 1 2 3

    -- Allocate and write
    ptr <- malloc :: IO (Ptr ReovimVersion)
    poke ptr version

    -- Read back
    read_version <- peek ptr

    -- Cleanup
    free ptr

    return $ read_version == version

-- | Test ReovimModuleProbe Storable instance
testProbeStorable :: IO Bool
testProbeStorable = do
    let probe = buildProbe $
            newProbe
                & withId "test-id"
                & withName "Test Module"
                & withVersion 1 2 3
                & withRequiredDep "dep1"
                & withOptionalDep "opt1"
          where (&) = flip ($)

    -- Allocate and write
    ptr <- mallocBytes probeSize :: IO (Ptr ReovimModuleProbe)
    poke ptr probe

    -- Read back
    read_probe <- peek ptr

    -- Cleanup
    free ptr

    -- Compare fields
    return $ probeId read_probe == probeId probe
          && probeName read_probe == probeName probe
          && probeVersion read_probe == probeVersion probe
          && probeRequiredDeps read_probe == probeRequiredDeps probe

-- | Test string truncation in probe builder
testStringTruncation :: IO Bool
testStringTruncation = do
    -- Create string longer than max ID length (64)
    let longId = BS8.pack $ replicate 100 'x'
    let probe = buildProbe $ newProbe & withId longId
          where (&) = flip ($)

    -- ID should be truncated to 63 chars (64 - 1 for null)
    return $ BS.length (probeId probe) == 63

-- | Test max dependencies limit
testMaxDependencies :: IO Bool
testMaxDependencies = do
    -- Try to add more than 8 dependencies
    let probe = buildProbe $
            newProbe
                & withRequiredDep "dep1"
                & withRequiredDep "dep2"
                & withRequiredDep "dep3"
                & withRequiredDep "dep4"
                & withRequiredDep "dep5"
                & withRequiredDep "dep6"
                & withRequiredDep "dep7"
                & withRequiredDep "dep8"
                & withRequiredDep "dep9"  -- Should be ignored
                & withRequiredDep "dep10" -- Should be ignored
          where (&) = flip ($)

    -- Should only have 8 dependencies
    return $ length (probeRequiredDeps probe) == 8

-- ============================================================================
-- Version Compatibility Tests
-- ============================================================================

-- | Test version compatibility logic
testVersionCompatibility :: IO Bool
testVersionCompatibility = do
    let v100 = ReovimVersion 1 0 0
        v110 = ReovimVersion 1 1 0
        v120 = ReovimVersion 1 2 0
        v200 = ReovimVersion 2 0 0

    -- Same version is compatible
    let test1 = isCompatible v100 v100

    -- Higher minor is compatible
    let test2 = isCompatible v100 v110

    -- Lower minor is not compatible
    let test3 = not $ isCompatible v120 v100

    -- Different major is not compatible
    let test4 = not $ isCompatible v100 v200

    return $ test1 && test2 && test3 && test4

-- ============================================================================
-- Main Test Runner
-- ============================================================================

main :: IO ()
main = do
    putStrLn "Running FFI Safety Tests"
    putStrLn "========================"
    putStrLn ""

    results <- sequence
        [ runTest "Exception boundary (Int32)" testExceptionBoundary
        , runTest "Exception boundary (void)" testExceptionBoundaryVoid
        , runTest "Nested exceptions" testNestedExceptions
        , runTest "StablePtr round-trip" testStablePtrRoundTrip
        , runTest "StablePtr complex data" testStablePtrComplex
        , runTest "Module pointer helpers" testModulePtrHelpers
        , runTest "Free null pointer" testFreeNullPtr
        , runTest "withModulePtr null" testWithNullPtr
        , runTest "Version Storable" testVersionStorable
        , runTest "Probe Storable" testProbeStorable
        , runTest "String truncation" testStringTruncation
        , runTest "Max dependencies" testMaxDependencies
        , runTest "Version compatibility" testVersionCompatibility
        ]

    putStrLn ""
    putStrLn "Results:"
    putStrLn "--------"

    mapM_ printResult results

    let passed = length $ filter testPassed results
    let total = length results

    putStrLn ""
    putStrLn $ "Passed: " ++ show passed ++ "/" ++ show total

    if passed == total
        then do
            putStrLn "All tests passed!"
            exitSuccess
        else do
            putStrLn "Some tests failed!"
            exitFailure

printResult :: TestResult -> IO ()
printResult (TestResult name passed msg) =
    putStrLn $ "[" ++ status ++ "] " ++ name
  where
    status = if passed then "PASS" else "FAIL"
