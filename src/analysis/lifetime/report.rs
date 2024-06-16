use crate::analysis::lifetime::process::ShortLivedType;

use crate::analysis::lifetime::utils::{
    decompose_projection_as_str, get_actual_type, get_defid_args_from_kind, get_drop_impl,
    get_name_from_param, FieldInfo,
    MyProjection::{self, MyDeref, MyField},
};

use crate::analysis::lifetime::mirfunc::MirFunc;

use crate::utils::{format_span, format_span_with_diag, print_span};

use rustc_hir::def_id::DefId;
use rustc_hir::LifetimeName;
use rustc_hir::ParamName::Plain;
use rustc_hir::{Param, Ty};

use rustc_middle::ty::TyCtxt;

use rustc_span::{symbol::Symbol, Span};

use comrak::plugins::syntect::SyntectAdapter;
use comrak::{markdown_to_html, markdown_to_html_with_plugins, ComrakOptions, ComrakPlugins};
use std::fs;

pub fn get_string_from_lifetimes(lifetimes: Vec<LifetimeName>) -> String {
    if lifetimes.len() == 0 {
        return "lives for the entire duration that it is owned".to_string();
    }
    let mut lifetime_str = "outlives the lifetime corresponding to ".to_string();

    for lt in lifetimes {
        match lt {
            rustc_hir::LifetimeName::Param(_, Plain(ident)) => {
                lifetime_str.push_str(&format!("`{}`, ", ident.as_str()));
            }
            rustc_hir::LifetimeName::Param(_, Fresh) => {
                lifetime_str.push_str("`'_`, ");
            }
            rustc_hir::LifetimeName::Static => {
                lifetime_str = String::from(
                    "lives for the entire lifetime of the running program (`'static`)",
                );
            }
            _ => {}
        }
    }
    return lifetime_str;
}

pub fn generate_trace(
    ty: &ShortLivedType,
    top_level_id_name: String,
    top_level_type_span: Span,
    tcx: &TyCtxt,
) -> String {
    let mut current_type_span: Span = top_level_type_span;
    let mut current_id_name: String = top_level_id_name;

    let mut trace = String::new();

    let current_type = format_span(*tcx, &current_type_span);
    trace.push_str(&format!(
        "`{current_id_name}` is of type `{current_type}`\n"
    ));

    for proj in ty.projection.iter() {
        match proj {
            MyProjection::MyField(FieldInfo {
                field_num,
                field_name,
                type_span,
                struct_decl_span,
                struct_def_id,
            }) => {
                if let Some(field_name) = &*field_name {
                    if let Some(struct_decl_span) = *struct_decl_span {
                        trace.push_str("```rust\n");
                        let struct_decl: String = format_span(*tcx, &struct_decl_span);
                        trace.push_str(&struct_decl);
                        trace.push_str("\n```\n");
                    }
                    if let Some(struct_def_id) = struct_def_id {
                        let drop_impl_span = get_drop_impl(*struct_def_id, tcx);

                        if let Some(drop_impl_span) = drop_impl_span {
                            trace.push_str(&format!(
                                "`{current_type}` has a custom `Drop` implementation.\n"
                            ));
                            trace.push_str("```rust\n");
                            let drop_impl: String = format_span(*tcx, &drop_impl_span);
                            trace.push_str(&drop_impl);
                            trace.push_str("\n```\n");
                        }
                    }
                    current_type_span = type_span.unwrap();
                    let current_type = format_span(*tcx, &current_type_span);
                    current_id_name.push_str(".");
                    current_id_name.push_str(&field_name);
                    trace.push_str(&format!(
                        "`{current_id_name}` is of type `{current_type}`\n"
                    ));
                    continue;
                }
                current_id_name.push_str(".");
                current_id_name.push_str(&field_num.to_string());
            }
            MyProjection::MyDeref => {
                current_id_name = format!("*({current_id_name})");
            }
        }
    }
    trace
}

