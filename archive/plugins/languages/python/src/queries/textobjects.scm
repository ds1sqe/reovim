; Python text object queries for semantic text objects
; Captures: @<object>.<scope> where scope is "inner" or "outer"

; Functions
(function_definition
  body: (block) @function.inner) @function.outer

; Lambdas
(lambda
  body: (_) @function.inner) @function.outer

; Classes
(class_definition
  body: (block) @class.inner) @class.outer

; Parameters/Arguments
(parameters
  (identifier) @parameter.inner) @parameter.outer

(parameters
  (default_parameter) @parameter.inner)

(parameters
  (typed_parameter) @parameter.inner)

(argument_list
  (_) @parameter.inner) @parameter.outer

; Conditionals
(if_statement
  consequence: (block) @conditional.inner) @conditional.outer

(elif_clause
  consequence: (block) @conditional.inner) @conditional.outer

(else_clause
  body: (block) @conditional.inner) @conditional.outer

(match_statement
  body: (block) @conditional.inner) @conditional.outer

(case_clause
  consequence: (block) @conditional.inner) @conditional.outer

; Conditional expressions (ternary)
(conditional_expression) @conditional.outer

; Loops
(for_statement
  body: (block) @loop.inner) @loop.outer

(while_statement
  body: (block) @loop.inner) @loop.outer

; Comprehensions
(list_comprehension) @loop.outer
(dictionary_comprehension) @loop.outer
(set_comprehension) @loop.outer
(generator_expression) @loop.outer

; Comments
(comment) @comment.outer

; Generic blocks
(block) @block.inner @block.outer

; Try/Except blocks
(try_statement
  body: (block) @block.inner) @block.outer

(except_clause
  (block) @block.inner) @block.outer

(finally_clause
  (block) @block.inner) @block.outer

; With statements
(with_statement
  body: (block) @block.inner) @block.outer
