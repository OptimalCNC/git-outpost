//! Source ownership, not executable-line measurement. LLVM supplies the latter.
//!
//! Positions use one-based lines and zero-based UTF-8 character columns, as
//! supplied by proc_macro2. End positions are exclusive. Syntax that could move
//! source ownership beyond this parser's understanding is rejected explicitly.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use proc_macro2::{Delimiter, LineColumn, Span, TokenStream, TokenTree};
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::parse::discouraged::Speculative;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, Meta};

#[derive(Clone, Debug)]
struct OwnedRange {
    start: LineColumn,
    end: LineColumn,
    reason: String,
}

impl OwnedRange {
    fn contains(&self, start: LineColumn, end: LineColumn) -> bool {
        self.start <= start && end <= self.end
    }

    fn json(&self) -> Value {
        json!({
            "start_line": self.start.line,
            "start_column": self.start.column,
            "end_line": self.end.line,
            "end_column": self.end.column,
            "reason": self.reason,
        })
    }
}

#[derive(Default)]
struct Ownership {
    ranges: Vec<OwnedRange>,
    errors: Vec<String>,
    enum_globs: Vec<ModuleEnumGlobs>,
}

struct ModuleEnumGlobs {
    unconditional: bool,
    names: BTreeSet<String>,
}

impl ModuleEnumGlobs {
    fn new(items: &[syn::Item], unconditional: bool) -> Self {
        let mut bindings = BindingNames::default();
        for item in items {
            bindings.visit_item(item);
        }
        let names = items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Enum(node) if unconditional && unconditional_attrs(&node.attrs) => {
                    let name = ident_name(&node.ident);
                    (bindings.counts.get(&name) == Some(&1)).then_some(name)
                }
                _ => None,
            })
            .collect();
        Self {
            unconditional,
            names,
        }
    }
}

// This deliberately stronger-than-resolution proof avoids a symbol resolver:
// an enum glob is accepted only when no other type/import binding anywhere in
// its module subtree could redirect its bare path, including conditional code.
#[derive(Default)]
struct BindingNames {
    counts: BTreeMap<String, usize>,
}

impl BindingNames {
    fn bind(&mut self, ident: &syn::Ident) {
        *self.counts.entry(ident_name(ident)).or_default() += 1;
    }

    fn imports(&mut self, tree: &syn::UseTree, prefix: Option<&syn::Ident>) {
        match tree {
            syn::UseTree::Path(path) => self.imports(&path.tree, Some(&path.ident)),
            syn::UseTree::Name(name) if name.ident == "self" => {
                if let Some(ident) = prefix {
                    self.bind(ident);
                }
            }
            syn::UseTree::Name(name) => self.bind(&name.ident),
            syn::UseTree::Rename(rename) => self.bind(&rename.rename),
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    self.imports(tree, prefix);
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }
}

impl<'ast> Visit<'ast> for BindingNames {
    fn visit_item(&mut self, node: &'ast syn::Item) {
        match node {
            syn::Item::Enum(node) => self.bind(&node.ident),
            syn::Item::ExternCrate(node) => self.bind(
                node.rename
                    .as_ref()
                    .map(|(_, ident)| ident)
                    .unwrap_or(&node.ident),
            ),
            syn::Item::Mod(node) => self.bind(&node.ident),
            syn::Item::Struct(node) => self.bind(&node.ident),
            syn::Item::Trait(node) => self.bind(&node.ident),
            syn::Item::TraitAlias(node) => self.bind(&node.ident),
            syn::Item::Type(node) => self.bind(&node.ident),
            syn::Item::Union(node) => self.bind(&node.ident),
            syn::Item::Use(node) => self.imports(&node.tree, None),
            _ => {}
        }
        visit::visit_item(self, node);
    }

    fn visit_type_param(&mut self, node: &'ast syn::TypeParam) {
        self.bind(&node.ident);
        visit::visit_type_param(self, node);
    }

    fn visit_variant(&mut self, node: &'ast syn::Variant) {
        self.bind(&node.ident);
        visit::visit_variant(self, node);
    }

