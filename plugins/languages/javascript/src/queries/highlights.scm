; JavaScript highlight queries

; Keywords
[
  "async"
  "await"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "default"
  "delete"
  "do"
  "else"
  "export"
  "extends"
  "finally"
  "for"
  "function"
  "if"
  "import"
  "in"
  "instanceof"
  "let"
  "new"
  "of"
  "return"
  "static"
  "switch"
  "throw"
  "try"
  "typeof"
  "var"
  "void"
  "while"
  "with"
  "yield"
] @keyword

"function" @keyword.function
"return" @keyword.return
["if" "else" "switch" "case" "default"] @keyword.conditional
["for" "while" "do"] @keyword.repeat
["import" "export" "from"] @keyword.import

; Types (JSDoc and TypeScript-style)
(type_identifier) @type

; Functions
(function_declaration name: (identifier) @function)
(function_expression name: (identifier) @function)
(method_definition name: (property_identifier) @function.method)
(call_expression function: (identifier) @function)
(call_expression function: (member_expression property: (property_identifier) @function.method))
(arrow_function) @function

; Variables
(identifier) @variable
(formal_parameters (identifier) @variable.parameter)
(shorthand_property_identifier) @variable
(this) @variable.builtin

; Properties
(property_identifier) @property
(shorthand_property_identifier_pattern) @property

; Literals
(string) @string
(template_string) @string
(template_substitution) @punctuation.special
(escape_sequence) @string.escape
(number) @number
(true) @boolean
(false) @boolean
(null) @constant.builtin
(undefined) @constant.builtin
(regex) @string.special

; Comments
(comment) @comment
(hash_bang_line) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "**"
  "="
  "=="
  "==="
  "!="
  "!=="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "??"
  "!"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  ">>>"
  "++"
  "--"
  "+="
  "-="
  "*="
  "/="
  "%="
  "**="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  ">>>="
  "&&="
  "||="
  "??="
  "=>"
  "..."
  "?."
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[";" "," ":" "."] @punctuation.delimiter
