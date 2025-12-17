; Rust fold queries
; Capture @fold for regions that can be collapsed

; Function bodies
(function_item body: (block) @fold)

; Closure bodies
(closure_expression body: (block) @fold)

; Struct definitions
(struct_item body: (field_declaration_list) @fold)

; Enum definitions
(enum_item body: (enum_variant_list) @fold)

; Impl blocks
(impl_item body: (declaration_list) @fold)

; Trait definitions
(trait_item body: (declaration_list) @fold)

; Module bodies
(mod_item body: (declaration_list) @fold)

; Match expressions
(match_expression body: (match_block) @fold)

; If expressions with blocks
(if_expression consequence: (block) @fold)
(if_expression alternative: (block) @fold)
(if_expression alternative: (else_clause (block) @fold))

; Loop blocks
(loop_expression body: (block) @fold)
(while_expression body: (block) @fold)
(for_expression body: (block) @fold)

; Use declarations (grouped imports)
(use_declaration (use_tree (use_tree)+ @fold))

; Attribute macros
(attribute_item) @fold

; Documentation comments (consecutive)
(line_comment)+ @fold
(block_comment) @fold

; Macro definitions
(macro_definition) @fold

; Unsafe blocks
(unsafe_block) @fold

; Generic blocks
(block) @fold
