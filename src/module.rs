//! File-based modules and imports.
//!
//! A module is a `.vtx` file containing only declarations (`fn`, `struct`,
//! `enum`) and further imports. The loader parses every file once, rejects
//! cycles and paths outside the entry file's directory, then flattens the
//! selected declarations into one [`Program`] for the existing checker,
//! interpreter, and native backend.
//!
//! Imports are deliberately simple in v0.1:
//!
//! ```text
//! import "lib/math.vtx"
//! from "lib/math.vtx" import add, Point
//! ```
//!
//! There are no namespaces, re-exports, or module initializers. Imported
//! declarations share the global scope, so duplicate names are reported by the
//! checker at the imported file's span.

use crate::ast::{ImportDecl, ImportName, Item, Program};
use crate::checker::{self, SemanticModel};
use crate::diagnostic::{Diagnostic, Diagnostics};
use crate::lexer;
use crate::parser;
use crate::span::SourceRegistry;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Resource guards for one module graph. They bound disk reads and recursion
/// independently of the parser's expression and block depth limits.
pub const MAX_MODULES: usize = 256;
pub const MAX_IMPORT_DEPTH: usize = 64;
pub const MAX_MODULE_BYTES: usize = 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;

/// A failure before any Vantyx source could be read.
#[derive(Clone, Debug)]
pub struct LoadError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot read `{}`: {}", self.path.display(), self.message)
    }
}

/// The result of loading an entry file and its modules.
pub struct LoadResult {
    pub program: Option<Program>,
    pub model: Option<SemanticModel>,
    pub diagnostics: Diagnostics,
    pub sources: SourceRegistry,
}

impl LoadResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }

    pub fn render(&self) -> String {
        self.diagnostics.render_all(&self.sources)
    }
}

/// Loads `path` and resolves its imports, requiring an entry point.
pub fn load_file(path: &Path) -> Result<LoadResult, LoadError> {
    load_file_with_options(path, true)
}