    fn visit_foreign_item_type(&mut self, node: &'ast syn::ForeignItemType) {
        self.bind(&node.ident);
        visit::visit_foreign_item_type(self, node);
    }

    fn visit_impl_item_type(&mut self, node: &'ast syn::ImplItemType) {
        self.bind(&node.ident);
        visit::visit_impl_item_type(self, node);
    }

    fn visit_trait_item_type(&mut self, node: &'ast syn::TraitItemType) {
        self.bind(&node.ident);
        visit::visit_trait_item_type(self, node);
    }
}

fn unconditional_attrs(attrs: &[Attribute]) -> bool {
    !attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            || attr.path().is_ident("cfg_attr")
            || attr.path().is_ident("test")
    })
}

impl Ownership {
    fn error(&mut self, span: Span, message: impl Into<String>) {
        let start = span.start();
        self.errors.push(format!(
            "{}:{}: {}",
            start.line,
            start.column,
            message.into()
        ));
    }

    fn owned(&self, span: Span) -> bool {
        self.ranges
            .iter()
            .any(|range| range.contains(span.start(), span.end()))
    }

    fn attributes(&mut self, attrs: &[Attribute], span: Span) {
        for attr in attrs {
            match test_attribute(attr) {
                Ok(Some(reason)) => self.ranges.push(OwnedRange {
                    start: span.start(),
                    end: span.end(),
                    reason,
                }),
                Ok(None) => {}
                Err(error) => self.error(attr.span(), error),
            }
        }
    }

    // A separator belongs to the attributed list element it follows. Leaving it
    // production-owned would falsely classify an ordinary cfg field as mixed.
    fn separators<T: Spanned, P: Spanned>(&mut self, items: &Punctuated<T, P>) {
        for pair in items.pairs() {
            if let Some(punctuation) = pair.punct() {
                self.separator(pair.value().span(), punctuation.span());
            }
        }
    }

    fn separator(&mut self, owner: Span, punctuation: Span) {
        if self.owned(owner) && !self.owned(punctuation) {
            self.ranges.push(OwnedRange {
                start: owner.start(),
                end: punctuation.end(),
                reason: "separator of classified test-only syntax".into(),
            });
        }
    }

    fn namespace_binding(&mut self, ident: &syn::Ident, span: Span) {
        if !self.owned(span) && reviewed_namespace(&ident_name(ident)) {
            self.error(
                span,
                "binding shadows a reviewed macro namespace; explicit ownership review required",
            );
        }
    }

    fn import_tree(&mut self, tree: &syn::UseTree, prefix: &[String], local_path: bool) {
        match tree {
            syn::UseTree::Path(path) => {
                let mut prefix = prefix.to_vec();
                prefix.push(ident_name(&path.ident));
                self.import_tree(&path.tree, &prefix, local_path);
            }
            syn::UseTree::Name(name) => {
                let name = ident_name(&name.ident);
                let mut path = prefix.to_vec();
                let binding = if name == "self" {
                    prefix.last().cloned().unwrap_or_default()
                } else {
                    path.push(name.clone());
                    name
                };
                self.import_binding(&binding, &path, tree.span());
            }
            syn::UseTree::Rename(rename) => {
                let mut path = prefix.to_vec();
                if rename.ident != "self" {
                    path.push(ident_name(&rename.ident));
                }
                self.import_binding(&ident_name(&rename.rename), &path, tree.span());
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.import_tree(item, prefix, local_path);
                }
            }
            syn::UseTree::Glob(_) => {
                // Enum globs import variants, never macros. Only a bare path to
                // the module's proven unconditional enum is understood here.
                let proven = local_path
                    && prefix.len() == 1
                    && self
                        .enum_globs
                        .last()
                        .is_some_and(|scope| scope.names.contains(&prefix[0]));
                if !proven {
                    self.error(tree.span(),
                        "production glob import is not a proven local enum; explicit ownership review required");
                }
            }
        }
    }

    fn import_binding(&mut self, binding: &str, path: &[String], span: Span) {
        let namespace = reviewed_namespace(binding);
        let macro_name = standard_macro_name(binding);
        let derive = reviewed_derive_name(binding);
        if !(namespace || macro_name || derive) {
            return;
        }
        let root = path.first().map(String::as_str).unwrap_or_default();
        let terminal = path.last().map(String::as_str).unwrap_or_default();
        let known = if namespace {
            path.len() == 1 && root == binding
        } else if macro_name {
            matches!(root, "std" | "core" | "alloc") && terminal == binding
        } else {
            match binding {
                "Serialize" | "Deserialize" => {
                    path.len() == 2 && root == "serde" && terminal == binding
                }
                "Error" => {
                    path.len() == 2 && root == "thiserror" && terminal == binding
                        || matches!(root, "std" | "core" | "alloc") && terminal == binding
                }
                _ => matches!(root, "std" | "core" | "alloc") && terminal == binding,
            }
        };
        if !known {
            self.error(span, format!("imported binding {binding} can shadow reviewed macro ownership; explicit review required"));
        }
    }
}

