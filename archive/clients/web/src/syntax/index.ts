/**
 * Syntax highlighting module.
 *
 * Provides token caching and integration with the theme system
 * for syntax highlighting in the web client.
 *
 * @module syntax
 */

export type { CachedToken, TokenSpan, TokenUpdate } from './cache';
export { TokenCache, TokenCacheManager } from './cache';
