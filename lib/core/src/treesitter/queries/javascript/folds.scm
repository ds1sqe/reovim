; JavaScript fold queries
; Capture @fold for regions that can be collapsed

; Function bodies
(function_declaration body: (statement_block) @fold)
(function_expression body: (statement_block) @fold)
(arrow_function body: (statement_block) @fold)
(method_definition body: (statement_block) @fold)
(generator_function_declaration body: (statement_block) @fold)

; Class definitions
(class_declaration body: (class_body) @fold)
(class_expression body: (class_body) @fold)

; If statements
(if_statement consequence: (statement_block) @fold)
(if_statement alternative: (statement_block) @fold)
(if_statement alternative: (else_clause (statement_block) @fold))

; Switch statements
(switch_statement body: (switch_body) @fold)

; Loop statements
(for_statement body: (statement_block) @fold)
(for_in_statement body: (statement_block) @fold)
(while_statement body: (statement_block) @fold)
(do_statement body: (statement_block) @fold)

; Try-catch-finally
(try_statement body: (statement_block) @fold)
(catch_clause body: (statement_block) @fold)
(finally_clause body: (statement_block) @fold)

; Object literals
(object) @fold

; Array literals
(array) @fold

; Comments
(comment) @fold

; Import/export statements
(import_statement) @fold
(export_statement) @fold

; Generic statement blocks
(statement_block) @fold