// Each method claims the entire attributed syntax node before visiting children.
// This makes ownership inherited without losing validation of nested syntax.
macro_rules! attributed_visitors {
    ($($method:ident: $node:ident),* $(,)?) => {
        $(fn $method(&mut self, node: &'ast syn::$node) {
            self.attributes(&node.attrs, node.span());
            visit::$method(self, node);
        })*
    };
}

macro_rules! attributed_list_visitors {
    ($($method:ident: $node:ident => $field:ident),* $(,)?) => {
        $(fn $method(&mut self, node: &'ast syn::$node) {
            self.attributes(&node.attrs, node.span());
            visit::$method(self, node);
            self.separators(&node.$field);
        })*
    };
}

macro_rules! namespace_visitors {
    ($($method:ident: $node:ident),* $(,)?) => {
        $(fn $method(&mut self, node: &'ast syn::$node) {
            self.namespace_binding(&node.ident, node.span());
            self.attributes(&node.attrs, node.span());
            visit::$method(self, node);
        })*
    };
}

impl<'ast> Visit<'ast> for Ownership {
    attributed_visitors! {
        visit_arm: Arm,
        visit_bare_fn_arg: BareFnArg,
        visit_bare_variadic: BareVariadic,
        visit_const_param: ConstParam,
        visit_expr_assign: ExprAssign,
        visit_expr_async: ExprAsync,
        visit_expr_await: ExprAwait,
        visit_expr_binary: ExprBinary,
        visit_expr_block: ExprBlock,
        visit_expr_break: ExprBreak,
        visit_expr_cast: ExprCast,
        visit_expr_closure: ExprClosure,
        visit_expr_const: ExprConst,
        visit_expr_continue: ExprContinue,
        visit_expr_field: ExprField,
        visit_expr_for_loop: ExprForLoop,
        visit_expr_group: ExprGroup,
        visit_expr_if: ExprIf,
        visit_expr_index: ExprIndex,
        visit_expr_infer: ExprInfer,
        visit_expr_let: ExprLet,
        visit_expr_lit: ExprLit,
        visit_expr_loop: ExprLoop,
        visit_expr_macro: ExprMacro,
        visit_expr_match: ExprMatch,
        visit_expr_paren: ExprParen,
        visit_expr_path: ExprPath,
        visit_expr_range: ExprRange,
        visit_expr_raw_addr: ExprRawAddr,
        visit_expr_reference: ExprReference,
        visit_expr_repeat: ExprRepeat,
        visit_expr_return: ExprReturn,
        visit_expr_try: ExprTry,
        visit_expr_try_block: ExprTryBlock,
        visit_expr_unary: ExprUnary,
        visit_expr_unsafe: ExprUnsafe,
        visit_expr_while: ExprWhile,
        visit_expr_yield: ExprYield,
        visit_field: Field,
        visit_field_pat: FieldPat,
        visit_field_value: FieldValue,
        visit_foreign_item_fn: ForeignItemFn,
        visit_foreign_item_static: ForeignItemStatic,
        visit_impl_item_const: ImplItemConst,
        visit_impl_item_fn: ImplItemFn,
        visit_item_const: ItemConst,
        visit_item_fn: ItemFn,
        visit_item_foreign_mod: ItemForeignMod,
        visit_item_impl: ItemImpl,
        visit_item_static: ItemStatic,
        visit_lifetime_param: LifetimeParam,
        visit_local: Local,
        visit_pat_ident: PatIdent,
        visit_pat_or: PatOr,
        visit_pat_paren: PatParen,
        visit_pat_reference: PatReference,
        visit_pat_rest: PatRest,
        visit_pat_type: PatType,
        visit_pat_wild: PatWild,
        visit_receiver: Receiver,
        visit_stmt_macro: StmtMacro,
        visit_trait_item_const: TraitItemConst,
        visit_trait_item_fn: TraitItemFn,
        visit_variadic: Variadic,
        visit_variant: Variant,
    }

