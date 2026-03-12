; Go highlight queries

; Package and imports
(package_clause "package" @keyword)
(import_declaration "import" @keyword)

; Functions
(function_declaration "func" @keyword.function)
(function_declaration name: (identifier) @function)
(method_declaration name: (field_identifier) @function.method)
(call_expression function: (identifier) @function.call)
(call_expression function: (selector_expression field: (field_identifier) @function.call))

; Type declarations
(type_declaration "type" @keyword)
(type_identifier) @type
(struct_type "struct" @keyword)
(interface_type "interface" @keyword)
(map_type "map" @type.builtin)

; Keywords
[
  "var" "const" "return" "if" "else" "for" "range"
  "switch" "case" "default" "break" "continue"
  "defer" "go" "select" "chan" "fallthrough" "goto"
] @keyword

; Strings
(interpreted_string_literal) @string
(raw_string_literal) @string
(rune_literal) @string

; Numbers
(int_literal) @number
(float_literal) @number.float
(imaginary_literal) @number

; Booleans and nil
(true) @boolean
(false) @boolean
(nil) @constant.builtin

; Comments
(comment) @comment

; Variables and properties
(identifier) @variable
(field_identifier) @property

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["." "," ":" ";"] @punctuation.delimiter

; Operators
["+" "-" "*" "/" "%" "&" "|" "^" "<" ">" "=" "!" ":="] @operator