/// Loads `path` and resolves its imports.
pub fn load_file_with_options(path: &Path, require_entry: bool) -> Result<LoadResult, LoadError> {
    let entry = fs::canonicalize(path).map_err(|error| LoadError {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if !entry.is_file() {
        return Err(LoadError {
            path: path.to_path_buf(),
            message: "not a regular file".to_string(),
        });
    }
    let root = entry
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut loader = Loader::new(root);
    let mut items = loader.expand(&entry, true, None, None);
    let mut diagnostics = std::mem::take(&mut loader.diagnostics);
    let sources = std::mem::take(&mut loader.sources);

    let program = Program {
        items: std::mem::take(&mut items),
        span: crate::span::Span::default(),
    };
    let program = Program {
        span: loader.entry_span,
        ..program
    };

    if diagnostics.has_errors() {
        return Ok(LoadResult {
            program: Some(program),
            model: None,
            diagnostics,
            sources,
        });
    }

    let (model, check_diagnostics) = checker::check_with_options(&program, require_entry);
    diagnostics.extend(check_diagnostics);
    Ok(LoadResult {
        program: Some(program),
        model,
        diagnostics,
        sources,
    })
}

struct Loader {
    root: PathBuf,
    sources: SourceRegistry,
    diagnostics: Diagnostics,
    units: HashMap<PathBuf, ModuleUnit>,
    emitted: HashSet<(PathBuf, u32)>,
    stack: Vec<PathBuf>,
    entry_span: crate::span::Span,
    total_bytes: usize,
}

struct ModuleUnit {
    directory: PathBuf,
    file: u32,
    items: Vec<Item>,
    imports: Vec<ImportDecl>,
    declarations: Vec<(String, u32)>,
}

impl Loader {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            sources: SourceRegistry::new(),
            diagnostics: Diagnostics::new(),
            units: HashMap::new(),
            emitted: HashSet::new(),
            stack: Vec::new(),
            entry_span: crate::span::Span::default(),
            total_bytes: 0,
        }
    }

    /// Expands one file into declarations. `selection` filters the file's own
    /// declarations; dependencies are always expanded because the selected
    /// declarations may call them.
    fn expand(
        &mut self,
        path: &Path,
        is_entry: bool,
        selection: Option<&[ImportName]>,
        import_span: Option<crate::span::Span>,
    ) -> Vec<Item> {
        if self.stack.len() >= MAX_IMPORT_DEPTH {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0508",
                    format!("import depth exceeds the limit of {MAX_IMPORT_DEPTH}"),
                    import_span.unwrap_or_default(),
                )
                .with_help("flatten the module dependency chain"),
            );
            return Vec::new();
        }
        if let Some(position) = self.stack.iter().position(|item| item == path) {
            let mut chain: Vec<String> = self.stack[position..]
                .iter()
                .map(|item| self.display(item))
                .collect();
            chain.push(self.display(path));
            let span = import_span.unwrap_or_default();
            self.diagnostics.push(
                Diagnostic::error("E0503", "import cycle detected", span)
                    .with_note(format!("import chain: {}", chain.join(" -> ")))
                    .with_help("remove one of the imports so module dependencies form a tree"),
            );
            return Vec::new();
        }

        let unit = match self.unit(path, is_entry, import_span) {
            Some(unit) => unit,
            None => return Vec::new(),
        };
        if is_entry {
            self.entry_span = crate::span::Span::point_in_file(unit.file, 0);
        }

        self.stack.push(path.to_path_buf());
        let mut result = Vec::new();
        let imports = unit.imports.clone();
        let directory = unit.directory.clone();
        for import in &imports {
            let Some(target) = self.resolve(&directory, import) else {
                continue;
            };
            let mut imported = self.expand(
                &target,
                false,
                import.names.as_deref(),
                Some(import.path_span),
            );
            result.append(&mut imported);
        }
        self.stack.pop();

        // Keep only the requested declarations. Top-level statements belong to
        // the entry file and are preserved in source order after imports, so
        // `vantyx run` still executes a script that contains only statements.
        for item in unit.items {
            let Some(name) = declaration_name(&item) else {
                if is_entry {
                    result.push(item);
                }
                continue;
            };
            if let Some(names) = selection
                && !names.iter().any(|wanted| wanted.name == name)
            {
                continue;
            }
            let identity = match &item {
                Item::Fn(decl) => (path.to_path_buf(), decl.name_span.start),
                Item::Struct(decl) => (path.to_path_buf(), decl.name_span.start),
                Item::Enum(decl) => (path.to_path_buf(), decl.name_span.start),
                _ => continue,
            };
            if self.emitted.insert(identity) {
                result.push(item);
            }
        }

        // A selective import asks for names this file does not declare.
        if let Some(names) = selection {
            let declared: HashSet<&str> = unit
                .declarations
                .iter()
                .map(|(name, _)| name.as_str())
                .collect();
            for name in names {
                if !declared.contains(name.name.as_str()) {
                    let span = name.span;
                    let mut available: Vec<&str> = declared.iter().copied().collect();
                    available.sort_unstable();
                    let note = if available.is_empty() {
                        "the module declares no functions, structs, or enums".to_string()
                    } else {
                        format!("available names: {}", available.join(", "))
                    };
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0504",
                            format!("module does not declare `{}`", name.name),
                            span,
                        )
                        .with_note(note),
                    );
                }
            }
        }

        result
    }

    fn unit(
        &mut self,
        path: &Path,
        is_entry: bool,
        import_span: Option<crate::span::Span>,
    ) -> Option<ModuleUnit> {
        if let Some(unit) = self.units.get(path) {
            return Some(clone_unit(unit));
        }
        if self.units.len() >= MAX_MODULES {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0507",
                    format!("module graph exceeds the limit of {MAX_MODULES} files"),
                    import_span.unwrap_or_default(),
                )
                .with_help("reduce the number of imported files"),
            );
            return None;
        }
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0501",
                        format!("cannot read module `{}`: {error}", self.display(path)),
                        import_span.unwrap_or_default(),
                    )
                    .with_help("check the path and file permissions"),
                );
                return None;
            }
        };
        if source.len() > MAX_MODULE_BYTES {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0509",
                    format!(
                        "module `{}` is {} bytes; the limit is {MAX_MODULE_BYTES}",
                        self.display(path),
                        source.len()
                    ),
                    import_span.unwrap_or_default(),
                )
                .with_help("split the module into smaller files"),
            );
            return None;
        }
        if self.total_bytes.saturating_add(source.len()) > MAX_TOTAL_BYTES {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0509",
                    format!("module graph exceeds the {MAX_TOTAL_BYTES}-byte source limit"),
                    import_span.unwrap_or_default(),
                )
                .with_help("reduce the size of imported modules"),
            );
            return None;
        }
        self.total_bytes += source.len();
        let display = self.display(path);
        let file = self.sources.add(display, source.clone());
        let lexed = lexer::lex_file(&source, file);
        let mut diagnostics = lexed.diagnostics;
        if diagnostics.has_errors() {
            self.diagnostics.extend(diagnostics);
            return None;
        }
        let (program, parse_diagnostics) = parser::parse_in_file(&source, lexed.tokens, file);
        diagnostics = parse_diagnostics;
        if diagnostics.has_errors() {
            self.diagnostics.extend(diagnostics);
            return None;
        }

        let directory = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let mut items = Vec::new();
        let mut imports = Vec::new();
        let mut declarations = Vec::new();
        for item in program.items {
            match item {
                Item::Import(import) => imports.push(import),
                Item::Stmt(stmt) if !is_entry => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0505",
                            "modules may contain only declarations",
                            stmt.span,
                        )
                        .with_help(
                            "move executable code into a function and call it from the entry file",
                        ),
                    );
                }
                Item::Fn(decl) if !is_entry && decl.name == "main" => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "E0506",
                            "`main` must be defined in the entry file",
                            decl.name_span,
                        )
                        .with_help("rename the module function or move `main` to the entry file"),
                    );
                    declarations.push((decl.name.clone(), decl.name_span.start));
                    items.push(Item::Fn(decl));
                }
                other => {
                    if let Some(name) = declaration_name(&other) {
                        let start = match &other {
                            Item::Fn(decl) => decl.name_span.start,
                            Item::Struct(decl) => decl.name_span.start,
                            Item::Enum(decl) => decl.name_span.start,
                            _ => 0,
                        };
                        declarations.push((name, start));
                    }
                    items.push(other);
                }
            }
        }

        let unit = ModuleUnit {
            directory,
            file,
            items,
            imports,
            declarations,
        };
        self.units.insert(path.to_path_buf(), clone_unit(&unit));
        Some(unit)
    }

    fn resolve(&mut self, directory: &Path, import: &ImportDecl) -> Option<PathBuf> {
        if Path::new(&import.path).is_absolute() {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0502",
                    "import paths must be relative to the importing file",
                    import.path_span,
                )
                .with_help("write a path such as `lib/math.vtx`"),
            );
            return None;
        }
        if Path::new(&import.path)
            .extension()
            .and_then(|ext| ext.to_str())
            != Some("vtx")
        {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0501",
                    "imported modules must use the `.vtx` extension",
                    import.path_span,
                )
                .with_help("write a path such as `lib/math.vtx`"),
            );
            return None;
        }
        let candidate = directory.join(&import.path);
        let canonical = match fs::canonicalize(&candidate) {
            Ok(path) => path,
            Err(error) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "E0501",
                        format!("cannot resolve import `{}`: {error}", import.path),
                        import.path_span,
                    )
                    .with_help("check that the file exists and the path is correct"),
                );
                return None;
            }
        };
        if !canonical.starts_with(&self.root) {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0502",
                    format!(
                        "import `{}` resolves outside the entry file's directory",
                        import.path
                    ),
                    import.path_span,
                )
                .with_help("keep modules under the entry file's directory"),
            );
            return None;
        }
        if !canonical.is_file() {
            self.diagnostics.push(
                Diagnostic::error(
                    "E0501",
                    format!("import `{}` is not a regular file", import.path),
                    import.path_span,
                )
                .with_help("import a `.vtx` source file"),
            );
            return None;
        }
        Some(canonical)
    }

    fn display(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .display()
            .to_string()
    }
}

