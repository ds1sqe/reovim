//! Tests for `ResolverDispatchProvider`.
//!
//! These tests verify the dispatch pipeline (resolve → handle → changes).
//! Integration tests that exercise the full module resolver chain live
//! in the server crate's test suite.

// TODO: Add unit tests for dispatch_provider
// - Test dispatch_key with mock resolver returning each ResolveResult variant
// - Test PendingBindings population
// - Test command_complete chain
// - Test InjectKeys recursion