    attributed_list_visitors! {
        visit_expr_array: ExprArray => elems,
        visit_expr_call: ExprCall => args,
        visit_expr_method_call: ExprMethodCall => args,
        visit_expr_struct: ExprStruct => fields,
        visit_expr_tuple: ExprTuple => elems,
        visit_pat_slice: PatSlice => elems,
        visit_pat_struct: PatStruct => fields,
        visit_pat_tuple: PatTuple => elems,
        visit_pat_tuple_struct: PatTupleStruct => elems,
    }

    namespace_visitors! {
        visit_foreign_item_type: ForeignItemType,
        visit_impl_item_type: ImplItemType,
        visit_item_struct: ItemStruct,
        visit_item_trait: ItemTrait,
        visit_item_trait_alias: ItemTraitAlias,
        visit_item_type: ItemType,
        visit_item_union: ItemUnion,
        visit_trait_item_type: TraitItemType,
        visit_type_param: TypeParam,
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.namespace_binding(&node.ident, node.span());
        self.attributes(&node.attrs, node.span());
        visit::visit_item_enum(self, node);
        self.separators(&node.variants);
    }

    fn visit_item_extern_crate(&mut self, node: &'ast syn::ItemExternCrate) {
        let binding = node
            .rename
            .as_ref()
            .map(|(_, ident)| ident)
            .unwrap_or(&node.ident);
        self.namespace_binding(binding, node.span());
        self.attributes(&node.attrs, node.span());
        visit::visit_item_extern_crate(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        // An attributed import in a production scope can change the macro
        // bindings of sibling production code in the test build. Only inherited
        // ownership of the containing scope makes a test-only glob harmless.
        if !self.owned(node.span()) {
            self.import_tree(&node.tree, &[], node.leading_colon.is_none());
        }
        self.attributes(&node.attrs, node.span());
        visit::visit_item_use(self, node);
    }

    fn visit_file(&mut self, node: &'ast syn::File) {
        self.attributes(&node.attrs, node.span());
        self.enum_globs.push(ModuleEnumGlobs::new(
            &node.items,
            unconditional_attrs(&node.attrs),
        ));
        visit::visit_file(self, node);
        self.enum_globs.pop();
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        self.namespace_binding(&node.ident, node.span());
        self.attributes(&node.attrs, node.span());
        if node.content.is_none() && self.owned(node.span()) {
            self.error(node.span(), "test-only external modules require inherited file ownership; move ownership into the file or extend the classifier explicitly");
        }
        if let Some((_, items)) = &node.content {
            let unconditional = unconditional_attrs(&node.attrs)
                && self
                    .enum_globs
                    .last()
                    .is_some_and(|scope| scope.unconditional);
            self.enum_globs
                .push(ModuleEnumGlobs::new(items, unconditional));
            visit::visit_item_mod(self, node);
            self.enum_globs.pop();
            return;
        }
        visit::visit_item_mod(self, node);
    }

    fn visit_fields_named(&mut self, node: &'ast syn::FieldsNamed) {
        visit::visit_fields_named(self, node);
        self.separators(&node.named);
    }

    fn visit_fields_unnamed(&mut self, node: &'ast syn::FieldsUnnamed) {
        visit::visit_fields_unnamed(self, node);
        self.separators(&node.unnamed);
    }

    fn visit_signature(&mut self, node: &'ast syn::Signature) {
        visit::visit_signature(self, node);
        self.separators(&node.inputs);
    }

    fn visit_generics(&mut self, node: &'ast syn::Generics) {
        visit::visit_generics(self, node);
        self.separators(&node.params);
    }

    fn visit_stmt(&mut self, node: &'ast syn::Stmt) {
        visit::visit_stmt(self, node);
        if let syn::Stmt::Expr(expr, Some(semi)) = node {
            self.separator(expr.span(), semi.span());
        }
    }

    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        match test_attribute(attr) {
            Ok(Some(_)) if !self.owned(attr.span()) => {
                self.error(attr.span(), "test attribute has no classified syntax owner");
            }
            Err(error) => self.error(attr.span(), error),
            _ => {}
        }
        // Attribute arguments are metadata, not executable Rust syntax.
    }

    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        self.error(
            node.span(),
            "item macros and macro definitions require explicit source ownership support",
        );
    }

    fn visit_impl_item_macro(&mut self, node: &'ast syn::ImplItemMacro) {
        self.error(
            node.span(),
            "associated item macros require explicit source ownership support",
        );
    }

    fn visit_trait_item_macro(&mut self, node: &'ast syn::TraitItemMacro) {
        self.error(
            node.span(),
            "trait item macros require explicit source ownership support",
        );
    }

    fn visit_foreign_item_macro(&mut self, node: &'ast syn::ForeignItemMacro) {
        self.error(
            node.span(),
            "foreign item macros require explicit source ownership support",
        );
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Err(error) = macro_ownership(&node.path, &node.tokens) {
            self.error(node.span(), error);
        }
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        if matches!(node, Expr::Verbatim(_)) {
            self.error(
                node.span(),
                "unparsed expression syntax cannot be classified",
            );
        }
        visit::visit_expr(self, node);
    }

    fn visit_item(&mut self, node: &'ast syn::Item) {
        if matches!(node, syn::Item::Verbatim(_)) {
            self.error(node.span(), "unparsed item syntax cannot be classified");
        }
        visit::visit_item(self, node);
    }

    fn visit_pat(&mut self, node: &'ast syn::Pat) {
        if matches!(node, syn::Pat::Verbatim(_)) {
            self.error(node.span(), "unparsed pattern syntax cannot be classified");
        }
        visit::visit_pat(self, node);
    }

    fn visit_type(&mut self, node: &'ast syn::Type) {
        if matches!(node, syn::Type::Verbatim(_)) {
            self.error(node.span(), "unparsed type syntax cannot be classified");
        }
        visit::visit_type(self, node);
    }

    fn visit_impl_item(&mut self, node: &'ast syn::ImplItem) {
        if matches!(node, syn::ImplItem::Verbatim(_)) {
            self.error(
                node.span(),
                "unparsed associated item syntax cannot be classified",
            );
        }
        visit::visit_impl_item(self, node);
    }

    fn visit_trait_item(&mut self, node: &'ast syn::TraitItem) {
        if matches!(node, syn::TraitItem::Verbatim(_)) {
            self.error(
                node.span(),
                "unparsed trait item syntax cannot be classified",
            );
        }
        visit::visit_trait_item(self, node);
    }

    fn visit_foreign_item(&mut self, node: &'ast syn::ForeignItem) {
        if matches!(node, syn::ForeignItem::Verbatim(_)) {
            self.error(
                node.span(),
                "unparsed foreign item syntax cannot be classified",
            );
        }
        visit::visit_foreign_item(self, node);
    }
}

