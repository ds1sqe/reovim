; Rust text object queries for semantic text objects
; Captures: @<object>.<scope> where scope is "inner" or "outer"

; Functions
(function_item
  body: (block) @function.inner) @function.outer

; Closures
(closure_expression
  body: (_) @function.inner) @function.outer

; Methods in impl blocks
(function_item
  body: (block) @function.inner) @function.outer

; Classes (structs, enums, impl blocks)
(struct_item
  body: (field_declaration_list) @class.inner) @class.outer

(enum_item
  body: (enum_variant_list) @class.inner) @class.outer

(impl_item
  body: (declaration_list) @class.inner) @class.outer

(trait_item
  body: (declaration_list) @class.inner) @class.outer

; Parameters/Arguments
(parameters
  (parameter) @parameter.inner) @parameter.outer

(arguments
  (_) @parameter.inner) @parameter.outer

; Conditionals
(if_expression
  consequence: (block) @conditional.inner) @conditional.outer

(match_expression
  body: (match_block) @conditional.inner) @conditional.outer

(match_arm
  value: (_) @conditional.inner) @conditional.outer

; Loops
(loop_expression
  body: (block) @loop.inner) @loop.outer

(while_expression
  body: (block) @loop.inner) @loop.outer

(for_expression
  body: (block) @loop.inner) @loop.outer

; Comments
(line_comment) @comment.outer
(block_comment) @comment.outer

; Generic blocks
(block) @block.inner @block.outer

; Module blocks
(mod_item
  body: (declaration_list) @block.inner) @block.outer
