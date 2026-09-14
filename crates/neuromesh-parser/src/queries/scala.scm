; NeuroMesh extract profile — tree-sitter-scala
(function_definition
  name: (identifier) @function.name) @function

(class_definition
  name: (identifier) @class.name) @class

(object_definition
  name: (identifier) @class.name) @class

(trait_definition
  name: (identifier) @class.name) @class

(import_declaration) @import

(call_expression) @call
