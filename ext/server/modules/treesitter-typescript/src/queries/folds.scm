; TypeScript fold queries

(function_declaration body: (statement_block) @fold)
(class_declaration body: (class_body) @fold)
(method_definition body: (statement_block) @fold)
(if_statement consequence: (statement_block) @fold)
(for_statement body: (statement_block) @fold)
(for_in_statement body: (statement_block) @fold)
(while_statement body: (statement_block) @fold)
(switch_statement body: (switch_body) @fold)
(try_statement body: (statement_block) @fold)
(interface_declaration body: (interface_body) @fold)
(enum_declaration body: (enum_body) @fold)
(object) @fold
(array) @fold
