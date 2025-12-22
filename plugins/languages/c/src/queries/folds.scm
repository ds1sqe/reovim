; C fold queries
; Capture @fold for regions that can be collapsed

; Function bodies
(function_definition body: (compound_statement) @fold)

; Struct definitions
(struct_specifier body: (field_declaration_list) @fold)

; Union definitions
(union_specifier body: (field_declaration_list) @fold)

; Enum definitions
(enum_specifier body: (enumerator_list) @fold)

; If statements
(if_statement consequence: (compound_statement) @fold)
(if_statement alternative: (compound_statement) @fold)
(if_statement alternative: (else_clause (compound_statement) @fold))

; Switch statements
(switch_statement body: (compound_statement) @fold)

; Loop statements
(for_statement body: (compound_statement) @fold)
(while_statement body: (compound_statement) @fold)
(do_statement body: (compound_statement) @fold)

; Comments
(comment) @fold

; Preprocessor conditionals
(preproc_if) @fold
(preproc_ifdef) @fold
(preproc_else) @fold
(preproc_elif) @fold

; Generic compound statements (blocks)
(compound_statement) @fold
