; JavaScript text object queries for semantic text objects
; Captures: @<object>.<scope> where scope is "inner" or "outer"

; Functions
(function_declaration
  body: (statement_block) @function.inner) @function.outer

(function_expression
  body: (statement_block) @function.inner) @function.outer

(arrow_function
  body: (_) @function.inner) @function.outer

(method_definition
  body: (statement_block) @function.inner) @function.outer

(generator_function_declaration
  body: (statement_block) @function.inner) @function.outer

; Classes
(class_declaration
  body: (class_body) @class.inner) @class.outer

; Parameters/Arguments
(formal_parameters
  (identifier) @parameter.inner) @parameter.outer

(arguments
  (_) @parameter.inner) @parameter.outer

; Conditionals
(if_statement
  consequence: (statement_block) @conditional.inner) @conditional.outer

(else_clause
  (statement_block) @conditional.inner) @conditional.outer

(switch_statement
  body: (switch_body) @conditional.inner) @conditional.outer

(ternary_expression) @conditional.outer

; Loops
(for_statement
  body: (statement_block) @loop.inner) @loop.outer

(for_in_statement
  body: (statement_block) @loop.inner) @loop.outer

(while_statement
  body: (statement_block) @loop.inner) @loop.outer

(do_statement
  body: (statement_block) @loop.inner) @loop.outer

; Comments
(comment) @comment.outer

; Generic blocks
(statement_block) @block.inner @block.outer

; Object literals
(object
  (_) @block.inner) @block.outer

; Array literals
(array
  (_) @block.inner) @block.outer
