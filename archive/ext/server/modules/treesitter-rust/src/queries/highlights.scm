; Rust highlights query (minimal version for tree-sitter-rust 0.23)
; Only using guaranteed node types

; Comments
(line_comment) @comment
(block_comment) @comment

; Literals
(string_literal) @string
(raw_string_literal) @string
(char_literal) @character
(escape_sequence) @string.escape
(boolean_literal) @boolean
(integer_literal) @number
(float_literal) @number

; Function keyword "fn"
(function_item "fn" @keyword.function)
(function_signature_item "fn" @keyword.function)

; Functions
(function_item
  name: (identifier) @function)

(call_expression
  function: (identifier) @function.call)

(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))

; Macros
(macro_invocation
  macro: (identifier) @function.macro)

(macro_definition
  name: (identifier) @function.macro)

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Structs and enums
(struct_item
  name: (type_identifier) @type)

(enum_item
  name: (type_identifier) @type)

; Traits
(trait_item
  name: (type_identifier) @type)

; Variables and parameters
(identifier) @variable

(parameter
  pattern: (identifier) @variable.parameter)

; Fields
(field_identifier) @variable.field

(field_declaration
  name: (field_identifier) @variable.field)

; Constants
(const_item
  name: (identifier) @constant)

; Lifetimes
(lifetime) @label

; Attributes
(attribute_item) @attribute
(inner_attribute_item) @attribute

; Modules
(mod_item
  name: (identifier) @namespace)

; Keywords - use mutable_specifier for "mut" instead of literal
(mutable_specifier) @keyword

; Self and super as builtins
(self) @variable.builtin
