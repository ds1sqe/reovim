# Text Objects

Text objects are a powerful Vim concept that allow you to select regions of text based on structure. Reovim supports delimiter-based text objects for matching pairs, quotes, words, sentences, and paragraphs.

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

### Tags

| Text Object | Description | Example |
|-------------|-------------|---------|
| `it` | Inner tag | `<div>▸content▸</div>` → selects `content` |
| `at` | Around tag | `▸<div>content</div>▸` → selects `<div>content</div>` |

### Sentence and Paragraph

| Text Object | Description |
|-------------|-------------|
| `is` | Inner sentence |
| `as` | Around sentence (includes trailing whitespace) |
| `ip` | Inner paragraph |
| `ap` | Around paragraph (includes trailing blank lines) |

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

## Common Operations

### Delete Operations

```vim
di(    " Delete inside parentheses
da{    " Delete around braces (including braces)
di"    " Delete inside double quotes
dip    " Delete inner paragraph
```

### Change Operations

```vim
ci[    " Change inside brackets
ca'    " Change around single quotes
cit    " Change inside tag
```

### Yank Operations

```vim
yi(    " Yank inside parentheses
ya{    " Yank around braces
yis    " Yank inner sentence
```

### Visual Selection

```vim
vi(    " Visually select inside parentheses
vaw    " Visually select around word
vap    " Visually select around paragraph
```

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

## Semantic Text Objects (Tree-sitter)

Semantic text objects are powered by the tree-sitter syntax driver. They understand
language structure rather than purely textual delimiters, and work across all languages
that have a tree-sitter grammar loaded.

> **Requirement**: A tree-sitter module must be active for the current file type (e.g.,
> `reovim-module-treesitter-rust` for Rust files). If no grammar is loaded, the text
> object silently does nothing.

### Available Semantic Text Objects

| Text Object | Inner | Around | Scope | Description |
|-------------|-------|--------|-------|-------------|
| Function | `if` | `af` | Linewise | Function/method body and signature |
| Class | `ic` | `ac` | Linewise | Class, struct, or impl block |
| Argument | `ia` | `aa` | Characterwise | Function call argument or parameter |
| Conditional | `io` | `ao` | Linewise | `if`/`else`/`match` body |
| Loop | `il` | `al` | Linewise | `for`/`while`/`loop` body |
| Comment | `i/` | `a/` | Characterwise | Comment text (without `//` markers) or full comment |

The TS-block kind (`inner-block-ts` / `around-block-ts`) is implemented in the command
layer but has no default keybinding yet.

### Inner vs Around

- **Inner** (`i`): selects the body, excluding the surrounding syntax (signature, keyword,
  delimiters).
- **Around** (`a`): selects the full construct including surrounding syntax.

### Examples

**Function (`if` / `af`) — Rust:**

```rust
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
// cursor anywhere inside the function body:
// `dif` deletes only the function body (between the braces)
// `daf` deletes the entire function including signature
```

**Argument (`ia` / `aa`) — any language:**

```rust
foo(first, second, third)
//          ^^^^^^ cursor on "second"
// `dia` deletes "second" (inner argument)
// `daa` deletes "second, " (around argument, including the separator)
```

**Conditional (`io` / `ao`) — Rust:**

```rust
if condition {
    do_something();
}
// `dio` deletes the body "do_something();"
// `dao` deletes the entire if-block
```

**Comment (`i/` / `a/`):**

```rust
// This is a comment
// `yi/` yanks the comment text without the `// ` prefix
// `ya/` yanks the whole comment line including markers
```

### Supported Languages

Semantic text objects are available for any language with a loaded tree-sitter grammar.
Built-in grammars include: Rust, Python, Go, C, JavaScript, TypeScript, Bash, JSON,
TOML, Markdown.

---

## Implementation Details

### Text Object Module

Implemented in `ext/server/modules/textobjects/src/`:
- Handles paired delimiters: `()`, `[]`, `{}`, `<>`, `""`, `''`, ``` `` ```
- Handles tags: `<tag>...</tag>`
- Handles word, WORD, sentence, and paragraph boundaries
- Semantic (tree-sitter) objects in `ext/server/modules/textobjects/src/semantic.rs`
- Command IDs defined in `ext/server/modules/textobjects/src/ids.rs`