fn ident_name(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}

fn reviewed_namespace(name: &str) -> bool {
    matches!(name, "std" | "core" | "alloc" | "serde" | "thiserror")
}

fn reviewed_derive_name(name: &str) -> bool {
    matches!(
        name,
        "Clone"
            | "Copy"
            | "Debug"
            | "Default"
            | "Eq"
            | "PartialEq"
            | "Ord"
            | "PartialOrd"
            | "Hash"
            | "Serialize"
            | "Deserialize"
            | "Error"
    )
}

fn standard_macro(path: &syn::Path) -> bool {
    let parts = path
        .segments
        .iter()
        .map(|segment| ident_name(&segment.ident))
        .collect::<Vec<_>>();
    let name = match parts.as_slice() {
        [name] => name.as_str(),
        [root, name] if matches!(root.as_str(), "std" | "core" | "alloc") => name.as_str(),
        _ => return false,
    };
    standard_macro_name(name)
}

fn standard_macro_name(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "debug_assert"
            | "debug_assert_eq"
            | "debug_assert_ne"
            | "matches"
            | "format"
            | "format_args"
            | "write"
            | "writeln"
            | "print"
            | "println"
            | "eprint"
            | "eprintln"
            | "panic"
            | "vec"
            | "concat"
            | "env"
            | "option_env"
            | "include_str"
            | "include_bytes"
            | "stringify"
            | "line"
            | "column"
            | "file"
            | "cfg"
            | "todo"
            | "unimplemented"
            | "unreachable"
            | "dbg"
            | "compile_error"
    )
}

