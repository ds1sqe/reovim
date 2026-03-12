; Go fold queries

(function_declaration body: (block) @fold)
(method_declaration body: (block) @fold)
(if_statement consequence: (block) @fold)
(for_statement body: (block) @fold)
(struct_type (field_declaration_list) @fold)
(interface_type) @fold
(import_spec_list) @fold
(const_declaration) @fold
(var_declaration) @fold
