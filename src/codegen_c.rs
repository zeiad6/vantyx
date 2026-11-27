//! C code generation for the typed IR.
//!
//! The generated program is GNU C11 (statement expressions are used to pin
//! down Vantyx's left-to-right evaluation order). It links against libc and
//! libm only; there is no Vantyx runtime library to install.
//!
//! Ownership model: every string and struct allocation is leaked on purpose.
//! Native programs are short-lived command-line processes, matching the
//! interpreter's documented reference-counting limitation (cycles leak).

use crate::ast::{BinOp, UnOp};
use crate::ir::*;
use crate::span::{SourceMap, SourceRegistry, Span};
use std::fmt::Write as _;

/// Emits a complete, self-contained C translation unit for one source file.
pub fn emit(program: &IrProgram, source_name: &str, source: &str) -> String {
    let mut sources = SourceRegistry::new();
    sources.add(source_name, source);
    emit_with_sources(program, &sources)
}

/// Emits C for a program whose spans may refer to several imported files.
pub fn emit_with_sources(program: &IrProgram, sources: &SourceRegistry) -> String {
    Emitter::new(program, sources).emit()
}

struct Emitter<'a> {
    program: &'a IrProgram,
    sources: &'a SourceRegistry,
    entry_name: &'a str,
    out: String,
    indent: usize,
    temp: usize,
    /// Concrete List and struct types that need runtime type descriptors,
    /// ordered inner-before-outer so definitions are well-formed.
    descs: Vec<IrType>,
    /// Whether the current statement is inside a user function, so `return`
    /// must release the native call-depth slot.
    in_function: bool,
}

impl<'a> Emitter<'a> {
    fn new(program: &'a IrProgram, sources: &'a SourceRegistry) -> Self {
        let entry_name = sources
            .get(0)
            .map(|file| file.name.as_str())
            .unwrap_or("<unknown>");
        Self {
            program,
            sources,
            entry_name,
            out: String::new(),
            indent: 0,
            temp: 0,
            descs: Vec::new(),
            in_function: false,
        }
    }

    fn emit(mut self) -> String {
        self.collect_descriptors();
        self.preamble();
        self.struct_forward_declarations();
        self.struct_definitions();
        self.enum_definitions();
        self.prototypes();
        self.type_descriptors();
        self.globals();
        self.constructors();
        self.display_functions();
        self.equals_functions();
        self.functions();
        self.top_level();
        self.c_main();
        self.out
    }

    /// Registers every list/struct type reachable from the IR. Primitive
    /// descriptors are emitted once in the preamble.
    fn collect_descriptors(&mut self) {
        let mut types = Vec::new();
        // Register enum descriptors first so struct/list/map descriptors that
        // mention an enum can refer to a descriptor declared earlier.
        for enumeration in &self.program.enums {
            register_desc_type(&IrType::Enum(enumeration.name.clone()), &mut types);
        }
        for structure in &self.program.structs {
            for field in &structure.fields {
                register_desc_type(&field.ty, &mut types);
            }
        }
        for enumeration in &self.program.enums {
            for variant in &enumeration.variants {
                for field in &variant.fields {
                    register_desc_type(&field.ty, &mut types);
                }
            }
        }
        for global in &self.program.globals {
            register_desc_type(&global.ty, &mut types);
        }
        for function in &self.program.functions {
            for param in &function.params {
                register_desc_type(&param.ty, &mut types);
            }
            register_desc_type(&function.ret, &mut types);
            for stmt in &function.body {
                collect_stmt_desc_types(stmt, &mut types);
            }
        }
        for stmt in &self.program.top_level {
            collect_stmt_desc_types(stmt, &mut types);
        }
        self.descs = types;
    }

    // ---- output helpers ----

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn blank(&mut self) {
        self.out.push('\n');
    }

    fn position(&self, span: Span) -> (usize, usize) {
        match self.sources.get(span.file) {
            Some(file) => SourceMap::new(&file.name, &file.source).line_col(span.start),
            None => (1, 1),
        }
    }

    fn source_name_for(&self, span: Span) -> &str {
        self.sources
            .get(span.file)
            .map(|file| file.name.as_str())
            .unwrap_or("<unknown>")
    }

    /// Wraps a rendered expression for a C condition. Expressions from
    /// `expr` are already parenthesized when needed; adding another pair
    /// around a comparison triggers clang's `-Wparentheses-equality`.
    ///
    /// GNU statement expressions are the exception: `({ ... })` is a complete
    /// expression whose outer parentheses do not satisfy the `if`/`while`
    /// grammar, so the whole thing needs one more pair around it.
    fn condition(rendered: &str) -> String {
        if rendered.starts_with("({") {
            format!("({rendered})")
        } else if rendered.starts_with('(') && rendered.ends_with(')') {
            rendered.to_string()
        } else {
            format!("({rendered})")
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        let name = format!("vantyx_{prefix}_{}", self.temp);
        self.temp += 1;
        name
    }

    // ---- names ----

    fn local(&self, name: &str) -> String {
        format!("vantyx_l_{name}")
    }

    fn global(&self, name: &str) -> String {
        format!("vantyx_g_{name}")
    }

    fn init_flag(&self, name: &str) -> String {
        format!("vantyx_g_{name}_init")
    }

    fn function_name(&self, name: &str) -> String {
        format!("vantyx_fn_{name}")
    }

    fn struct_name(&self, name: &str) -> String {
        format!("Vantyx_s_{name}")
    }

    fn enum_name(&self, name: &str) -> String {
        format!("Vantyx_e_{name}")
    }

    fn variant_name(&self, enum_name: &str, variant: &str) -> String {
        format!("vantyx_variant_{enum_name}_{variant}")
    }

    fn variant_member(&self, variant: &str) -> String {
        format!("vantyx_v_{variant}")
    }

    fn constructor_name(&self, name: &str) -> String {
        format!("vantyx_new_{name}")
    }

    fn display_name(&self, name: &str) -> String {
        format!("vantyx_display_{name}")
    }

    fn equals_name(&self, name: &str) -> String {
        format!("vantyx_equals_{name}")
    }

    fn desc_name(&self, ty: &IrType) -> String {
        match ty {
            IrType::Unit => "vantyx_desc_unit".to_string(),
            IrType::Int => "vantyx_desc_int".to_string(),
            IrType::Float => "vantyx_desc_float".to_string(),
            IrType::Bool => "vantyx_desc_bool".to_string(),
            IrType::Str => "vantyx_desc_str".to_string(),
            IrType::List(_) | IrType::Map(_, _) | IrType::Struct(_) | IrType::Enum(_) => {
                let index = self
                    .descs
                    .iter()
                    .position(|candidate| candidate == ty)
                    .expect("list/map/struct/enum descriptor registered before emission");
                format!("vantyx_desc_{index}")
            }
        }
    }

    fn field(&self, name: &str) -> String {
        format!("vantyx_f_{name}")
    }

    fn ctype(&self, ty: &IrType) -> String {
        match ty {
            IrType::Unit => "vantyx_unit".to_string(),
            IrType::Int => "int64_t".to_string(),
            IrType::Float => "double".to_string(),
            IrType::Bool => "bool".to_string(),
            IrType::Str => "vantyx_str".to_string(),
            IrType::List(_) => "VantyxList*".to_string(),
            IrType::Map(_, _) => "VantyxMap*".to_string(),
            IrType::Struct(name) => format!("{}*", self.struct_name(name)),
            IrType::Enum(name) => format!("{}*", self.enum_name(name)),
        }
    }

    fn zero_value(&self, ty: &IrType) -> &'static str {
        match ty {
            IrType::Unit => "0",
            IrType::Int => "0",
            IrType::Float => "0.0",
            IrType::Bool => "false",
            IrType::Str => "NULL",
            IrType::List(_) => "NULL",
            IrType::Map(_, _) => "NULL",
            IrType::Struct(_) => "NULL",
            IrType::Enum(_) => "NULL",
        }
    }

    // ---- top-level emission ----

    fn preamble(&mut self) {
        self.out.push_str(PREAMBLE);
        self.blank();
    }

    fn struct_forward_declarations(&mut self) {
        if self.program.structs.is_empty() && self.program.enums.is_empty() {
            return;
        }
        for structure in &self.program.structs {
            self.line(&format!(
                "typedef struct {} {};",
                self.struct_name(&structure.name),
                self.struct_name(&structure.name)
            ));
        }
        for enumeration in &self.program.enums {
            self.line(&format!(
                "typedef struct {} {};",
                self.enum_name(&enumeration.name),
                self.enum_name(&enumeration.name)
            ));
        }
        self.blank();
    }

    fn struct_definitions(&mut self) {
        for structure in &self.program.structs {
            let name = self.struct_name(&structure.name);
            self.line(&format!("struct {name} {{"));
            self.indent += 1;
            if structure.fields.is_empty() {
                self.line("char vantyx_empty;");
            }
            for field in &structure.fields {
                self.line(&format!(
                    "{} {};",
                    self.ctype(&field.ty),
                    self.field(&field.name)
                ));
            }
            self.indent -= 1;
            self.line("};");
            self.blank();
        }
    }

    fn enum_definitions(&mut self) {
        for enumeration in &self.program.enums {
            let name = self.enum_name(&enumeration.name);
            self.line(&format!("struct {name} {{"));
            self.indent += 1;
            self.line("int64_t tag;");
            self.line("union {");
            self.indent += 1;
            self.line("int64_t vantyx_dummy;");
            for variant in &enumeration.variants {
                if variant.fields.is_empty() {
                    continue;
                }
                self.line("struct {");
                self.indent += 1;
                for field in &variant.fields {
                    self.line(&format!(
                        "{} {};",
                        self.ctype(&field.ty),
                        self.field(&field.name)
                    ));
                }
                self.indent -= 1;
                self.line(&format!("}} {};", self.variant_member(&variant.name)));
            }
            self.indent -= 1;
            self.line("} payload;");
            self.indent -= 1;
            self.line("};");
            self.blank();
        }
    }

    fn prototypes(&mut self) {
        let mut emitted = false;
        for structure in &self.program.structs {
            let params = self.constructor_params(&structure.fields);
            self.line(&format!(
                "static {}* {}({params});",
                self.struct_name(&structure.name),
                self.constructor_name(&structure.name)
            ));
            emitted = true;
        }
        for enumeration in &self.program.enums {
            for variant in &enumeration.variants {
                let params = self.constructor_params(&variant.fields);
                self.line(&format!(
                    "static {}* {}({params});",
                    self.enum_name(&enumeration.name),
                    self.variant_name(&enumeration.name, &variant.name)
                ));
            }
            emitted = true;
        }
        if emitted {
            self.blank();
        }
        emitted = false;
        for structure in &self.program.structs {
            self.line(&format!(
                "static vantyx_str {}(void* value, int depth);",
                self.display_name(&structure.name)
            ));
            emitted = true;
        }
        for enumeration in &self.program.enums {
            self.line(&format!(
                "static vantyx_str {}(void* value, int depth);",
                self.display_name(&enumeration.name)
            ));
            emitted = true;
        }
        if emitted {
            self.blank();
        }
        emitted = false;
        for structure in &self.program.structs {
            self.line(&format!(
                "static bool {}(void* left, void* right, int depth);",
                self.equals_name(&structure.name)
            ));
            emitted = true;
        }
        for enumeration in &self.program.enums {
            self.line(&format!(
                "static bool {}(void* left, void* right, int depth);",
                self.equals_name(&enumeration.name)
            ));
            emitted = true;
        }
        if emitted {
            self.blank();
        }
        for function in &self.program.functions {
            let params = self.function_params(&function.params);
            self.line(&format!(
                "static {} {}({params});",
                self.ctype(&function.ret),
                self.function_name(&function.name)
            ));
        }
        if !self.program.functions.is_empty() {
            self.blank();
        }
    }

    fn type_descriptors(&mut self) {
        if self.descs.is_empty() {
            return;
        }
        for ty in self.descs.clone() {
            let name = self.desc_name(&ty);
            match &ty {
                IrType::List(inner) => {
                    let element = self.desc_name(inner);
                    self.line(&format!(
                        "static const VantyxTypeDesc {name} = {{ VANTYX_T_LIST, sizeof(VantyxList*), &{element}, NULL, NULL, NULL, NULL }};"
                    ));
                }
                IrType::Map(key, value) => {
                    let key = self.desc_name(key);
                    let value = self.desc_name(value);
                    self.line(&format!(
                        "static const VantyxTypeDesc {name} = {{ VANTYX_T_MAP, sizeof(VantyxMap*), NULL, &{key}, &{value}, NULL, NULL }};"
                    ));
                }
                IrType::Struct(struct_name) => {
                    self.line(&format!(
                        "static const VantyxTypeDesc {name} = {{ VANTYX_T_STRUCT, sizeof({}*), NULL, NULL, NULL, {}, {} }};",
                        self.struct_name(struct_name),
                        self.display_name(struct_name),
                        self.equals_name(struct_name)
                    ));
                }
                IrType::Enum(enum_name) => {
                    self.line(&format!(
                        "static const VantyxTypeDesc {name} = {{ VANTYX_T_ENUM, sizeof({}*), NULL, NULL, NULL, {}, {} }};",
                        self.enum_name(enum_name),
                        self.display_name(enum_name),
                        self.equals_name(enum_name)
                    ));
                }
                _ => unreachable!("only List, Map, Struct, and Enum types need descriptors"),
            }
        }
        self.blank();
    }

