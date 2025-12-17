; Rust highlight queries

; Keywords
[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "crate"
  "dyn"
  "else"
  "enum"
  "extern"
  "fn"
  "for"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "match"
  "mod"
  "move"
  "mut"
  "pub"
  "ref"
  "return"
  "self"
  "Self"
  "static"
  "struct"
  "super"
  "trait"
  "type"
  "union"
  "unsafe"
  "use"
  "where"
  "while"
] @keyword

"fn" @keyword.function
"return" @keyword.return
["if" "else" "match"] @keyword.conditional
["for" "while" "loop"] @keyword.repeat
["use" "mod" "crate"] @keyword.import

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(self_parameter (self) @type.builtin)

; Functions
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function.method))
(macro_invocation macro: (identifier) @function.macro)
(macro_invocation macro: (scoped_identifier name: (identifier) @function.macro))

; Variables
(identifier) @variable
(parameter pattern: (identifier) @variable.parameter)
(self_parameter (self) @variable.builtin)

; Fields/Properties
(field_identifier) @property
(shorthand_field_initializer (identifier) @property)

; Constants
(const_item name: (identifier) @constant)
(static_item name: (identifier) @constant)
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))

; Literals
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(escape_sequence) @string.escape
(integer_literal) @number
(float_literal) @number.float
(boolean_literal) @boolean

; Comments
(line_comment) @comment
(block_comment) @comment
((line_comment) @comment.documentation
  (#match? @comment.documentation "^///"))
((block_comment) @comment.documentation
  (#match? @comment.documentation "^/\\*\\*"))

; Attributes
(attribute_item) @attribute
(inner_attribute_item) @attribute

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  ".."
  "..="
  "=>"
  "->"
  "::"
  "?"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[";" "," ":" "."] @punctuation.delimiter
["#" "!"] @punctuation.special

; Lifetime
(lifetime) @label
