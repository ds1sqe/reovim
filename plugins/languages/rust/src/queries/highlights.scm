; Rust highlight queries for tree-sitter-rust 0.24
; Based on official tree-sitter-rust queries

; Identifiers

(type_identifier) @type
(primitive_type) @type.builtin
(field_identifier) @property

; Identifier conventions

; Assume all-caps names are constants
((identifier) @constant
 (#match? @constant "^[A-Z][A-Z\\d_]+$"))

; Assume uppercase names are constructors
((identifier) @constructor
 (#match? @constructor "^[A-Z]"))

; Assume that uppercase names in paths are types
((scoped_identifier
  path: (identifier) @type)
 (#match? @type "^[A-Z]"))
((scoped_type_identifier
  path: (identifier) @type)
 (#match? @type "^[A-Z]"))

; Function calls

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

(macro_invocation
  macro: (identifier) @function.macro
  "!" @function.macro)
(macro_invocation
  macro: (scoped_identifier
    name: (identifier) @function.macro)
  "!" @function.macro)

; Function definitions

(function_item (identifier) @function)
(function_signature_item (identifier) @function)

; Comments

(line_comment) @comment
(block_comment) @comment
(line_comment (doc_comment)) @comment.documentation
(block_comment (doc_comment)) @comment.documentation

; Punctuation - specific bracket types

"(" @punctuation.bracket.round
")" @punctuation.bracket.round
"[" @punctuation.bracket.square
"]" @punctuation.bracket.square
"{" @punctuation.bracket.curly
"}" @punctuation.bracket.curly

(type_arguments
  "<" @punctuation.bracket.angle
  ">" @punctuation.bracket.angle)
(type_parameters
  "<" @punctuation.bracket.angle
  ">" @punctuation.bracket.angle)

"::" @punctuation.delimiter
":" @punctuation.delimiter
"." @punctuation.delimiter
"," @punctuation.delimiter
";" @punctuation.delimiter

; Parameters and variables

(parameter (identifier) @variable.parameter)
(identifier) @variable

; Lifetimes

(lifetime (identifier) @label)

; Keywords

"as" @keyword
"async" @keyword
"await" @keyword
"break" @keyword
"continue" @keyword
"default" @keyword
"dyn" @keyword
"else" @keyword.conditional
"extern" @keyword
"fn" @keyword.function
"for" @keyword.repeat
"gen" @keyword
"if" @keyword.conditional
"in" @keyword
"loop" @keyword.repeat
"macro_rules!" @keyword
"match" @keyword.conditional
"mod" @keyword
"move" @keyword
"pub" @keyword
"raw" @keyword
"ref" @keyword
"return" @keyword.return
"unsafe" @keyword
"use" @keyword.import
"where" @keyword
"while" @keyword.repeat
"yield" @keyword

; Declaration keywords
"let" @keyword.storage
"const" @keyword.storage
"static" @keyword.storage
"type" @keyword.type
"struct" @keyword.struct
"enum" @keyword.enum
"trait" @keyword.struct
"impl" @keyword.struct
"union" @keyword.struct

; Named keyword nodes
(crate) @keyword
(mutable_specifier) @keyword
(super) @keyword
(use_list (self) @keyword)
(scoped_use_list (self) @keyword)
(scoped_identifier (self) @keyword)
(self) @variable.builtin

; Literals

(char_literal) @string
(string_literal) @string
(raw_string_literal) @string
(boolean_literal) @boolean
(integer_literal) @number
(float_literal) @number
(escape_sequence) @string.escape

; Attributes

(attribute_item) @attribute
(inner_attribute_item) @attribute

; Operators

"*" @operator
"&" @operator
"'" @operator