    fn constructor_params(&self, fields: &[IrField]) -> String {
        if fields.is_empty() {
            return "void".to_string();
        }
        fields
            .iter()
            .map(|field| format!("{} {}", self.ctype(&field.ty), self.field(&field.name)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn function_params(&self, params: &[IrParam]) -> String {
        if params.is_empty() {
            return "void".to_string();
        }
        params
            .iter()
            .map(|param| format!("{} {}", self.ctype(&param.ty), self.local(&param.name)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn globals(&mut self) {
        for global in &self.program.globals {
            self.line(&format!(
                "static {} {} = {};",
                self.ctype(&global.ty),
                self.global(&global.name),
                self.zero_value(&global.ty)
            ));
            self.line(&format!(
                "static bool {} = false;",
                self.init_flag(&global.name)
            ));
        }
        if !self.program.globals.is_empty() {
            self.blank();
        }
    }

    fn constructors(&mut self) {
        for structure in &self.program.structs {
            let name = self.struct_name(&structure.name);
            let params = self.constructor_params(&structure.fields);
            self.line(&format!(
                "static {}* {}({params}) {{",
                name,
                self.constructor_name(&structure.name)
            ));
            self.indent += 1;
            self.line(&format!(
                "{}* value = ({}*)vantyx_alloc(sizeof({}));",
                name, name, name
            ));
            for field in &structure.fields {
                self.line(&format!(
                    "value->{} = {};",
                    self.field(&field.name),
                    self.field(&field.name)
                ));
            }
            self.line("return value;");
            self.indent -= 1;
            self.line("}");
            self.blank();
        }
        for enumeration in &self.program.enums {
            let name = self.enum_name(&enumeration.name);
            for variant in &enumeration.variants {
                let params = self.constructor_params(&variant.fields);
                self.line(&format!(
                    "static {}* {}({params}) {{",
                    name,
                    self.variant_name(&enumeration.name, &variant.name)
                ));
                self.indent += 1;
                self.line(&format!(
                    "{}* value = ({}*)vantyx_alloc(sizeof({}));",
                    name, name, name
                ));
                self.line("memset(value, 0, sizeof(*value));");
                self.line(&format!("value->tag = {};", variant.tag));
                for field in &variant.fields {
                    self.line(&format!(
                        "value->payload.{}.{} = {};",
                        self.variant_member(&variant.name),
                        self.field(&field.name),
                        self.field(&field.name)
                    ));
                }
                self.line("return value;");
                self.indent -= 1;
                self.line("}");
                self.blank();
            }
        }
    }

    fn display_field(&self, ty: &IrType, value: &str, depth: &str) -> String {
        match ty {
            IrType::Str => format!("vantyx_str_quote({value})"),
            IrType::Int => format!("vantyx_str_from_int({value})"),
            IrType::Float => format!("vantyx_str_from_float({value})"),
            IrType::Bool => format!("vantyx_str_from_bool({value})"),
            IrType::Unit => "vantyx_str_from_unit()".to_string(),
            IrType::List(_) => format!("vantyx_list_display({value}, {depth})"),
            IrType::Map(_, _) => format!("vantyx_map_display({value}, {depth})"),
            IrType::Struct(inner) => format!("{}({value}, {depth})", self.display_name(inner)),
            IrType::Enum(inner) => format!("{}({value}, {depth})", self.display_name(inner)),
        }
    }

    fn display_functions(&mut self) {
        for structure in &self.program.structs {
            let name = self.struct_name(&structure.name);
            self.line(&format!(
                "static vantyx_str {}(void* raw, int depth) {{",
                self.display_name(&structure.name)
            ));
            self.indent += 1;
            self.line(&format!("{}* value = ({}*)raw;", name, name));
            self.line("if (depth > 64) return vantyx_strdup(\"\\xE2\\x80\\xA6\");");
            self.line(&format!(
                "if (!vantyx_enter_struct(raw)) return vantyx_strdup({});",
                c_string(&format!("{}(...)", structure.name))
            ));
            self.line(&format!(
                "vantyx_str result = vantyx_strdup({});",
                c_string(&format!("{}(", structure.name))
            ));
            for (index, field) in structure.fields.iter().enumerate() {
                if index > 0 {
                    self.line("result = vantyx_str_concat(result, vantyx_strdup(\", \"));");
                }
                self.line(&format!(
                    "result = vantyx_str_concat(result, vantyx_strdup({}));",
                    c_string(&format!("{}=", field.name))
                ));
                let value = format!("value->{}", self.field(&field.name));
                let rendered = self.display_field(&field.ty, &value, "depth + 1");
                self.line(&format!("result = vantyx_str_concat(result, {rendered});"));
            }
            self.line("result = vantyx_str_concat(result, vantyx_strdup(\")\"));");
            self.line("vantyx_leave_struct();");
            self.line("return result;");
            self.indent -= 1;
            self.line("}");
            self.blank();
        }

        for enumeration in &self.program.enums {
            let name = self.enum_name(&enumeration.name);
            self.line(&format!(
                "static vantyx_str {}(void* raw, int depth) {{",
                self.display_name(&enumeration.name)
            ));
            self.indent += 1;
            self.line(&format!("{}* value = ({}*)raw;", name, name));
            self.line("if (value == NULL) return vantyx_strdup(\"...\");");
            self.line("if (depth > 64) return vantyx_strdup(\"\\xE2\\x80\\xA6\");");
            self.line("switch (value->tag) {");
            self.indent += 1;
            for variant in &enumeration.variants {
                self.line(&format!("case {}: {{", variant.tag));
                self.indent += 1;
                if variant.fields.is_empty() {
                    self.line(&format!(
                        "return vantyx_strdup({});",
                        c_string(&variant.name)
                    ));
                } else {
                    self.line(&format!(
                        "vantyx_str result = vantyx_strdup({});",
                        c_string(&format!("{}(", variant.name))
                    ));
                    for (index, field) in variant.fields.iter().enumerate() {
                        if index > 0 {
                            self.line("result = vantyx_str_concat(result, vantyx_strdup(\", \"));");
                        }
                        let value = format!(
                            "value->payload.{}.{}",
                            self.variant_member(&variant.name),
                            self.field(&field.name)
                        );
                        let rendered = self.display_field(&field.ty, &value, "depth + 1");
                        self.line(&format!("result = vantyx_str_concat(result, {rendered});"));
                    }
                    self.line("result = vantyx_str_concat(result, vantyx_strdup(\")\"));");
                    self.line("return result;");
                }
                self.indent -= 1;
                self.line("}");
            }
            self.indent -= 1;
            self.line("}");
            self.line("return vantyx_strdup(\"<invalid enum tag>\");");
            self.indent -= 1;
            self.line("}");
            self.blank();
        }
    }

    fn equals_field(&self, ty: &IrType, left: &str, right: &str) -> String {
        match ty {
            IrType::Unit => "true".to_string(),
            IrType::Int | IrType::Float | IrType::Bool => {
                format!("({left}) == ({right})")
            }
            IrType::Str => format!("strcmp({left}, {right}) == 0"),
            IrType::List(_) | IrType::Map(_, _) | IrType::Struct(_) | IrType::Enum(_) => {
                format!(
                    "vantyx_value_equals(&({left}), &({right}), &{}, depth + 1)",
                    self.desc_name(ty)
                )
            }
        }
    }

    fn equals_functions(&mut self) {
        for structure in &self.program.structs {
            let name = self.struct_name(&structure.name);
            self.line(&format!(
                "static bool {}(void* left_raw, void* right_raw, int depth) {{",
                self.equals_name(&structure.name)
            ));
            self.indent += 1;
            self.line(&format!("{}* left = ({}*)left_raw;", name, name));
            self.line(&format!("{}* right = ({}*)right_raw;", name, name));
            self.line("if (left == right) return true;");
            self.line("if (left == NULL || right == NULL) return false;");
            if structure.fields.is_empty() {
                self.line("return true;");
            } else {
                let mut comparisons = Vec::with_capacity(structure.fields.len());
                for field in &structure.fields {
                    let left = format!("left->{}", self.field(&field.name));
                    let right = format!("right->{}", self.field(&field.name));
                    comparisons.push(self.equals_field(&field.ty, &left, &right));
                }
                self.line(&format!("return {};", comparisons.join(" && ")));
            }
            self.indent -= 1;
            self.line("}");
            self.blank();
        }

        for enumeration in &self.program.enums {
            let name = self.enum_name(&enumeration.name);
            self.line(&format!(
                "static bool {}(void* left_raw, void* right_raw, int depth) {{",
                self.equals_name(&enumeration.name)
            ));
            self.indent += 1;
            self.line(&format!("{}* left = ({}*)left_raw;", name, name));
            self.line(&format!("{}* right = ({}*)right_raw;", name, name));
            self.line("if (left == right) return true;");
            self.line("if (left == NULL || right == NULL) return false;");
            self.line("if (left->tag != right->tag) return false;");
            self.line("switch (left->tag) {");
            self.indent += 1;
            for variant in &enumeration.variants {
                self.line(&format!("case {}:", variant.tag));
                self.indent += 1;
                if variant.fields.is_empty() {
                    self.line("return true;");
                } else {
                    let mut comparisons = Vec::with_capacity(variant.fields.len());
                    for field in &variant.fields {
                        let left = format!(
                            "left->payload.{}.{}",
                            self.variant_member(&variant.name),
                            self.field(&field.name)
                        );
                        let right = format!(
                            "right->payload.{}.{}",
                            self.variant_member(&variant.name),
                            self.field(&field.name)
                        );
                        comparisons.push(self.equals_field(&field.ty, &left, &right));
                    }
                    self.line(&format!("return {};", comparisons.join(" && ")));
                }
                self.indent -= 1;
            }
            self.indent -= 1;
            self.line("}");
            self.line("return false;");
            self.indent -= 1;
            self.line("}");
            self.blank();
        }
    }

    fn functions(&mut self) {
        for function in &self.program.functions {
            let params = self.function_params(&function.params);
            self.line(&format!(
                "static {} {}({params}) {{",
                self.ctype(&function.ret),
                self.function_name(&function.name)
            ));
            self.indent += 1;
            let source = self.source_name_for(function.span).to_string();
            // Report the depth limit at the call site, matching the
            // interpreter. `vantyx_call_line`/`column` are set by `call_expr`
            // immediately before each user-function call, and the caller's
            // file is still in `vantyx_source_name` until the check passes.
            self.line("if (vantyx_call_depth >= 256) {");
            self.indent += 1;
            self.line(
                "vantyx_rt_error(\"R0012\", \"call depth limit of 256 exceeded; possible infinite recursion\", vantyx_call_line, vantyx_call_column);",
            );
            self.indent -= 1;
            self.line("}");
            self.line(&format!("vantyx_source_name = {};", c_string(&source)));
            self.line("vantyx_call_depth++;");
            self.in_function = true;
            for stmt in &function.body {
                self.stmt(stmt);
            }
            self.in_function = false;
            if function.ret == IrType::Unit {
                self.line("vantyx_call_depth--;");
                self.line("return 0;");
            }
            self.indent -= 1;
            self.line("}");
            self.blank();
        }
    }

    fn top_level(&mut self) {
        self.line("static void vantyx_top_level(void) {");
        self.indent += 1;
        for stmt in &self.program.top_level {
            self.stmt(stmt);
        }
        self.indent -= 1;
        self.line("}");
        self.blank();
    }

    fn c_main(&mut self) {
        self.line("int main(void) {");
        self.indent += 1;
        // Rust's runtime replaces closed standard descriptors with /dev/null
        // before `main`. Without this, a closed stdout makes `print` fail with
        // EBADF and a later file open can receive descriptor 0, 1, or 2.
        self.line("vantyx_sanitize_standard_fd(STDIN_FILENO, O_RDONLY);");
        self.line("vantyx_sanitize_standard_fd(STDOUT_FILENO, O_WRONLY);");
        self.line("vantyx_sanitize_standard_fd(STDERR_FILENO, O_WRONLY);");
        // Rust ignores SIGPIPE by default, so the interpreter turns a closed
        // output pipe into an R0013 write error. Ignore it here as well so
        // `fflush`/`fputs` can report EPIPE instead of terminating silently.
        self.line("signal(SIGPIPE, SIG_IGN);");
        self.line(&format!(
            "vantyx_source_name = {};",
            c_string(self.entry_name)
        ));
        match self.program.entry {
            IrEntry::TopLevel => self.line("vantyx_top_level();"),
            IrEntry::Main => self.line("(void)vantyx_fn_main();"),
        }
        self.line("if (fflush(stdout) != 0) {");
        self.indent += 1;
        self.line("fputs(\"error: failed to flush output\\n\", stderr);");
        self.line("return 2;");
        self.indent -= 1;
        self.line("}");
        self.line("return 0;");
        self.indent -= 1;
        self.line("}");
    }

    // ---- statements ----

    /// Returns the C condition for a refutable pattern, or `None` when the
    /// pattern is irrefutable (`_` or a binding).
    fn pattern_condition(&self, pattern: &IrPattern, base: &str) -> Option<String> {
        match pattern {
            IrPattern::Wildcard | IrPattern::Bind(_) => None,
            IrPattern::Literal { value } => Some(match value {
                IrLiteral::Int(value) => format!("{base} == {}", c_int(*value)),
                IrLiteral::Bool(true) => base.to_string(),
                IrLiteral::Bool(false) => format!("!{base}"),
                IrLiteral::Str(value) => {
                    format!("strcmp({base}, {}) == 0", c_string(value))
                }
            }),
            IrPattern::Range {
                start,
                end,
                inclusive,
            } => {
                let upper = if *inclusive { "<=" } else { "<" };
                Some(format!(
                    "({base} >= {} && {base} {upper} {})",
                    c_int(*start),
                    c_int(*end)
                ))
            }
            IrPattern::Variant {
                variant,
                tag,
                fields,
                ..
            } => {
                let mut conditions = vec![format!("{base}->tag == {tag}")];
                for field in fields {
                    let child = format!(
                        "{}->payload.{}.{}",
                        base,
                        self.variant_member(variant),
                        self.field(&field.name)
                    );
                    if let Some(condition) = self.pattern_condition(&field.pattern, &child) {
                        conditions.push(condition);
                    }
                }
                Some(conditions.join(" && "))
            }
            IrPattern::Or { alternatives } => {
                let mut conditions = Vec::with_capacity(alternatives.len());
                for alternative in alternatives {
                    let condition = self.pattern_condition(alternative, base)?;
                    conditions.push(format!("({condition})"));
                }
                Some(conditions.join(" || "))
            }
        }
    }

    fn collect_pattern_bindings<'p>(&self, pattern: &'p IrPattern, out: &mut Vec<&'p IrBinding>) {
        match pattern {
            IrPattern::Bind(binding) => out.push(binding),
            IrPattern::Variant { fields, .. } => {
                for field in fields {
                    self.collect_pattern_bindings(&field.pattern, out);
                }
            }
            IrPattern::Or { alternatives } => {
                // Alternatives share one binding set. The first alternative
                // supplies the names and types; initialization is emitted per
                // alternative by `binding_declarations`.
                if let Some(first) = alternatives.first() {
                    self.collect_pattern_bindings(first, out);
                }
            }
            IrPattern::Wildcard | IrPattern::Literal { .. } | IrPattern::Range { .. } => {}
        }
    }

    fn binding_expr(&self, binding: &IrBinding, scrutinee: &str) -> String {
        let mut expression = scrutinee.to_string();
        for step in &binding.path {
            let _ = write!(
                expression,
                "->payload.{}.{}",
                self.variant_member(&step.variant),
                self.field(&step.field)
            );
        }
        expression
    }

    fn binding_declarations(&self, pattern: &IrPattern, scrutinee: &str) -> String {
        if let IrPattern::Or { alternatives } = pattern {
            return self.or_binding_declarations(alternatives, scrutinee);
        }
        let mut bindings = Vec::new();
        self.collect_pattern_bindings(pattern, &mut bindings);
        let mut text = String::new();
        for binding in bindings {
            let _ = write!(
                text,
                "{} {} = {}; ",
                self.ctype(&binding.ty),
                self.local(&binding.name),
                self.binding_expr(binding, scrutinee)
            );
        }
        text
    }

    /// Declares the shared or-pattern bindings once, then initializes each
    /// from the alternative that matched. The arm condition already proved
    /// that at least one alternative matches, so the assignments are total.
    fn or_binding_declarations(&self, alternatives: &[IrPattern], scrutinee: &str) -> String {
        let mut shared = Vec::new();
        if let Some(first) = alternatives.first() {
            self.collect_pattern_bindings(first, &mut shared);
        }
        if shared.is_empty() {
            return String::new();
        }
        let mut text = String::new();
        for binding in &shared {
            let _ = write!(
                text,
                "{} {} = {}; ",
                self.ctype(&binding.ty),
                self.local(&binding.name),
                self.zero_value(&binding.ty)
            );
        }
        let mut first_branch = true;
        for alternative in alternatives {
            let mut bindings = Vec::new();
            self.collect_pattern_bindings(alternative, &mut bindings);
            let mut assignments = String::new();
            for binding in bindings {
                let _ = write!(
                    assignments,
                    "{} = {}; ",
                    self.local(&binding.name),
                    self.binding_expr(binding, scrutinee)
                );
            }
            match self.pattern_condition(alternative, scrutinee) {
                Some(condition) => {
                    if first_branch {
                        let _ = write!(text, "if ({condition}) {{ {assignments} }} ");
                        first_branch = false;
                    } else {
                        let _ = write!(text, "else if ({condition}) {{ {assignments} }} ");
                    }
                }
                None => {
                    if first_branch {
                        let _ = write!(text, "{{ {assignments} }} ");
                    } else {
                        let _ = write!(text, "else {{ {assignments} }} ");
                    }
                    break;
                }
            }
        }
        text
    }

    /// Returns the C condition for an arm, or `None` for an unguarded wildcard.
    ///
    /// A guard on a pattern with bindings cannot see those bindings in the
    /// surrounding scope, so the condition uses a GNU C statement expression
    /// that tests the full nested pattern, declares every binding, and
    /// evaluates the guard.
    fn arm_condition(&mut self, arm: &IrMatchArm, scrutinee: &str) -> Option<String> {
        let pattern = self.pattern_condition(&arm.pattern, scrutinee);
        let guard = arm.guard.as_ref().map(|guard| self.expr(guard));
        match (pattern, guard) {
            (None, None) => None,
            (Some(condition), None) => Some(condition),
            (None, Some(guard)) => Some(guard),
            (Some(condition), Some(guard)) => {
                let mut bindings = Vec::new();
                self.collect_pattern_bindings(&arm.pattern, &mut bindings);
                if bindings.is_empty() {
                    return Some(format!("({condition}) && ({guard})"));
                }
                let ok = self.fresh("arm_ok");
                let declarations = self.binding_declarations(&arm.pattern, scrutinee);
                Some(format!(
                    "({{ bool {ok} = ({condition}); if ({ok}) {{ {declarations}{ok} = ({guard}); }} {ok}; }})"
                ))
            }
        }
    }

    fn match_arm_statements(&mut self, arm: &IrMatchArm, scrutinee: &str) {
        let declarations = self.binding_declarations(&arm.pattern, scrutinee);
        if !declarations.is_empty() {
            self.line(&declarations);
        }
        let IrMatchBody::Block(stmts) = &arm.body else {
            unreachable!("match statement arms contain blocks");
        };
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &IrStmt) {
        match &stmt.kind {
            IrStmtKind::Let { name, ty, init } => {
                let value = self.expr(init);
                self.line(&format!(
                    "{} {} = {};",
                    self.ctype(ty),
                    self.local(name),
                    value
                ));
            }
            IrStmtKind::GlobalInit { name, value } => {
                let value = self.expr(value);
                self.line(&format!("{} = {};", self.global(name), value));
                self.line(&format!("{} = true;", self.init_flag(name)));
            }
            IrStmtKind::Try {
                operand,
                bind,
                ok_variant,
                ok_field,
                err_tag,
            } => {
                let value = self.expr(operand);
                let value_name = self.fresh("try");
                self.line(&format!(
                    "{} {} = {};",
                    self.ctype(&operand.ty),
                    value_name,
                    value
                ));
                self.line(&format!("if ({value_name}->tag == {err_tag}) {{"));
                self.indent += 1;
                if self.in_function {
                    self.line("vantyx_call_depth--;");
                }
                self.line(&format!("return {value_name};"));
                self.indent -= 1;
                self.line("}");
                if let Some((name, ty)) = bind {
                    self.line(&format!(
                        "{} {} = {value_name}->payload.{}.{};",
                        self.ctype(ty),
                        self.local(name),
                        self.variant_member(ok_variant),
                        self.field(ok_field)
                    ));
                }
            }
            IrStmtKind::Assign { target, value } => match target {
                IrPlace::Var(name) => {
                    let value = self.expr(value);
                    self.line(&format!("{} = {};", self.local(name), value));
                }
                IrPlace::Global(name) => {
                    // The interpreter evaluates the right side before
                    // resolving the assignment place, so side effects in the
                    // value still happen when the global is uninitialized.
                    // Materialize the value first, then apply the same R0009
                    // check as a global read.
                    let (line, column) = self.position(stmt.span);
                    let value_text = self.expr(value);
                    let value_name = self.fresh("value");
                    self.line(&format!(
                        "{} {} = {};",
                        self.ctype(&value.ty),
                        value_name,
                        value_text
                    ));
                    self.line(&format!(
                        "vantyx_check_init({}, {}, {line}, {column});",
                        self.init_flag(name),
                        c_string(name)
                    ));
                    self.line(&format!("{} = {};", self.global(name), value_name));
                    self.line(&format!("{} = true;", self.init_flag(name)));
                }
                IrPlace::Field { base, field } => {
                    let value_text = self.expr(value);
                    if has_effects(value) {
                        let temp = self.fresh("value");
                        self.line(&format!(
                            "{} {} = {};",
                            self.ctype(&value.ty),
                            temp,
                            value_text
                        ));
                        let base_text = self.expr(base);
                        self.line(&format!(
                            "({})->{} = {};",
                            base_text,
                            self.field(field),
                            temp
                        ));
                    } else {
                        let base_text = self.expr(base);
                        self.line(&format!(
                            "({})->{} = {};",
                            base_text,
                            self.field(field),
                            value_text
                        ));
                    }
                }
                IrPlace::Index { base, index } => {
                    // Python-style simple assignment: the right-hand side is
                    // evaluated before the target place.
                    let (line, column) = self.position(stmt.span);
                    let value_text = self.expr(value);
                    let value_name = self.fresh("value");
                    let ctype = self.ctype(&value.ty);
                    self.line(&format!("{ctype} {value_name} = {value_text};"));
                    let base_text = self.expr(base);
                    let index_text = self.expr(index);
                    let base_name = self.fresh("base");
                    let index_name = self.fresh("index");
                    match &base.ty {
                        IrType::Map(_, _) => {
                            let key_ctype = self.ctype(&index.ty);
                            self.line(&format!("VantyxMap* {base_name} = {base_text};"));
                            self.line(&format!("{key_ctype} {index_name} = {index_text};"));
                            self.line(&format!(
                                "vantyx_map_set_raw({base_name}, &{index_name}, &{value_name}, {line}, {column});"
                            ));
                        }
                        _ => {
                            let slot_name = self.fresh("slot");
                            self.line(&format!("VantyxList* {base_name} = {base_text};"));
                            self.line(&format!("int64_t {index_name} = {index_text};"));
                            self.line(&format!(
                                "void* {slot_name} = vantyx_list_slot({base_name}, {index_name}, {line}, {column});"
                            ));
                            self.line(&format!("*(({ctype}*){slot_name}) = {value_name};"));
                        }
                    }
                }
            },
            IrStmtKind::Expr(expr) => {
                let text = self.expr(expr);
                self.line(&format!("(void)({text});"));
            }
            IrStmtKind::If {
                cond,
                then_body,
                elifs,
                else_body,
            } => {
                let cond = Self::condition(&self.expr(cond));
                self.line(&format!("if {cond} {{"));
                self.indent += 1;
                for stmt in then_body {
                    self.stmt(stmt);
                }
                self.indent -= 1;
                for (elif_cond, body) in elifs {
                    let cond = Self::condition(&self.expr(elif_cond));
                    self.line(&format!("}} else if {cond} {{"));
                    self.indent += 1;
                    for stmt in body {
                        self.stmt(stmt);
                    }
                    self.indent -= 1;
                }
                if let Some(body) = else_body {
                    self.line("} else {");
                    self.indent += 1;
                    for stmt in body {
                        self.stmt(stmt);
                    }
                    self.indent -= 1;
                }
                self.line("}");
            }
            IrStmtKind::While { cond, body } => {
                let cond = self.expr(cond);
                let cond = Self::condition(&cond);
                self.line(&format!("while {cond} {{"));
                self.indent += 1;
                for stmt in body {
                    self.stmt(stmt);
                }
                self.indent -= 1;
                self.line("}");
            }
            IrStmtKind::ForRange {
                name,
                start,
                end,
                step,
                body,
            } => {
                let (line, column) = self.position(stmt.span);
                let suffix = self.fresh("range");
                let start_name = format!("vantyx_start_{suffix}");
                let end_name = format!("vantyx_end_{suffix}");
                let step_name = format!("vantyx_step_{suffix}");
                let count_name = format!("vantyx_count_{suffix}");
                let index_name = format!("vantyx_index_{suffix}");
                let start_text = self.expr(start);
                self.line(&format!("int64_t {start_name} = {start_text};"));
                let end_text = self.expr(end);
                self.line(&format!("int64_t {end_name} = {end_text};"));
                let step_text = self.expr(step);
                self.line(&format!("int64_t {step_name} = {step_text};"));
                self.line(&format!(
                    "int64_t {count_name} = vantyx_range_count({start_name}, {end_name}, {step_name}, {line}, {column});"
                ));
                self.line(&format!(
                    "for (int64_t {index_name} = 0; {index_name} < {count_name}; {index_name}++) {{"
                ));
                self.indent += 1;
                self.line(&format!(
                    "int64_t {} = (int64_t)((__int128){} + (__int128){} * {});",
                    self.local(name),
                    start_name,
                    step_name,
                    index_name
                ));
                self.line("{");
                self.indent += 1;
                for stmt in body {
                    self.stmt(stmt);
                }
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
            }
            IrStmtKind::ForList { name, list, body } => {
                let element_ty = list
                    .ty
                    .element_type()
                    .cloned()
                    .expect("ForList carries a List type");
                let elem_ctype = self.ctype(&element_ty);
                let local_name = self.local(name);
                let list_text = self.expr(list);
                let list_name = self.fresh("iter");
                let len_name = self.fresh("len");
                let index_name = self.fresh("idx");
                self.line(&format!("VantyxList* {list_name} = {list_text};"));
                self.line(&format!("int64_t {len_name} = {list_name}->len;"));
                self.line(&format!(
                    "for (int64_t {index_name} = 0; {index_name} < {len_name}; {index_name}++) {{"
                ));
                self.indent += 1;
                self.line(&format!("if ({index_name} >= {list_name}->len) break;"));
                self.line(&format!(
                    "{elem_ctype} {local_name} = *(({elem_ctype}*)vantyx_list_slot_unchecked({list_name}, {index_name}));"
                ));
                self.line("{");
                self.indent += 1;
                for stmt in body {
                    self.stmt(stmt);
                }
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
            }
            IrStmtKind::ForStr { name, text, body } => {
                let local_name = self.local(name);
                let text_text = self.expr(text);
                let text_name = self.fresh("text");
                let cursor_name = self.fresh("cursor");
                let bytes_name = self.fresh("bytes");
                self.line(&format!("vantyx_str {text_name} = {text_text};"));
                self.line(&format!(
                    "for (const char* {cursor_name} = {text_name}; *{cursor_name} != 0; ) {{"
                ));
                self.indent += 1;
                self.line(&format!(
                    "int {bytes_name} = vantyx_utf8_char_len((const unsigned char)*{cursor_name});"
                ));
                self.line(&format!(
                    "vantyx_str {local_name} = vantyx_str_slice({cursor_name}, {bytes_name});"
                ));
                self.line(&format!("{cursor_name} += {bytes_name};"));
                self.line("{");
                self.indent += 1;
                for stmt in body {
                    self.stmt(stmt);
                }
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
            }
            IrStmtKind::ForMap { name, map, body } => {
                let IrType::Map(key, _) = &map.ty else {
                    unreachable!("ForMap carries a Map type");
                };
                let key_ctype = self.ctype(key);
                let local_name = self.local(name);
                let map_text = self.expr(map);
                let map_name = self.fresh("iter_map");
                let len_name = self.fresh("len");
                let index_name = self.fresh("idx");
                self.line(&format!("VantyxMap* {map_name} = {map_text};"));
                self.line(&format!("int64_t {len_name} = {map_name}->len;"));
                self.line(&format!(
                    "for (int64_t {index_name} = 0; {index_name} < {len_name}; {index_name}++) {{"
                ));
                self.indent += 1;
                self.line(&format!("if ({index_name} >= {map_name}->len) break;"));
                self.line(&format!(
                    "{key_ctype} {local_name} = *(({key_ctype}*)vantyx_map_key_unchecked({map_name}, {index_name}));"
                ));
                self.line("{");
                self.indent += 1;
                for stmt in body {
                    self.stmt(stmt);
                }
                self.indent -= 1;
                self.line("}");
                self.indent -= 1;
                self.line("}");
            }
            IrStmtKind::Match { scrutinee, arms } => {
                let (line, column) = self.position(stmt.span);
                let scrutinee_text = self.expr(scrutinee);
                let scrutinee_name = self.fresh("match");
                self.line(&format!(
                    "{} {} = {};",
                    self.ctype(&scrutinee.ty),
                    scrutinee_name,
                    scrutinee_text
                ));
                let mut first = true;
                let mut wildcard = false;
                for arm in arms {
                    match self.arm_condition(arm, &scrutinee_name) {
                        None => {
                            if first {
                                self.line("{");
                                first = false;
                            } else {
                                self.line("} else {");
                            }
                            self.indent += 1;
                            self.match_arm_statements(arm, &scrutinee_name);
                            self.indent -= 1;
                            wildcard = true;
                            break;
                        }
                        Some(condition) => {
                            if first {
                                self.line(&format!("if ({condition}) {{"));
                                first = false;
                            } else {
                                self.line(&format!("}} else if ({condition}) {{"));
                            }
                            self.indent += 1;
                            self.match_arm_statements(arm, &scrutinee_name);
                            self.indent -= 1;
                        }
                    }
                }
                if first {
                    self.line(&format!("(void){scrutinee_name};"));
                } else {
                    if !wildcard {
                        self.line("} else {");
                        self.indent += 1;
                        self.line(&format!(
                            "vantyx_rt_error(\"R0014\", \"no match arm matched the value\", {line}, {column});"
                        ));
                        self.indent -= 1;
                    }
                    self.line("}");
                }
            }
            IrStmtKind::Return(value) => {
                if let Some(value) = value {
                    let text = self.expr(value);
                    let ctype = self.ctype(&value.ty);
                    let result = self.fresh("return");
                    self.line(&format!("{ctype} {result} = {text};"));
                    if self.in_function {
                        self.line("vantyx_call_depth--;");
                    }
                    self.line(&format!("return {result};"));
                } else {
                    if self.in_function {
                        self.line("vantyx_call_depth--;");
                    }
                    self.line("return 0;");
                }
            }
            IrStmtKind::Break => self.line("break;"),
            IrStmtKind::Continue => self.line("continue;"),
        }
    }

    // ---- expressions ----

    fn expr(&mut self, expr: &IrExpr) -> String {
        match &expr.kind {
            IrExprKind::Int(value) => c_int(*value),
            IrExprKind::Float(value) => c_float(*value),
            IrExprKind::Bool(value) => {
                if *value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            IrExprKind::Str(parts) => self.string_expr(parts),
            IrExprKind::List(elements) => self.list_expr(elements, &expr.ty, expr.span),
            IrExprKind::Map(entries) => self.map_expr(entries, &expr.ty, expr.span),
            IrExprKind::Index { base, index } => self.index_expr(base, index, expr),
            IrExprKind::StrIndex { base, index } => self.str_index_expr(base, index, expr),
            IrExprKind::Var(name) => self.local(name),
            IrExprKind::Global(name) => {
                let (line, column) = self.position(expr.span);
                format!(
                    "(vantyx_check_init({}, {}, {line}, {column}), {})",
                    self.init_flag(name),
                    c_string(name),
                    self.global(name)
                )
            }
            IrExprKind::Binary { op, lhs, rhs } => {
                self.binary_expr(*op, lhs, rhs, &expr.ty, expr.span)
            }
            IrExprKind::Unary { op, operand } => {
                let value = self.expr(operand);
                match op {
                    UnOp::Neg => match operand.ty {
                        IrType::Int => {
                            let (line, column) = self.position(expr.span);
                            format!("vantyx_neg_int({value}, {line}, {column})")
                        }
                        IrType::Float => format!("(-({value}))"),
                        _ => unreachable!("IR rejected non-numeric negation"),
                    },
                    UnOp::Not => format!("(!({value}))"),
                }
            }
            IrExprKind::Call { name, args } => {
                self.call_expr(&self.function_name(name), args, Some(expr.span))
            }
            IrExprKind::Builtin { name, args } => self.builtin_expr(name, args, expr),
            IrExprKind::Field { base, name } => {
                let base = self.expr(base);
                format!("({base})->{}", self.field(name))
            }
            IrExprKind::Construct {
                name,
                args,
                eval_order,
            } => {
                let callee = self.constructor_name(name);
                self.construct_expr(&callee, args, eval_order)
            }
            IrExprKind::ConstructVariant {
                enum_name,
                variant,
                args,
                ..
            } => self.call_expr(&self.variant_name(enum_name, variant), args, None),
            IrExprKind::Match { scrutinee, arms } => {
                self.match_expr(scrutinee, arms, &expr.ty, expr.span)
            }
        }
    }

    fn string_expr(&mut self, parts: &[IrStrPart]) -> String {
        if parts.is_empty() {
            return "\"\"".to_string();
        }
        if parts.len() == 1 {
            return match &parts[0] {
                IrStrPart::Text(text) => c_string(text),
                IrStrPart::Value(value) => self.display_expr(value),
            };
        }
        if !parts.iter().any(part_has_effects) {
            let mut rendered = Vec::with_capacity(parts.len());
            for part in parts {
                rendered.push(match part {
                    IrStrPart::Text(text) => c_string(text),
                    IrStrPart::Value(value) => self.display_expr(value),
                });
            }
            let mut result = rendered[0].clone();
            for next in &rendered[1..] {
                result = format!("vantyx_str_concat({result}, {next})");
            }
            return result;
        }
        let mut prelude = String::new();
        let mut names = Vec::with_capacity(parts.len());
        for part in parts {
            let rendered = match part {
                IrStrPart::Text(text) => c_string(text),
                IrStrPart::Value(value) => self.display_expr(value),
            };
            let name = self.fresh("str");
            let _ = write!(prelude, "vantyx_str {name} = {rendered}; ");
            names.push(name);
        }
        let mut result = names[0].clone();
        for next in &names[1..] {
            result = format!("vantyx_str_concat({result}, {next})");
        }
        format!("({{ {prelude}{result}; }})")
    }

    fn list_expr(&mut self, elements: &[IrExpr], ty: &IrType, span: crate::span::Span) -> String {
        let IrType::List(inner) = ty else {
            unreachable!("list literal must have a List type");
        };
        let element_ctype = self.ctype(inner);
        let desc = self.desc_name(inner);
        let (line, column) = self.position(span);
        let list_name = self.fresh("list");
        let mut prelude = format!(
            "VantyxList* {list_name} = vantyx_list_new(sizeof({element_ctype}), &{desc}); "
        );
        for element in elements {
            let value = self.expr(element);
            let value_name = self.fresh("item");
            let _ = write!(
                prelude,
                "{element_ctype} {value_name} = {value}; vantyx_list_push_raw({list_name}, &{value_name}, {line}, {column}); "
            );
        }
        format!("({{ {prelude}{list_name}; }})")
    }

    fn index_expr(&mut self, base: &IrExpr, index: &IrExpr, call: &IrExpr) -> String {
        let (line, column) = self.position(call.span);
        let base_text = self.expr(base);
        let index_text = self.expr(index);
        let base_name = self.fresh("base");
        let index_name = self.fresh("index");
        let ctype = self.ctype(&call.ty);
        match &base.ty {
            IrType::Map(_, _) => {
                let key_ctype = self.ctype(&index.ty);
                let slot = self.fresh("slot");
                format!(
                    "({{ VantyxMap* {base_name} = {base_text}; {key_ctype} {index_name} = {index_text}; void* {slot} = vantyx_map_get({base_name}, &{index_name}, {line}, {column}); *(({ctype}*){slot}); }})"
                )
            }
            _ => format!(
                "({{ VantyxList* {base_name} = {base_text}; int64_t {index_name} = {index_text}; *(({ctype}*)vantyx_list_slot({base_name}, {index_name}, {line}, {column})); }})"
            ),
        }
    }

    fn map_expr(
        &mut self,
        entries: &[(IrExpr, IrExpr)],
        ty: &IrType,
        span: crate::span::Span,
    ) -> String {
        let IrType::Map(key, value) = ty else {
            unreachable!("map literal must have a Map type");
        };
        let key_ctype = self.ctype(key);
        let value_ctype = self.ctype(value);
        let key_desc = self.desc_name(key);
        let value_desc = self.desc_name(value);
        let (line, column) = self.position(span);
        let map_name = self.fresh("map");
        let mut prelude = format!(
            "VantyxMap* {map_name} = vantyx_map_new(sizeof({key_ctype}), sizeof({value_ctype}), &{key_desc}, &{value_desc}); "
        );
        for (entry_key, entry_value) in entries {
            let key_text = self.expr(entry_key);
            let value_text = self.expr(entry_value);
            let key_name = self.fresh("key");
            let value_name = self.fresh("value");
            let _ = write!(
                prelude,
                "{key_ctype} {key_name} = {key_text}; {value_ctype} {value_name} = {value_text}; vantyx_map_set_raw({map_name}, &{key_name}, &{value_name}, {line}, {column}); "
            );
        }
        format!("({{ {prelude}{map_name}; }})")
    }

    fn str_index_expr(&mut self, base: &IrExpr, index: &IrExpr, call: &IrExpr) -> String {
        let (line, column) = self.position(call.span);
        let base_text = self.expr(base);
        let index_text = self.expr(index);
        let base_name = self.fresh("text");
        let index_name = self.fresh("index");
        format!(
            "({{ vantyx_str {base_name} = {base_text}; int64_t {index_name} = {index_text}; vantyx_str_index({base_name}, {index_name}, {line}, {column}); }})"
        )
    }

    fn display_expr(&mut self, expr: &IrExpr) -> String {
        let value = self.expr(expr);
        self.display_value(&value, &expr.ty)
    }

    /// Renders an already-evaluated C expression as its display string.
    fn display_value(&mut self, value: &str, ty: &IrType) -> String {
        match ty {
            IrType::Str => value.to_string(),
            IrType::Int => format!("vantyx_str_from_int({value})"),
            IrType::Float => format!("vantyx_str_from_float({value})"),
            IrType::Bool => format!("vantyx_str_from_bool({value})"),
            // Rendering a Unit is already evaluated by the caller in `print`,
            // but interpolation passes an unevaluated expression here. Keep
            // the evaluation so calls, nested output, and runtime errors are
            // observable; the value itself renders as `()`.
            IrType::Unit => format!("((void)({value}), vantyx_str_from_unit())"),
            IrType::List(_) => format!("vantyx_list_to_str({value})"),
            IrType::Map(_, _) => format!("vantyx_map_to_str({value})"),
            IrType::Struct(name) => format!("{}({value}, 0)", self.display_name(name)),
            IrType::Enum(name) => format!("{}({value}, 0)", self.display_name(name)),
        }
    }

    fn match_expr(
        &mut self,
        scrutinee: &IrExpr,
        arms: &[IrMatchArm],
        result_ty: &IrType,
        span: crate::span::Span,
    ) -> String {
        let (line, column) = self.position(span);
        let result_ctype = self.ctype(result_ty);
        let zero = self.zero_value(result_ty).to_string();
        let scrutinee_text = self.expr(scrutinee);
        let scrutinee_name = self.fresh("match");
        let result_name = self.fresh("match_result");
        let prelude = format!(
            "{} {} = {}; {} {} = {}; ",
            self.ctype(&scrutinee.ty),
            scrutinee_name,
            scrutinee_text,
            result_ctype,
            result_name,
            zero
        );
        let mut branches = String::new();
        let mut first = true;
        let mut wildcard = false;
        for arm in arms {
            match self.arm_condition(arm, &scrutinee_name) {
                None => {
                    let body = self.match_arm_expr(arm, &scrutinee_name);
                    if first {
                        let _ = write!(branches, "{result_name} = {body}; ");
                        first = false;
                    } else {
                        let _ = write!(branches, "else {result_name} = {body}; ");
                    }
                    wildcard = true;
                    break;
                }
                Some(condition) => {
                    let body = self.match_arm_expr(arm, &scrutinee_name);
                    if first {
                        let _ = write!(branches, "if ({condition}) {result_name} = {body}; ");
                        first = false;
                    } else {
                        let _ = write!(branches, "else if ({condition}) {result_name} = {body}; ");
                    }
                }
            }
        }
        if !first && !wildcard {
            let _ = write!(
                branches,
                "else {{ vantyx_rt_error(\"R0014\", \"no match arm matched the value\", {line}, {column}); }} "
            );
        }
        format!("({{ {prelude}{branches}{result_name}; }})")
    }

    fn match_arm_expr(&mut self, arm: &IrMatchArm, scrutinee: &str) -> String {
        let IrMatchBody::Expr(expr) = &arm.body else {
            unreachable!("match expression arms contain expressions");
        };
        let prelude = self.binding_declarations(&arm.pattern, scrutinee);
        let value = self.expr(expr);
        format!("({{ {prelude}{value}; }})")
    }

    fn binary_expr(
        &mut self,
        op: BinOp,
        lhs: &IrExpr,
        rhs: &IrExpr,
        result_ty: &IrType,
        span: crate::span::Span,
    ) -> String {
        // `and`/`or` must not evaluate the right operand when the left
        // operand decides the result. Hoisting both operands into temporaries
        // (as the general effectful-binary path does) would silently change
        // semantics whenever the skipped operand has side effects.
        if matches!(op, BinOp::And | BinOp::Or) {
            let left_value = self.expr(lhs);
            let right_value = self.expr(rhs);
            if has_effects(rhs) {
                let left_name = self.fresh("lhs");
                let right_name = self.fresh("rhs");
                let (true_branch, false_branch) = match op {
                    BinOp::And => (right_value.as_str(), "false"),
                    BinOp::Or => ("true", right_value.as_str()),
                    _ => unreachable!(),
                };
                return format!(
                    "({{ bool {left_name} = {left_value}; bool {right_name} = {left_name} ? {true_branch} : {false_branch}; {right_name}; }})"
                );
            }
            let symbol = if op == BinOp::And { "&&" } else { "||" };
            return format!("(({left_value}) {symbol} ({right_value}))");
        }

        // Bind both operands whenever either one has effects. Semantically
        // both are always evaluated left to right, and some lowerings (Unit
        // equality) render a constant result and would otherwise drop an
        // effectful operand entirely. Hoisting both also guarantees the
        // source order regardless of the C compiler's argument order.
        if has_effects(lhs) || has_effects(rhs) {
            let left_name = self.fresh("lhs");
            let right_name = self.fresh("rhs");
            let left_value = self.expr(lhs);
            let right_value = self.expr(rhs);
            let body = self.binary_c(
                op,
                &left_name,
                &right_name,
                &lhs.ty,
                &rhs.ty,
                result_ty,
                span,
            );
            format!(
                "({{ {} {} = {}; {} {} = {}; {}; }})",
                self.ctype(&lhs.ty),
                left_name,
                left_value,
                self.ctype(&rhs.ty),
                right_name,
                right_value,
                body
            )
        } else {
            let left_value = self.expr(lhs);
            let right_value = self.expr(rhs);
            self.binary_c(
                op,
                &left_value,
                &right_value,
                &lhs.ty,
                &rhs.ty,
                result_ty,
                span,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn binary_c(
        &self,
        op: BinOp,
        left: &str,
        right: &str,
        left_ty: &IrType,
        _right_ty: &IrType,
        result_ty: &IrType,
        span: crate::span::Span,
    ) -> String {
        let (line, column) = self.position(span);
        let _ = result_ty;
        match op {
            BinOp::Add => match left_ty {
                IrType::Int => format!("vantyx_add_int({left}, {right}, {line}, {column})"),
                IrType::Float => format!("(({left}) + ({right}))"),
                IrType::Str => format!("vantyx_str_concat({left}, {right})"),
                _ => unreachable!(),
            },
            BinOp::Sub => match left_ty {
                IrType::Int => format!("vantyx_sub_int({left}, {right}, {line}, {column})"),
                IrType::Float => format!("(({left}) - ({right}))"),
                _ => unreachable!(),
            },
            BinOp::Mul => match left_ty {
                IrType::Int => format!("vantyx_mul_int({left}, {right}, {line}, {column})"),
                IrType::Float => format!("(({left}) * ({right}))"),
                _ => unreachable!(),
            },
            BinOp::Div => match left_ty {
                IrType::Int => format!("vantyx_div_int({left}, {right}, {line}, {column})"),
                IrType::Float => format!("vantyx_div_float({left}, {right}, {line}, {column})"),
                _ => unreachable!(),
            },
            BinOp::FloorDiv => {
                format!("vantyx_floordiv_int({left}, {right}, {line}, {column})")
            }
            BinOp::Mod => match left_ty {
                IrType::Int => format!("vantyx_mod_int({left}, {right}, {line}, {column})"),
                IrType::Float => format!("vantyx_mod_float({left}, {right}, {line}, {column})"),
                _ => unreachable!(),
            },
            BinOp::Pow => match left_ty {
                IrType::Int => format!("vantyx_pow_int({left}, {right}, {line}, {column})"),
                IrType::Float => format!("vantyx_pow_float({left}, {right})"),
                _ => unreachable!(),
            },
            BinOp::Eq => match left_ty {
                IrType::Str => format!("(strcmp({left}, {right}) == 0)"),
                IrType::Unit => "1".to_string(),
                IrType::List(_) | IrType::Map(_, _) | IrType::Struct(_) | IrType::Enum(_) => {
                    format!(
                        "vantyx_ptr_equals({left}, {right}, &{})",
                        self.desc_name(left_ty)
                    )
                }
                _ => format!("(({left}) == ({right}))"),
            },
            BinOp::NotEq => match left_ty {
                IrType::Str => format!("(strcmp({left}, {right}) != 0)"),
                IrType::Unit => "0".to_string(),
                IrType::List(_) | IrType::Map(_, _) | IrType::Struct(_) | IrType::Enum(_) => {
                    format!(
                        "!vantyx_ptr_equals({left}, {right}, &{})",
                        self.desc_name(left_ty)
                    )
                }
                _ => format!("(({left}) != ({right}))"),
            },
            BinOp::Lt => match left_ty {
                IrType::Str => format!("(strcmp({left}, {right}) < 0)"),
                _ => format!("(({left}) < ({right}))"),
            },
            BinOp::LtEq => match left_ty {
                IrType::Str => format!("(strcmp({left}, {right}) <= 0)"),
                _ => format!("(({left}) <= ({right}))"),
            },
            BinOp::Gt => match left_ty {
                IrType::Str => format!("(strcmp({left}, {right}) > 0)"),
                _ => format!("(({left}) > ({right}))"),
            },
            BinOp::GtEq => match left_ty {
                IrType::Str => format!("(strcmp({left}, {right}) >= 0)"),
                _ => format!("(({left}) >= ({right}))"),
            },
            BinOp::And => format!("(({left}) && ({right}))"),
            BinOp::Or => format!("(({left}) || ({right}))"),
        }
    }

    fn call_expr(&mut self, callee: &str, args: &[IrExpr], call_span: Option<Span>) -> String {
        let (prelude, invocation) = if !args.iter().any(has_effects) {
            let rendered = args
                .iter()
                .map(|arg| self.expr(arg))
                .collect::<Vec<_>>()
                .join(", ");
            (String::new(), format!("{callee}({rendered})"))
        } else {
            let mut prelude = String::new();
            let mut names = Vec::with_capacity(args.len());
            for arg in args {
                let value = self.expr(arg);
                let name = self.fresh("arg");
                let _ = write!(prelude, "{} {name} = {value}; ", self.ctype(&arg.ty));
                names.push(name);
            }
            (prelude, format!("{callee}({})", names.join(", ")))
        };

        // A user function sets `vantyx_source_name` on entry. Restore the call
        // site's file before continuing so later errors report the right file.
        // The call-site coordinates are set after argument evaluation, because
        // a nested call in an argument would otherwise leave its own
        // coordinates behind for the outer depth-limit check.
        let Some(span) = call_span else {
            if prelude.is_empty() {
                return invocation;
            }
            return format!("({{ {prelude}{invocation}; }})");
        };
        let caller = self.source_name_for(span).to_string();
        let (line, column) = self.position(span);
        let temp = self.fresh("call");
        format!(
            "({{ {prelude}vantyx_call_line = {line}; vantyx_call_column = {column}; __typeof__({invocation}) {temp} = {invocation}; vantyx_source_name = {}; {temp}; }})",
            c_string(&caller)
        )
    }

    /// Calls a struct constructor while preserving the order in which its
    /// named arguments were written. `args` is in declaration order, while
    /// `eval_order` lists argument indices in source order. Positional
    /// constructors have an identity order and use the ordinary call path.
    fn construct_expr(&mut self, callee: &str, args: &[IrExpr], eval_order: &[usize]) -> String {
        let identity = eval_order.len() == args.len()
            && eval_order
                .iter()
                .enumerate()
                .all(|(index, position)| index == *position);
        if identity || !args.iter().any(has_effects) {
            return self.call_expr(callee, args, None);
        }
        let mut values: Vec<Option<String>> = vec![None; args.len()];
        let mut prelude = String::new();
        for &index in eval_order {
            let value = self.expr(&args[index]);
            let name = self.fresh("arg");
            let _ = write!(
                prelude,
                "{} {name} = {value}; ",
                self.ctype(&args[index].ty)
            );
            values[index] = Some(name);
        }
        let rendered = values
            .into_iter()
            .map(|value| {
                value.expect("constructor evaluation order covers every argument exactly once")
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("({{ {prelude}{callee}({rendered}); }})")
    }

    fn builtin_expr(&mut self, name: &str, args: &[IrExpr], call: &IrExpr) -> String {
        let (line, column) = self.position(call.span);
        match name {
            "print" => {
                // Match the interpreter: evaluate every argument first, then
                // render and write the whole line. Rendering as arguments are
                // evaluated would (a) leak partial output when a later
                // argument fails and (b) display collections before later
                // arguments can mutate them.
                let mut init = Vec::new();
                let mut calls = Vec::new();
                for (index, arg) in args.iter().enumerate() {
                    let value = self.expr(arg);
                    let name = self.fresh("print_arg");
                    init.push(format!("{} {name} = {value};", self.ctype(&arg.ty)));
                    if index > 0 {
                        calls.push("vantyx_print_sep()".to_string());
                    }
                    let rendered = self.display_value(&name, &arg.ty);
                    calls.push(format!("vantyx_print_text({rendered})"));
                }
                calls.push(format!("vantyx_print_end({line}, {column})"));
                calls.push("(vantyx_unit)0".to_string());
                format!("({{ {} {}; }})", init.join(" "), calls.join(", "))
            }
            "len" => {
                let value = self.expr(&args[0]);
                match &args[0].ty {
                    IrType::Str => format!("vantyx_str_len({value})"),
                    IrType::List(_) => format!("vantyx_list_len({value})"),
                    IrType::Map(_, _) => format!("vantyx_map_len({value})"),
                    _ => unreachable!("IR rejected len on this type"),
                }
            }
            "str" => self.display_expr(&args[0]),
            "int" => {
                let value = self.expr(&args[0]);
                match &args[0].ty {
                    IrType::Int => value,
                    IrType::Float => format!("vantyx_int_from_float({value}, {line}, {column})"),
                    IrType::Str => format!("vantyx_int_from_str({value}, {line}, {column})"),
                    _ => unreachable!("IR rejected int conversion"),
                }
            }
            "float" => {
                let value = self.expr(&args[0]);
                match &args[0].ty {
                    IrType::Int => format!("vantyx_float_from_int({value})"),
                    IrType::Float => value,
                    IrType::Str => format!("vantyx_float_from_str({value}, {line}, {column})"),
                    _ => unreachable!("IR rejected float conversion"),
                }
            }
            "bool" => {
                // Like `type`, `bool` is an ordinary call: the interpreter
                // evaluates the argument before accepting a Bool or raising
                // R0007. Bind first so calls, output, mutation, and runtime
                // errors inside the argument are observable.
                let value = self.expr(&args[0]);
                let name = self.fresh("bool_arg");
                match &args[0].ty {
                    IrType::Bool => format!("({{ bool {name} = {value}; {name}; }})"),
                    other => format!(
                        "({{ {} {name} = {value}; (void){name}; vantyx_bool_from_other({}, {line}, {column}); }})",
                        self.ctype(&args[0].ty),
                        c_string(&other.name())
                    ),
                }
            }
            "type" => {
                // `type` reports a static type name, but it is still an
                // ordinary call: the interpreter evaluates its argument, so
                // native code must preserve observable side effects (calls,
                // output, runtime errors) before yielding the name.
                let value = self.expr(&args[0]);
                let name = self.fresh("type_arg");
                format!(
                    "({{ {} {name} = {value}; (void){name}; {}; }})",
                    self.ctype(&args[0].ty),
                    c_string(&args[0].ty.runtime_name())
                )
            }
            "read_file" => format!(
                "vantyx_read_file({}, {line}, {column})",
                self.expr(&args[0])
            ),
            "write_file" | "append_file" => {
                let append = if name == "append_file" { 1 } else { 0 };
                let path_name = self.fresh("path");
                let text_name = self.fresh("text");
                let path = self.expr(&args[0]);
                let text = self.expr(&args[1]);
                format!(
                    "({{ vantyx_str {path_name} = {path}; vantyx_str {text_name} = {text}; vantyx_write_file({path_name}, {text_name}, {append}, {line}, {column}); }})"
                )
            }
            "file_size" => format!(
                "vantyx_file_size({}, {line}, {column})",
                self.expr(&args[0])
            ),
            "is_file" | "is_dir" | "is_symlink" => {
                let function = match name {
                    "is_file" => "vantyx_is_file",
                    "is_dir" => "vantyx_is_dir",
                    _ => "vantyx_is_symlink",
                };
                format!("{function}({})", self.expr(&args[0]))
            }
            "file_exists" => {
                format!("vantyx_file_exists({})", self.expr(&args[0]))
            }
            "list_dir" => format!("vantyx_list_dir({}, {line}, {column})", self.expr(&args[0])),
            "glob" => format!("vantyx_glob({}, {line}, {column})", self.expr(&args[0])),
            "make_dir" | "remove_file" | "remove_dir" | "make_dir_all" | "remove_dir_all" => {
                let function = match name {
                    "make_dir" => "vantyx_make_dir",
                    "make_dir_all" => "vantyx_make_dir_all",
                    "remove_file" => "vantyx_remove_file",
                    "remove_dir_all" => "vantyx_remove_dir_all",
                    _ => "vantyx_remove_dir",
                };
                format!("{function}({}, {line}, {column})", self.expr(&args[0]))
            }
            "rename" | "copy_file" | "copy_dir_all" => {
                let function = match name {
                    "rename" => "vantyx_rename",
                    "copy_dir_all" => "vantyx_copy_dir_all",
                    _ => "vantyx_copy_file",
                };
                let source_name = self.fresh("source");
                let target_name = self.fresh("target");
                let source = self.expr(&args[0]);
                let target = self.expr(&args[1]);
                format!(
                    "({{ vantyx_str {source_name} = {source}; vantyx_str {target_name} = {target}; {function}({source_name}, {target_name}, {line}, {column}); }})"
                )
            }
            "push" => {
                let IrType::List(inner) = &args[0].ty else {
                    unreachable!("IR rejected push on this type");
                };
                let list_text = self.expr(&args[0]);
                let value_text = self.expr(&args[1]);
                let list_name = self.fresh("list");
                let value_name = self.fresh("value");
                let ctype = self.ctype(inner);
                format!(
                    "({{ VantyxList* {list_name} = {list_text}; {ctype} {value_name} = {value_text}; vantyx_list_push_raw({list_name}, &{value_name}, {line}, {column}); (vantyx_unit)0; }})"
                )
            }
            "pop" => {
                let IrType::List(inner) = &args[0].ty else {
                    unreachable!("IR rejected pop on this type");
                };
                let list_text = self.expr(&args[0]);
                let list_name = self.fresh("list");
                let out_name = self.fresh("popped");
                let ctype = self.ctype(inner);
                format!(
                    "({{ VantyxList* {list_name} = {list_text}; {ctype} {out_name}; vantyx_list_pop_raw({list_name}, &{out_name}, {line}, {column}); {out_name}; }})"
                )
            }
            "contains" => match &args[0].ty {
                IrType::List(inner) => {
                    let list_text = self.expr(&args[0]);
                    let needle_text = self.expr(&args[1]);
                    let list_name = self.fresh("list");
                    let needle_name = self.fresh("needle");
                    let ctype = self.ctype(inner);
                    format!(
                        "({{ VantyxList* {list_name} = {list_text}; {ctype} {needle_name} = {needle_text}; vantyx_list_contains({list_name}, &{needle_name}); }})"
                    )
                }
                IrType::Str => {
                    // C does not specify argument evaluation order, and GCC
                    // evaluates these right to left. Bind both operands in
                    // order so effectful arguments agree with the interpreter.
                    let haystack = self.expr(&args[0]);
                    let needle = self.expr(&args[1]);
                    let haystack_name = self.fresh("haystack");
                    let needle_name = self.fresh("needle");
                    format!(
                        "({{ vantyx_str {haystack_name} = {haystack}; vantyx_str {needle_name} = {needle}; vantyx_str_contains({haystack_name}, {needle_name}); }})"
                    )
                }
                IrType::Map(_, _) => {
                    let map_text = self.expr(&args[0]);
                    let key_text = self.expr(&args[1]);
                    let map_name = self.fresh("map");
                    let key_name = self.fresh("key");
                    let key_ctype = self.ctype(&args[1].ty);
                    format!(
                        "({{ VantyxMap* {map_name} = {map_text}; {key_ctype} {key_name} = {key_text}; vantyx_map_contains({map_name}, &{key_name}); }})"
                    )
                }
                _ => unreachable!("IR rejected contains on this type"),
            },
            "sorted" => {
                let IrType::List(inner) = &args[0].ty else {
                    unreachable!("IR rejected sorted on this type");
                };
                let function = match &**inner {
                    IrType::Int => "vantyx_list_sorted_int",
                    IrType::Float => "vantyx_list_sorted_float",
                    IrType::Str => "vantyx_list_sorted_str",
                    IrType::Bool => "vantyx_list_sorted_bool",
                    _ => unreachable!("IR rejected sorted on this element type"),
                };
                format!("{function}({})", self.expr(&args[0]))
            }
            "split" => {
                let text = self.expr(&args[0]);
                let separator = self.expr(&args[1]);
                let text_name = self.fresh("text");
                let separator_name = self.fresh("separator");
                format!(
                    "({{ vantyx_str {text_name} = {text}; vantyx_str {separator_name} = {separator}; vantyx_text_split({text_name}, {separator_name}, {line}, {column}); }})"
                )
            }
            "join" => {
                let list = self.expr(&args[0]);
                let separator = self.expr(&args[1]);
                let list_name = self.fresh("list");
                let separator_name = self.fresh("separator");
                format!(
                    "({{ VantyxList* {list_name} = {list}; vantyx_str {separator_name} = {separator}; vantyx_text_join({list_name}, {separator_name}); }})"
                )
            }
            "trim" => format!("vantyx_text_trim({})", self.expr(&args[0])),
            "starts_with" | "ends_with" => {
                let function = if name == "starts_with" {
                    "vantyx_text_starts_with"
                } else {
                    "vantyx_text_ends_with"
                };
                let text = self.expr(&args[0]);
                let affix = self.expr(&args[1]);
                let text_name = self.fresh("text");
                let affix_name = self.fresh("affix");
                format!(
                    "({{ vantyx_str {text_name} = {text}; vantyx_str {affix_name} = {affix}; {function}({text_name}, {affix_name}); }})"
                )
            }
            "replace" => {
                let text = self.expr(&args[0]);
                let pattern = self.expr(&args[1]);
                let replacement = self.expr(&args[2]);
                let text_name = self.fresh("text");
                let pattern_name = self.fresh("pattern");
                let replacement_name = self.fresh("replacement");
                format!(
                    "({{ vantyx_str {text_name} = {text}; vantyx_str {pattern_name} = {pattern}; vantyx_str {replacement_name} = {replacement}; vantyx_text_replace({text_name}, {pattern_name}, {replacement_name}, {line}, {column}); }})"
                )
            }
            "path_join" => {
                let base = self.expr(&args[0]);
                let child = self.expr(&args[1]);
                let base_name = self.fresh("base");
                let child_name = self.fresh("child");
                format!(
                    "({{ vantyx_str {base_name} = {base}; vantyx_str {child_name} = {child}; vantyx_path_join({base_name}, {child_name}); }})"
                )
            }
            "path_parent" | "path_name" | "path_stem" | "path_extension" => {
                let function = match name {
                    "path_parent" => "vantyx_path_parent",
                    "path_name" => "vantyx_path_name",
                    "path_stem" => "vantyx_path_stem",
                    _ => "vantyx_path_extension",
                };
                format!("{function}({})", self.expr(&args[0]))
            }
            "path_is_absolute" => {
                format!("vantyx_path_is_absolute({})", self.expr(&args[0]))
            }
            "range" => {
                // Bind the bounds left to right before building the list so
                // effectful arguments match the interpreter.
                let mut init = Vec::new();
                let mut names = Vec::new();
                for arg in args {
                    let value = self.expr(arg);
                    let name = self.fresh("range_arg");
                    init.push(format!("int64_t {name} = {value};"));
                    names.push(name);
                }
                let (start, end, step) = match names.as_slice() {
                    [end] => ("0".to_string(), end.clone(), "1".to_string()),
                    [start, end] => (start.clone(), end.clone(), "1".to_string()),
                    [start, end, step] => (start.clone(), end.clone(), step.clone()),
                    _ => unreachable!("IR rejected range arity"),
                };
                format!(
                    "({{ {} vantyx_range_list({start}, {end}, {step}, {line}, {column}); }})",
                    init.join(" ")
                )
            }
            "keys" | "values" => {
                let IrType::Map(key, value) = &args[0].ty else {
                    unreachable!("IR rejected keys/values on this type");
                };
                let element = if name == "keys" { key } else { value };
                let element_ctype = self.ctype(element);
                let desc = self.desc_name(element);
                let map_text = self.expr(&args[0]);
                let map_name = self.fresh("map");
                let function = if name == "keys" {
                    "vantyx_map_keys"
                } else {
                    "vantyx_map_values"
                };
                format!(
                    "({{ VantyxMap* {map_name} = {map_text}; {function}({map_name}, sizeof({element_ctype}), &{desc}); }})"
                )
            }
            "abs" => {
                let value = self.expr(&args[0]);
                match &args[0].ty {
                    IrType::Int => format!("vantyx_abs_int({value}, {line}, {column})"),
                    IrType::Float => format!("vantyx_abs_float({value})"),
                    _ => unreachable!("IR rejected abs"),
                }
            }
            "min" | "max" => {
                let left = self.expr(&args[0]);
                let right = self.expr(&args[1]);
                let left_name = self.fresh("minmax");
                let right_name = self.fresh("minmax");
                let comparison = if name == "min" { "<" } else { ">" };
                let operation = if args[0].ty == IrType::Float {
                    if name == "min" {
                        format!("vantyx_min_float({left_name}, {right_name})")
                    } else {
                        format!("vantyx_max_float({left_name}, {right_name})")
                    }
                } else {
                    format!("({left_name} {comparison} {right_name} ? {left_name} : {right_name})")
                };
                format!(
                    "({{ {} {left_name} = {left}; {} {right_name} = {right}; {operation}; }})",
                    self.ctype(&args[0].ty),
                    self.ctype(&args[1].ty)
                )
            }
            "sqrt" => {
                let value = self.expr(&args[0]);
                format!("vantyx_sqrt({value}, {line}, {column})")
            }
            "floor" => {
                let value = self.expr(&args[0]);
                format!("vantyx_floor_int({value}, {line}, {column})")
            }
            "ceil" => {
                let value = self.expr(&args[0]);
                format!("vantyx_ceil_int({value}, {line}, {column})")
            }
            "assert" => {
                let value = self.expr(&args[0]);
                format!("vantyx_assert({value}, {line}, {column})")
            }
            _ => unreachable!("IR rejected unsupported built-in `{name}`"),
        }
    }
}

fn register_desc_type(ty: &IrType, out: &mut Vec<IrType>) {
    match ty {
        IrType::List(inner) => register_desc_type(inner, out),
        IrType::Map(key, value) => {
            register_desc_type(key, out);
            register_desc_type(value, out);
        }
        IrType::Struct(_) | IrType::Enum(_) => {}
        _ => return,
    }
    if !out.contains(ty) {
        out.push(ty.clone());
    }
}

fn collect_stmt_desc_types(stmt: &IrStmt, out: &mut Vec<IrType>) {
    match &stmt.kind {
        IrStmtKind::Let { ty, init, .. } => {
            register_desc_type(ty, out);
            collect_expr_desc_types(init, out);
        }
        IrStmtKind::GlobalInit { value, .. } => collect_expr_desc_types(value, out),
        IrStmtKind::Try { operand, .. } => collect_expr_desc_types(operand, out),
        IrStmtKind::Assign { target, value } => {
            collect_place_desc_types(target, out);
            collect_expr_desc_types(value, out);
        }
        IrStmtKind::Expr(expr) => collect_expr_desc_types(expr, out),
        IrStmtKind::If {
            cond,
            then_body,
            elifs,
            else_body,
        } => {
            collect_expr_desc_types(cond, out);
            for stmt in then_body {
                collect_stmt_desc_types(stmt, out);
            }
            for (cond, body) in elifs {
                collect_expr_desc_types(cond, out);
                for stmt in body {
                    collect_stmt_desc_types(stmt, out);
                }
            }
            if let Some(body) = else_body {
                for stmt in body {
                    collect_stmt_desc_types(stmt, out);
                }
            }
        }
        IrStmtKind::While { cond, body } => {
            collect_expr_desc_types(cond, out);
            for stmt in body {
                collect_stmt_desc_types(stmt, out);
            }
        }
        IrStmtKind::ForRange {
            start,
            end,
            step,
            body,
            ..
        } => {
            collect_expr_desc_types(start, out);
            collect_expr_desc_types(end, out);
            collect_expr_desc_types(step, out);
            for stmt in body {
                collect_stmt_desc_types(stmt, out);
            }
        }
        IrStmtKind::ForList { list, body, .. } => {
            collect_expr_desc_types(list, out);
            for stmt in body {
                collect_stmt_desc_types(stmt, out);
            }
        }
        IrStmtKind::ForStr { text, body, .. } => {
            collect_expr_desc_types(text, out);
            for stmt in body {
                collect_stmt_desc_types(stmt, out);
            }
        }
        IrStmtKind::ForMap { map, body, .. } => {
            collect_expr_desc_types(map, out);
            for stmt in body {
                collect_stmt_desc_types(stmt, out);
            }
        }
        IrStmtKind::Match { scrutinee, arms } => {
            collect_expr_desc_types(scrutinee, out);
            for arm in arms {
                collect_pattern_desc_types(&arm.pattern, out);
                if let Some(guard) = &arm.guard {
                    collect_expr_desc_types(guard, out);
                }
                match &arm.body {
                    IrMatchBody::Expr(expr) => collect_expr_desc_types(expr, out),
                    IrMatchBody::Block(stmts) => {
                        for stmt in stmts {
                            collect_stmt_desc_types(stmt, out);
                        }
                    }
                }
            }
        }
        IrStmtKind::Return(Some(expr)) => collect_expr_desc_types(expr, out),
        IrStmtKind::Return(None) | IrStmtKind::Break | IrStmtKind::Continue => {}
    }
}

fn collect_pattern_desc_types(pattern: &IrPattern, out: &mut Vec<IrType>) {
    match pattern {
        IrPattern::Bind(binding) => register_desc_type(&binding.ty, out),
        IrPattern::Variant { fields, .. } => {
            for field in fields {
                collect_pattern_desc_types(&field.pattern, out);
            }
        }
        IrPattern::Or { alternatives } => {
            for alternative in alternatives {
                collect_pattern_desc_types(alternative, out);
            }
        }
        IrPattern::Wildcard | IrPattern::Literal { .. } | IrPattern::Range { .. } => {}
    }
}

fn collect_place_desc_types(place: &IrPlace, out: &mut Vec<IrType>) {
    match place {
        IrPlace::Var(_) | IrPlace::Global(_) => {}
        IrPlace::Field { base, .. } => collect_expr_desc_types(base, out),
        IrPlace::Index { base, index } => {
            collect_expr_desc_types(base, out);
            collect_expr_desc_types(index, out);
        }
    }
}

fn collect_expr_desc_types(expr: &IrExpr, out: &mut Vec<IrType>) {
    register_desc_type(&expr.ty, out);
    match &expr.kind {
        IrExprKind::Int(_)
        | IrExprKind::Float(_)
        | IrExprKind::Bool(_)
        | IrExprKind::Var(_)
        | IrExprKind::Global(_) => {}
        IrExprKind::Str(parts) => {
            for part in parts {
                if let IrStrPart::Value(value) = part {
                    collect_expr_desc_types(value, out);
                }
            }
        }
        IrExprKind::List(elements) => {
            for element in elements {
                collect_expr_desc_types(element, out);
            }
        }
        IrExprKind::Map(entries) => {
            for (key, value) in entries {
                collect_expr_desc_types(key, out);
                collect_expr_desc_types(value, out);
            }
        }
        IrExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_desc_types(lhs, out);
            collect_expr_desc_types(rhs, out);
        }
        IrExprKind::Unary { operand, .. } => collect_expr_desc_types(operand, out),
        IrExprKind::Call { args, .. }
        | IrExprKind::Builtin { args, .. }
        | IrExprKind::Construct { args, .. }
        | IrExprKind::ConstructVariant { args, .. } => {
            for arg in args {
                collect_expr_desc_types(arg, out);
            }
        }
        IrExprKind::Match { scrutinee, arms } => {
            collect_expr_desc_types(scrutinee, out);
            for arm in arms {
                collect_pattern_desc_types(&arm.pattern, out);
                if let Some(guard) = &arm.guard {
                    collect_expr_desc_types(guard, out);
                }
                match &arm.body {
                    IrMatchBody::Expr(expr) => collect_expr_desc_types(expr, out),
                    IrMatchBody::Block(stmts) => {
                        for stmt in stmts {
                            collect_stmt_desc_types(stmt, out);
                        }
                    }
                }
            }
        }
        IrExprKind::Field { base, .. } => collect_expr_desc_types(base, out),
        IrExprKind::Index { base, index } | IrExprKind::StrIndex { base, index } => {
            collect_expr_desc_types(base, out);
            collect_expr_desc_types(index, out);
        }
    }
}

fn part_has_effects(part: &IrStrPart) -> bool {
    match part {
        IrStrPart::Text(_) => false,
        IrStrPart::Value(value) => has_effects(value),
    }
}

/// Conservative effect analysis. Anything that can allocate, print, read an
/// uninitialized global, or raise a runtime error counts as observable.
fn has_effects(expr: &IrExpr) -> bool {
    match &expr.kind {
        IrExprKind::Int(_) | IrExprKind::Float(_) | IrExprKind::Bool(_) | IrExprKind::Var(_) => {
            false
        }
        IrExprKind::Str(parts) => parts.iter().any(part_has_effects),
        IrExprKind::List(_)
        | IrExprKind::Map(_)
        | IrExprKind::Index { .. }
        | IrExprKind::StrIndex { .. } => true,
        IrExprKind::Global(_)
        | IrExprKind::Binary { .. }
        | IrExprKind::Unary { .. }
        | IrExprKind::Call { .. }
        | IrExprKind::Builtin { .. }
        | IrExprKind::Construct { .. }
        | IrExprKind::ConstructVariant { .. }
        | IrExprKind::Match { .. }
        | IrExprKind::Field { .. } => true,
    }
}

fn c_int(value: i64) -> String {
    if value == i64::MIN {
        "INT64_MIN".to_string()
    } else {
        format!("{value}LL")
    }
}

fn c_float(value: f64) -> String {
    if value.is_nan() {
        "NAN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "(-INFINITY)".to_string()
        } else {
            "INFINITY".to_string()
        }
    } else {
        format!("{value:?}")
    }
}

fn c_string(text: &str) -> String {
    let mut out = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => {
                let _ = write!(out, "\\{byte:03o}");
            }
        }
    }
    out.push('"');
    out
}

const PREAMBLE: &str = r#"/* Generated by the Vantyx native backend. */
#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <math.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

typedef int vantyx_unit;
typedef const char* vantyx_str;

typedef enum {
    VANTYX_T_UNIT,
    VANTYX_T_INT,
    VANTYX_T_FLOAT,
    VANTYX_T_BOOL,
    VANTYX_T_STR,
    VANTYX_T_LIST,
    VANTYX_T_MAP,
    VANTYX_T_STRUCT,
    VANTYX_T_ENUM
} VantyxTypeKind;

typedef struct VantyxTypeDesc VantyxTypeDesc;
typedef struct VantyxList VantyxList;
typedef struct VantyxMap VantyxMap;

struct VantyxTypeDesc {
    VantyxTypeKind kind;
    int64_t size;
    const VantyxTypeDesc* elem;
    const VantyxTypeDesc* key;
    const VantyxTypeDesc* value;
    vantyx_str (*display_value)(void* value, int depth);
    bool (*equals_value)(void* left, void* right, int depth);
};

struct VantyxList {
    int64_t len;
    int64_t cap;
    int64_t elem_size;
    const VantyxTypeDesc* elem_desc;
    void* items;
};

struct VantyxMap {
    int64_t len;
    int64_t cap;
    int64_t key_size;
    int64_t value_size;
    const VantyxTypeDesc* key_desc;
    const VantyxTypeDesc* value_desc;
    void* keys;
    void* values;
    bool hashed;
    int64_t index_cap;
    int64_t* index;
};

static const VantyxTypeDesc vantyx_desc_unit = { VANTYX_T_UNIT, sizeof(vantyx_unit), NULL, NULL, NULL, NULL, NULL };
static const VantyxTypeDesc vantyx_desc_int = { VANTYX_T_INT, sizeof(int64_t), NULL, NULL, NULL, NULL, NULL };
static const VantyxTypeDesc vantyx_desc_float = { VANTYX_T_FLOAT, sizeof(double), NULL, NULL, NULL, NULL, NULL };
static const VantyxTypeDesc vantyx_desc_bool = { VANTYX_T_BOOL, sizeof(bool), NULL, NULL, NULL, NULL, NULL };
static const VantyxTypeDesc vantyx_desc_str = { VANTYX_T_STR, sizeof(vantyx_str), NULL, NULL, NULL, NULL, NULL };

static const char* vantyx_source_name = "<unknown>";
static int vantyx_call_depth = 0;
static int vantyx_call_line = 0;
static int vantyx_call_column = 0;

_Noreturn static void vantyx_rt_error(const char* code, const char* message, int line, int column) {
    fflush(stdout);
    fprintf(stderr, "error[%s]: %s\n", code, message);
    fprintf(stderr, " --> %s:%d:%d\n", vantyx_source_name, line, column);
    exit(2);
}

/* Rust's runtime opens /dev/null for every standard descriptor that is closed
   before `main`. Match that so a closed stdout silently discards `print`
   output instead of failing with EBADF, and so later file opens cannot reuse
   descriptor 0, 1, or 2. */
static void vantyx_sanitize_standard_fd(int fd, int flags) {
    if (fcntl(fd, F_GETFD) != -1 || errno != EBADF) return;
    int replacement = open("/dev/null", flags);
    if (replacement < 0) return;
    if (replacement == fd) return;
    if (dup2(replacement, fd) == -1) {
        close(replacement);
        return;
    }
    close(replacement);
}

static void* vantyx_alloc(size_t size) {
    void* pointer = malloc(size == 0 ? 1 : size);
    if (pointer == NULL) {
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    return pointer;
}

static size_t vantyx_checked_add(size_t left, size_t right, const char* what) {
    if (left > SIZE_MAX - right) {
        fprintf(stderr, "error: %s is too large\n", what);
        exit(2);
    }
    return left + right;
}

static void* vantyx_alloc_array(size_t count, size_t size) {
    if (size != 0 && count > SIZE_MAX / size) {
        fputs("error: allocation is too large\n", stderr);
        exit(2);
    }
    return vantyx_alloc(count * size);
}

static vantyx_str vantyx_strdup(const char* text) {
    size_t length = strlen(text);
    size_t total = vantyx_checked_add(length, 1, "string");
    char* copy = (char*)vantyx_alloc_array(total, 1);
    memcpy(copy, text, total);
    return copy;
}

static vantyx_str vantyx_strndup(const char* text, size_t length) {
    size_t total = vantyx_checked_add(length, 1, "string");
    char* copy = (char*)vantyx_alloc_array(total, 1);
    memcpy(copy, text, length);
    copy[length] = '\0';
    return copy;
}

/* Rust's `std::io::Error` Display appends `(os error N)` on Unix. The native
   runtime must use the same suffix so filesystem diagnostics match the
   interpreter instead of printing only `strerror`. */
static vantyx_str vantyx_os_error(int error_number) {
    const char* text = strerror(error_number);
    if (text == NULL) text = "Unknown error";
    char buffer[512];
    snprintf(buffer, sizeof buffer, "%s (os error %d)", text, error_number);
    return vantyx_strdup(buffer);
}

/* File I/O (ADR-0026). Reads are capped so a runaway file cannot exhaust the
   process, contents must be valid UTF-8 without NUL bytes, and writes use the
   host filesystem with the caller's privileges. There is no sandbox. */
#define VANTYX_MAX_FILE_BYTES ((size_t)64 * 1024 * 1024)

static bool vantyx_utf8_valid(const unsigned char* text, size_t length) {
    size_t index = 0;
    while (index < length) {
        unsigned char lead = text[index];
        if (lead < 0x80) {
            index++;
            continue;
        }
        size_t continuation;
        uint32_t code;
        if ((lead & 0xE0) == 0xC0) {
            continuation = 1;
            code = lead & 0x1F;
            if (code < 2) return false;
        } else if ((lead & 0xF0) == 0xE0) {
            continuation = 2;
            code = lead & 0x0F;
        } else if ((lead & 0xF8) == 0xF0) {
            continuation = 3;
            code = lead & 0x07;
        } else {
            return false;
        }
        if (index + continuation >= length) return false;
        for (size_t offset = 1; offset <= continuation; offset++) {
            unsigned char next = text[index + offset];
            if ((next & 0xC0) != 0x80) return false;
            code = (code << 6) | (next & 0x3F);
        }
        if (continuation == 2 && code < 0x800) return false;
        if (continuation == 3 && code < 0x10000) return false;
        if (code > 0x10FFFF) return false;
        if (code >= 0xD800 && code <= 0xDFFF) return false;
        index += continuation + 1;
    }
    return true;
}

static vantyx_str vantyx_read_file(vantyx_str path, int line, int column) {
    FILE* file = fopen(path, "rb");
    if (file == NULL) {
        char message[512];
        snprintf(message, sizeof message, "cannot read file `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t capacity = 8192;
    size_t length = 0;
    char* buffer = (char*)vantyx_alloc_array(capacity, 1);
    for (;;) {
        if (length == capacity) {
            if (capacity > VANTYX_MAX_FILE_BYTES) {
                fclose(file);
                char message[512];
                snprintf(message, sizeof message,
                         "file `%.200s` exceeds the 64 MiB read limit", path);
                vantyx_rt_error("R0015", message, line, column);
            }
            size_t grown = capacity * 2;
            if (grown > VANTYX_MAX_FILE_BYTES + 1) grown = VANTYX_MAX_FILE_BYTES + 1;
            char* next = (char*)realloc(buffer, grown);
            if (next == NULL) {
                fclose(file);
                fputs("error: out of memory\n", stderr);
                exit(2);
            }
            buffer = next;
            capacity = grown;
        }
        size_t count = fread(buffer + length, 1, capacity - length, file);
        length += count;
        if (count == 0) {
            if (ferror(file)) {
                int error_number = errno;
                fclose(file);
                char message[512];
                snprintf(message, sizeof message,
                         "cannot read file `%.200s`: %.200s", path, vantyx_os_error(error_number));
                vantyx_rt_error("R0015", message, line, column);
            }
            break;
        }
        if (length > VANTYX_MAX_FILE_BYTES) {
            fclose(file);
            char message[512];
            snprintf(message, sizeof message,
                     "file `%.200s` exceeds the 64 MiB read limit", path);
            vantyx_rt_error("R0015", message, line, column);
        }
    }
    if (fclose(file) != 0) {
        int error_number = errno;
        char message[512];
        snprintf(message, sizeof message,
                 "cannot read file `%.200s`: %.200s", path, vantyx_os_error(error_number));
        vantyx_rt_error("R0015", message, line, column);
    }
    if (memchr(buffer, '\0', length) != NULL) {
        char message[512];
        snprintf(message, sizeof message, "file `%.200s` contains a NUL byte", path);
        vantyx_rt_error("R0015", message, line, column);
    }
    if (!vantyx_utf8_valid((const unsigned char*)buffer, length)) {
        char message[512];
        snprintf(message, sizeof message,
                 "file `%.200s` contents are not valid UTF-8", path);
        vantyx_rt_error("R0015", message, line, column);
    }
    char* result = (char*)realloc(buffer, length + 1);
    if (result == NULL) {
        free(buffer);
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    result[length] = '\0';
    return result;
}

static vantyx_unit vantyx_write_file(vantyx_str path, vantyx_str text, int append,
                                     int line, int column) {
    FILE* file = fopen(path, append ? "ab" : "wb");
    if (file == NULL) {
        char message[512];
        snprintf(message, sizeof message, "cannot write file `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t length = strlen(text);
    if (length != 0 && fwrite(text, 1, length, file) != length) {
        int error_number = errno;
        fclose(file);
        char message[512];
        snprintf(message, sizeof message,
                 "cannot write file `%.200s`: %.200s", path, vantyx_os_error(error_number));
        vantyx_rt_error("R0015", message, line, column);
    }
    if (fclose(file) != 0) {
        int error_number = errno;
        char message[512];
        snprintf(message, sizeof message,
                 "cannot write file `%.200s`: %.200s", path, vantyx_os_error(error_number));
        vantyx_rt_error("R0015", message, line, column);
    }
    return (vantyx_unit)0;
}

static bool vantyx_file_exists(vantyx_str path) {
    struct stat info;
    return stat(path, &info) == 0;
}

/* Follows symbolic links and requires a regular file. Reports the logical
   size (`st_size`), not allocated blocks. */
static int64_t vantyx_file_size(vantyx_str path, int line, int column) {
    struct stat info;
    if (stat(path, &info) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot read metadata for `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    if (!S_ISREG(info.st_mode)) {
        char message[512];
        snprintf(message, sizeof message,
                 "`%.200s` is not a regular file", path);
        vantyx_rt_error("R0015", message, line, column);
    }
    return (int64_t)info.st_size;
}

static bool vantyx_is_file(vantyx_str path) {
    struct stat info;
    return stat(path, &info) == 0 && S_ISREG(info.st_mode);
}

static bool vantyx_is_dir(vantyx_str path) {
    struct stat info;
    return stat(path, &info) == 0 && S_ISDIR(info.st_mode);
}

static bool vantyx_is_symlink(vantyx_str path) {
    struct stat info;
    return lstat(path, &info) == 0 && S_ISLNK(info.st_mode);
}

/* Single-byte ASCII characters are immutable and extremely common in loops.
   Return a pointer into a static table instead of allocating a 2-byte string
   for every character. Native strings are never mutated in place, so sharing
   these pointers is safe. */
static char vantyx_ascii_char_table[128][2];
static int vantyx_ascii_char_ready = 0;

static vantyx_str vantyx_str_slice(const char* text, int bytes) {
    if (bytes == 1) {
        unsigned char byte = (unsigned char)text[0];
        if (byte < 128) {
            if (!vantyx_ascii_char_ready) {
                for (int i = 0; i < 128; i++) {
                    vantyx_ascii_char_table[i][0] = (char)i;
                    vantyx_ascii_char_table[i][1] = 0;
                }
                vantyx_ascii_char_ready = 1;
            }
            return vantyx_ascii_char_table[byte];
        }
    }
    return vantyx_strndup(text, (size_t)bytes);
}

static int vantyx_utf8_char_len(const unsigned char lead) {
    if (lead < 0x80) return 1;
    if ((lead & 0xE0) == 0xC0) return 2;
    if ((lead & 0xF0) == 0xE0) return 3;
    if ((lead & 0xF8) == 0xF0) return 4;
    return 1;
}

static int64_t vantyx_str_len(vantyx_str text);

static int64_t vantyx_str_char_offset(vantyx_str text, int64_t index) {
    int64_t position = 0;
    const char* cursor = text;
    while (*cursor != 0) {
        if (position == index) return (int64_t)(cursor - text);
        cursor += vantyx_utf8_char_len((const unsigned char)*cursor);
        position++;
    }
    return -1;
}

static vantyx_str vantyx_str_index(vantyx_str text, int64_t index, int line, int column) {
    if (index < 0) {
        char message[128];
        snprintf(message, sizeof message, "negative index %" PRId64 " is not allowed", index);
        vantyx_rt_error("R0003", message, line, column);
    }
    int64_t offset = vantyx_str_char_offset(text, index);
    if (offset < 0) {
        char message[192];
        snprintf(message, sizeof message,
                 "index %" PRId64 " is out of bounds for a string of length %" PRId64,
                 index, vantyx_str_len(text));
        vantyx_rt_error("R0003", message, line, column);
    }
    int bytes = vantyx_utf8_char_len((const unsigned char)text[offset]);
    return vantyx_str_slice(text + offset, bytes);
}

static vantyx_str vantyx_str_concat(vantyx_str left, vantyx_str right) {
    size_t left_length = strlen(left);
    size_t right_length = strlen(right);
    size_t total = vantyx_checked_add(left_length, right_length, "string");
    total = vantyx_checked_add(total, 1, "string");
    char* result = (char*)vantyx_alloc_array(total, 1);
    memcpy(result, left, left_length);
    memcpy(result + left_length, right, right_length + 1);
    return result;
}

static vantyx_str vantyx_str_quote(vantyx_str text) {
    size_t length = strlen(text);
    size_t total = vantyx_checked_add(length, length, "string");
    total = vantyx_checked_add(total, 3, "string");
    char* result = (char*)vantyx_alloc_array(total, 1);
    size_t out = 0;
    result[out++] = '"';
    for (size_t i = 0; i < length; i++) {
        char c = text[i];
        switch (c) {
            case '\\': result[out++] = '\\'; result[out++] = '\\'; break;
            case '"': result[out++] = '\\'; result[out++] = '"'; break;
            case '\n': result[out++] = '\\'; result[out++] = 'n'; break;
            case '\r': result[out++] = '\\'; result[out++] = 'r'; break;
            case '\t': result[out++] = '\\'; result[out++] = 't'; break;
            default: result[out++] = c; break;
        }
    }
    result[out++] = '"';
    result[out] = '\0';
    return result;
}

static vantyx_str vantyx_str_from_int(int64_t value) {
    char buffer[32];
    snprintf(buffer, sizeof buffer, "%" PRId64, value);
    return vantyx_strdup(buffer);
}

static vantyx_str vantyx_float_str(double value) {
    if (isnan(value)) return "NaN";
    if (isinf(value)) return value < 0 ? "-Infinity" : "Infinity";
    if (value == 0.0) return signbit(value) ? "-0.0" : "0.0";

    // Rust's `f64` Display prints the shortest decimal that round-trips and
    // never uses exponent notation. Find the shortest significant-digit
    // precision, then expand the scientific form to plain decimal.
    char scientific[64];
    int precision = 1;
    for (; precision <= 17; precision++) {
        snprintf(scientific, sizeof scientific, "%.*e", precision - 1, value);
        char* end = NULL;
        errno = 0;
        double parsed = strtod(scientific, &end);
        if (end != scientific && *end == '\0' && parsed == value
            && signbit(parsed) == signbit(value)) {
            break;
        }
    }
    if (precision > 17) {
        snprintf(scientific, sizeof scientific, "%.17e", value);
    }

    const char* p = scientific;
    bool negative = false;
    if (*p == '-') {
        negative = true;
        p++;
    }
    char digits[32];
    int digit_count = 0;
    while (*p != '\0' && *p != 'e' && *p != 'E') {
        if (*p >= '0' && *p <= '9') digits[digit_count++] = *p;
        p++;
    }
    int exponent = 0;
    if (*p == 'e' || *p == 'E') {
        p++;
        int exponent_sign = 1;
        if (*p == '+') {
            p++;
        } else if (*p == '-') {
            exponent_sign = -1;
            p++;
        }
        while (*p >= '0' && *p <= '9') {
            exponent = exponent * 10 + (*p - '0');
            p++;
        }
        exponent *= exponent_sign;
    }
    int point = exponent + 1;

    char buffer[512];
    size_t out = 0;
    if (negative) buffer[out++] = '-';
    if (point <= 0) {
        buffer[out++] = '0';
        buffer[out++] = '.';
        for (int i = 0; i < -point; i++) buffer[out++] = '0';
        for (int i = 0; i < digit_count; i++) buffer[out++] = digits[i];
    } else if (point >= digit_count) {
        for (int i = 0; i < digit_count; i++) buffer[out++] = digits[i];
        for (int i = digit_count; i < point; i++) buffer[out++] = '0';
        buffer[out++] = '.';
        buffer[out++] = '0';
    } else {
        for (int i = 0; i < point; i++) buffer[out++] = digits[i];
        buffer[out++] = '.';
        for (int i = point; i < digit_count; i++) buffer[out++] = digits[i];
    }
    buffer[out] = '\0';
    return vantyx_strdup(buffer);
}

static vantyx_str vantyx_str_from_float(double value) { return vantyx_float_str(value); }
static vantyx_str vantyx_str_from_bool(bool value) { return value ? "true" : "false"; }
static vantyx_str vantyx_str_from_unit(void) { return "()"; }

static int64_t vantyx_str_len(vantyx_str text) {
    int64_t count = 0;
    for (const unsigned char* p = (const unsigned char*)text; *p != 0; p++) {
        if ((*p & 0xC0) != 0x80) count++;
    }
    return count;
}

static bool vantyx_str_contains(vantyx_str haystack, vantyx_str needle) {
    return strstr(haystack, needle) != NULL;
}

static int64_t vantyx_add_int(int64_t left, int64_t right, int line, int column) {
    int64_t result;
    if (__builtin_add_overflow(left, right, &result)) {
        vantyx_rt_error("R0002", "integer overflow in `+`", line, column);
    }
    return result;
}

static int64_t vantyx_sub_int(int64_t left, int64_t right, int line, int column) {
    int64_t result;
    if (__builtin_sub_overflow(left, right, &result)) {
        vantyx_rt_error("R0002", "integer overflow in `-`", line, column);
    }
    return result;
}

static int64_t vantyx_mul_int(int64_t left, int64_t right, int line, int column) {
    int64_t result;
    if (__builtin_mul_overflow(left, right, &result)) {
        vantyx_rt_error("R0002", "integer overflow in `*`", line, column);
    }
    return result;
}

static double vantyx_div_int(int64_t left, int64_t right, int line, int column) {
    if (right == 0) vantyx_rt_error("R0001", "division by zero", line, column);
    return (double)left / (double)right;
}

static double vantyx_div_float(double left, double right, int line, int column) {
    if (right == 0.0) vantyx_rt_error("R0001", "division by zero", line, column);
    return left / right;
}

static int64_t vantyx_floordiv_int(int64_t left, int64_t right, int line, int column) {
    if (right == 0) vantyx_rt_error("R0001", "division by zero", line, column);
    if (left == INT64_MIN && right == -1) {
        vantyx_rt_error("R0002", "integer overflow in `//`", line, column);
    }
    int64_t quotient = left / right;
    int64_t remainder = left % right;
    if (remainder != 0 && ((remainder < 0) != (right < 0))) quotient -= 1;
    return quotient;
}

static int64_t vantyx_mod_int(int64_t left, int64_t right, int line, int column) {
    if (right == 0) vantyx_rt_error("R0001", "division by zero", line, column);
    if (left == INT64_MIN && right == -1) {
        vantyx_rt_error("R0002", "integer overflow in `%`", line, column);
    }
    return left % right;
}

static double vantyx_mod_float(double left, double right, int line, int column) {
    if (right == 0.0) vantyx_rt_error("R0001", "division by zero", line, column);
    return fmod(left, right);
}

static int64_t vantyx_pow_int(int64_t base, int64_t exponent, int line, int column) {
    if (exponent < 0) {
        vantyx_rt_error("R0004", "`**` requires a non-negative Int exponent", line, column);
    }
    int64_t result = 1;
    while (exponent > 0) {
        if (exponent & 1) {
            if (__builtin_mul_overflow(result, base, &result)) {
                vantyx_rt_error("R0002", "integer overflow in `**`", line, column);
            }
        }
        exponent >>= 1;
        if (exponent > 0) {
            if (__builtin_mul_overflow(base, base, &base)) {
                vantyx_rt_error("R0002", "integer overflow in `**`", line, column);
            }
        }
    }
    return result;
}

static double vantyx_pow_float(double base, double exponent) { return pow(base, exponent); }

static int64_t vantyx_neg_int(int64_t value, int line, int column) {
    if (value == INT64_MIN) {
        vantyx_rt_error("R0002", "integer overflow while negating", line, column);
    }
    return -value;
}

static int64_t vantyx_abs_int(int64_t value, int line, int column) {
    if (value == INT64_MIN) {
        vantyx_rt_error("R0002", "integer overflow in `abs`", line, column);
    }
    return value < 0 ? -value : value;
}

static double vantyx_abs_float(double value) { return fabs(value); }

// Vantyx min/max use the interpreter's explicit rule: a single NaN operand
// yields the other operand, and ties (including -0.0/0.0) return the second
// operand. C fmin/fmax return the first operand on ties, so they cannot be
// used directly.
static double vantyx_min_float(double left, double right) {
    if (isnan(right)) return left;
    if (isnan(left)) return right;
    return left < right ? left : right;
}

static double vantyx_max_float(double left, double right) {
    if (isnan(right)) return left;
    if (isnan(left)) return right;
    return left > right ? left : right;
}

static double vantyx_sqrt(double value, int line, int column) {
    if (value < 0.0) vantyx_rt_error("R0004", "`sqrt` of a negative number", line, column);
    return sqrt(value);
}

static int64_t vantyx_floor_int(double value, int line, int column) {
    double result = floor(value);
    if (!isfinite(result) || result < -9223372036854775808.0 || result >= 9223372036854775808.0) {
        vantyx_rt_error("R0007", "`floor` result does not fit in Int", line, column);
    }
    return (int64_t)result;
}

static int64_t vantyx_ceil_int(double value, int line, int column) {
    double result = ceil(value);
    if (!isfinite(result) || result < -9223372036854775808.0 || result >= 9223372036854775808.0) {
        vantyx_rt_error("R0007", "`ceil` result does not fit in Int", line, column);
    }
    return (int64_t)result;
}

static int64_t vantyx_int_from_float(double value, int line, int column) {
    if (!isfinite(value) || value != trunc(value) || value < -9223372036854775808.0 || value >= 9223372036854775808.0) {
        char message[512];
        snprintf(message, sizeof message, "cannot convert `%s` to Int exactly", vantyx_float_str(value));
        vantyx_rt_error("R0007", message, line, column);
    }
    return (int64_t)value;
}

static vantyx_str vantyx_text_trim(vantyx_str text);

/* The interpreter trims Unicode whitespace and then uses Rust's strict
   integer/float grammars. `strtoll`/`strtod` alone accept forms Rust rejects
   (hex floats, `nan(payload)`) and reject Unicode whitespace, so validate the
   exact grammar before delegating the conversion. */
static bool vantyx_ascii_prefix_ci(const char* text, const char* lower) {
    while (*lower != '\0') {
        char c = *text++;
        if (c >= 'A' && c <= 'Z') c = (char)(c - 'A' + 'a');
        if (c != *lower++) return false;
    }
    return true;
}

static bool vantyx_valid_int_text(const char* text) {
    if (*text == '+' || *text == '-') text++;
    if (*text < '0' || *text > '9') return false;
    while (*text >= '0' && *text <= '9') text++;
    return *text == '\0';
}

static bool vantyx_valid_float_text(const char* text) {
    if (*text == '+' || *text == '-') text++;
    if (vantyx_ascii_prefix_ci(text, "infinity") && text[8] == '\0') return true;
    if (vantyx_ascii_prefix_ci(text, "inf") && text[3] == '\0') return true;
    if (vantyx_ascii_prefix_ci(text, "nan") && text[3] == '\0') return true;
    bool mantissa_digit = false;
    while (*text >= '0' && *text <= '9') {
        text++;
        mantissa_digit = true;
    }
    if (*text == '.') {
        text++;
        while (*text >= '0' && *text <= '9') {
            text++;
            mantissa_digit = true;
        }
    }
    if (!mantissa_digit) return false;
    if (*text == 'e' || *text == 'E') {
        text++;
        if (*text == '+' || *text == '-') text++;
        if (*text < '0' || *text > '9') return false;
        while (*text >= '0' && *text <= '9') text++;
    }
    return *text == '\0';
}

static int64_t vantyx_int_from_str(vantyx_str text, int line, int column) {
    vantyx_str trimmed = vantyx_text_trim(text);
    errno = 0;
    long long value = strtoll(trimmed, NULL, 10);
    if (!vantyx_valid_int_text(trimmed) || errno == ERANGE) {
        char message[512];
        snprintf(message, sizeof message, "cannot convert `%s` to Int", text);
        vantyx_rt_error("R0007", message, line, column);
    }
    return (int64_t)value;
}

static double vantyx_float_from_int(int64_t value) { return (double)value; }

static double vantyx_float_from_str(vantyx_str text, int line, int column) {
    vantyx_str trimmed = vantyx_text_trim(text);
    if (!vantyx_valid_float_text(trimmed)) {
        char message[512];
        snprintf(message, sizeof message, "cannot convert `%s` to Float", text);
        vantyx_rt_error("R0007", message, line, column);
    }
    /* Rust parses overflow to infinity and underflow to zero, so ERANGE is
       not an error here. */
    return strtod(trimmed, NULL);
}

static bool vantyx_bool_from_other(const char* type_name, int line, int column) {
    char message[128];
    snprintf(message, sizeof message, "cannot convert `%s` to Bool; only Bool is accepted", type_name);
    vantyx_rt_error("R0007", message, line, column);
    return false;
}

static vantyx_unit vantyx_assert(bool condition, int line, int column) {
    if (!condition) vantyx_rt_error("R0006", "assertion failed", line, column);
    return 0;
}

static vantyx_unit vantyx_print_end(int line, int column) {
    // Flush each completed line so a full disk or closed descriptor is
    // reported at the print that caused it, matching the interpreter's
    // line-buffered stdout. Keep the first errno for the message.
    int error_number = 0;
    errno = 0;
    if (fputc('\n', stdout) == EOF) error_number = errno;
    if (fflush(stdout) != 0 && error_number == 0) error_number = errno;
    if (ferror(stdout) && error_number == 0) error_number = errno;
    if (error_number != 0 || ferror(stdout)) {
        char message[512];
        if (error_number != 0) {
            snprintf(message, sizeof message,
                     "failed to write output: %s (os error %d)",
                     strerror(error_number), error_number);
        } else {
            snprintf(message, sizeof message, "failed to write output");
        }
        vantyx_rt_error("R0013", message, line, column);
    }
    return 0;
}

static void vantyx_print_text(vantyx_str text) { fputs(text, stdout); }
static void vantyx_print_sep(void) { fputc(' ', stdout); }

static vantyx_unit vantyx_check_init(bool initialized, const char* name, int line, int column) {
    if (!initialized) {
        char message[256];
        snprintf(message, sizeof message, "variable `%s` is not initialized yet", name);
        vantyx_rt_error("R0009", message, line, column);
    }
    return 0;
}

static int64_t vantyx_range_count(int64_t start, int64_t end, int64_t step, int line, int column) {
    if (step == 0) vantyx_rt_error("R0004", "`range` step must not be zero", line, column);
    __int128 s = start, e = end, st = step;
    __int128 count;
    if (st > 0) {
        count = s >= e ? 0 : (e - s + st - 1) / st;
    } else {
        count = s <= e ? 0 : (s - e - st - 1) / -st;
    }
    if (count > 10000000) {
        char message[128];
        snprintf(message, sizeof message,
                 "`range` would produce %llu elements, exceeding the limit of 10000000",
                 (unsigned long long)count);
        vantyx_rt_error("R0004", message, line, column);
    }
    return (int64_t)count;
}

static int64_t vantyx_list_len(VantyxList* list) { return list->len; }

static VantyxList* vantyx_list_new(int64_t elem_size, const VantyxTypeDesc* elem_desc) {
    VantyxList* list = (VantyxList*)vantyx_alloc(sizeof(VantyxList));
    list->len = 0;
    list->cap = 0;
    list->elem_size = elem_size > 0 ? elem_size : 1;
    list->elem_desc = elem_desc;
    list->items = NULL;
    return list;
}

static void vantyx_list_reserve(VantyxList* list, int64_t needed, int line, int column) {
    if (needed <= list->cap) return;
    int64_t cap = list->cap < 4 ? 4 : list->cap;
    while (cap < needed) {
        if (cap > INT64_MAX / 2) vantyx_rt_error("R0004", "list capacity overflow", line, column);
        cap *= 2;
    }
    if (cap > INT64_MAX / list->elem_size ||
        (uint64_t)cap > (uint64_t)SIZE_MAX / (uint64_t)list->elem_size) {
        vantyx_rt_error("R0004", "list allocation is too large", line, column);
    }
    void* items = realloc(list->items, (size_t)(cap * list->elem_size));
    if (items == NULL) {
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    list->items = items;
    list->cap = cap;
}

static void* vantyx_list_slot(VantyxList* list, int64_t index, int line, int column) {
    if (index < 0) {
        char message[128];
        snprintf(message, sizeof message, "negative index %" PRId64 " is not allowed", index);
        vantyx_rt_error("R0003", message, line, column);
    }
    if (index >= list->len) {
        char message[192];
        snprintf(message, sizeof message,
                 "index %" PRId64 " is out of bounds for a list of length %" PRId64,
                 index, list->len);
        vantyx_rt_error("R0003", message, line, column);
    }
    return (char*)list->items + index * list->elem_size;
}

static void* vantyx_list_slot_unchecked(VantyxList* list, int64_t index) {
    return (char*)list->items + index * list->elem_size;
}

static void vantyx_list_push_raw(VantyxList* list, const void* value, int line, int column) {
    vantyx_list_reserve(list, list->len + 1, line, column);
    memcpy((char*)list->items + list->len * list->elem_size, value, (size_t)list->elem_size);
    list->len++;
}

static VantyxList* vantyx_range_list(int64_t start, int64_t end, int64_t step, int line, int column) {
    int64_t count = vantyx_range_count(start, end, step, line, column);
    VantyxList* list = vantyx_list_new(sizeof(int64_t), &vantyx_desc_int);
    if (count > 0) vantyx_list_reserve(list, count, line, column);
    __int128 value = start;
    for (int64_t index = 0; index < count; index++) {
        int64_t item = (int64_t)value;
        vantyx_list_push_raw(list, &item, line, column);
        value += step;
    }
    return list;
}

static void vantyx_list_pop_raw(VantyxList* list, void* out, int line, int column) {
    if (list->len == 0) vantyx_rt_error("R0003", "`pop` on an empty list", line, column);
    list->len--;
    memcpy(out, (char*)list->items + list->len * list->elem_size, (size_t)list->elem_size);
}

/* Stable scalar-list sorting for the `sorted` builtin. The interpreter uses a
   stable sort, so ties (-0.0/0.0 and NaN) keep their input order. A merge sort
   also keeps the result deterministic across C libraries, unlike qsort. */
typedef int (*VantyxSortCmp)(const void*, const void*);

static int vantyx_sort_cmp_int(const void* left, const void* right) {
    int64_t a = *(const int64_t*)left;
    int64_t b = *(const int64_t*)right;
    return (a > b) - (a < b);
}

static int vantyx_sort_cmp_float(const void* left, const void* right) {
    double a = *(const double*)left;
    double b = *(const double*)right;
    bool a_nan = isnan(a);
    bool b_nan = isnan(b);
    if (a_nan || b_nan) return (int)a_nan - (int)b_nan;
    return (a > b) - (a < b);
}

static int vantyx_sort_cmp_str(const void* left, const void* right) {
    return strcmp(*(vantyx_str const*)left, *(vantyx_str const*)right);
}

static int vantyx_sort_cmp_bool(const void* left, const void* right) {
    return (int)*(const bool*)left - (int)*(const bool*)right;
}

static void vantyx_merge_sort(void* base, size_t count, size_t size,
                              VantyxSortCmp compare, unsigned char* temp) {
    if (count < 2) return;
    unsigned char* bytes = (unsigned char*)base;
    size_t middle = count / 2;
    vantyx_merge_sort(bytes, middle, size, compare, temp);
    vantyx_merge_sort(bytes + middle * size, count - middle, size, compare, temp);
    size_t left = 0;
    size_t right = middle;
    size_t out = 0;
    while (left < middle && right < count) {
        const void* a = bytes + left * size;
        const void* b = bytes + right * size;
        if (compare(a, b) <= 0) {
            memcpy(temp + out * size, a, size);
            left++;
        } else {
            memcpy(temp + out * size, b, size);
            right++;
        }
        out++;
    }
    while (left < middle) {
        memcpy(temp + out * size, bytes + left * size, size);
        left++;
        out++;
    }
    while (right < count) {
        memcpy(temp + out * size, bytes + right * size, size);
        right++;
        out++;
    }
    memcpy(base, temp, count * size);
}

static VantyxList* vantyx_list_sorted(VantyxList* list, int64_t elem_size,
                                     const VantyxTypeDesc* elem_desc,
                                     VantyxSortCmp compare) {
    VantyxList* result = vantyx_list_new(elem_size, elem_desc);
    vantyx_list_reserve(result, list->len, 0, 0);
    if (list->len > 0) {
        memcpy(result->items, list->items, (size_t)(list->len * elem_size));
    }
    result->len = list->len;
    if (result->len > 1) {
        unsigned char* temp =
            (unsigned char*)vantyx_alloc_array((size_t)result->len, (size_t)elem_size);
        vantyx_merge_sort(result->items, (size_t)result->len, (size_t)elem_size, compare, temp);
        free(temp);
    }
    return result;
}

static VantyxList* vantyx_list_sorted_int(VantyxList* list) {
    return vantyx_list_sorted(list, sizeof(int64_t), &vantyx_desc_int, vantyx_sort_cmp_int);
}

static VantyxList* vantyx_list_sorted_float(VantyxList* list) {
    return vantyx_list_sorted(list, sizeof(double), &vantyx_desc_float, vantyx_sort_cmp_float);
}

static VantyxList* vantyx_list_sorted_str(VantyxList* list) {
    return vantyx_list_sorted(list, sizeof(vantyx_str), &vantyx_desc_str, vantyx_sort_cmp_str);
}

static VantyxList* vantyx_list_sorted_bool(VantyxList* list) {
    return vantyx_list_sorted(list, sizeof(bool), &vantyx_desc_bool, vantyx_sort_cmp_bool);
}

/* Text builtins (ADR-0028). Strings are UTF-8 and immutable; helpers may
   return a shared slice when no allocation is required. */
static uint32_t vantyx_utf8_decode(const unsigned char* text, int* bytes) {
    unsigned char lead = text[0];
    if (lead < 0x80) {
        *bytes = 1;
        return lead;
    }
    if ((lead & 0xE0) == 0xC0) {
        *bytes = 2;
        return ((uint32_t)(lead & 0x1F) << 6) | (uint32_t)(text[1] & 0x3F);
    }
    if ((lead & 0xF0) == 0xE0) {
        *bytes = 3;
        return ((uint32_t)(lead & 0x0F) << 12)
             | ((uint32_t)(text[1] & 0x3F) << 6)
             | (uint32_t)(text[2] & 0x3F);
    }
    *bytes = 4;
    return ((uint32_t)(lead & 0x07) << 18)
         | ((uint32_t)(text[1] & 0x3F) << 12)
         | ((uint32_t)(text[2] & 0x3F) << 6)
         | (uint32_t)(text[3] & 0x3F);
}

/* Encodes one Unicode scalar value into `out` (at least 4 bytes) and returns
   the byte length. Used for diagnostics that quote pattern characters. */
static size_t vantyx_utf8_encode(uint32_t code, char* out) {
    if (code < 0x80) {
        out[0] = (char)code;
        return 1;
    }
    if (code < 0x800) {
        out[0] = (char)(0xC0 | (code >> 6));
        out[1] = (char)(0x80 | (code & 0x3F));
        return 2;
    }
    if (code < 0x10000) {
        out[0] = (char)(0xE0 | (code >> 12));
        out[1] = (char)(0x80 | ((code >> 6) & 0x3F));
        out[2] = (char)(0x80 | (code & 0x3F));
        return 3;
    }
    out[0] = (char)(0xF0 | (code >> 18));
    out[1] = (char)(0x80 | ((code >> 12) & 0x3F));
    out[2] = (char)(0x80 | ((code >> 6) & 0x3F));
    out[3] = (char)(0x80 | (code & 0x3F));
    return 4;
}

static bool vantyx_text_is_whitespace(uint32_t code) {
    return (code >= 0x0009 && code <= 0x000D)
        || code == 0x0020
        || code == 0x0085
        || code == 0x00A0
        || code == 0x1680
        || (code >= 0x2000 && code <= 0x200A)
        || code == 0x2028
        || code == 0x2029
        || code == 0x202F
        || code == 0x205F
        || code == 0x3000;
}

static VantyxList* vantyx_text_split(vantyx_str text, vantyx_str separator,
                                     int line, int column) {
    VantyxList* parts = vantyx_list_new(sizeof(vantyx_str), &vantyx_desc_str);
    size_t separator_length = strlen(separator);
    if (separator_length == 0) {
        const char* cursor = text;
        while (*cursor != '\0') {
            int bytes = vantyx_utf8_char_len((const unsigned char)*cursor);
            vantyx_str part = vantyx_str_slice(cursor, bytes);
            vantyx_list_push_raw(parts, &part, line, column);
            cursor += bytes;
        }
        return parts;
    }
    const char* cursor = text;
    for (;;) {
        const char* match = strstr(cursor, separator);
        size_t length = match == NULL ? strlen(cursor) : (size_t)(match - cursor);
        vantyx_str part = vantyx_strndup(cursor, length);
        vantyx_list_push_raw(parts, &part, line, column);
        if (match == NULL) break;
        cursor = match + separator_length;
    }
    return parts;
}

static vantyx_str vantyx_text_join(VantyxList* parts, vantyx_str separator) {
    size_t separator_length = strlen(separator);
    size_t total = 0;
    for (size_t index = 0; index < parts->len; index++) {
        vantyx_str part = ((vantyx_str*)parts->items)[index];
        total = vantyx_checked_add(total, strlen(part), "string");
        if (index != 0) {
            total = vantyx_checked_add(total, separator_length, "string");
        }
    }
    char* result = (char*)vantyx_alloc_array(
        vantyx_checked_add(total, 1, "string"), 1);
    size_t out = 0;
    for (size_t index = 0; index < parts->len; index++) {
        vantyx_str part = ((vantyx_str*)parts->items)[index];
        size_t part_length = strlen(part);
        if (index != 0) {
            memcpy(result + out, separator, separator_length);
            out += separator_length;
        }
        memcpy(result + out, part, part_length);
        out += part_length;
    }
    result[out] = '\0';
    return result;
}

static vantyx_str vantyx_text_trim(vantyx_str text) {
    size_t length = strlen(text);
    size_t start = 0;
    while (start < length) {
        int bytes = 0;
        uint32_t code = vantyx_utf8_decode((const unsigned char*)text + start, &bytes);
        if (!vantyx_text_is_whitespace(code)) break;
        start += (size_t)bytes;
    }
    size_t end = start;
    size_t position = start;
    while (position < length) {
        int bytes = 0;
        uint32_t code = vantyx_utf8_decode((const unsigned char*)text + position, &bytes);
        if (!vantyx_text_is_whitespace(code)) {
            end = position + (size_t)bytes;
        }
        position += (size_t)bytes;
    }
    if (start == 0 && end == length) return text;
    return vantyx_strndup(text + start, end - start);
}

static bool vantyx_text_starts_with(vantyx_str text, vantyx_str prefix) {
    return strncmp(text, prefix, strlen(prefix)) == 0;
}

static bool vantyx_text_ends_with(vantyx_str text, vantyx_str suffix) {
    size_t text_length = strlen(text);
    size_t suffix_length = strlen(suffix);
    if (suffix_length > text_length) return false;
    return memcmp(text + text_length - suffix_length, suffix, suffix_length) == 0;
}

static vantyx_str vantyx_text_replace(vantyx_str text, vantyx_str pattern,
                                      vantyx_str replacement,
                                      int line, int column) {
    size_t pattern_length = strlen(pattern);
    if (pattern_length == 0) {
        vantyx_rt_error("R0016", "`replace` requires a non-empty pattern",
                        line, column);
    }
    size_t replacement_length = strlen(replacement);
    size_t total = 0;
    const char* cursor = text;
    for (;;) {
        const char* match = strstr(cursor, pattern);
        size_t segment = match == NULL ? strlen(cursor) : (size_t)(match - cursor);
        total = vantyx_checked_add(total, segment, "string");
        if (match == NULL) break;
        total = vantyx_checked_add(total, replacement_length, "string");
        cursor = match + pattern_length;
    }
    char* result = (char*)vantyx_alloc_array(
        vantyx_checked_add(total, 1, "string"), 1);
    size_t out = 0;
    cursor = text;
    for (;;) {
        const char* match = strstr(cursor, pattern);
        size_t segment = match == NULL ? strlen(cursor) : (size_t)(match - cursor);
        memcpy(result + out, cursor, segment);
        out += segment;
        if (match == NULL) break;
        memcpy(result + out, replacement, replacement_length);
        out += replacement_length;
        cursor = match + pattern_length;
    }
    result[out] = '\0';
    return result;
}

/* Lexical path helpers (ADR-0030). `/` is the only separator, paths are not
   normalized, `.` and `..` are preserved, and no filesystem access occurs. */
static void vantyx_path_name_range(vantyx_str path, size_t* start, size_t* length) {
    size_t end = strlen(path);
    while (end > 0 && path[end - 1] == '/') end--;
    size_t begin = end;
    while (begin > 0 && path[begin - 1] != '/') begin--;
    *start = begin;
    *length = end - begin;
}

static vantyx_str vantyx_path_join(vantyx_str base, vantyx_str child) {
    if (child[0] == '/') return vantyx_strdup(child);
    if (child[0] == '\0') return vantyx_strdup(base);
    if (base[0] == '\0') return vantyx_strdup(child);
    size_t base_length = strlen(base);
    if (base[base_length - 1] == '/') {
        return vantyx_str_concat(base, child);
    }
    return vantyx_str_concat(vantyx_str_concat(base, "/"), child);
}

static vantyx_str vantyx_path_parent(vantyx_str path) {
    size_t end = strlen(path);
    while (end > 0 && path[end - 1] == '/') end--;
    if (end == 0) {
        return vantyx_strdup(path[0] == '/' ? "/" : ".");
    }
    size_t slash = end;
    while (slash > 0 && path[slash - 1] != '/') slash--;
    size_t parent_end = slash;
    while (parent_end > 0 && path[parent_end - 1] == '/') parent_end--;
    if (parent_end == 0) {
        return vantyx_strdup(path[0] == '/' ? "/" : ".");
    }
    return vantyx_strndup(path, parent_end);
}

static vantyx_str vantyx_path_name(vantyx_str path) {
    size_t start = 0;
    size_t length = 0;
    vantyx_path_name_range(path, &start, &length);
    return vantyx_strndup(path + start, length);
}

/* Match the interpreter exactly: the final dot in the component is the
   extension separator, but a dot or pair of dots at the very end (and a
   leading dot) is part of the stem, not an extension. The interpreter finds
   the last dot first and then rejects it when it is trailing, so an earlier
   dot in `a..` must not be treated as a separator either. */
static size_t vantyx_path_extension_dot(vantyx_str path, size_t start, size_t length) {
    size_t dot = SIZE_MAX;
    for (size_t index = 1; index < length; index++) {
        if (path[start + index] == '.') dot = index;
    }
    if (dot == SIZE_MAX || dot + 1 >= length) return SIZE_MAX;
    return dot;
}

static vantyx_str vantyx_path_stem(vantyx_str path) {
    size_t start = 0;
    size_t length = 0;
    vantyx_path_name_range(path, &start, &length);
    size_t dot = vantyx_path_extension_dot(path, start, length);
    if (dot == SIZE_MAX) return vantyx_strndup(path + start, length);
    return vantyx_strndup(path + start, dot);
}

static vantyx_str vantyx_path_extension(vantyx_str path) {
    size_t start = 0;
    size_t length = 0;
    vantyx_path_name_range(path, &start, &length);
    size_t dot = vantyx_path_extension_dot(path, start, length);
    if (dot == SIZE_MAX) return vantyx_strdup("");
    return vantyx_strndup(path + start + dot + 1, length - dot - 1);
}

static bool vantyx_path_is_absolute(vantyx_str path) {
    return path[0] == '/';
}

/* Directory operations (ADR-0027). Entries are sorted by byte order and names
   must be valid UTF-8. Errors are fatal R0015 until a Result type exists. */
static int vantyx_name_compare(const void* left, const void* right) {
    const char* a = *(const char* const*)left;
    const char* b = *(const char* const*)right;
    return strcmp(a, b);
}

static VantyxList* vantyx_list_dir(vantyx_str path, int line, int column) {
    DIR* directory = opendir(path);
    if (directory == NULL) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot list directory `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t length = 0;
    size_t capacity = 16;
    vantyx_str* names =
        (vantyx_str*)vantyx_alloc_array(capacity, sizeof(vantyx_str));
    for (;;) {
        errno = 0;
        struct dirent* entry = readdir(directory);
        if (entry == NULL) {
            if (errno != 0) {
                int error_number = errno;
                (void)closedir(directory);
                char message[512];
                snprintf(message, sizeof message,
                         "cannot list directory `%.200s`: %.200s",
                         path, vantyx_os_error(error_number));
                vantyx_rt_error("R0015", message, line, column);
            }
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        if (!vantyx_utf8_valid((const unsigned char*)entry->d_name,
                               strlen(entry->d_name))) {
            (void)closedir(directory);
            char message[512];
            snprintf(message, sizeof message,
                     "directory `%.200s` contains a name that is not valid UTF-8",
                     path);
            vantyx_rt_error("R0015", message, line, column);
        }
        if (length == capacity) {
            if (capacity > SIZE_MAX / 2 / sizeof(vantyx_str)) {
                (void)closedir(directory);
                fputs("error: directory entry count is too large\n", stderr);
                exit(2);
            }
            capacity *= 2;
            vantyx_str* grown =
                (vantyx_str*)realloc(names, capacity * sizeof(vantyx_str));
            if (grown == NULL) {
                (void)closedir(directory);
                fputs("error: out of memory\n", stderr);
                exit(2);
            }
            names = grown;
        }
        names[length++] = vantyx_strdup(entry->d_name);
    }
    (void)closedir(directory);
    qsort(names, length, sizeof(vantyx_str), vantyx_name_compare);
    VantyxList* list = vantyx_list_new(sizeof(vantyx_str), &vantyx_desc_str);
    for (size_t index = 0; index < length; index++) {
        vantyx_list_push_raw(list, &names[index], line, column);
    }
    free(names);
    return list;
}

/* Glob expansion (ADR-0033). Patterns are split on '/'; '*' and '?' match
   Unicode scalar values within one component, classes use [a-z]/[!a-z], and a
   complete '**' component recurses through real directories without following
   symbolic links. Results are byte-sorted and deduplicated. Malformed
   patterns are R0016; filesystem failures and caps are R0015. */
#define VANTYX_GLOB_MAX_RESULTS ((size_t)100000)
#define VANTYX_GLOB_MAX_DEPTH ((size_t)128)
#define VANTYX_GLOB_MAX_COMPONENTS ((size_t)1024)

typedef enum {
    VANTYX_GLOB_LITERAL,
    VANTYX_GLOB_STAR,
    VANTYX_GLOB_QUESTION,
    VANTYX_GLOB_CLASS
} VantyxGlobTokenKind;

typedef struct {
    uint32_t first;
    uint32_t last;
} VantyxGlobRange;

typedef struct {
    VantyxGlobTokenKind kind;
    uint32_t literal;
    bool negated;
    VantyxGlobRange* ranges;
    size_t range_count;
} VantyxGlobToken;

typedef struct {
    vantyx_str text;
    VantyxGlobToken* tokens;
    size_t token_count;
    bool has_meta;
    bool allows_hidden;
    bool double_star;
} VantyxGlobComponent;

typedef struct {
    VantyxGlobComponent* components;
    size_t component_count;
    bool absolute;
} VantyxGlobPattern;

typedef struct {
    char** names;
    size_t length;
    size_t capacity;
} VantyxGlobNames;

typedef struct {
    vantyx_str* items;
    size_t length;
    size_t capacity;
} VantyxGlobResults;

static void* vantyx_glob_grow(void* pointer, size_t* capacity, size_t element_size) {
    if (*capacity > SIZE_MAX / 2) {
        fputs("error: glob data is too large\n", stderr);
        exit(2);
    }
    size_t next = *capacity == 0 ? 8 : *capacity * 2;
    if (element_size != 0 && next > SIZE_MAX / element_size) {
        fputs("error: glob data is too large\n", stderr);
        exit(2);
    }
    void* grown = realloc(pointer, next * element_size);
    if (grown == NULL) {
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    *capacity = next;
    return grown;
}

static void vantyx_glob_pattern_free(VantyxGlobPattern* pattern) {
    for (size_t index = 0; index < pattern->component_count; index++) {
        VantyxGlobComponent* component = &pattern->components[index];
        for (size_t token = 0; token < component->token_count; token++) {
            free(component->tokens[token].ranges);
        }
        free(component->tokens);
        free((void*)component->text);
    }
    free(pattern->components);
    pattern->components = NULL;
    pattern->component_count = 0;
}

static VantyxGlobComponent vantyx_glob_component(const char* text, size_t length,
                                                 int line, int column) {
    VantyxGlobComponent component;
    memset(&component, 0, sizeof component);
    component.text = vantyx_strndup(text, length);
    size_t token_capacity = 0;
    size_t index = 0;
    while (index < length) {
        VantyxGlobToken token;
        memset(&token, 0, sizeof token);
        unsigned char lead = (unsigned char)text[index];
        if (lead == '*') {
            token.kind = VANTYX_GLOB_STAR;
            index++;
        } else if (lead == '?') {
            token.kind = VANTYX_GLOB_QUESTION;
            index++;
        } else if (lead == '[') {
            token.kind = VANTYX_GLOB_CLASS;
            index++;
            if (index < length && (text[index] == '!' || text[index] == '^')) {
                token.negated = true;
                index++;
            }
            size_t range_capacity = 0;
            if (index < length && text[index] == ']') {
                token.ranges = (VantyxGlobRange*)vantyx_glob_grow(
                    token.ranges, &range_capacity, sizeof(VantyxGlobRange));
                token.ranges[token.range_count].first = ']';
                token.ranges[token.range_count].last = ']';
                token.range_count++;
                index++;
            }
            bool closed = false;
            while (index < length) {
                if (text[index] == ']') {
                    index++;
                    closed = true;
                    break;
                }
                int first_bytes = 0;
                uint32_t first = vantyx_utf8_decode(
                    (const unsigned char*)text + index, &first_bytes);
                index += (size_t)first_bytes;
                if (index + 1 < length && text[index] == '-' && text[index + 1] != ']') {
                    int last_bytes = 0;
                    uint32_t last = vantyx_utf8_decode(
                        (const unsigned char*)text + index + 1, &last_bytes);
                    if (last < first) {
                        char first_text[5] = {0};
                        char last_text[5] = {0};
                        vantyx_utf8_encode(first, first_text);
                        vantyx_utf8_encode(last, last_text);
                        char message[256];
                        snprintf(message, sizeof message,
                                 "invalid glob pattern: descending character range `%s-%s`",
                                 first_text, last_text);
                        vantyx_rt_error("R0016", message, line, column);
                    }
                    if (token.range_count == range_capacity) {
                        token.ranges = (VantyxGlobRange*)vantyx_glob_grow(
                            token.ranges, &range_capacity, sizeof(VantyxGlobRange));
                    }
                    token.ranges[token.range_count].first = first;
                    token.ranges[token.range_count].last = last;
                    token.range_count++;
                    index += 1 + (size_t)last_bytes;
                } else {
                    if (token.range_count == range_capacity) {
                        token.ranges = (VantyxGlobRange*)vantyx_glob_grow(
                            token.ranges, &range_capacity, sizeof(VantyxGlobRange));
                    }
                    token.ranges[token.range_count].first = first;
                    token.ranges[token.range_count].last = first;
                    token.range_count++;
                }
            }
            if (!closed) {
                vantyx_rt_error("R0016",
                                "invalid glob pattern: unclosed character class",
                                line, column);
            }
            if (token.range_count == 0) {
                vantyx_rt_error("R0016",
                                "invalid glob pattern: empty character class",
                                line, column);
            }
        } else {
            int bytes = 0;
            token.kind = VANTYX_GLOB_LITERAL;
            token.literal = vantyx_utf8_decode(
                (const unsigned char*)text + index, &bytes);
            index += (size_t)bytes;
        }
        if (component.token_count == token_capacity) {
            component.tokens = (VantyxGlobToken*)vantyx_glob_grow(
                component.tokens, &token_capacity, sizeof(VantyxGlobToken));
        }
        component.tokens[component.token_count++] = token;
    }
    for (size_t token = 0; token < component.token_count; token++) {
        if (component.tokens[token].kind != VANTYX_GLOB_LITERAL) {
            component.has_meta = true;
        }
    }
    if (component.token_count > 0) {
        VantyxGlobToken* first = &component.tokens[0];
        if (first->kind == VANTYX_GLOB_LITERAL && first->literal == '.') {
            component.allows_hidden = true;
        } else if (first->kind == VANTYX_GLOB_CLASS && !first->negated) {
            for (size_t range = 0; range < first->range_count; range++) {
                if (first->ranges[range].first <= '.' &&
                    '.' <= first->ranges[range].last) {
                    component.allows_hidden = true;
                    break;
                }
            }
        }
    }
    if (component.token_count == 2 &&
        component.tokens[0].kind == VANTYX_GLOB_STAR &&
        component.tokens[1].kind == VANTYX_GLOB_STAR) {
        component.double_star = true;
    }
    return component;
}

static VantyxGlobPattern vantyx_glob_parse(vantyx_str pattern, int line, int column) {
    VantyxGlobPattern parsed;
    memset(&parsed, 0, sizeof parsed);
    if (pattern[0] == '\0') {
        vantyx_rt_error("R0016", "glob pattern is empty", line, column);
    }
    parsed.absolute = pattern[0] == '/';
    size_t component_capacity = 0;
    const char* cursor = pattern;
    while (*cursor != '\0') {
        while (*cursor == '/') {
            cursor++;
        }
        if (*cursor == '\0') {
            break;
        }
        const char* start = cursor;
        while (*cursor != '\0' && *cursor != '/') {
            cursor++;
        }
        size_t length = (size_t)(cursor - start);
        if (parsed.component_count >= VANTYX_GLOB_MAX_COMPONENTS) {
            vantyx_rt_error("R0016", "glob pattern has more than 1024 components",
                            line, column);
        }
        if (parsed.component_count == component_capacity) {
            parsed.components = (VantyxGlobComponent*)vantyx_glob_grow(
                parsed.components, &component_capacity, sizeof(VantyxGlobComponent));
        }
        parsed.components[parsed.component_count++] =
            vantyx_glob_component(start, length, line, column);
    }
    return parsed;
}

static vantyx_str vantyx_glob_join(vantyx_str base, const char* name) {
    size_t base_length = strlen(base);
    size_t name_length = strlen(name);
    if (base_length == 0) {
        return vantyx_strdup(name);
    }
    bool root = base_length == 1 && base[0] == '/';
    size_t separator = root ? 0 : 1;
    size_t total = vantyx_checked_add(base_length, separator, "glob path");
    total = vantyx_checked_add(total, name_length, "glob path");
    total = vantyx_checked_add(total, 1, "glob path");
    char* path = (char*)vantyx_alloc_array(total, 1);
    memcpy(path, base, base_length);
    size_t at = base_length;
    if (separator == 1) {
        path[at++] = '/';
    }
    memcpy(path + at, name, name_length + 1);
    return path;
}

static void vantyx_glob_names_free(VantyxGlobNames* names) {
    for (size_t index = 0; index < names->length; index++) {
        free(names->names[index]);
    }
    free(names->names);
    names->names = NULL;
    names->length = 0;
    names->capacity = 0;
}

static bool vantyx_glob_read_names(vantyx_str path, VantyxGlobNames* names,
                                   int line, int column) {
    const char* target = path[0] == '\0' ? "." : path;
    DIR* directory = opendir(target);
    if (directory == NULL) {
        int error_number = errno;
        if (error_number == ENOENT || error_number == ENOTDIR) {
            return false;
        }
        char message[512];
        snprintf(message, sizeof message,
                 "cannot list directory `%.200s`: %.200s", target,
                 vantyx_os_error(error_number));
        vantyx_rt_error("R0015", message, line, column);
    }
    for (;;) {
        errno = 0;
        struct dirent* entry = readdir(directory);
        if (entry == NULL) {
            if (errno != 0) {
                int error_number = errno;
                (void)closedir(directory);
                vantyx_glob_names_free(names);
                char message[512];
                snprintf(message, sizeof message,
                         "cannot list directory `%.200s`: %.200s", target,
                         vantyx_os_error(error_number));
                vantyx_rt_error("R0015", message, line, column);
            }
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        if (!vantyx_utf8_valid((const unsigned char*)entry->d_name,
                               strlen(entry->d_name))) {
            (void)closedir(directory);
            vantyx_glob_names_free(names);
            char message[512];
            snprintf(message, sizeof message,
                     "directory `%.200s` contains a non-UTF-8 entry name", target);
            vantyx_rt_error("R0015", message, line, column);
        }
        if (names->length == names->capacity) {
            names->names = (char**)vantyx_glob_grow(
                names->names, &names->capacity, sizeof(char*));
        }
        names->names[names->length++] = (char*)vantyx_strdup(entry->d_name);
    }
    (void)closedir(directory);
    return true;
}

static void vantyx_glob_push(VantyxGlobResults* results, vantyx_str path,
                             int line, int column) {
    if (results->length >= VANTYX_GLOB_MAX_RESULTS) {
        char message[128];
        snprintf(message, sizeof message,
                 "glob produced more than %zu candidate paths",
                 VANTYX_GLOB_MAX_RESULTS);
        vantyx_rt_error("R0015", message, line, column);
    }
    if (results->length == results->capacity) {
        results->items = (vantyx_str*)vantyx_glob_grow(
            results->items, &results->capacity, sizeof(vantyx_str));
    }
    results->items[results->length++] = path;
}

static bool vantyx_glob_class_matches(const VantyxGlobToken* token, uint32_t code) {
    bool hit = false;
    for (size_t index = 0; index < token->range_count; index++) {
        if (token->ranges[index].first <= code &&
            code <= token->ranges[index].last) {
            hit = true;
            break;
        }
    }
    return token->negated ? !hit : hit;
}

static bool vantyx_glob_component_matches(const VantyxGlobComponent* component,
                                          const char* name) {
    size_t byte_length = strlen(name);
    uint32_t* codes = (uint32_t*)vantyx_alloc_array(
        byte_length + 1, sizeof(uint32_t));
    size_t code_count = 0;
    size_t byte = 0;
    while (byte < byte_length) {
        int bytes = 0;
        codes[code_count++] = vantyx_utf8_decode(
            (const unsigned char*)name + byte, &bytes);
        byte += (size_t)bytes;
    }
    size_t token_index = 0;
    size_t name_index = 0;
    size_t star_token = SIZE_MAX;
    size_t star_name = 0;
    while (name_index < code_count) {
        bool advanced = false;
        if (token_index < component->token_count) {
            const VantyxGlobToken* token = &component->tokens[token_index];
            if (token->kind == VANTYX_GLOB_STAR) {
                token_index++;
                star_token = token_index;
                star_name = name_index;
                advanced = true;
            } else if (token->kind == VANTYX_GLOB_QUESTION) {
                token_index++;
                name_index++;
                advanced = true;
            } else if (token->kind == VANTYX_GLOB_CLASS &&
                       vantyx_glob_class_matches(token, codes[name_index])) {
                token_index++;
                name_index++;
                advanced = true;
            } else if (token->kind == VANTYX_GLOB_LITERAL &&
                       token->literal == codes[name_index]) {
                token_index++;
                name_index++;
                advanced = true;
            }
        }
        if (advanced) {
            continue;
        }
        if (star_token == SIZE_MAX) {
            free(codes);
            return false;
        }
        token_index = star_token;
        star_name++;
        name_index = star_name;
    }
    while (token_index < component->token_count &&
           component->tokens[token_index].kind == VANTYX_GLOB_STAR) {
        token_index++;
    }
    bool matched = token_index == component->token_count;
    free(codes);
    return matched;
}

static void vantyx_glob_depth_error(int line, int column) {
    char message[128];
    snprintf(message, sizeof message,
             "glob exceeded maximum depth of %zu", VANTYX_GLOB_MAX_DEPTH);
    vantyx_rt_error("R0015", message, line, column);
}

static void vantyx_glob_collect_descendants(vantyx_str base, size_t depth,
                                            VantyxGlobResults* results,
                                            int line, int column) {
    if (depth > VANTYX_GLOB_MAX_DEPTH) {
        vantyx_glob_depth_error(line, column);
    }
    VantyxGlobNames names;
    memset(&names, 0, sizeof names);
    if (!vantyx_glob_read_names(base, &names, line, column)) {
        return;
    }
    for (size_t index = 0; index < names.length; index++) {
        const char* name = names.names[index];
        if (name[0] == '.') {
            continue;
        }
        vantyx_str child = vantyx_glob_join(base, name);
        vantyx_glob_push(results, child, line, column);
        struct stat info;
        if (lstat(child, &info) != 0) {
            int error_number = errno;
            if (error_number == ENOENT || error_number == ENOTDIR) {
                continue;
            }
            char message[512];
            snprintf(message, sizeof message,
                     "cannot inspect `%.200s`: %.200s", child,
                     vantyx_os_error(error_number));
            vantyx_glob_names_free(&names);
            vantyx_rt_error("R0015", message, line, column);
        }
        if (S_ISDIR(info.st_mode)) {
            vantyx_glob_collect_descendants(child, depth + 1, results, line, column);
        }
    }
    vantyx_glob_names_free(&names);
}

static void vantyx_glob_walk(const VantyxGlobPattern* pattern, size_t index,
                             vantyx_str base, size_t depth,
                             VantyxGlobResults* results, int line, int column) {
    if (index == pattern->component_count) {
        if (base[0] != '\0') {
            vantyx_glob_push(results, vantyx_strdup(base), line, column);
        }
        return;
    }
    const VantyxGlobComponent* component = &pattern->components[index];
    bool last = index + 1 == pattern->component_count;
    if (component->double_star) {
        if (last) {
            vantyx_glob_collect_descendants(base, depth, results, line, column);
            return;
        }
        vantyx_glob_walk(pattern, index + 1, base, depth, results, line, column);
        VantyxGlobNames names;
        memset(&names, 0, sizeof names);
        if (!vantyx_glob_read_names(base, &names, line, column)) {
            return;
        }
        for (size_t name_index = 0; name_index < names.length; name_index++) {
            const char* name = names.names[name_index];
            if (name[0] == '.') {
                continue;
            }
            vantyx_str child = vantyx_glob_join(base, name);
            struct stat info;
            if (lstat(child, &info) != 0) {
                int error_number = errno;
                if (error_number == ENOENT || error_number == ENOTDIR) {
                    free((void*)child);
                    continue;
                }
                char message[512];
                snprintf(message, sizeof message,
                         "cannot inspect `%.200s`: %.200s", child,
                         vantyx_os_error(error_number));
                vantyx_glob_names_free(&names);
                free((void*)child);
                vantyx_rt_error("R0015", message, line, column);
            }
            if (S_ISDIR(info.st_mode)) {
                if (depth + 1 > VANTYX_GLOB_MAX_DEPTH) {
                    vantyx_glob_names_free(&names);
                    free((void*)child);
                    vantyx_glob_depth_error(line, column);
                }
                vantyx_glob_walk(pattern, index, child, depth + 1, results,
                                 line, column);
            }
            free((void*)child);
        }
        vantyx_glob_names_free(&names);
        return;
    }
    if (component->has_meta) {
        VantyxGlobNames names;
        memset(&names, 0, sizeof names);
        if (!vantyx_glob_read_names(base, &names, line, column)) {
            return;
        }
        for (size_t name_index = 0; name_index < names.length; name_index++) {
            const char* name = names.names[name_index];
            if (name[0] == '.' && !component->allows_hidden) {
                continue;
            }
            if (!vantyx_glob_component_matches(component, name)) {
                continue;
            }
            vantyx_str child = vantyx_glob_join(base, name);
            if (last) {
                vantyx_glob_push(results, child, line, column);
            } else {
                vantyx_glob_walk(pattern, index + 1, child, depth, results,
                                 line, column);
                free((void*)child);
            }
        }
        vantyx_glob_names_free(&names);
        return;
    }
    vantyx_str child = vantyx_glob_join(base, component->text);
    if (last) {
        struct stat info;
        if (lstat(child, &info) == 0) {
            vantyx_glob_push(results, child, line, column);
        } else {
            free((void*)child);
        }
    } else {
        vantyx_glob_walk(pattern, index + 1, child, depth, results, line, column);
        free((void*)child);
    }
}

static VantyxList* vantyx_glob(vantyx_str pattern, int line, int column) {
    VantyxGlobPattern parsed = vantyx_glob_parse(pattern, line, column);
    VantyxGlobResults results;
    memset(&results, 0, sizeof results);
    if (parsed.component_count == 0) {
        if (parsed.absolute) {
            struct stat info;
            if (lstat("/", &info) == 0) {
                vantyx_glob_push(&results, vantyx_strdup("/"), line, column);
            }
        }
    } else {
        vantyx_str base = parsed.absolute ? vantyx_strdup("/") : vantyx_strdup("");
        vantyx_glob_walk(&parsed, 0, base, 0, &results, line, column);
        free((void*)base);
    }
    if (results.length > 1) {
        qsort(results.items, results.length, sizeof(vantyx_str),
              vantyx_name_compare);
    }
    size_t unique = 0;
    for (size_t index = 0; index < results.length; index++) {
        if (unique == 0 ||
            strcmp(results.items[unique - 1], results.items[index]) != 0) {
            results.items[unique++] = results.items[index];
        } else {
            free((void*)results.items[index]);
        }
    }
    VantyxList* list = vantyx_list_new(sizeof(vantyx_str), &vantyx_desc_str);
    for (size_t index = 0; index < unique; index++) {
        vantyx_list_push_raw(list, &results.items[index], line, column);
    }
    free(results.items);
    vantyx_glob_pattern_free(&parsed);
    return list;
}

static vantyx_unit vantyx_make_dir(vantyx_str path, int line, int column) {
    if (mkdir(path, 0777) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot create directory `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    return (vantyx_unit)0;
}

static vantyx_unit vantyx_remove_file(vantyx_str path, int line, int column) {
    if (unlink(path) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot remove file `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    return (vantyx_unit)0;
}

static vantyx_unit vantyx_remove_dir(vantyx_str path, int line, int column) {
    if (rmdir(path) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot remove directory `%.200s`: %.200s", path, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    return (vantyx_unit)0;
}

static bool vantyx_is_directory(const char* path) {
    struct stat info;
    return stat(path, &info) == 0 && S_ISDIR(info.st_mode);
}

static _Noreturn void vantyx_directory_error(
    const char* action, const char* path, int error_number, int line, int column) {
    char message[512];
    snprintf(message, sizeof message,
             "cannot %s `%.200s`: %.200s", action, path, vantyx_os_error(error_number));
    vantyx_rt_error("R0015", message, line, column);
}

/* Creates every missing component. An empty path is a no-op, matching
   `std::fs::create_dir_all("")`. An existing directory or symlink to one is
   accepted; a non-directory in the way is an error. */
static vantyx_unit vantyx_make_dir_all(vantyx_str path, int line, int column) {
    size_t length = strlen(path);
    if (length == 0) {
        return (vantyx_unit)0;
    }
    char* buffer = (char*)vantyx_alloc(length + 1);
    memcpy(buffer, path, length + 1);
    if (mkdir(buffer, 0777) == 0) {
        free(buffer);
        return (vantyx_unit)0;
    }
    int first_error = errno;
    if (first_error != ENOENT) {
        if (first_error == EEXIST && vantyx_is_directory(buffer)) {
            free(buffer);
            return (vantyx_unit)0;
        }
        free(buffer);
        vantyx_directory_error("create directory", path, first_error, line, column);
    }
    for (size_t index = 1; index < length; index++) {
        if (buffer[index] != '/') continue;
        char saved = buffer[index];
        buffer[index] = '\0';
        if (buffer[0] != '\0') {
            if (mkdir(buffer, 0777) != 0 && errno != EEXIST) {
                int error_number = errno;
                free(buffer);
                vantyx_directory_error(
                    "create directory", path, error_number, line, column);
            }
            if (!vantyx_is_directory(buffer)) {
                free(buffer);
                vantyx_directory_error("create directory", path, ENOTDIR, line, column);
            }
        }
        buffer[index] = saved;
    }
    if (mkdir(buffer, 0777) != 0 && errno != EEXIST) {
        int error_number = errno;
        free(buffer);
        vantyx_directory_error("create directory", path, error_number, line, column);
    }
    if (!vantyx_is_directory(buffer)) {
        free(buffer);
        vantyx_directory_error("create directory", path, EEXIST, line, column);
    }
    free(buffer);
    return (vantyx_unit)0;
}

/* Removes a tree without traversing symbolic links. The displayed path is the
   caller's original root so messages stay stable across recursion. */
static void vantyx_remove_tree(
    const char* path, const char* display, int line, int column) {
    struct stat info;
    if (lstat(path, &info) != 0) {
        vantyx_directory_error("remove directory", display, errno, line, column);
    }
    if (S_ISLNK(info.st_mode)) {
        if (unlink(path) != 0) {
            vantyx_directory_error("remove directory", display, errno, line, column);
        }
        return;
    }
    if (!S_ISDIR(info.st_mode)) {
        vantyx_directory_error("remove directory", display, ENOTDIR, line, column);
    }
    DIR* directory = opendir(path);
    if (directory == NULL) {
        vantyx_directory_error("remove directory", display, errno, line, column);
    }
    for (;;) {
        errno = 0;
        struct dirent* entry = readdir(directory);
        if (entry == NULL) {
            if (errno != 0) {
                int error_number = errno;
                (void)closedir(directory);
                vantyx_directory_error(
                    "remove directory", display, error_number, line, column);
            }
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        size_t base = strlen(path);
        size_t name = strlen(entry->d_name);
        char* child = (char*)vantyx_alloc(base + name + 2);
        memcpy(child, path, base);
        child[base] = '/';
        memcpy(child + base + 1, entry->d_name, name + 1);
        struct stat child_info;
        if (lstat(child, &child_info) != 0) {
            int error_number = errno;
            free(child);
            (void)closedir(directory);
            vantyx_directory_error(
                "remove directory", display, error_number, line, column);
        }
        if (S_ISDIR(child_info.st_mode) && !S_ISLNK(child_info.st_mode)) {
            vantyx_remove_tree(child, display, line, column);
        } else if (unlink(child) != 0) {
            int error_number = errno;
            free(child);
            (void)closedir(directory);
            vantyx_directory_error(
                "remove directory", display, error_number, line, column);
        }
        free(child);
    }
    if (closedir(directory) != 0) {
        vantyx_directory_error("remove directory", display, errno, line, column);
    }
    if (rmdir(path) != 0) {
        vantyx_directory_error("remove directory", display, errno, line, column);
    }
}

/* Returns 1 for a final `.`/`..` component, 2 for the filesystem root, and 0
   when the root is acceptable. Kept in sync with `src/fs_safety.rs`. */
static int vantyx_remove_root_refusal(const char* path, size_t length) {
    if (length == 0) return 0;
    size_t trimmed = length;
    while (trimmed > 0 && path[trimmed - 1] == '/') {
        trimmed--;
    }
    if (trimmed == 0) return 2;
    size_t start = trimmed;
    while (start > 0 && path[start - 1] != '/') {
        start--;
    }
    size_t component = trimmed - start;
    if (component == 1 && path[start] == '.') return 1;
    if (component == 2 && path[start] == '.' && path[start + 1] == '.') return 1;
    return 0;
}

/* Trailing separators are trimmed before the root type check so that a
   symlink named with a trailing slash is unlinked instead of followed.
   Destructive roots are refused before any filesystem access. */
static vantyx_unit vantyx_remove_dir_all(vantyx_str path, int line, int column) {
    size_t length = strlen(path);
    int refusal = vantyx_remove_root_refusal(path, length);
    if (refusal != 0) {
        const char* reason = refusal == 1
            ? "refusing to remove a directory whose final component is `.` or `..`"
            : "refusing to remove the filesystem root";
        char message[512];
        snprintf(message, sizeof message,
                 "cannot remove directory `%.200s`: %.200s", path, reason);
        vantyx_rt_error("R0015", message, line, column);
    }
    while (length > 1 && path[length - 1] == '/') {
        length--;
    }
    char* target = (char*)vantyx_alloc(length + 1);
    memcpy(target, path, length);
    target[length] = '\0';
    vantyx_remove_tree(target, path, line, column);
    free(target);
    return (vantyx_unit)0;
}

static vantyx_unit vantyx_rename(vantyx_str from, vantyx_str to, int line, int column) {
    if (rename(from, to) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot rename `%.200s` to `%.200s`: %.200s",
                 from, to, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    return (vantyx_unit)0;
}

/* Copy bytes only: permissions, timestamps, links, and other metadata are not
   preserved. The contents are not validated as UTF-8, so this can copy files
   that `read_file` would reject. */
static vantyx_unit vantyx_copy_file(vantyx_str from, vantyx_str to, int line, int column) {
    struct stat info;
    if (stat(from, &info) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy file `%.200s`: %.200s", from, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    if (!S_ISREG(info.st_mode)) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy file `%.200s`: not a regular file", from);
        vantyx_rt_error("R0015", message, line, column);
    }
    /* Opening the target for writing truncates it before any bytes are read.
       Refuse a target that already resolves to the source, including a hard
       link or symbolic-link alias. This is a best-effort guard against an
       accidental self-copy, not an atomic transaction: a caller can still race
       the check. Metadata failures are left to fopen to report. */
    struct stat target_info;
    if (stat(to, &target_info) == 0 &&
        target_info.st_dev == info.st_dev &&
        target_info.st_ino == info.st_ino) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy file `%.200s` to `%.200s`: source and target are the same file",
                 from, to);
        vantyx_rt_error("R0015", message, line, column);
    }
    FILE* source = fopen(from, "rb");
    if (source == NULL) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy file `%.200s`: %.200s", from, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    FILE* target = fopen(to, "wb");
    if (target == NULL) {
        int error_number = errno;
        (void)fclose(source);
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy file `%.200s` to `%.200s`: %.200s",
                 from, to, vantyx_os_error(error_number));
        vantyx_rt_error("R0015", message, line, column);
    }
    char buffer[65536];
    int error_number = 0;
    for (;;) {
        size_t count = fread(buffer, 1, sizeof buffer, source);
        if (count == 0) {
            if (ferror(source)) error_number = errno;
            break;
        }
        if (fwrite(buffer, 1, count, target) != count) {
            error_number = errno;
            break;
        }
        if (count < sizeof buffer) {
            if (ferror(source)) error_number = errno;
            break;
        }
    }
    if (fflush(target) != 0 && error_number == 0) error_number = errno;
    if (fclose(source) != 0 && error_number == 0) error_number = errno;
    if (fclose(target) != 0 && error_number == 0) error_number = errno;
    if (error_number != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy file `%.200s` to `%.200s`: %.200s",
                 from, to, vantyx_os_error(error_number));
        vantyx_rt_error("R0015", message, line, column);
    }
    return (vantyx_unit)0;
}

/* Recursive directory copy (ADR-0034). The root is inspected with `stat`, so
   an explicit root link is followed; links below it are rejected. Metadata is
   not preserved, names are copied as raw bytes, and the target must be absent
   and outside the resolved source tree. */
#define VANTYX_COPY_MAX_ENTRIES ((size_t)100000)
#define VANTYX_COPY_MAX_DEPTH ((size_t)128)

static void vantyx_copy_tree(
    const char* source, const char* target, size_t depth,
    size_t* entries, int line, int column) {
    DIR* directory = opendir(source);
    if (directory == NULL) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot read directory `%.200s`: %.200s",
                 source, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    for (;;) {
        errno = 0;
        struct dirent* entry = readdir(directory);
        if (entry == NULL) {
            if (errno != 0) {
                int error_number = errno;
                (void)closedir(directory);
                char message[512];
                snprintf(message, sizeof message,
                         "cannot read directory `%.200s`: %.200s",
                         source, vantyx_os_error(error_number));
                vantyx_rt_error("R0015", message, line, column);
            }
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        *entries += 1;
        if (*entries > VANTYX_COPY_MAX_ENTRIES) {
            (void)closedir(directory);
            char message[512];
            snprintf(message, sizeof message,
                     "directory tree exceeds %zu entries",
                     VANTYX_COPY_MAX_ENTRIES);
            vantyx_rt_error("R0015", message, line, column);
        }
        size_t source_base = strlen(source);
        size_t target_base = strlen(target);
        size_t name_length = strlen(entry->d_name);
        char* child_source = (char*)vantyx_alloc(source_base + name_length + 2);
        char* child_target = (char*)vantyx_alloc(target_base + name_length + 2);
        memcpy(child_source, source, source_base);
        child_source[source_base] = '/';
        memcpy(child_source + source_base + 1, entry->d_name, name_length + 1);
        memcpy(child_target, target, target_base);
        child_target[target_base] = '/';
        memcpy(child_target + target_base + 1, entry->d_name, name_length + 1);
        struct stat info;
        if (lstat(child_source, &info) != 0) {
            int error_number = errno;
            char message[512];
            snprintf(message, sizeof message,
                     "cannot inspect `%.200s`: %.200s",
                     child_source, vantyx_os_error(error_number));
            free(child_source);
            free(child_target);
            (void)closedir(directory);
            vantyx_rt_error("R0015", message, line, column);
        }
        if (S_ISLNK(info.st_mode)) {
            char message[512];
            snprintf(message, sizeof message,
                     "symbolic link `%.200s` is not supported", child_source);
            free(child_source);
            free(child_target);
            (void)closedir(directory);
            vantyx_rt_error("R0015", message, line, column);
        } else if (S_ISDIR(info.st_mode)) {
            if (depth > VANTYX_COPY_MAX_DEPTH) {
                free(child_source);
                free(child_target);
                (void)closedir(directory);
                char message[512];
                snprintf(message, sizeof message,
                         "directory tree exceeds maximum depth %zu",
                         VANTYX_COPY_MAX_DEPTH);
                vantyx_rt_error("R0015", message, line, column);
            }
            if (mkdir(child_target, 0777) != 0) {
                int error_number = errno;
                char message[512];
                snprintf(message, sizeof message,
                         "cannot create directory `%.200s`: %.200s",
                         child_target, vantyx_os_error(error_number));
                free(child_source);
                free(child_target);
                (void)closedir(directory);
                vantyx_rt_error("R0015", message, line, column);
            }
            vantyx_copy_tree(
                child_source, child_target, depth + 1, entries, line, column);
        } else if (S_ISREG(info.st_mode)) {
            vantyx_copy_file(child_source, child_target, line, column);
        } else {
            char message[512];
            snprintf(message, sizeof message,
                     "`%.200s` is not a regular file or directory", child_source);
            free(child_source);
            free(child_target);
            (void)closedir(directory);
            vantyx_rt_error("R0015", message, line, column);
        }
        free(child_source);
        free(child_target);
    }
    if (closedir(directory) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot read directory `%.200s`: %.200s",
                 source, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
}

static vantyx_unit vantyx_copy_dir_all(vantyx_str from, vantyx_str to, int line, int column) {
    if (from[0] == '\0') {
        vantyx_rt_error("R0015", "source path is empty", line, column);
    }
    struct stat info;
    if (stat(from, &info) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy directory `%.200s`: %.200s",
                 from, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    if (!S_ISDIR(info.st_mode)) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy directory `%.200s`: not a directory", from);
        vantyx_rt_error("R0015", message, line, column);
    }

    size_t length = strlen(to);
    while (length > 0 && to[length - 1] == '/') {
        length--;
    }
    if (length == 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy directory `%.200s` to `%.200s`: "
                 "target must name a new directory",
                 from, to);
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t name_start = length;
    while (name_start > 0 && to[name_start - 1] != '/') {
        name_start--;
    }
    size_t name_length = length - name_start;
    if (name_length == 0 ||
        (name_length == 1 && to[name_start] == '.') ||
        (name_length == 2 && to[name_start] == '.' && to[name_start + 1] == '.')) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy directory `%.200s` to `%.200s`: "
                 "target must name a new directory",
                 from, to);
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t parent_length = name_start;
    while (parent_length > 1 && to[parent_length - 1] == '/') {
        parent_length--;
    }
    char* parent;
    if (parent_length == 0) {
        parent = (char*)vantyx_alloc(2);
        parent[0] = '.';
        parent[1] = '\0';
    } else {
        parent = (char*)vantyx_alloc(parent_length + 1);
        memcpy(parent, to, parent_length);
        parent[parent_length] = '\0';
    }
    char* target = (char*)vantyx_alloc(length + 1);
    memcpy(target, to, length);
    target[length] = '\0';

    struct stat target_info;
    if (lstat(target, &target_info) == 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy directory `%.200s` to `%.200s`: "
                 "target already exists",
                 from, to);
        vantyx_rt_error("R0015", message, line, column);
    } else if (errno != ENOENT) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot inspect target `%.200s` for directory copy: %.200s",
                 to, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }

    char* source_real = realpath(from, NULL);
    if (source_real == NULL) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot resolve source `%.200s`: %.200s",
                 from, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    char* parent_real = realpath(parent, NULL);
    if (parent_real == NULL) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot resolve target parent `%.200s`: %.200s",
                 parent, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t source_length = strlen(source_real);
    size_t parent_real_length = strlen(parent_real);
    char* target_real = (char*)vantyx_alloc(parent_real_length + name_length + 2);
    memcpy(target_real, parent_real, parent_real_length);
    size_t target_real_length = parent_real_length;
    if (target_real_length == 0 || target_real[target_real_length - 1] != '/') {
        target_real[target_real_length++] = '/';
    }
    memcpy(target_real + target_real_length, to + name_start, name_length);
    target_real[target_real_length + name_length] = '\0';
    bool inside = false;
    if (strncmp(source_real, target_real, source_length) == 0) {
        if (source_real[source_length - 1] == '/') {
            inside = true;
        } else if (target_real[source_length] == '/' ||
                   target_real[source_length] == '\0') {
            inside = true;
        }
    }
    if (inside) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot copy directory `%.200s` to `%.200s`: "
                 "target is inside the source",
                 from, to);
        vantyx_rt_error("R0015", message, line, column);
    }
    free(source_real);
    free(parent_real);
    free(target_real);
    free(parent);

    if (mkdir(target, 0777) != 0) {
        char message[512];
        snprintf(message, sizeof message,
                 "cannot create target directory `%.200s`: %.200s",
                 to, vantyx_os_error(errno));
        vantyx_rt_error("R0015", message, line, column);
    }
    size_t entries = 0;
    vantyx_copy_tree(from, target, 1, &entries, line, column);
    free(target);
    return (vantyx_unit)0;
}

static vantyx_str vantyx_value_display(const void* value, const VantyxTypeDesc* desc, int depth);
static bool vantyx_value_equals(const void* left, const void* right, const VantyxTypeDesc* desc, int depth);
static bool vantyx_list_equals(VantyxList* left, VantyxList* right, int depth);
static bool vantyx_map_equals(VantyxMap* left, VantyxMap* right, int depth);
static bool vantyx_ptr_equals(const void* left, const void* right, const VantyxTypeDesc* desc) {
    return vantyx_value_equals(&left, &right, desc, 0);
}

// Coinductive cycle handling for structural equality. The interpreter tracks
// visited container pairs and treats a revisited pair as equal; native equality
// must do the same or cyclic values compare differently between engines.
typedef struct {
    const void* left;
    const void* right;
    const VantyxTypeDesc* desc;
} VantyxEqFrame;

static VantyxEqFrame vantyx_eq_frames[128];
static int vantyx_eq_frames_len = 0;

static bool vantyx_eq_seen_pair(const void* left, const void* right, const VantyxTypeDesc* desc) {
    for (int index = 0; index < vantyx_eq_frames_len; index++) {
        if (vantyx_eq_frames[index].left == left &&
            vantyx_eq_frames[index].right == right &&
            vantyx_eq_frames[index].desc == desc) {
            return true;
        }
    }
    return false;
}

static bool vantyx_eq_push_pair(const void* left, const void* right, const VantyxTypeDesc* desc) {
    if (vantyx_eq_frames_len >= 128) return false;
    vantyx_eq_frames[vantyx_eq_frames_len].left = left;
    vantyx_eq_frames[vantyx_eq_frames_len].right = right;
    vantyx_eq_frames[vantyx_eq_frames_len].desc = desc;
    vantyx_eq_frames_len++;
    return true;
}

static void vantyx_eq_pop_pair(void) {
    if (vantyx_eq_frames_len > 0) vantyx_eq_frames_len--;
}

static void* vantyx_seen_structs[128];
static int vantyx_seen_structs_len = 0;

static bool vantyx_enter_struct(void* value) {
    for (int i = 0; i < vantyx_seen_structs_len; i++) {
        if (vantyx_seen_structs[i] == value) return false;
    }
    if (vantyx_seen_structs_len >= 128) return false;
    vantyx_seen_structs[vantyx_seen_structs_len++] = value;
    return true;
}

static void vantyx_leave_struct(void) { vantyx_seen_structs_len--; }

static vantyx_str vantyx_list_display(VantyxList* list, int depth) {
    static VantyxList* seen[128];
    static int seen_len = 0;
    if (depth > 64) return vantyx_strdup("\xE2\x80\xA6");
    for (int i = 0; i < seen_len; i++) {
        if (seen[i] == list) return vantyx_strdup("[...]");
    }
    if (seen_len >= 128) return vantyx_strdup("\xE2\x80\xA6");
    seen[seen_len++] = list;
    vantyx_str result = vantyx_strdup("[");
    for (int64_t index = 0; index < list->len; index++) {
        if (index > 0) result = vantyx_str_concat(result, vantyx_strdup(", "));
        const void* item = (const char*)list->items + index * list->elem_size;
        result = vantyx_str_concat(result, vantyx_value_display(item, list->elem_desc, depth + 1));
    }
    result = vantyx_str_concat(result, vantyx_strdup("]"));
    seen_len--;
    return result;
}

static vantyx_str vantyx_list_to_str(VantyxList* list) {
    return vantyx_list_display(list, 0);
}

// ---- maps (ADR-0010, ADR-0016): insertion-ordered arrays with an optional
// ---- open-addressing index for scalar keys. Struct, enum, and list keys keep
// ---- the linear structural-equality path.

static bool vantyx_map_hashable(const VantyxTypeDesc* desc) {
    if (desc == NULL) return false;
    switch (desc->kind) {
        case VANTYX_T_UNIT:
        case VANTYX_T_INT:
        case VANTYX_T_FLOAT:
        case VANTYX_T_BOOL:
        case VANTYX_T_STR:
            return true;
        default:
            return false;
    }
}

static uint64_t vantyx_map_mix(uint64_t value) {
    value ^= value >> 30;
    value *= UINT64_C(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value *= UINT64_C(0x94d049bb133111eb);
    value ^= value >> 31;
    return value;
}

static uint64_t vantyx_map_hash(const void* key, const VantyxTypeDesc* desc) {
    switch (desc->kind) {
        case VANTYX_T_UNIT:
            return vantyx_map_mix(0);
        case VANTYX_T_INT:
            return vantyx_map_mix((uint64_t)*(const int64_t*)key);
        case VANTYX_T_FLOAT: {
            double value = *(const double*)key;
            uint64_t bits = 0;
            // Match `==`: -0.0 and 0.0 are the same key.
            if (value != 0.0) memcpy(&bits, &value, sizeof bits);
            return vantyx_map_mix(bits);
        }
        case VANTYX_T_BOOL:
            return vantyx_map_mix(*(const bool*)key ? 1u : 0u);
        case VANTYX_T_STR: {
            const unsigned char* text = (const unsigned char*)*(const vantyx_str*)key;
            uint64_t hash = UINT64_C(1469598103934665603);
            while (*text != 0) {
                hash ^= (uint64_t)*text++;
                hash *= UINT64_C(1099511628211);
            }
            return vantyx_map_mix(hash);
        }
        default:
            return 0;
    }
}

static VantyxMap* vantyx_map_new(int64_t key_size, int64_t value_size,
                               const VantyxTypeDesc* key_desc,
                               const VantyxTypeDesc* value_desc) {
    VantyxMap* map = (VantyxMap*)vantyx_alloc(sizeof(VantyxMap));
    map->len = 0;
    map->cap = 0;
    map->key_size = key_size > 0 ? key_size : 1;
    map->value_size = value_size > 0 ? value_size : 1;
    map->key_desc = key_desc;
    map->value_desc = value_desc;
    map->keys = NULL;
    map->values = NULL;
    map->hashed = vantyx_map_hashable(key_desc);
    map->index_cap = 0;
    map->index = NULL;
    return map;
}

static void vantyx_map_reserve(VantyxMap* map, int64_t needed, int line, int column) {
    if (needed <= map->cap) return;
    int64_t cap = map->cap < 4 ? 4 : map->cap;
    while (cap < needed) {
        if (cap > INT64_MAX / 2) vantyx_rt_error("R0004", "map capacity overflow", line, column);
        cap *= 2;
    }
    if (cap > INT64_MAX / map->key_size || cap > INT64_MAX / map->value_size ||
        (uint64_t)cap > (uint64_t)SIZE_MAX / (uint64_t)map->key_size ||
        (uint64_t)cap > (uint64_t)SIZE_MAX / (uint64_t)map->value_size) {
        vantyx_rt_error("R0004", "map allocation is too large", line, column);
    }
    void* keys = realloc(map->keys, (size_t)(cap * map->key_size));
    if (keys == NULL) {
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    map->keys = keys;
    void* values = realloc(map->values, (size_t)(cap * map->value_size));
    if (values == NULL) {
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    map->values = values;
    map->cap = cap;
}

static void vantyx_map_index_rehash(VantyxMap* map, int64_t cap, int line, int column) {
    if (cap < 8) cap = 8;
    if ((uint64_t)cap > (uint64_t)SIZE_MAX / sizeof(int64_t)) {
        vantyx_rt_error("R0004", "map index allocation is too large", line, column);
    }
    int64_t* table = (int64_t*)calloc((size_t)cap, sizeof(int64_t));
    if (table == NULL) {
        fputs("error: out of memory\n", stderr);
        exit(2);
    }
    int64_t* old = map->index;
    map->index = table;
    map->index_cap = cap;
    for (int64_t entry = 0; entry < map->len; entry++) {
        const void* key = (const char*)map->keys + entry * map->key_size;
        int64_t mask = cap - 1;
        int64_t slot = (int64_t)(vantyx_map_hash(key, map->key_desc) & (uint64_t)mask);
        while (map->index[slot] != 0) slot = (slot + 1) & mask;
        map->index[slot] = entry + 1;
    }
    free(old);
}

static void vantyx_map_index_reserve(VantyxMap* map, int64_t needed, int line, int column) {
    if (!map->hashed) return;
    // Keep the load factor at or below 50% so linear probing stays short.
    if (map->index_cap > 0 && map->index_cap / 2 >= needed) return;
    int64_t cap = map->index_cap < 8 ? 8 : map->index_cap;
    while (cap / 2 < needed) {
        if (cap > INT64_MAX / 2) {
            vantyx_rt_error("R0004", "map index capacity overflow", line, column);
        }
        cap *= 2;
    }
    vantyx_map_index_rehash(map, cap, line, column);
}

static void vantyx_map_index_insert(VantyxMap* map, int64_t entry) {
    const void* key = (const char*)map->keys + entry * map->key_size;
    int64_t mask = map->index_cap - 1;
    int64_t slot = (int64_t)(vantyx_map_hash(key, map->key_desc) & (uint64_t)mask);
    while (map->index[slot] != 0) slot = (slot + 1) & mask;
    map->index[slot] = entry + 1;
}

static int64_t vantyx_map_find(VantyxMap* map, const void* key) {
    if (map->hashed && map->index_cap > 0) {
        int64_t mask = map->index_cap - 1;
        int64_t slot = (int64_t)(vantyx_map_hash(key, map->key_desc) & (uint64_t)mask);
        for (;;) {
            int64_t stored = map->index[slot];
            if (stored == 0) return -1;
            int64_t index = stored - 1;
            const void* candidate = (const char*)map->keys + index * map->key_size;
            if (vantyx_value_equals(candidate, key, map->key_desc, 0)) return index;
            slot = (slot + 1) & mask;
        }
    }
    for (int64_t index = 0; index < map->len; index++) {
        const void* candidate = (const char*)map->keys + index * map->key_size;
        if (vantyx_value_equals(candidate, key, map->key_desc, 0)) return index;
    }
    return -1;
}

static int64_t vantyx_map_len(VantyxMap* map) { return map->len; }

static void* vantyx_map_get(VantyxMap* map, const void* key, int line, int column) {
    int64_t index = vantyx_map_find(map, key);
    if (index < 0) {
        // The interpreter renders map keys with top-level display, so a Str
        // key appears without quotes here even though it is quoted inside a
        // displayed collection.
        vantyx_str rendered = map->key_desc->kind == VANTYX_T_STR
                                  ? *(const vantyx_str*)key
                                  : vantyx_value_display(key, map->key_desc, 0);
        char message[256];
        snprintf(message, sizeof message, "key `%s` is not present in the map", rendered);
        vantyx_rt_error("R0008", message, line, column);
    }
    return (char*)map->values + index * map->value_size;
}

static bool vantyx_map_contains(VantyxMap* map, const void* key) {
    return vantyx_map_find(map, key) >= 0;
}

static void vantyx_map_set_raw(VantyxMap* map, const void* key, const void* value,
                              int line, int column) {
    int64_t index = vantyx_map_find(map, key);
    if (index >= 0) {
        memcpy((char*)map->values + index * map->value_size, value, (size_t)map->value_size);
        return;
    }
    vantyx_map_reserve(map, map->len + 1, line, column);
    vantyx_map_index_reserve(map, map->len + 1, line, column);
    memcpy((char*)map->keys + map->len * map->key_size, key, (size_t)map->key_size);
    memcpy((char*)map->values + map->len * map->value_size, value, (size_t)map->value_size);
    if (map->hashed) vantyx_map_index_insert(map, map->len);
    map->len++;
}

static void* vantyx_map_key_unchecked(VantyxMap* map, int64_t index) {
    return (char*)map->keys + index * map->key_size;
}

static VantyxList* vantyx_map_keys(VantyxMap* map, int64_t elem_size,
                                 const VantyxTypeDesc* elem_desc) {
    VantyxList* list = vantyx_list_new(elem_size, elem_desc);
    if (map->len > 0) {
        vantyx_list_reserve(list, map->len, 0, 0);
        memcpy(list->items, map->keys, (size_t)(map->len * map->key_size));
        list->len = map->len;
    }
    return list;
}

static VantyxList* vantyx_map_values(VantyxMap* map, int64_t elem_size,
                                   const VantyxTypeDesc* elem_desc) {
    VantyxList* list = vantyx_list_new(elem_size, elem_desc);
    if (map->len > 0) {
        vantyx_list_reserve(list, map->len, 0, 0);
        memcpy(list->items, map->values, (size_t)(map->len * map->value_size));
        list->len = map->len;
    }
    return list;
}

static vantyx_str vantyx_map_display(VantyxMap* map, int depth) {
    static VantyxMap* seen[128];
    static int seen_len = 0;
    if (depth > 64) return vantyx_strdup("\xE2\x80\xA6");
    for (int i = 0; i < seen_len; i++) {
        if (seen[i] == map) return vantyx_strdup("{...}");
    }
    if (seen_len >= 128) return vantyx_strdup("\xE2\x80\xA6");
    seen[seen_len++] = map;
    vantyx_str result = vantyx_strdup("{");
    for (int64_t index = 0; index < map->len; index++) {
        if (index > 0) result = vantyx_str_concat(result, vantyx_strdup(", "));
        const void* key = (const char*)map->keys + index * map->key_size;
        const void* value = (const char*)map->values + index * map->value_size;
        result = vantyx_str_concat(result, vantyx_value_display(key, map->key_desc, depth + 1));
        result = vantyx_str_concat(result, vantyx_strdup(": "));
        result = vantyx_str_concat(result, vantyx_value_display(value, map->value_desc, depth + 1));
    }
    result = vantyx_str_concat(result, vantyx_strdup("}"));
    seen_len--;
    return result;
}

static vantyx_str vantyx_map_to_str(VantyxMap* map) {
    return vantyx_map_display(map, 0);
}

static vantyx_str vantyx_value_display(const void* value, const VantyxTypeDesc* desc, int depth) {
    switch (desc->kind) {
        case VANTYX_T_UNIT: return vantyx_str_from_unit();
        case VANTYX_T_INT: return vantyx_str_from_int(*(const int64_t*)value);
        case VANTYX_T_FLOAT: return vantyx_str_from_float(*(const double*)value);
        case VANTYX_T_BOOL: return vantyx_str_from_bool(*(const bool*)value);
        case VANTYX_T_STR: return vantyx_str_quote(*(const vantyx_str*)value);
        case VANTYX_T_LIST: return vantyx_list_display(*(VantyxList* const*)value, depth);
        case VANTYX_T_MAP: return vantyx_map_display(*(VantyxMap* const*)value, depth);
        case VANTYX_T_STRUCT: return desc->display_value(*(void* const*)value, depth);
        case VANTYX_T_ENUM: return desc->display_value(*(void* const*)value, depth);
    }
    return vantyx_strdup("\xE2\x80\xA6");
}

static bool vantyx_list_equals(VantyxList* left, VantyxList* right, int depth) {
    if (left == right) return true;
    if (left == NULL || right == NULL) return false;
    if (depth > 64) return false;
    if (left->len != right->len) return false;
    for (int64_t index = 0; index < left->len; index++) {
        const void* a = (const char*)left->items + index * left->elem_size;
        const void* b = (const char*)right->items + index * right->elem_size;
        if (!vantyx_value_equals(a, b, left->elem_desc, depth + 1)) return false;
    }
    return true;
}

static bool vantyx_map_equals(VantyxMap* left, VantyxMap* right, int depth) {
    if (left == right) return true;
    if (left == NULL || right == NULL) return false;
    if (depth > 64) return false;
    if (left->len != right->len) return false;
    for (int64_t index = 0; index < left->len; index++) {
        const void* key = (const char*)left->keys + index * left->key_size;
        int64_t other = vantyx_map_find(right, key);
        if (other < 0) return false;
        const void* a = (const char*)left->values + index * left->value_size;
        const void* b = (const char*)right->values + other * right->value_size;
        if (!vantyx_value_equals(a, b, left->value_desc, depth + 1)) return false;
    }
    return true;
}

static bool vantyx_value_equals(const void* left, const void* right, const VantyxTypeDesc* desc, int depth) {
    if (depth > 64) return false;
    switch (desc->kind) {
        case VANTYX_T_UNIT: return true;
        case VANTYX_T_INT: return *(const int64_t*)left == *(const int64_t*)right;
        case VANTYX_T_FLOAT: return *(const double*)left == *(const double*)right;
        case VANTYX_T_BOOL: return *(const bool*)left == *(const bool*)right;
        case VANTYX_T_STR: return strcmp(*(const vantyx_str*)left, *(const vantyx_str*)right) == 0;
        case VANTYX_T_LIST: {
            VantyxList* a = *(VantyxList* const*)left;
            VantyxList* b = *(VantyxList* const*)right;
            if (a == b) return true;
            if (a == NULL || b == NULL) return false;
            if (vantyx_eq_seen_pair(a, b, desc)) return true;
            if (!vantyx_eq_push_pair(a, b, desc)) return false;
            bool equal = vantyx_list_equals(a, b, depth);
            vantyx_eq_pop_pair();
            return equal;
        }
        case VANTYX_T_MAP: {
            VantyxMap* a = *(VantyxMap* const*)left;
            VantyxMap* b = *(VantyxMap* const*)right;
            if (a == b) return true;
            if (a == NULL || b == NULL) return false;
            if (vantyx_eq_seen_pair(a, b, desc)) return true;
            if (!vantyx_eq_push_pair(a, b, desc)) return false;
            bool equal = vantyx_map_equals(a, b, depth);
            vantyx_eq_pop_pair();
            return equal;
        }
        case VANTYX_T_STRUCT: {
            void* a = *(void* const*)left;
            void* b = *(void* const*)right;
            if (a == b) return true;
            if (a == NULL || b == NULL) return false;
            if (vantyx_eq_seen_pair(a, b, desc)) return true;
            if (!vantyx_eq_push_pair(a, b, desc)) return false;
            bool equal = desc->equals_value(a, b, depth);
            vantyx_eq_pop_pair();
            return equal;
        }
        case VANTYX_T_ENUM: {
            void* a = *(void* const*)left;
            void* b = *(void* const*)right;
            if (a == b) return true;
            if (a == NULL || b == NULL) return false;
            if (vantyx_eq_seen_pair(a, b, desc)) return true;
            if (!vantyx_eq_push_pair(a, b, desc)) return false;
            bool equal = desc->equals_value(a, b, depth);
            vantyx_eq_pop_pair();
            return equal;
        }
    }
    return false;
}

static bool vantyx_list_contains(VantyxList* list, const void* needle) {
    for (int64_t index = 0; index < list->len; index++) {
        const void* item = (const char*)list->items + index * list->elem_size;
        if (vantyx_value_equals(item, needle, list->elem_desc, 0)) return true;
    }
    return false;
}
"#;
