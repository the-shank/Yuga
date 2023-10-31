use crate::analysis::lifetime::process::ShortLivedType;

use crate::analysis::lifetime::utils::{
    get_name_from_param,
    decompose_projection_as_str,
    get_actual_type,
    get_defid_args_from_kind,
    MyProjection::{self, MyDeref, MyField},
    FieldInfo,
};

use crate::analysis::lifetime::mirfunc::MirFunc;

use crate::utils::{print_span, format_span, format_span_with_diag};

use rustc_hir::def_id::DefId;
use rustc_hir::{Param, Ty};
use rustc_hir::ParamName::{Plain};
use rustc_hir::LifetimeName;

use rustc_middle::ty::TyCtxt;

use rustc_span::{Span, symbol::Symbol};

use comrak::{markdown_to_html, markdown_to_html_with_plugins, ComrakOptions, ComrakPlugins};
use comrak::plugins::syntect::SyntectAdapter;
use std::fs;


pub fn get_drop_impl(struct_def_id: DefId, tcx: &TyCtxt) -> Option<Span> {

    let hir_map = tcx.hir();

    for item_id in hir_map.items() {

        let item = hir_map.expect_item(item_id.owner_id.def_id);

        if let rustc_hir::ItemKind::Impl(this_impl) = &item.kind {
            let (impl_def_id, _) = get_defid_args_from_kind(&this_impl.self_ty.kind);

            if impl_def_id == Some(struct_def_id) {

                for impl_item in this_impl.items {

                    if let Some(
                            rustc_hir::Node::ImplItem(
                                rustc_hir::ImplItem{
                                    kind: rustc_hir::ImplItemKind::Fn(_, _),
                                    span,
                                    ..
                                }
                            )
                        ) = hir_map.find(impl_item.id.hir_id())
                    {
                        if impl_item.ident.name.as_str() == "drop" {
                            return Some(*span);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn get_string_from_lifetime(lifetime: Option<LifetimeName>) -> String {

    match lifetime {
        Some(rustc_hir::LifetimeName::Param(_, Plain(ident))) => {
            let ident_name = ident.as_str();
            format!("outlives the lifetime corresponding to `{ident_name}`")
        },
        Some(rustc_hir::LifetimeName::Param(_, Fresh)) => {
            format!("outlives the lifetime corresponding to `'_`")
        },
        Some(rustc_hir::LifetimeName::Static) => {
            format!("lives for the entire lifetime of the running program (`'static`)")
        },
        None => {
            format!("lives for the entire duration that it is owned")
        },
        _ => "".to_string()
    }
}

pub fn generate_trace(ty: &ShortLivedType, top_level_id_name: String, top_level_type_span: Span, tcx: &TyCtxt) -> String {

    let mut current_type_span: Span = top_level_type_span;
    let mut current_id_name: String = top_level_id_name;

    let mut trace = String::new();

    let current_type = format_span(*tcx, &current_type_span);
    trace.push_str(&format!("`{current_id_name}` is of type `{current_type}`\n"));

    for proj in ty.projection.iter() {

        match proj {
             MyProjection::MyField(FieldInfo{field_num,
                                             field_name,
                                             type_span,
                                             struct_decl_span,
                                             struct_def_id
                                        }) =>
             {
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
                            trace.push_str(&format!("`{current_type}` has a custom `Drop` implementation.\n"));
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
                    trace.push_str(&format!("`{current_id_name}` is of type `{current_type}`\n"));
                    continue;
                }
                current_id_name.push_str(".");
                current_id_name.push_str(&field_num.to_string());
            },
            MyProjection::MyDeref => {
                current_id_name = format!("*({current_id_name})");
            }
        }
    }
    trace
}

pub fn generate_report<'tcx>( tcx: &TyCtxt<'tcx>,
                        func: &MirFunc<'tcx, '_>,
                        inp_num: usize,
                        src_ty: &ShortLivedType,
                        tgt_ty: &ShortLivedType,
                        src_bounding_lt: Option<LifetimeName>,
                        tgt_bounding_lt: Option<LifetimeName>
                ) -> String
{
    let mut human_report: String = String::new();

    human_report.push_str("## Potential use-after-free!\n");
    human_report.push_str(&format_span_with_diag(*tcx, &func.fn_sig.span));
    human_report.push_str("\n");

    let arg_name: Symbol = get_name_from_param(&func.params[inp_num]).unwrap();
    let src_name: String = decompose_projection_as_str(&src_ty.projection, arg_name.as_str().to_string());
    let tgt_name: String = decompose_projection_as_str(&tgt_ty.projection, "ret".to_string());

    let src_type_name = format_span(*tcx, &src_ty.type_span);
    let tgt_type_name = format_span(*tcx, &tgt_ty.type_span);

    let src_lifetime_str = get_string_from_lifetime(src_bounding_lt);
    let tgt_lifetime_str = get_string_from_lifetime(tgt_bounding_lt);

    human_report.push_str(&format!("`{src_name}` is of type `{src_type_name}` and {src_lifetime_str}\n\n"));
    human_report.push_str(&format!("It is (probably) returned as `{tgt_name}` which is of type `{tgt_type_name}`, and {tgt_lifetime_str}\n\n"));
    human_report.push_str(&format!("This is a potential use-after-free bug!\n\n"));

    human_report.push_str("**Detailed report:**\n\n");

    let inp: &Ty = &func.fn_sig.decl.inputs[inp_num];

    let trace = generate_trace(&src_ty, arg_name.as_str().to_string(), get_actual_type(inp, tcx).span, tcx);
    human_report.push_str(&trace);
    human_report.push_str("\n");
    if let rustc_hir::FnRetTy::Return(ret_type) = func.fn_sig.decl.output {
        let trace = generate_trace(&tgt_ty, "ret".to_string(), ret_type.span, tcx);
        human_report.push_str(&trace);
        human_report.push_str("\n");
    }

    let adapter = SyntectAdapter::new("base16-ocean.dark");
    let options = ComrakOptions::default();
    let mut plugins = ComrakPlugins::default();

    plugins.render.codefence_syntax_highlighter = Some(&adapter);

    markdown_to_html_with_plugins(&human_report, &options, &plugins)
}
