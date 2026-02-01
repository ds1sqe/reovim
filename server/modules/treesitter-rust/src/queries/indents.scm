; Rust indentation hints
;
; These queries identify constructs that affect indentation level.
; Used by indent_for() to suggest proper indentation.
;
; Capture conventions (following nvim-treesitter):
; - @indent: Increases indent for children
; - @indent_end: Marks end of indented block (closing braces)
; - @dedent: Decreases indent for this node
; - @branch: Node that starts a new branch at same level
; - @ignore: Content that shouldn't affect indent calculation

; Constructs that increase indentation for their children
[
  (function_item)
  (closure_expression)
  (struct_item)
  (enum_item)
  (impl_item)
  (trait_item)
  (mod_item)
  (match_expression)
  (if_expression)
  (while_expression)
  (for_expression)
  (loop_expression)
  (block)
  (use_list)
  (array_expression)
  (tuple_expression)
  (struct_expression)
  (arguments)
  (parameters)
  (type_parameters)
  (field_declaration_list)
  (enum_variant_list)
  (declaration_list)
  (where_clause)
  (match_block)
] @indent

; Closing delimiters end indentation
[
  "}"
  "]"
  ")"
] @indent_end

; Branch nodes (same indent level as siblings)
[
  (else_clause)
  (match_arm)
] @branch

; These should not affect indentation calculation
(line_comment) @ignore
(block_comment) @ignore
(string_literal) @ignore
(raw_string_literal) @ignore
