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

## Implementation Details

### Text Object Module

Implemented in `server/modules/textobjects/src/`:
- Handles paired delimiters: `()`, `[]`, `{}`, `<>`, `""`, `''`, ``` `` ```
- Handles tags: `<tag>...</tag>`
- Handles word, WORD, sentence, and paragraph boundaries
- Command IDs defined in `server/modules/textobjects/src/ids.rs`
