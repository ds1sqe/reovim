# Text Objects

Text objects are a powerful Vim concept that allow you to select regions of text based on structure. Reovim supports both delimiter-based text objects and semantic text objects powered by treesitter.

## Syntax

Text objects are used with operators (`d`, `y`, `c`, `v`) followed by:
- `i` (inner) - Select content inside delimiters, excluding them
- `a` (around) - Select content including delimiters/surrounding whitespace

## Delimiter-Based Text Objects

These work on matching pairs of characters.

### Parentheses / Brackets / Braces

| Text Object | Description | Example |
|-------------|-------------|---------|
| `i(` / `i)` / `ib` | Inner parentheses | `func(▸arg▸)` → selects `arg` |
| `a(` / `a)` / `ab` | Around parentheses | `func▸(arg)▸` → selects `(arg)` |
| `i[` / `i]` | Inner brackets | `arr[▸0▸]` → selects `0` |
| `a[` / `a]` | Around brackets | `arr▸[0]▸` → selects `[0]` |
| `i{` / `i}` / `iB` | Inner braces | `{ ▸code▸ }` → selects `code` |
| `a{` / `a}` / `aB` | Around braces | `▸{ code }▸` → selects `{ code }` |
| `i<` / `i>` | Inner angle brackets | `Vec<▸T▸>` → selects `T` |
| `a<` / `a>` | Around angle brackets | `Vec▸<T>▸` → selects `<T>` |

### Quotes

| Text Object | Description | Example |
|-------------|-------------|---------|
| `i"` | Inner double quotes | `"▸hello▸"` → selects `hello` |
| `a"` | Around double quotes | `▸"hello"▸` → selects `"hello"` |
| `i'` | Inner single quotes | `'▸x▸'` → selects `x` |
| `a'` | Around single quotes | `▸'x'▸` → selects `'x'` |
| `` i` `` | Inner backticks | `` `▸code▸` `` → selects `code` |
| `` a` `` | Around backticks | `` ▸`code`▸ `` → selects `` `code` `` |

## Word Text Objects

| Text Object | Description |
|-------------|-------------|
| `iw` | Inner word (letters, digits, underscores) |
| `aw` | Around word (includes surrounding whitespace) |
| `iW` | Inner WORD (non-whitespace characters) |
| `aW` | Around WORD (includes surrounding whitespace) |

### Examples

Given: `hello_world foo`
- `diw` on `hello_world` deletes `hello_world`
- `daw` on `hello_world` deletes `hello_world ` (including trailing space)
- `diW` on `hello_world` deletes `hello_world`
- `daW` on `hello_world` deletes `hello_world ` (including trailing space)

Given: `file-name.txt`
- `diw` on `file` deletes `file` (stops at `-`)
- `diW` on `file` deletes `file-name.txt` (all non-whitespace)

## Semantic Text Objects (Treesitter)

Semantic text objects use treesitter to understand code structure. They work across supported languages (Rust, C, JavaScript, Python, JSON, TOML, Markdown).

### Function Text Objects

| Text Object | Description |
|-------------|-------------|
| `if` | Inner function (body only) |
| `af` | Around function (including signature) |

**Rust example:**
```rust
fn example(x: i32) -> i32 {
    let y = x + 1;  // ← `if` selects this
    y * 2
}
// `af` selects the entire function including `fn example...`
```

### Class/Struct Text Objects

| Text Object | Description |
|-------------|-------------|
| `ic` | Inner class/struct (fields/methods only) |
| `ac` | Around class/struct (including declaration) |

**Rust example:**
```rust
struct Point {
    x: f64,  // ← `ic` selects fields
    y: f64,
}
// `ac` selects entire struct including `struct Point`
```

### Argument/Parameter Text Objects

| Text Object | Description |
|-------------|-------------|
| `ia` | Inner argument (single parameter) |
| `aa` | Around argument (including comma/whitespace) |

**Example:**
```rust
fn process(first: i32, second: String, third: bool)
//         ▲          ▲               ▲
//         `ia` selects each individual parameter
```

## Common Operations

### Delete Operations

```vim
di(    " Delete inside parentheses
da{    " Delete around braces (including braces)
di"    " Delete inside double quotes
dif    " Delete inner function body
daf    " Delete entire function
```

### Change Operations

```vim
ci[    " Change inside brackets
ca'    " Change around single quotes
cif    " Change function body
```

### Yank Operations

```vim
yi(    " Yank inside parentheses
ya{    " Yank around braces
yaf    " Yank entire function
```

### Visual Selection

```vim
vi(    " Visually select inside parentheses
vaf    " Visually select entire function
vic    " Visually select inside class/struct
```

## Language Support for Semantic Objects

| Language | Functions | Classes/Structs | Arguments |
|----------|-----------|-----------------|-----------|
| Rust | `fn` | `struct`, `enum`, `impl` | function params |
| C | functions | `struct` | function params |
| JavaScript | `function`, arrow funcs | `class` | function params |
| Python | `def` | `class` | function params |
| JSON | - | objects | - |
| TOML | - | tables | - |
| Markdown | - | - | - |

## Nested Text Objects

Text objects work with nesting. When inside nested structures, the innermost matching pair is selected:

```rust
fn outer() {
    fn inner() {
        let x = (1 + (2 * 3));
        //           ▲ cursor here
        //       `di(` deletes `2 * 3`
        //       `da(` deletes `(2 * 3)`
    }
}
```

## Repeat Count

Text objects support counts to select outer levels:

```rust
let x = ((nested));
//        ▲ cursor here
// `di(` deletes `nested`
// `d2i(` deletes `(nested)`
```

## Implementation Details

### Delimiter Text Objects

Implemented in `server/modules/textobjects/src/`:
- `DelimiterTextObject` handles paired delimiters
- Supports: `()`, `[]`, `{}`, `<>`, `""`, `''`, ``` `` ```

### Semantic Text Objects

Implemented via treesitter in `server/lib/drivers/syntax/`:
- Language-specific queries define what constitutes functions, classes, etc.
- Query files are bundled with the syntax driver
- Falls back gracefully when treesitter is unavailable

### Adding Custom Text Objects

To add semantic text objects for a new language:

1. Create `textobjects.scm` in your language plugin's queries directory
2. Define captures for `@function.inner`, `@function.outer`, etc.
3. Register the language with the treesitter plugin

Example query for functions:
```scheme
(function_definition
  body: (_) @function.inner) @function.outer
```