pub fn arg_return_uaf_report<'tcx>(
    tcx: &TyCtxt<'tcx>,
    func: &MirFunc<'tcx, '_>,
    inp_num: usize,
    src_ty: &ShortLivedType,
    tgt_ty: &ShortLivedType,
    src_bounding_lt: Vec<LifetimeName>,
    tgt_bounding_lt: Vec<LifetimeName>,
) -> (String, String) {
    let mut human_report: String = String::new();

    human_report.push_str("## Potential use-after-free!\n");
    human_report.push_str(&format_span_with_diag(*tcx, &func.fn_sig.span));
    human_report.push_str("\n");

    let arg_name: Symbol = get_name_from_param(&func.params[inp_num]).unwrap();
    let src_name: String =
        decompose_projection_as_str(&src_ty.projection, arg_name.as_str().to_string());
    let tgt_name: String = decompose_projection_as_str(&tgt_ty.projection, "ret".to_string());

    let src_type_name = format_span(*tcx, &src_ty.type_span);
    let tgt_type_name = format_span(*tcx, &tgt_ty.type_span);

    let src_lifetime_str = get_string_from_lifetimes(src_bounding_lt);
    let tgt_lifetime_str = get_string_from_lifetimes(tgt_bounding_lt);

    human_report.push_str(&format!(
        "`{src_name}` is of type `{src_type_name}` and {src_lifetime_str}\n\n"
    ));
    human_report.push_str(&format!("It is (probably) returned as `{tgt_name}` which is of type `{tgt_type_name}`, and {tgt_lifetime_str}. Here, `ret` denotes the value returned by the function.\n\n"));
    human_report.push_str(
        "The latter can be longer than the former, which could lead to use-after-free!\n\n",
    );

    human_report.push_str("**Detailed report:**\n\n");

    let inp: &Ty = &func.fn_sig.decl.inputs[inp_num];

    let trace = generate_trace(
        &src_ty,
        arg_name.as_str().to_string(),
        get_actual_type(inp, tcx).span,
        tcx,
    );
    human_report.push_str(&trace);
    human_report.push_str("\n");
    if let rustc_hir::FnRetTy::Return(ret_type) = func.fn_sig.decl.output {
        let trace = generate_trace(&tgt_ty, "ret".to_string(), ret_type.span, tcx);
        human_report.push_str(&trace);
        human_report.push_str("\n");
    }

    human_report.push_str("\nHere is the full body of the function:\n\n");
    human_report.push_str("```rust\n");
    human_report.push_str(&format_span(*tcx, &func.fn_sig.span));
    human_report.push_str(&format_span(*tcx, &func.body_span));
    human_report.push_str("\n```\n");

    let adapter = SyntectAdapter::new("base16-ocean.dark");
    let options = ComrakOptions::default();
    let mut plugins = ComrakPlugins::default();

    plugins.render.codefence_syntax_highlighter = Some(&adapter);
    let html_report = markdown_to_html_with_plugins(&human_report, &options, &plugins);
    let markdown_report = human_report.clone();

    (markdown_report, html_report)
}

pub fn arg_return_mut_report<'tcx>(
    tcx: &TyCtxt<'tcx>,
    func: &MirFunc<'tcx, '_>,
    inp_num: usize,
    src_ty: &ShortLivedType,
    tgt_ty: &ShortLivedType,
    src_bounding_lt: Vec<LifetimeName>,
    tgt_bounding_lt: Vec<LifetimeName>,
) -> (String, String) {
    let mut human_report: String = String::new();

    human_report.push_str("## Potential aliased mutability!\n");
    human_report.push_str(&format_span_with_diag(*tcx, &func.fn_sig.span));
    human_report.push_str("\n");

    let arg_name: Symbol = get_name_from_param(&func.params[inp_num]).unwrap();
    let src_name: String =
        decompose_projection_as_str(&src_ty.projection, arg_name.as_str().to_string());
    let tgt_name: String = decompose_projection_as_str(&tgt_ty.projection, "ret".to_string());

    let src_type_name = format_span(*tcx, &src_ty.type_span);
    let tgt_type_name = format_span(*tcx, &tgt_ty.type_span);

    let src_lifetime_str = match src_bounding_lt.first() {
        Some(rustc_hir::LifetimeName::Param(_, Plain(ident))) => {
            format!("`{}`", ident.as_str())
        }
        Some(rustc_hir::LifetimeName::Param(_, Fresh)) => "`'_`".to_string(),
        Some(rustc_hir::LifetimeName::Static) => "`'static`".to_string(),
        _ => "".to_string(),
    };
    let tgt_lifetime_str = get_string_from_lifetimes(tgt_bounding_lt);

    human_report.push_str(&format!(
        "`{src_name}` is of type `{src_type_name}`, and it is behind a `&mut` or `*mut`.\n\n"
    ));
    human_report.push_str(&format!("During the lifetime corresponding to {src_lifetime_str}, there cannot be another mutable borrow (exclusive mutability).\n\n"));
    human_report.push_str(&format!("But this value is (probably) returned as `{tgt_name}` which is of type `{tgt_type_name}`, and {tgt_lifetime_str}. Here, `ret` denotes the value returned by the function.\n\n"));
    human_report.push_str("The latter lifetime can be larger than the former, so we can have non-exclusive mutability!\n\n");

    human_report.push_str("**Detailed report:**\n\n");

    let inp: &Ty = &func.fn_sig.decl.inputs[inp_num];

    let trace = generate_trace(
        &src_ty,
        arg_name.as_str().to_string(),
        get_actual_type(inp, tcx).span,
        tcx,
    );
    human_report.push_str(&trace);
    human_report.push_str("\n");
    if let rustc_hir::FnRetTy::Return(ret_type) = func.fn_sig.decl.output {
        let trace = generate_trace(&tgt_ty, "ret".to_string(), ret_type.span, tcx);
        human_report.push_str(&trace);
        human_report.push_str("\n");
    }
    human_report.push_str("\nHere is the full body of the function:\n\n");
    human_report.push_str("```rust\n");
    human_report.push_str(&format_span(*tcx, &func.fn_sig.span));
    human_report.push_str(&format_span(*tcx, &func.body_span));
    human_report.push_str("\n```\n");

    let adapter = SyntectAdapter::new("base16-ocean.dark");
    let options = ComrakOptions::default();
    let mut plugins = ComrakPlugins::default();

    plugins.render.codefence_syntax_highlighter = Some(&adapter);

    let html_report = markdown_to_html_with_plugins(&human_report, &options, &plugins);
    let markdown_report = human_report.clone();

    (markdown_report, html_report)
}

