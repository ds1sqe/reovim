; Fold regions for Rust
; Captures nodes that should be foldable

; Functions and closures
(function_item
  body: (block) @fold)

(closure_expression
  body: (block) @fold)

; Structs, enums, unions with fields
(struct_item
  body: (field_declaration_list) @fold)

(enum_item
  body: (enum_variant_list) @fold)

(union_item
  body: (field_declaration_list) @fold)

; Impl and trait blocks
(impl_item
  body: (declaration_list) @fold)

(trait_item
  body: (declaration_list) @fold)

; Modules with body
(mod_item
  body: (declaration_list) @fold)

; Match expressions
(match_expression
  body: (match_block) @fold)

; Loops with blocks
(loop_expression
  body: (block) @fold)

(while_expression
  body: (block) @fold)

(for_expression
  body: (block) @fold)

; If expressions with blocks
(if_expression
  consequence: (block) @fold)

; Block comments
(block_comment) @fold

; Macro definitions
(macro_definition) @fold
