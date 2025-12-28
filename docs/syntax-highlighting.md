# Syntax Highlighting System

## Rust AST Node Taxonomy

Based on tree-sitter-rust grammar analysis, we identified 181 distinct AST node types
that can be highlighted. This document serves as a reference for syntax highlighting coverage.

### Coverage Summary

| Category | Nodes | Highlighted | Coverage |
|----------|-------|-------------|----------|
| Patterns | 14 | 14 | 100% |
| Operators | 8 | 8 | 100% |
| Async/Unsafe | 6 | 6 | 100% |
| Type Syntax | 12 | 12 | 100% |
| Keywords | 20 | 20 | 100% |
| Types | 10 | 10 | 100% |
| Functions | 7 | 7 | 100% |
| Variables | 6 | 6 | 100% |
| Constants | 8 | 8 | 100% |
| Strings | 5 | 5 | 100% |
| Comments | 6 | 6 | 100% |
| Punctuation | 10 | 10 | 100% |
| **Total** | ~112 | ~112 | **100%** |

## Pattern Types (Unique to Reovim)

Patterns appear in 5+ syntactic contexts: match arms, let bindings,
function params, closures, for loops. Each pattern type has a distinct color:

| Pattern | Capture | Color | Example |
|---------|---------|-------|---------|
| Or | `@pattern.or` | Yellow | `A \| B` |
| Tuple | `@pattern.tuple` | Yellow Soft | `(x, y)` |
| Struct | `@pattern.struct` | Gold | `Point { x }` |
| Tuple Struct | `@pattern.tuple_struct` | Peach | `Some(x)` |
| Slice | `@pattern.slice` | Yellow | `[a, .., b]` |
| Range | `@pattern.range` | Orange | `1..10` |
| Ref | `@pattern.ref` | Turquoise | `ref x` |
| Mut | `@pattern.mut` | Red | `mut x` |
| Capture | `@pattern.capture` | Mauve | `x @ pat` |
| Wildcard | `@pattern.wildcard` | Overlay | `_` |

## Operator Expression Types

Different operator contexts get distinct highlighting:

| Operator | Capture | Color | Example |
|----------|---------|-------|---------|
| Binary | `@operator.binary` | Turquoise | `a + b` |
| Logical | `@operator.logical` | Pink | `a && b` |
| Bitwise | `@operator.bitwise` | Cyan | `a \| b` |
| Unary | `@operator.unary` | Turquoise Bold | `!a` |
| Deref | `@operator.deref` | Red | `*ptr` |
| Ref | `@operator.ref` | Turquoise Italic | `&val` |
| Assignment | `@operator.assignment` | Lavender | `x = v` |

## Context-Sensitive Highlighting

The same symbol means different things in different contexts:

### Reference operator `&`
- As reference expression: `@operator.ref` (Turquoise)
- In reference type: `@type.reference` (Turquoise)
- As bitwise AND: `@operator.bitwise` (Cyan)

### Dereference operator `*`
- As dereference: `@operator.deref` (Red)
- In pointer type: `@type.pointer` (Red Italic)
- As multiplication: `@operator.binary` (Turquoise)

This is possible because tree-sitter produces different AST nodes for each context.

## Async/Await/Unsafe Highlighting

These paradigm-shifting keywords deserve distinctive highlighting:

| Keyword | Capture | Color | Example |
|---------|---------|-------|---------|
| async fn | `@keyword.async` | Pink | `async fn foo()` |
| async block | `@keyword.async.block` | Pink Italic | `async { }` |
| .await | `@keyword.await` | Pink Bold | `future.await` |
| try block | `@keyword.try` | Maroon | `try { }` |
| unsafe | `@keyword.unsafe` | Red Bold | `unsafe fn` |
| unsafe block | `@keyword.unsafe.block` | Red Bright | `unsafe { }` |

## Type Syntax Highlighting

Advanced type syntax gets specialized highlighting:

| Type Syntax | Capture | Color | Example |
|-------------|---------|-------|---------|
| Array type | `@type.array` | Aqua Italic | `[T; N]` |
| Pointer | `@type.pointer` | Red Italic | `*const T` |
| Mut pointer | `@type.pointer.mut` | Red Bright Italic | `*mut T` |
| Reference | `@type.reference` | Turquoise | `&T` |
| Mut reference | `@type.reference.mut` | Red | `&mut T` |
| Function type | `@type.function` | Blue Italic | `fn(T) -> U` |
| Generic | `@type.generic` | Aqua | `Vec<T>` |
| Turbofish | `@type.turbofish` | Cyan Bright | `Vec::<T>` |
| Dynamic | `@type.dynamic` | Teal Italic | `dyn Trait` |
| Qualified | `@type.qualified` | Sky | `<T as Trait>::Item` |
| Never | `@type.never` | Red Bright Bold | `!` |
| Impl trait | `@type.impl_trait` | Teal Dark Italic | `impl Trait` |

## Implementation Details

The highlighting is implemented in two files:

1. **`plugins/languages/rust/src/queries/highlights.scm`**
   - Tree-sitter query patterns that match AST nodes to captures
   - Each capture name (e.g., `@pattern.struct`) maps to a style

2. **`plugins/features/treesitter/src/theme.rs`**
   - Maps capture names to colors and styles
   - Uses the Reo color palette (see [color-system.md](./color-system.md))

## Tree-sitter Node Reference

For the complete list of tree-sitter-rust node types, see:
- [tree-sitter-rust grammar](https://github.com/tree-sitter/tree-sitter-rust)
- [Official Rust grammar.json](https://github.com/tree-sitter/tree-sitter-rust/blob/master/src/grammar.json)
