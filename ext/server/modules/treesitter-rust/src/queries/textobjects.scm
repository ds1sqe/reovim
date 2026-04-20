; Semantic text objects for Rust
; Capture convention: @kind.inner / @kind.outer

; Functions
(function_item
  body: (block) @function.inner) @function.outer
(closure_expression
  body: (_) @function.inner) @function.outer

; Classes (structs, enums, traits, impl blocks)
(struct_item
  body: (field_declaration_list) @class.inner) @class.outer
(enum_item
  body: (enum_variant_list) @class.inner) @class.outer
(trait_item
  body: (declaration_list) @class.inner) @class.outer
(impl_item
  body: (declaration_list) @class.inner) @class.outer

; Arguments (function parameters and call arguments)
(parameters
  (_) @argument.inner) @argument.outer
(arguments
  (_) @argument.inner) @argument.outer
(type_arguments
  (_) @argument.inner) @argument.outer

; Conditionals
(if_expression
  consequence: (block) @conditional.inner) @conditional.outer
(match_expression
  body: (match_block) @conditional.inner) @conditional.outer

; Loops
(for_expression
  body: (block) @loop.inner) @loop.outer
(while_expression
  body: (block) @loop.inner) @loop.outer
(loop_expression
  body: (block) @loop.inner) @loop.outer

; Comments
(line_comment) @comment.inner @comment.outer
(block_comment) @comment.inner @comment.outer

; Blocks
(block) @block.inner @block.outer
