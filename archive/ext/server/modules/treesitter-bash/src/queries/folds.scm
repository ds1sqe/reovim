; Bash fold queries

; Function bodies
(function_definition
  body: (compound_statement) @fold)

; If statements
(if_statement) @fold

; Case statements
(case_statement) @fold

; For loops
(for_statement) @fold

; While loops
(while_statement) @fold

; Note: until_statement doesn't exist in tree-sitter-bash

; Subshells
(subshell) @fold

; Compound statements (blocks)
(compound_statement) @fold

; Heredocs
(heredoc_body) @fold