fn clone_unit(unit: &ModuleUnit) -> ModuleUnit {
    ModuleUnit {
        directory: unit.directory.clone(),
        file: unit.file,
        items: unit.items.clone(),
        imports: unit.imports.clone(),
        declarations: unit.declarations.clone(),
    }
}

fn declaration_name(item: &Item) -> Option<String> {
    match item {
        Item::Fn(decl) => Some(decl.name.clone()),
        Item::Struct(decl) => Some(decl.name.clone()),
        Item::Enum(decl) => Some(decl.name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(name: &str) -> TempDir {
        let directory = std::env::temp_dir().join(format!(
            "vantyx-module-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).expect("create temp dir");
        TempDir(directory)
    }

    fn write(directory: &TempDir, name: &str, source: &str) {
        let path = directory.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create module dir");
        }
        fs::write(path, source).expect("write module");
    }

    fn codes(result: &LoadResult) -> Vec<&str> {
        result
            .diagnostics
            .items
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    #[test]
    fn imports_all_declarations() {
        let directory = temp_dir("all");
        write(
            &directory,
            "lib.vtx",
            "fn double(x: Int) -> Int:\n    return x * 2\n",
        );
        write(
            &directory,
            "main.vtx",
            "import \"lib.vtx\"\n\nfn main():\n    print(double(21))\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(!result.has_errors(), "{}", result.render());
        assert!(result.model.is_some());
    }

    #[test]
    fn selective_import_brings_only_requested_names() {
        let directory = temp_dir("selective");
        write(
            &directory,
            "lib.vtx",
            "fn used() -> Int:\n    return 1\n\nfn unused() -> Int:\n    return 2\n",
        );
        write(
            &directory,
            "main.vtx",
            "from \"lib.vtx\" import used\n\nfn main():\n    print(used())\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(!result.has_errors(), "{}", result.render());
    }

    #[test]
    fn missing_module_reports_e0301() {
        let directory = temp_dir("missing");
        write(
            &directory,
            "main.vtx",
            "import \"nope.vtx\"\n\nfn main():\n    print(1)\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(codes(&result).contains(&"E0501"), "{}", result.render());
        assert!(result.model.is_none());
    }

    #[test]
    fn cycle_reports_e0303_with_chain() {
        let directory = temp_dir("cycle");
        write(
            &directory,
            "a.vtx",
            "import \"b.vtx\"\n\nfn a():\n    return\n",
        );
        write(
            &directory,
            "b.vtx",
            "import \"a.vtx\"\n\nfn b():\n    return\n",
        );
        write(
            &directory,
            "main.vtx",
            "import \"a.vtx\"\n\nfn main():\n    a()\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(codes(&result).contains(&"E0503"), "{}", result.render());
        assert!(result.render().contains("a.vtx -> b.vtx -> a.vtx"));
    }

    #[test]
    fn path_outside_root_reports_e0302() {
        let directory = temp_dir("outside");
        let outside = temp_dir("outside-target");
        write(
            &outside,
            "secret.vtx",
            "fn secret() -> Int:\n    return 1\n",
        );
        let relative = format!(
            "../{}/secret.vtx",
            outside.0.file_name().expect("temp name").to_string_lossy()
        );
        write(
            &directory,
            "main.vtx",
            &format!("import \"{relative}\"\n\nfn main():\n    print(1)\n"),
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(codes(&result).contains(&"E0502"), "{}", result.render());
    }

    #[test]
    fn unknown_imported_name_reports_e0304() {
        let directory = temp_dir("unknown");
        write(
            &directory,
            "lib.vtx",
            "fn present() -> Int:\n    return 1\n",
        );
        write(
            &directory,
            "main.vtx",
            "from \"lib.vtx\" import absent\n\nfn main():\n    print(1)\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        let diagnostic = result
            .diagnostics
            .items
            .iter()
            .find(|diagnostic| diagnostic.code == "E0504")
            .expect("E0504");
        assert!(diagnostic.message.contains("absent"));
        assert!(result.render().contains("available names: present"));
    }

    #[test]
    fn module_top_level_statement_reports_e0305() {
        let directory = temp_dir("stmt");
        write(&directory, "lib.vtx", "print(1)\n");
        write(
            &directory,
            "main.vtx",
            "import \"lib.vtx\"\n\nfn main():\n    print(2)\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(codes(&result).contains(&"E0505"), "{}", result.render());
    }

    #[test]
    fn module_main_reports_e0306() {
        let directory = temp_dir("main");
        write(&directory, "lib.vtx", "fn main():\n    print(1)\n");
        write(
            &directory,
            "main.vtx",
            "import \"lib.vtx\"\n\nfn main():\n    print(2)\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(codes(&result).contains(&"E0506"), "{}", result.render());
    }

    /// Writes `count` imported modules plus an entry file that imports each of
    /// them. The entry file is the `(count + 1)`th file in the module graph.
    fn write_import_graph(directory: &TempDir, count: usize) -> PathBuf {
        let mut main = String::new();
        for index in 0..count {
            let name = format!("m{index:03}.vtx");
            write(
                directory,
                &name,
                &format!("fn imported_{index:03}() -> Int:\n    return {index}\n"),
            );
            main.push_str(&format!("import \"{name}\"\n"));
        }
        main.push_str("\nfn main():\n    print(0)\n");
        write(directory, "main.vtx", &main);
        directory.0.join("main.vtx")
    }

    #[test]
    fn module_cap_accepts_exactly_max_modules() {
        let directory = temp_dir("module-cap-exact");
        let entry = write_import_graph(&directory, MAX_MODULES - 1);
        let result = load_file(&entry).expect("load");
        assert!(!result.has_errors(), "{}", result.render());
        let program = result.program.expect("program");
        assert_eq!(program.items.len(), MAX_MODULES);
    }

    #[test]
    fn module_cap_rejects_one_module_over_max() {
        let directory = temp_dir("module-cap-over");
        let entry = write_import_graph(&directory, MAX_MODULES);
        let result = load_file(&entry).expect("load");
        let failures: Vec<_> = result
            .diagnostics
            .items
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0507")
            .collect();
        assert_eq!(failures.len(), 1, "{}", result.render());
        assert!(
            failures[0]
                .message
                .contains(&format!("limit of {MAX_MODULES} files")),
            "{}",
            result.render()
        );
        assert!(result.model.is_none());
    }

    #[test]
    fn duplicate_declaration_across_modules_reports_e0202() {
        let directory = temp_dir("duplicate");
        write(&directory, "a.vtx", "fn same() -> Int:\n    return 1\n");
        write(&directory, "b.vtx", "fn same() -> Int:\n    return 2\n");
        write(
            &directory,
            "main.vtx",
            "import \"a.vtx\"\nimport \"b.vtx\"\n\nfn main():\n    print(same())\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(codes(&result).contains(&"E0202"), "{}", result.render());
        assert!(result.render().contains("a.vtx") || result.render().contains("b.vtx"));
    }

    #[test]
    fn imported_diagnostics_name_the_imported_file() {
        let directory = temp_dir("span");
        write(
            &directory,
            "lib.vtx",
            "fn broken() -> Int:\n    return \"no\"\n",
        );
        write(
            &directory,
            "main.vtx",
            "import \"lib.vtx\"\n\nfn main():\n    print(broken())\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        let rendered = result.render();
        assert!(rendered.contains("lib.vtx"), "{rendered}");
    }

    #[test]
    fn duplicate_import_is_emitted_once() {
        let directory = temp_dir("repeat");
        write(&directory, "lib.vtx", "fn one() -> Int:\n    return 1\n");
        write(
            &directory,
            "main.vtx",
            "import \"lib.vtx\"\nfrom \"lib.vtx\" import one\n\nfn main():\n    print(one())\n",
        );
        let result = load_file(&directory.0.join("main.vtx")).expect("load");
        assert!(!result.has_errors(), "{}", result.render());
    }

    #[test]
    fn imports_in_a_direct_compile_are_rejected() {
        let result = crate::compile("import \"lib.vtx\"\n\nfn main():\n    print(1)\n");
        assert!(result.has_errors());
        assert!(
            result
                .diagnostics
                .items
                .iter()
                .any(|diagnostic| diagnostic.code == "E0510")
        );
    }
}
