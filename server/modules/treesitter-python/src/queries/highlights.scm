; Python highlight queries

; Keywords
[
  "and"
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "exec"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "in"
  "is"
  "lambda"
  "nonlocal"
  "not"
  "or"
  "pass"
  "print"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
  "match"
  "case"
] @keyword

"def" @keyword.function
"return" @keyword.return
["if" "elif" "else" "match" "case"] @keyword.conditional
["for" "while"] @keyword.repeat
["import" "from"] @keyword.import

; Types
(type) @type
((identifier) @type
  (#match? @type "^[A-Z]"))

; Builtins
((identifier) @type.builtin
  (#any-of? @type.builtin
    "int" "float" "str" "bool" "list" "dict" "set" "tuple"
    "bytes" "bytearray" "memoryview" "range" "frozenset"
    "complex" "object" "type"))

((identifier) @function.builtin
  (#any-of? @function.builtin
    "abs" "all" "any" "ascii" "bin" "bool" "breakpoint" "bytearray"
    "bytes" "callable" "chr" "classmethod" "compile" "complex"
    "delattr" "dict" "dir" "divmod" "enumerate" "eval" "exec"
    "filter" "float" "format" "frozenset" "getattr" "globals"
    "hasattr" "hash" "help" "hex" "id" "input" "int" "isinstance"
    "issubclass" "iter" "len" "list" "locals" "map" "max"
    "memoryview" "min" "next" "object" "oct" "open" "ord" "pow"
    "print" "property" "range" "repr" "reversed" "round" "set"
    "setattr" "slice" "sorted" "staticmethod" "str" "sum" "super"
    "tuple" "type" "vars" "zip" "__import__"))

; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function)
(call function: (attribute attribute: (identifier) @function.method))
(decorator "@" @attribute (identifier) @attribute)
(decorator "@" @attribute (attribute attribute: (identifier) @attribute))

; Variables
(identifier) @variable
(parameters (identifier) @variable.parameter)
(default_parameter name: (identifier) @variable.parameter)
(typed_parameter (identifier) @variable.parameter)
(keyword_argument name: (identifier) @variable.parameter)
((identifier) @variable.builtin
  (#any-of? @variable.builtin "self" "cls"))

; Properties/Attributes
(attribute attribute: (identifier) @property)

; Literals
(string) @string
(escape_sequence) @string.escape
(interpolation) @punctuation.special
(integer) @number
(float) @number.float
(true) @boolean
(false) @boolean
(none) @constant.builtin

; Comments
(comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "//"
  "%"
  "**"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "+="
  "-="
  "*="
  "/="
  "//="
  "%="
  "**="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  "@="
  "->"
  ":="
  "@"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
[":" "," "." ";"] @punctuation.delimiter
