//! Build script — compiles tree-sitter grammars from `vendor/grammars/`.
//!
//! For each grammar directory:
//!   1. Reads `grammar.toml` for metadata (id, name, extensions, parser_fn).
//!   2. Compiles `parser.c` + optional `scanner.c`/`scanner.cc` via the `cc` crate.
//!   3. Generates `$OUT_DIR/grammars.rs` with FFI bindings and embedded `.scm` queries.
//!
//! Adding a new language = drop a directory into `vendor/grammars/` with the right layout.
//! Zero Rust code changes needed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

// ── Grammar metadata (grammar.toml) ────────────────────────────────

#[derive(Deserialize)]
struct GrammarConfig {
    id: String,
    name: String,
    extensions: Vec<String>,
    parser_fn: String,
}

struct GrammarEntry {
    config: GrammarConfig,
    dir: PathBuf,
    has_scanner_c: bool,
    has_scanner_cc: bool,
    has_highlights: bool,
    has_injections: bool,
    has_locals: bool,
}

// ── Entry point ────────────────────────────────────────────────────

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env("OUT_DIR"));
    let grammars_dir = manifest_dir.join("vendor").join("grammars");
    let include_dir = grammars_dir.join("include");

    let grammars = discover_grammars(&grammars_dir);
    emit_rerun_directives(&grammars);
    compile_c_sources(&grammars, &include_dir);
    compile_cpp_sources(&grammars, &include_dir);
    generate_rust_bindings(&grammars, &manifest_dir, &out_dir);
}

// ── Discovery ──────────────────────────────────────────────────────

