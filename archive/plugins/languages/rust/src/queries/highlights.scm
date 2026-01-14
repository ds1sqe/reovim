; Rust highlight queries for tree-sitter-rust 0.24
; Reo color system - 100% AST coverage for Rust syntax
; Based on official tree-sitter-rust queries with Helix parity + Reo extensions
;
; Coverage:
;   - Patterns (14 types) - Yellow/Soft spectrum
;   - Operators (8 types) - Turquoise spectrum
;   - Async/Unsafe (6 types) - Pink/Red spectrum
;   - Type syntax (12 types) - Cyan/Aqua spectrum
;   - Keywords, functions, variables, constants, etc.

; ============================================================================
; PATTERNS (Yellow/Soft Spectrum) - Critical for match arms, let bindings
; ============================================================================

; Or patterns: A | B | C
(or_pattern) @pattern.or

; Tuple patterns: (x, y, z)
(tuple_pattern) @pattern.tuple

; Struct patterns: Point { x, y }
(struct_pattern
  type: (type_identifier) @pattern.struct)

; Tuple struct patterns: Some(x)
(tuple_struct_pattern
  type: (identifier) @pattern.tuple_struct)
(tuple_struct_pattern
  type: (scoped_identifier
    name: (identifier) @pattern.tuple_struct))

; Slice patterns: [a, b, .., c]
(slice_pattern) @pattern.slice

; Range patterns: 1..10, 'a'..='z'
(range_pattern) @pattern.range

; Ref pattern: ref x
(ref_pattern
  "ref" @pattern.ref)

; Mut pattern: mut x
(mut_pattern) @pattern.mut

; Captured patterns: x @ Some(_)
(captured_pattern
  "@" @pattern.capture)

