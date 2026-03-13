; Python fold queries
; Capture @fold for regions that can be collapsed

; Function bodies
(function_definition body: (block) @fold)

; Note: lambda expressions have expression bodies, not blocks, so no fold here

; Class definitions
(class_definition body: (block) @fold)

; If statements
(if_statement consequence: (block) @fold)
(elif_clause consequence: (block) @fold)
(else_clause body: (block) @fold)

; Match statements (Python 3.10+)
(match_statement body: (block) @fold)
(case_clause consequence: (block) @fold)

; Loop statements
(for_statement body: (block) @fold)
(while_statement body: (block) @fold)

; Try-except-finally
(try_statement body: (block) @fold)
(except_clause (block) @fold)
(finally_clause (block) @fold)

; With statements
(with_statement body: (block) @fold)

; Comprehensions
(list_comprehension) @fold
(dictionary_comprehension) @fold
(set_comprehension) @fold
(generator_expression) @fold

; Comments and docstrings
(comment) @fold
(expression_statement (string) @fold)

; Decorated definitions
(decorated_definition) @fold

; Import statements (multiline)
(import_from_statement) @fold
