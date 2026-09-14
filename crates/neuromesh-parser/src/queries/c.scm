; NeuroMesh extract profile — tree-sitter-c
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function.name)) @function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @function.name))) @function

(struct_specifier
  name: (type_identifier) @class.name
  body: (field_declaration_list)) @class

(union_specifier
  name: (type_identifier) @class.name
  body: (field_declaration_list)) @class

(enum_specifier
  name: (type_identifier) @symbol.name
  body: (enumerator_list)) @symbol

(type_definition
  declarator: (type_identifier) @symbol.name) @symbol

(preproc_include) @import

(call_expression) @call