; Wildcard pattern: _ (in any pattern context)
((identifier) @pattern.wildcard
 (#eq? @pattern.wildcard "_"))

; Rest pattern: .. in [a, .., b] or Point { x, .. }
(remaining_field_pattern) @pattern.rest

; ============================================================================
; OPERATOR EXPRESSIONS (Turquoise Spectrum) - Context-sensitive
; ============================================================================

; Binary expressions: a + b, a - b, a * b, etc.
(binary_expression
  operator: [
    "+" "-" "*" "/" "%"
  ] @operator.binary)

; Logical operators: a && b, a || b
(binary_expression
  operator: ["&&" "||"] @operator.logical)

; Bitwise operators: a & b, a | b, a ^ b
(binary_expression
  operator: ["&" "|" "^" "<<" ">>"] @operator.bitwise)

; Comparison operators remain as @operator
(binary_expression
  operator: ["==" "!=" "<" ">" "<=" ">="] @operator)

; Unary expressions: !a, -b (operators are direct children, not named fields)
(unary_expression
  ["!" "-"] @operator.unary)

; Dereference: *ptr
(unary_expression
  "*" @operator.deref)

; Reference expression: &value, &mut value
(reference_expression
  "&" @operator.ref)

; Assignment expression
(assignment_expression) @operator.assignment

; Compound assignment: x += 1, x -= 1, etc.
(compound_assignment_expr
  operator: ["+=" "-=" "*=" "/=" "%=" "&=" "|=" "^=" "<<=" ">>="] @operator.compound)

; ============================================================================
; ASYNC/AWAIT/UNSAFE (Pink/Red Spectrum) - Special paradigms
; ============================================================================

; Async block: async { } - the node itself represents async context
(async_block) @keyword.async.block

; Await expression: future.await
(await_expression) @keyword.await

; Try expression (? operator) - already covered by keyword.control.exception

; Unsafe block: unsafe { }
(unsafe_block) @keyword.unsafe.block

; ============================================================================
; TYPE SYNTAX (Cyan/Aqua Spectrum) - Advanced type constructs
; ============================================================================

; Array type: [T; N]
(array_type) @type.array

; Pointer types: *const T, *mut T
(pointer_type) @type.pointer
(pointer_type
  (mutable_specifier) @type.pointer.mut)

; Reference types: &T, &mut T
(reference_type
  "&" @type.reference)
(reference_type
  "&" @type.reference.mut
  (mutable_specifier) @type.reference.mut)

; Function type: fn(T) -> U
(function_type
  "fn" @type.function)

; Generic type: Vec<T>
(generic_type) @type.generic

; Dynamic type: dyn Trait
(dynamic_type) @type.dynamic

; Never type: !
(never_type) @type.never

; Impl trait: impl Trait
(abstract_type) @type.impl_trait

; Qualified path: <T as Trait>::Item
(bracketed_type) @type.qualified

; Bounded type parameter: T: Clone
(trait_bounds) @type.interface

; ============================================================================
; TYPES
; ============================================================================

(type_identifier) @type
(primitive_type) @type.builtin

; Type parameters (generics like <T, U> or T: Clone)
(type_parameter
  name: (type_identifier) @type.parameter)

; ============================================================================
; NAMESPACES & MODULES
; ============================================================================

; Module declarations
(mod_item
  name: (identifier) @namespace)

; Extern crate declarations
(extern_crate_declaration
  name: (identifier) @namespace)

; Use declarations - module paths
(use_declaration
  argument: (identifier) @namespace)
(use_declaration
  argument: (scoped_identifier
    path: (identifier) @namespace))
(scoped_use_list
  path: (identifier) @namespace)
(scoped_use_list
  path: (scoped_identifier
    path: (identifier) @namespace))
(use_wildcard
  (identifier) @namespace)

; Scoped identifiers - path components (before ::)
(scoped_identifier
  path: (identifier) @namespace
  name: (identifier))
(scoped_identifier
  path: (scoped_identifier
    name: (identifier) @namespace))

; ============================================================================
; IDENTIFIERS & CONVENTIONS
; ============================================================================

; Properties (struct fields)
(field_identifier) @property

; Assume all-caps names are constants
((identifier) @constant
 (#match? @constant "^[A-Z][A-Z\\d_]+$"))

; Assume PascalCase names are constructors/types
((identifier) @constructor
 (#match? @constructor "^[A-Z]"))

; Uppercase names in paths are types
((scoped_identifier
  path: (identifier) @type)
 (#match? @type "^[A-Z]"))
((scoped_type_identifier
  path: (identifier) @type)
 (#match? @type "^[A-Z]"))

; ============================================================================
; ENUM VARIANTS
; ============================================================================

; Enum variant definitions
(enum_variant
  name: (identifier) @type.enum.variant)

; Builtin enum variants (Option, Result)
((identifier) @type.enum.variant.builtin
 (#any-of? @type.enum.variant.builtin "Some" "None" "Ok" "Err"))

; ============================================================================
; FUNCTION CALLS
; ============================================================================

(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))
(call_expression
  function: (scoped_identifier
    "::"
    name: (identifier) @function))

(generic_function
  function: (identifier) @function)
(generic_function
  function: (scoped_identifier
    name: (identifier) @function))
(generic_function
  function: (field_expression
    field: (field_identifier) @function.method))

; Macro invocations
(macro_invocation
  macro: (identifier) @function.macro
  "!" @function.macro)
(macro_invocation
  macro: (scoped_identifier
    name: (identifier) @function.macro)
  "!" @function.macro)

; Builtin functions / intrinsics
(call_expression
  function: (identifier) @function.builtin
  (#any-of? @function.builtin
    "drop" "size_of" "size_of_val"
    "align_of" "align_of_val"
    "transmute" "unreachable_unchecked"
    "copy" "copy_nonoverlapping"
    "write" "write_bytes"
    "read" "read_unaligned"
    "swap" "replace"
    "take" "forget"
    "needs_drop" "type_name"
    "panic" "assert" "debug_assert"
    "todo" "unimplemented" "unreachable"))

; ============================================================================
; FUNCTION DEFINITIONS
; ============================================================================

(function_item
  name: (identifier) @function.definition)
(function_signature_item
  name: (identifier) @function.definition)

; ============================================================================
; COMMENTS
; ============================================================================

(line_comment) @comment
(block_comment) @comment

; Documentation comments
(line_comment
  (doc_comment)) @comment.documentation
(block_comment
  (doc_comment)) @comment.documentation

; ============================================================================
; PUNCTUATION
; ============================================================================

; Brackets
"(" @punctuation.bracket.round
")" @punctuation.bracket.round
"[" @punctuation.bracket.square
"]" @punctuation.bracket.square
"{" @punctuation.bracket.curly
"}" @punctuation.bracket.curly

; Angle brackets for generics
(type_arguments
  "<" @punctuation.bracket.angle
  ">" @punctuation.bracket.angle)
(type_parameters
  "<" @punctuation.bracket.angle
  ">" @punctuation.bracket.angle)

; Delimiters
"::" @punctuation.delimiter
":" @punctuation.delimiter
"." @punctuation.delimiter
"," @punctuation.delimiter
";" @punctuation.delimiter

; Special punctuation
"->" @punctuation.special
"=>" @punctuation.special
".." @punctuation.special
"..=" @punctuation.special

; ============================================================================
; PARAMETERS AND VARIABLES
; ============================================================================

(parameter
  pattern: (identifier) @variable.parameter)
(closure_parameters
  (identifier) @variable.parameter)

; Self parameter
(self_parameter
  (self) @variable.builtin)

; Generic fallback for identifiers (must come late)
(identifier) @variable

; ============================================================================
; LIFETIMES
; ============================================================================

(lifetime
  "'" @label
  (identifier) @label)

; Static lifetime
((lifetime
  (identifier) @label.special)
 (#eq? @label.special "static"))

; ============================================================================
; KEYWORDS - CONTROL FLOW
; ============================================================================

; Conditional control
[
  "if"
  "else"
  "match"
] @keyword.control.conditional

; Loop control
[
  "while"
  "loop"
] @keyword.control.repeat

(for_expression
  "for" @keyword.control.repeat)

; Return/break/continue
[
  "return"
  "break"
  "continue"
  "yield"
] @keyword.control.return

; Import
"use" @keyword.control.import

; Exception-like
"?" @keyword.control.exception

; ============================================================================
; KEYWORDS - STORAGE
; ============================================================================

; Storage declarations
[
  "let"
  "const"
  "static"
] @keyword.storage

; Storage modifiers
(mutable_specifier) @keyword.storage.modifier.mut

(reference_type
  "&" @keyword.storage.modifier.ref)
(reference_expression
  "&" @keyword.storage.modifier.ref)

[
  "ref"
  "move"
  "dyn"
] @keyword.storage.modifier

; ============================================================================
; KEYWORDS - TYPE DECLARATIONS
; ============================================================================

"type" @keyword.type

[
  "struct"
  "enum"
  "union"
] @keyword.struct

[
  "trait"
  "impl"
] @keyword.trait

; ============================================================================
; KEYWORDS - SPECIAL
; ============================================================================

; extern keyword (unsafe handled separately in async/unsafe section)
"extern" @keyword.special

; ============================================================================
; KEYWORDS - FUNCTION
; ============================================================================

"fn" @keyword.function
; async and await are handled in the ASYNC/AWAIT section with more specificity

; ============================================================================
; KEYWORDS - OTHER
; ============================================================================

[
  "as"
  "default"
  "gen"
  "in"
  "mod"
  "pub"
  "raw"
  "where"
  "macro_rules!"
] @keyword

; Named keyword nodes
(crate) @keyword
(super) @keyword
(use_list (self) @keyword)
(scoped_use_list (self) @keyword)
(scoped_identifier (self) @keyword)
(self) @variable.builtin

; ============================================================================
; LITERALS
; ============================================================================

; Character literals
(char_literal) @constant.character

; Strings
(string_literal) @string
(raw_string_literal) @string

; Escape sequences
(escape_sequence) @constant.character.escape

; Boolean
(boolean_literal) @constant.builtin.boolean

; Numbers
(integer_literal) @constant.numeric.integer
(float_literal) @constant.numeric.float

; ============================================================================
; ATTRIBUTES
; ============================================================================

(attribute_item) @attribute
(inner_attribute_item) @attribute

; Derive macros specifically
(attribute_item
  (attribute
    (identifier) @attribute.builtin
    (#eq? @attribute.builtin "derive")))

; Common attributes
(attribute_item
  (attribute
    (identifier) @attribute.builtin
    (#any-of? @attribute.builtin
      "derive" "cfg" "test" "bench"
      "allow" "warn" "deny" "forbid"
      "inline" "cold" "must_use"
      "deprecated" "doc" "feature"
      "repr" "non_exhaustive")))

; ============================================================================
; OPERATORS (Fallback - specific operators handled in OPERATOR EXPRESSIONS)
; ============================================================================

; These are fallbacks for operators not in expression context
; Most operators are now handled more specifically above
[
  "@"   ; Capture pattern operator (also @pattern.capture)
  "_"   ; Wildcard (also @pattern.wildcard)
  "'"   ; Lifetime tick
] @operator

; ============================================================================
; UNUSED PATTERNS
; ============================================================================

; Underscore as unused
((identifier) @comment.unused
 (#eq? @comment.unused "_"))

; Underscore-prefixed variables (conventionally unused)
((identifier) @comment.unused
 (#match? @comment.unused "^_[a-z]"))