fn meta_list(meta: &syn::MetaList) -> Result<Vec<Meta>, String> {
    meta.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map(|items| items.into_iter().collect())
        .map_err(|error| format!("malformed cfg predicate: {error}"))
}

fn test_feature(meta: &Meta) -> bool {
    match meta {
        Meta::NameValue(value) if value.path.is_ident("feature") => {
            matches!(&value.value, Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(value) if value.value() == "test-helpers"))
        }
        _ => false,
    }
}

fn references_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::NameValue(value) => value.path.is_ident("test") || test_feature(meta),
        Meta::List(list) => {
            list.path.is_ident("test")
                || meta_list(list)
                    .map(|items| items.iter().any(references_test))
                    .unwrap_or(true)
        }
    }
}

fn test_attribute(attr: &Attribute) -> Result<Option<String>, String> {
    let path = attr.path();
    if path.is_ident("test") {
        return Ok(Some("#[test] implementation".into()));
    }
    if path.is_ident("cfg") {
        let predicate: Meta = attr
            .parse_args()
            .map_err(|error| format!("malformed cfg: {error}"))?;
        if matches!(&predicate, Meta::Path(path) if path.is_ident("test")) {
            return Ok(Some("#[cfg(test)] source".into()));
        }
        if test_feature(&predicate) {
            return Ok(Some(
                "#[cfg(feature = \"test-helpers\")] instrumentation".into(),
            ));
        }
        if let Meta::List(list) = &predicate {
            let items = meta_list(list)?;
            if list.path.is_ident("any")
                && items.len() == 2
                && items
                    .iter()
                    .any(|item| matches!(item, Meta::Path(path) if path.is_ident("test")))
                && items.iter().any(test_feature)
            {
                return Ok(Some(
                    "#[cfg(any(test, feature = \"test-helpers\"))] instrumentation".into(),
                ));
            }
        }
        if references_test(&predicate) {
            return Err("unsupported cfg predicate involving test or test-helpers".into());
        }
        return Ok(None);
    }
    if path.is_ident("derive") {
        let derives = attr
            .parse_args_with(Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
            .map_err(|error| format!("malformed derive: {error}"))?;
        for derive in derives {
            let name = derive
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            if !matches!(
                name.as_str(),
                "Clone"
                    | "Copy"
                    | "Debug"
                    | "Default"
                    | "Eq"
                    | "PartialEq"
                    | "Ord"
                    | "PartialOrd"
                    | "Hash"
                    | "Serialize"
                    | "Deserialize"
                    | "serde::Serialize"
                    | "serde::Deserialize"
                    | "Error"
                    | "thiserror::Error"
            ) {
                return Err(format!(
                    "derive {name} requires explicit generated-source ownership review"
                ));
            }
        }
        return Ok(None);
    }
    // Existing derive-helper attributes and compiler metadata do not change
    // ownership. New procedural attributes must be reviewed rather than guessed.
    if path.get_ident().is_some_and(|name| {
        matches!(
            name.to_string().as_str(),
            "doc"
                | "allow"
                | "warn"
                | "deny"
                | "forbid"
                | "expect"
                | "must_use"
                | "deprecated"
                | "inline"
                | "cold"
                | "repr"
                | "non_exhaustive"
                | "track_caller"
                | "should_panic"
                | "ignore"
                | "serde"
                | "error"
                | "from"
                | "source"
                | "backtrace"
        )
    }) {
        return Ok(None);
    }
    Err(format!(
        "attribute {} requires explicit source ownership support",
        path.segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::")
    ))
}

fn macro_ownership(path: &syn::Path, tokens: &TokenStream) -> Result<(), String> {
    let name = path
        .segments
        .last()
        .map(|segment| ident_name(&segment.ident))
        .unwrap_or_default();
    if matches!(name.as_str(), "include" | "macro_rules") {
        return Err("include! and macro definitions can introduce unclassified source".into());
    }
    if !standard_macro(path) {
        return Err("macro expansion requires explicit source ownership review".into());
    }
    if name == "cfg" {
        match syn::parse2::<Meta>(tokens.clone()) {
            Ok(meta) if references_test(&meta) => {
                return Err(
                    "cfg! involving test ownership changes production behavior between builds"
                        .into(),
                );
            }
            Err(error) => return Err(format!("malformed cfg!: {error}")),
            _ => {}
        }
    }
    // stringify! consumes literal token input without expanding nested macros or
    // creating the declarations spelled within it, even inside another macro.
    if name == "stringify" {
        return Ok(());
    }
    opaque_ownership(tokens)
}

fn opaque_ownership(stream: &TokenStream) -> Result<(), String> {
    // syn does not visit macro arguments. Use its macro/path parser to preserve
    // the complete qualified name rather than trusting the final identifier.
    // Other tokens need no expression parser: only groups and ownership-changing
    // constructs matter here. Declarations require explicit AST ownership review.
    let inspect = |input: syn::parse::ParseStream<'_>| -> syn::Result<()> {
        while !input.is_empty() {
            if input.peek(syn::Token![#]) {
                return Err(input.error("opaque macro attributes require source ownership review"));
            }
            if input.peek(syn::Token![use])
                || input.peek(syn::Token![extern])
                || input.peek(syn::Token![mod])
                || input.peek(syn::Token![type])
                || input.peek(syn::Token![struct])
                || input.peek(syn::Token![enum])
                || input.peek(syn::Token![union])
                || input.peek(syn::Token![trait])
                || input.peek(syn::Token![impl])
                || input.peek(syn::Token![fn])
                || input.peek(syn::Token![static])
                || input.peek(syn::Token![const])
                || input.peek(syn::Token![macro])
            {
                return Err(input.error(
                    "opaque macro declarations or imports require source ownership review",
                ));
            }
            let fork = input.fork();
            if let Ok(node) = fork.parse::<syn::Macro>() {
                input.advance_to(&fork);
                macro_ownership(&node.path, &node.tokens)
                    .map_err(|error| syn::Error::new(node.span(), error))?;
            } else {
                match input.parse::<TokenTree>()? {
                    TokenTree::Group(group) => opaque_ownership(&group.stream())
                        .map_err(|error| syn::Error::new(group.span(), error))?,
                    TokenTree::Ident(ident) if ident_name(&ident) == "macro_rules" => {
                        return Err(syn::Error::new(
                            ident.span(),
                            "opaque macro definitions require source ownership review",
                        ));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    };
    inspect
        .parse2(stream.clone())
        .map_err(|error| error.to_string())
}

fn token_spans(stream: TokenStream, spans: &mut Vec<Span>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => {
                if group.delimiter() != Delimiter::None {
                    spans.push(group.span_open());
                    spans.push(group.span_close());
                }
                token_spans(group.stream(), spans);
            }
            token => spans.push(token.span()),
        }
    }
}

fn span_lines(start: LineColumn, end: LineColumn) -> std::ops::RangeInclusive<usize> {
    start.line..=if end.column == 0 && end.line > start.line {
        end.line - 1
    } else {
        end.line
    }
}

pub fn classify_source(path: &str, source: &str) -> Result<Value, String> {
    let syntax = syn::parse_file(source).map_err(|error| format!("{path}: {error}"))?;
    let tokens = TokenStream::from_str(source).map_err(|error| format!("{path}: {error}"))?;
    let mut ownership = Ownership::default();
    ownership.visit_file(&syntax);
    if !ownership.errors.is_empty() {
        return Err(format!("{path}: {}", ownership.errors.join("; ")));
    }
    ownership
        .ranges
        .sort_by_key(|range| (range.start, range.end));
    ownership
        .ranges
        .dedup_by(|a, b| a.start == b.start && a.end == b.end);
    let mut spans = Vec::new();
    token_spans(tokens, &mut spans);
    let mut production = BTreeSet::new();
    let mut nonproduction_tokens = BTreeSet::new();
    for span in spans {
        let start = span.start();
        let end = span.end();
        if start.line == 0 || end.line == 0 || start >= end {
            return Err(format!("{path}: missing or invalid token positions"));
        }
        let owned = ownership
            .ranges
            .iter()
            .any(|range| range.contains(start, end));
        if !owned
            && ownership
                .ranges
                .iter()
                .any(|range| start < range.end && range.start < end)
        {
            return Err(format!(
                "{path}: token overlaps a source ownership boundary at {}:{}",
                start.line, start.column
            ));
        }
        if owned {
            nonproduction_tokens.extend(span_lines(start, end));
        } else {
            production.extend(span_lines(start, end));
        }
    }
    let nonproduction_lines: BTreeSet<_> = ownership
        .ranges
        .iter()
        .flat_map(|range| span_lines(range.start, range.end))
        .filter(|line| !production.contains(line))
        .collect();
    Ok(json!({
        "path": path,
        "nonproduction": ownership.ranges.iter().map(OwnedRange::json).collect::<Vec<_>>(),
        "production_token_lines": production.into_iter().collect::<Vec<_>>(),
        "nonproduction_token_lines": nonproduction_tokens.into_iter().collect::<Vec<_>>(),
        "nonproduction_lines": nonproduction_lines.into_iter().collect::<Vec<_>>(),
    }))
}

fn source_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_symlink() {
            return Err(format!(
                "{}: symlinks require explicit source ownership support",
                path.display()
            ));
        }
        if kind.is_dir() {
            source_files(&path, files)?;
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        } else if !kind.is_file() {
            return Err(format!(
                "{}: source tree contains a non-regular entry",
                path.display()
            ));
        }
    }
    Ok(())
}

pub fn classify_tree(repo_root: &Path, source_dir: &Path) -> Result<Value, String> {
    if source_dir.as_os_str().is_empty()
        || source_dir
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("source directory must be a nonempty relative path without traversal".into());
    }
    let repo_root = repo_root
        .canonicalize()
        .map_err(|error| format!("repository root: {error}"))?;
    let mut directory = repo_root.clone();
    for component in source_dir.components() {
        directory.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "{}: source directory must contain only ordinary directories",
                directory.display()
            ));
        }
    }
    let mut paths = Vec::new();
    source_files(&directory, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return Err("source directory contains no Rust source files".into());
    }
    let mut files = Vec::new();
    for path in paths {
        let relative = path
            .strip_prefix(&repo_root)
            .map_err(|error| error.to_string())?;
        let name = relative
            .to_str()
            .ok_or_else(|| format!("{}: source path must be UTF-8", path.display()))?;
        let source = std::fs::read_to_string(&path).map_err(|error| format!("{name}: {error}"))?;
        files.push(classify_source(name, &source)?);
    }
    Ok(json!({"schema": 1, "files": files}))
}
