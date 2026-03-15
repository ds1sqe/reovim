; Semantic text objects for Python
; Capture convention: @kind.inner / @kind.outer

; Functions
(function_definition
  body: (block) @function.inner) @function.outer

; Classes
(class_definition
  body: (block) @class.inner) @class.outer

; Arguments (function parameters and call arguments)
(parameters
  (_) @argument.inner) @argument.outer
(argument_list
  (_) @argument.inner) @argument.outer

; Conditionals
(if_statement
  consequence: (block) @conditional.inner) @conditional.outer
(elif_clause
  consequence: (block) @conditional.inner) @conditional.outer

; Loops
(for_statement
  body: (block) @loop.inner) @loop.outer
(while_statement
  body: (block) @loop.inner) @loop.outer

; Comments
(comment) @comment.inner @comment.outer

; Blocks
(block) @block.inner @block.outer
