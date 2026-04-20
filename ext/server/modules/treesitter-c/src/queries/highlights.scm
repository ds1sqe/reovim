; C highlight queries

; Keywords
[
  "break"
  "case"
  "const"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extern"
  "for"
  "goto"
  "if"
  "inline"
  "register"
  "return"
  "sizeof"
  "static"
  "struct"
  "switch"
  "typedef"
  "union"
  "volatile"
  "while"
] @keyword

"return" @keyword.return
["if" "else" "switch" "case" "default"] @keyword.conditional
["for" "while" "do"] @keyword.repeat

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(sized_type_specifier) @type.builtin

; Functions
(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function.method))

; Variables
(identifier) @variable
(parameter_declaration declarator: (identifier) @variable.parameter)

; Fields
(field_identifier) @property
(field_expression field: (field_identifier) @property)

; Preprocessor
(preproc_directive) @keyword
(preproc_include) @keyword.import
(preproc_def name: (identifier) @constant)
(preproc_function_def name: (identifier) @function.macro)

; Literals
(string_literal) @string
(system_lib_string) @string
(char_literal) @string
(escape_sequence) @string.escape
(number_literal) @number
(null) @constant.builtin
(true) @boolean
(false) @boolean

; Comments
(comment) @comment

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
  "++"
  "--"
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
  "->"
  "."
  "?"
  ":"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[";" ","] @punctuation.delimiter