pub fn arg_arg_uaf_report<'tcx>(
    tcx: &TyCtxt<'tcx>,
    func: &MirFunc<'tcx, '_>,
    inp_num1: usize,
    inp_num2: usize,
    src_ty: &ShortLivedType,
    tgt_ty: &ShortLivedType,
    src_bounding_lt: Vec<LifetimeName>,
    tgt_bounding_lt: Vec<LifetimeName>,
) -> (String, String) {
    let mut human_report: String = String::new();

    human_report.push_str("## Potential use-after-free!\n");
    human_report.push_str(&format_span_with_diag(*tcx, &func.fn_sig.span));
    human_report.push_str("\n");

    let arg_name1: Symbol = get_name_from_param(&func.params[inp_num1]).unwrap();
    let arg_name2: Symbol = get_name_from_param(&func.params[inp_num2]).unwrap();
    let src_name: String =
        decompose_projection_as_str(&src_ty.projection, arg_name1.as_str().to_string());
    let tgt_name: String =
        decompose_projection_as_str(&tgt_ty.projection, arg_name2.as_str().to_string());

    let src_type_name = format_span(*tcx, &src_ty.type_span);
    let tgt_type_name = format_span(*tcx, &tgt_ty.type_span);

    let src_lifetime_str = get_string_from_lifetimes(src_bounding_lt);
    let tgt_lifetime_str = get_string_from_lifetimes(tgt_bounding_lt);

    human_report.push_str(&format!(
        "`{src_name}` is of type `{src_type_name}` and {src_lifetime_str}\n\n"
    ));
    human_report.push_str(&format!("It is (probably) assigned to `{tgt_name}` which is of type `{tgt_type_name}`, and {tgt_lifetime_str}\n\n"));
    human_report.push_str(&format!("This is a potential use-after-free bug!\n\n"));

    human_report.push_str("**Detailed report:**\n\n");

    let inp1: &Ty = &func.fn_sig.decl.inputs[inp_num1];
    let inp2: &Ty = &func.fn_sig.decl.inputs[inp_num2];

    let trace = generate_trace(
        &src_ty,
        arg_name1.as_str().to_string(),
        get_actual_type(inp1, tcx).span,
        tcx,
    );
    human_report.push_str(&trace);
    human_report.push_str("\n");

    let trace = generate_trace(
        &tgt_ty,
        arg_name2.as_str().to_string(),
        get_actual_type(inp2, tcx).span,
        tcx,
    );
    human_report.push_str(&trace);
    human_report.push_str("\n");

    human_report.push_str("\nHere is the full body of the function:\n\n");
    human_report.push_str("```rust\n");
    human_report.push_str(&format_span(*tcx, &func.fn_sig.span));
    human_report.push_str(&format_span(*tcx, &func.body_span));
    human_report.push_str("\n```\n");

    let adapter = SyntectAdapter::new("base16-ocean.dark");
    let options = ComrakOptions::default();
    let mut plugins = ComrakPlugins::default();

    plugins.render.codefence_syntax_highlighter = Some(&adapter);

    let html_report = markdown_to_html_with_plugins(&human_report, &options, &plugins);
    let markdown_report = human_report.clone();

    (markdown_report, html_report)
}

pub fn generate_llm_query<'tcx>(
    tcx: &TyCtxt<'tcx>,
    arg_name: String,
    arg_type_span: Span,
    src_ty: &ShortLivedType,
) -> String {
    let mut report: String = String::new();

    let src_name: String = decompose_projection_as_str(&src_ty.projection, arg_name.clone());
    let src_type_name = format_span(*tcx, &src_ty.type_span);

    report.push_str("Here are some snippets of Rust code from a larger project.\n\n");
    report.push_str(&format!("A function has an argument `{}`\n\n", &arg_name));

    let trace = generate_trace(&src_ty, arg_name.clone(), arg_type_span, tcx);
    report.push_str(&trace);

    report.push_str(&format!("When `{arg_name}` is dropped, does `{src_name}` of type `{src_type_name}` also get dropped?\n"));
    report.push_str("Use the function implementations provided above, as well as comments and other semantic information in the above code, to infer this.\n");
    report.push_str("Your response should be only a single word, 'Yes' or 'No'");

    report
}
