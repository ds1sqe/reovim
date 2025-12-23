; Bash highlight queries for tree-sitter-bash

; Commands
(command_name) @function

; Builtins
((command_name) @function.builtin
 (#any-of? @function.builtin
  "cd" "echo" "exit" "export" "source" "alias" "unalias"
  "set" "unset" "shift" "return" "read" "printf" "eval"
  "exec" "trap" "wait" "kill" "jobs" "bg" "fg"
  "pushd" "popd" "dirs" "history" "type" "which"
  "true" "false" "test" "let" "local" "declare" "typeset"
  "readonly" "getopts" "shopt" "builtin" "command" "enable"))

; Variables
(variable_name) @variable
(special_variable_name) @variable.builtin

; Variable expansions
(simple_expansion) @variable
(expansion) @variable
(command_substitution) @embedded

; Strings
(string) @string
(raw_string) @string
(ansi_c_string) @string
(heredoc_body) @string
(heredoc_start) @label

; Numbers
(number) @number

; Comments
(comment) @comment

; Operators
[
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "=~"
  "+"
  "-"
  "+="
  "-="
  "&&"
  "||"
] @operator

; Delimiters
[
  ";"
  ";;"
  "&"
  "|"
  "|&"
] @punctuation.delimiter

; Keywords - conditionals
[
  "if"
  "then"
  "else"
  "elif"
  "fi"
  "case"
  "esac"
  "in"
] @keyword.conditional

; Keywords - loops
[
  "for"
  "while"
  "until"
  "do"
  "done"
  "select"
] @keyword.repeat

; Keywords - functions
"function" @keyword.function

; Keywords - other
[
  "local"
  "declare"
  "typeset"
  "export"
  "readonly"
] @keyword

; Return/exit
[
  "return"
  "exit"
] @keyword.return

; Brackets
[
  "{"
  "}"
] @punctuation.bracket

[
  "("
  ")"
] @punctuation.bracket

[
  "["
  "]"
  "[["
  "]]"
] @punctuation.bracket

; Redirections
[
  "<"
  ">"
  ">>"
  "<<"
  "<<<"
  "<&"
  ">&"
  ">|"
  "&>"
  "&>>"
] @operator

; Function definitions
(function_definition
  name: (word) @function)

; Array access
(array) @variable
(subscript) @variable

; Negation
"!" @operator

; Glob patterns
(extglob_pattern) @string.special
