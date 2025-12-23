; Bash text object queries

; Functions
(function_definition) @function.outer
(function_definition
  body: (compound_statement) @function.inner)

; Conditionals
(if_statement) @conditional.outer

(case_statement) @conditional.outer

; Loops
(for_statement) @loop.outer

(while_statement) @loop.outer

(until_statement) @loop.outer

; Comments
(comment) @comment.outer

; Parameters (command arguments)
(command
  argument: (_) @parameter.inner)

; Blocks
(compound_statement) @block.outer
(subshell) @block.outer