fn discover_grammars(grammars_dir: &Path) -> BTreeMap<String, GrammarEntry> {
    let mut grammars = BTreeMap::new();

    for entry in fs::read_dir(grammars_dir).expect("cannot read vendor/grammars") {
        let entry = entry.expect("cannot read directory entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path.file_name().unwrap().to_str().unwrap().to_string();
        if dir_name == "include" {
            continue;
        }

        let toml_path = path.join("grammar.toml");
        if !toml_path.exists() {
            eprintln!("cargo:warning={dir_name} has no grammar.toml — skipping");
            continue;
        }

        let toml_str = fs::read_to_string(&toml_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", toml_path.display()));
        let config: GrammarConfig = toml::from_str(&toml_str)
            .unwrap_or_else(|e| panic!("invalid grammar.toml in {dir_name}: {e}"));

        let src_dir = path.join("src");
        let queries_dir = path.join("queries");

        let grammar = GrammarEntry {
            has_scanner_c: src_dir.join("scanner.c").exists(),
            has_scanner_cc: src_dir.join("scanner.cc").exists(),
            has_highlights: queries_dir.join("highlights.scm").exists(),
            has_injections: queries_dir.join("injections.scm").exists(),
            has_locals: queries_dir.join("locals.scm").exists(),
            dir: path,
            config,
        };

        grammars.insert(grammar.config.id.clone(), grammar);
    }

    grammars
}

// ── Cargo rerun-if-changed ─────────────────────────────────────────

fn emit_rerun_directives(grammars: &BTreeMap<String, GrammarEntry>) {
    for grammar in grammars.values() {
        let src = grammar.dir.join("src");
        let queries = grammar.dir.join("queries");

        println!(
            "cargo:rerun-if-changed={}",
            grammar.dir.join("grammar.toml").display()
        );
        println!("cargo:rerun-if-changed={}", src.join("parser.c").display());

        if grammar.has_scanner_c {
            println!("cargo:rerun-if-changed={}", src.join("scanner.c").display());
        }
        if grammar.has_scanner_cc {
            println!(
                "cargo:rerun-if-changed={}",
                src.join("scanner.cc").display()
            );
        }
        for name in &["highlights.scm", "injections.scm", "locals.scm"] {
            let p = queries.join(name);
            if p.exists() {
                println!("cargo:rerun-if-changed={}", p.display());
            }
        }
    }
}

// ── C compilation ──────────────────────────────────────────────────

fn compile_c_sources(grammars: &BTreeMap<String, GrammarEntry>, include_dir: &Path) {
    let mut build = cc::Build::new();
    build
        .include(include_dir)
        .warnings(false)
        .extra_warnings(false)
        .flag_if_supported("-std=c11")
        .opt_level(2);

    let mut has_files = false;

    for grammar in grammars.values() {
        // Only compile grammars that have highlight queries (others are useless)
        if !grammar.has_highlights {
            continue;
        }

        let src_dir = grammar.dir.join("src");

        // Per-grammar include for local headers (e.g. scanner.h, unicode.h)
        build.include(&src_dir);
        build.file(src_dir.join("parser.c"));
        has_files = true;

        if grammar.has_scanner_c {
            build.file(src_dir.join("scanner.c"));
        }
    }

    if has_files {
        build.compile("tree_sitter_grammars");
    }
}

// ── C++ compilation (scanner.cc) ───────────────────────────────────

fn compile_cpp_sources(grammars: &BTreeMap<String, GrammarEntry>, include_dir: &Path) {
    let cpp_grammars: Vec<&GrammarEntry> = grammars
        .values()
        .filter(|g| g.has_scanner_cc && g.has_highlights)
        .collect();

    if cpp_grammars.is_empty() {
        return;
    }

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .include(include_dir)
        .warnings(false)
        .extra_warnings(false)
        .flag_if_supported("-std=c++14")
        .opt_level(2);

    for grammar in &cpp_grammars {
        let src_dir = grammar.dir.join("src");
        build.include(&src_dir);
        build.file(src_dir.join("scanner.cc"));
    }

    build.compile("tree_sitter_grammars_cpp");
}

// ── Code generation ────────────────────────────────────────────────

fn generate_rust_bindings(
    grammars: &BTreeMap<String, GrammarEntry>,
    manifest_dir: &Path,
    out_dir: &Path,
) {
    // Only generate bindings for grammars with highlight queries
    let usable: Vec<&GrammarEntry> = grammars.values().filter(|g| g.has_highlights).collect();

    let mut code = String::with_capacity(8192);

    code.push_str("// Auto-generated by build.rs — do not edit.\n\n");

    // Extern "C" declarations
    code.push_str("unsafe extern \"C\" {\n");
    for g in &usable {
        code.push_str(&format!("    fn {}() -> *const ();\n", g.config.parser_fn,));
    }
    code.push_str("}\n\n");

    // Type alias for the FFI function pointer
    code.push_str("type ParserFn = unsafe extern \"C\" fn() -> *const ();\n\n");

    // GrammarDef struct
    code.push_str(
        "/// Compiled grammar definition (generated by build.rs).\n\
         pub struct GrammarDef {\n\
         \x20   pub name: &'static str,\n\
         \x20   pub extensions: &'static [&'static str],\n\
         \x20   pub language_fn: tree_sitter_language::LanguageFn,\n\
         \x20   pub highlights_query: &'static str,\n\
         \x20   pub injections_query: &'static str,\n\
         \x20   pub locals_query: &'static str,\n\
         }\n\n",
    );

    // all_grammars() function
    code.push_str("/// Returns all compiled grammars with embedded queries.\n");
    code.push_str("pub fn all_grammars() -> Vec<GrammarDef> {\n");
    code.push_str("    vec![\n");

    for g in &usable {
        let queries_dir = manifest_dir
            .join("vendor")
            .join("grammars")
            .join(&g.config.id)
            .join("queries");

        let ext_list: Vec<String> = g
            .config
            .extensions
            .iter()
            .map(|e| format!("\"{e}\""))
            .collect();

        code.push_str("        GrammarDef {\n");
        code.push_str(&format!("            name: \"{}\",\n", g.config.name));
        code.push_str(&format!(
            "            extensions: &[{}],\n",
            ext_list.join(", ")
        ));
        code.push_str(&format!(
            "            language_fn: unsafe {{ tree_sitter_language::LanguageFn::from_raw({} as ParserFn) }},\n",
            g.config.parser_fn,
        ));

        // Highlights — always present (we filtered above)
        code.push_str(&format!(
            "            highlights_query: include_str!(\"{}\"),\n",
            to_forward_slash(&queries_dir.join("highlights.scm")),
        ));

        // Injections — optional
        if g.has_injections {
            code.push_str(&format!(
                "            injections_query: include_str!(\"{}\"),\n",
                to_forward_slash(&queries_dir.join("injections.scm")),
            ));
        } else {
            code.push_str("            injections_query: \"\",\n");
        }

        // Locals — optional
        if g.has_locals {
            code.push_str(&format!(
                "            locals_query: include_str!(\"{}\"),\n",
                to_forward_slash(&queries_dir.join("locals.scm")),
            ));
        } else {
            code.push_str("            locals_query: \"\",\n");
        }

        code.push_str("        },\n");
    }

    code.push_str("    ]\n");
    code.push_str("}\n");

    let out_file = out_dir.join("grammars.rs");
    fs::write(&out_file, &code)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", out_file.display()));
}

// ── Helpers ────────────────────────────────────────────────────────

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("{key} not set"))
}

/// Normalize path to forward slashes for cross-platform `include_str!()`.
fn to_forward_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
