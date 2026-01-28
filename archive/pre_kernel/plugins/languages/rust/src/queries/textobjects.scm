; Rust text object queries for semantic text objects
; Based on tree-sitter-rust 0.24 grammar

; Functions
(function_item) @function.outer
(function_item
  body: (block) @function.inner)

(function_signature_item) @function.outer

; Closures
(closure_expression) @function.outer
(closure_expression
  body: (_) @function.inner)

; Classes (structs, enums, impl blocks)
(struct_item) @class.outer
(struct_item
  body: (field_declaration_list) @class.inner)

(enum_item) @class.outer
(enum_item
  body: (enum_variant_list) @class.inner)

(impl_item) @class.outer
(impl_item
  body: (declaration_list) @class.inner)

(trait_item) @class.outer
(trait_item
  body: (declaration_list) @class.inner)

(union_item) @class.outer
(union_item
  body: (field_declaration_list) @class.inner)

; Parameters/Arguments
(parameters) @parameter.outer
(parameters
  (parameter) @parameter.inner)

(arguments) @parameter.outer
(arguments
  (_) @parameter.inner)

; Conditionals
(if_expression) @conditional.outer
(if_expression
  consequence: (block) @conditional.inner)

(match_expression) @conditional.outer
(match_expression
  body: (match_block) @conditional.inner)

(match_arm) @conditional.outer

; Loops
(loop_expression) @loop.outer
(loop_expression
  body: (block) @loop.inner)

(while_expression) @loop.outer
(while_expression
  body: (block) @loop.inner)

(for_expression) @loop.outer
(for_expression
  body: (block) @loop.inner)

; Comments
(line_comment) @comment.outer
(block_comment) @comment.outer

; Generic blocks
(block) @block.inner
(block) @block.outer

; Module blocks
(mod_item) @block.outer
(mod_item
  body: (declaration_list) @block.inner)
