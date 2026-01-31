; C text object queries for semantic text objects
; Captures: @<object>.<scope> where scope is "inner" or "outer"

; Functions
(function_definition
  body: (compound_statement) @function.inner) @function.outer

; Classes (structs, unions)
(struct_specifier
  body: (field_declaration_list) @class.inner) @class.outer

(union_specifier
  body: (field_declaration_list) @class.inner) @class.outer

(enum_specifier
  body: (enumerator_list) @class.inner) @class.outer

; Parameters/Arguments
(parameter_list
  (parameter_declaration) @parameter.inner) @parameter.outer

(argument_list
  (_) @parameter.inner) @parameter.outer

; Conditionals
(if_statement
  consequence: (compound_statement) @conditional.inner) @conditional.outer

(else_clause
  (compound_statement) @conditional.inner) @conditional.outer

(switch_statement
  body: (compound_statement) @conditional.inner) @conditional.outer

(case_statement) @conditional.outer

; Loops
(for_statement
  body: (compound_statement) @loop.inner) @loop.outer

(while_statement
  body: (compound_statement) @loop.inner) @loop.outer

(do_statement
  body: (compound_statement) @loop.inner) @loop.outer

; Comments
(comment) @comment.outer

; Generic blocks
(compound_statement) @block.inner @block.outer
