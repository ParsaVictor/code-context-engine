; NeuroMesh extract profile — tree-sitter-cpp
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function.name)) @function

(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @function.name)) @function

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      scope: (namespace_identifier) @function.parent
      name: (identifier) @function.name))) @function

(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      scope: (namespace_identifier) @function.parent
      name: (destructor_name) @function.name))) @function

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @function.name))) @function

(function_definition
  declarator: (reference_declarator
    (function_declarator
      declarator: (identifier) @function.name))) @function

(class_specifier
  name: (type_identifier) @class.name
  body: (field_declaration_list)) @class

(struct_specifier
  name: (type_identifier) @class.name
  body: (field_declaration_list)) @class

(enum_specifier
  name: (type_identifier) @symbol.name
  body: (enumerator_list)) @symbol

(type_definition
  declarator: (type_identifier) @symbol.name) @symbol

(alias_declaration
  name: (type_identifier) @symbol.name) @symbol

(preproc_include) @import

(call_expression) @call
