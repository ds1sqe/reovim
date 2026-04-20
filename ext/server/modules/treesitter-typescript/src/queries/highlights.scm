; TypeScript highlight queries

; Keywords
[
  "abstract" "as" "async" "await" "break" "case" "catch"
  "class" "const" "continue" "debugger" "default" "delete"
  "do" "else" "enum" "export" "extends" "finally" "for"
  "from" "function" "get" "if" "implements" "import" "in"
  "instanceof" "interface" "keyof" "let" "new" "of"
  "readonly" "return" "set" "static" "switch" "throw"
  "try" "type" "typeof" "var" "void" "while" "with" "yield"
] @keyword

; Functions
(function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function.method)
(call_expression function: (identifier) @function.call)
(call_expression function: (member_expression property: (property_identifier) @function.call))
(arrow_function) @function

; Types
(type_identifier) @type
(predefined_type) @type.builtin

; Strings
(string) @string
(template_string) @string
(template_substitution) @punctuation.special
(escape_sequence) @string.escape
(regex) @string.special

; Numbers
(number) @number

; Booleans and special
(true) @boolean
(false) @boolean
(null) @constant.builtin
(undefined) @constant.builtin

; Comments
(comment) @comment

; Variables and properties
(identifier) @variable
(property_identifier) @property
(shorthand_property_identifier) @variable

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["." "," ":" ";"] @punctuation.delimiter
[
  "+" "-" "*" "/" "%" "**"
  "=" "+=" "-=" "*=" "/="
  "==" "===" "!=" "!=="
  "<" ">" "<=" ">="
  "&&" "||" "!" "??"
  "&" "|" "^" "~" "<<" ">>"
  "=>" "..." "?"
] @operator
