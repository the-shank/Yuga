#![allow(unused_parens)]

pub mod utils;
pub mod process;
pub mod iterators;
pub mod config;
mod report;
mod checks;
mod taint;
mod alias;
mod mirfunc;

use utils::{
    get_mir_value_from_hir_param,
    compare_lifetimes,
    get_first_field,
    get_type_definition,
};

use report::generate_report;

use crate::utils::{print_span, format_span, format_span_with_diag};

use process::{
    get_sub_types,
    get_sub_types_dbg,
    get_implicit_lifetime_bounds,
    MyLifetime
};

use taint::TaintAnalyzer;
use alias::AliasAnalyzer;
use mirfunc::MirFunc;
use iterators::fn_iter;

use crate::progress_info;

use rustc_hir::{def_id::DefId, BodyId, Param, Ty, Mutability, FnSig};
use rustc_hir::LifetimeName;
use rustc_hir::ParamName::{Plain, Fresh};

use rustc_middle::mir::{Place, Local, Body};
use rustc_middle::hir::map::Map;
use rustc_middle::ty::TyCtxt;

use rustc_span::{Span, symbol::Symbol};

use snafu::{Backtrace, Snafu};
use termcolor::Color;

use std::boxed::Box;
use std::convert::TryInto;
use std::clone;
use std::collections::{HashMap, HashSet};
use std::fs;

// use rand::{distributions::Alphanumeric, Rng}; // 0.8

pub struct Report {
    html: String,
    func_name: String,
    error_type: String,
}

pub struct LifetimeChecker<'tcx> {
    tcx: &'tcx TyCtxt<'tcx>
}

impl<'tcx> LifetimeChecker<'tcx> {

    pub fn new(tcx: &'tcx TyCtxt<'tcx>) -> Self {
        LifetimeChecker { tcx: tcx }
    }

    fn debug_output<'a>(&self, func: &MirFunc<'tcx, 'a>) {

        for (inp_num1, inp1) in func.fn_sig.decl.inputs.iter().enumerate() {

            let subtypes = get_sub_types_dbg(inp1,
                                            &func.generic_bounds,
                                            self.tcx,
                                            Vec::new(),
                                            HashMap::new(),
                                            HashMap::new(),
                                            false
                                        );

            println!("Arg {} Data lifetimes: {:#?}", inp_num1, subtypes);

            let bounds = get_implicit_lifetime_bounds(inp1, &func.generic_bounds, self.tcx);

            println!("Arg {} Implicit bounds: {:#?}", inp_num1, bounds);
        }

        if let rustc_hir::FnRetTy::Return(ret_type) = func.fn_sig.decl.output {

            let subtypes = get_sub_types_dbg(ret_type,
                                            &func.generic_bounds,
                                            self.tcx,
                                            Vec::new(),
                                            HashMap::new(),
                                            HashMap::new(),
                                            false
                                        );

            let bounds = get_implicit_lifetime_bounds(ret_type, &func.generic_bounds, self.tcx);
            println!("Return type data lifetimes: {:#?}", subtypes);
            println!("Return type implicit bounds: {:#?}", bounds);
        }
    }

    fn check_arg_return_violations<'a>(&self, func: &MirFunc<'tcx, 'a>) -> Option<Report> {

        let mut taint_analyzer = TaintAnalyzer::new(&func.mir_body);
        let mut alias_analyzer = AliasAnalyzer::new(&func.mir_body);

        if let rustc_hir::FnRetTy::Return(ret_type) = func.fn_sig.decl.output {

            let ret_subtypes = get_sub_types(ret_type, &func.generic_bounds, self.tcx);
            // We shouldn't get implicit bounds from the return type, because
            // that is what we're *creating*. We can only assume bounds based on what is *given to us*,
            // namely the arguments.

            for (inp_num, inp) in func.fn_sig.decl.inputs.iter().enumerate() {

                let inp_subtypes    = get_sub_types(inp, &func.generic_bounds, self.tcx);
                let mut bounds      = get_implicit_lifetime_bounds(inp, &func.generic_bounds, self.tcx);

                bounds.append(&mut func.lifetime_bounds.clone());

                for tgt_ty in ret_subtypes.iter() {
                    for src_ty in inp_subtypes.iter() {

                        if config::generic_matches_all {
                            // def_id `None` matches everything
                            if src_ty.def_id.is_some() && tgt_ty.def_id.is_some()
                                && (src_ty.def_id != tgt_ty.def_id)
                                && (!src_ty.is_closure) && (!tgt_ty.is_closure)
                            {
                                continue;
                            }
                        }
                        else {
                            // def_id `None` only matches `None`
                            if src_ty.def_id != tgt_ty.def_id
                                    && (!src_ty.is_closure) && (!tgt_ty.is_closure)
                            {
                                continue;
                            }
                        }

                        let source_field = get_first_field(&src_ty.projection);
                        let target_field = get_first_field(&tgt_ty.projection);

                        // First check for use-after-free violations
                        let (violation, (src_bounding_lt, tgt_bounding_lt)) = checks::arg_return_outlives(&src_ty, &tgt_ty, &bounds);

                        if violation {
                            let source_place: Option<Place> = get_mir_value_from_hir_param(&func.params[inp_num], &func.mir_body);
                            if source_place.is_none() {
                                continue;
                            }
                            let source_place = source_place.unwrap();
                            let mut debug = false;
                            let dataflow_detected: bool;

                            if config::no_mir {
                                dataflow_detected = true;
                            }
                            else if config::alias_analysis {
                                dataflow_detected = alias_analyzer.check_alias(&source_place.local, &src_ty.projection,
                                                                               &Local::from_usize(0), &tgt_ty.projection, debug);
                                alias_analyzer.reset();
                            }
                            else {
                                taint_analyzer.mark_taint(&source_place.local, source_field, debug);
                                dataflow_detected = taint_analyzer.check_taint(&Local::from_usize(0), target_field);
                            }
                            if dataflow_detected {
                                let html: String = generate_report(self.tcx, &func, inp_num, src_ty, tgt_ty, src_bounding_lt, tgt_bounding_lt);

                                return Some(Report{html, func_name: String::new(), error_type: "uaf".to_string()});
                            }
                            taint_analyzer.clear_taint();
                        }

                        if checks::arg_return_mut(&src_ty, &tgt_ty, &bounds) {

                            let source_place: Option<Place> = get_mir_value_from_hir_param(&func.params[inp_num], &func.mir_body);
                            if source_place.is_none() {
                                continue;
                            }
                            let source_place = source_place.unwrap();

                            let dataflow_detected: bool;

                            if config::no_mir {
                                dataflow_detected = true;
                            }
                            else if config::alias_analysis {
                                dataflow_detected = alias_analyzer.check_alias(&source_place.local, &src_ty.projection,
                                                                               &Local::from_usize(0), &tgt_ty.projection, false);
                                alias_analyzer.reset();
                            }
                            else {
                                taint_analyzer.mark_taint(&source_place.local, source_field, false);
                                dataflow_detected = taint_analyzer.check_taint(&Local::from_usize(0), target_field);
                            }
                            if dataflow_detected {
                                // println!("---------- Potential aliased mutability! ----------");
                                // print_span(*self.tcx, &func.fn_sig.span);
                                // println!("Arg {:?}, Return {:?}", format_span(*self.tcx, &inp.span), format_span(*self.tcx, &ret_type.span));
                                // println!("The compatible types are {:?} , {:?}", src_ty.def_id, tgt_ty.def_id);
                                // println!("Arg Lifetime {:?}", &src_ty.lifetimes);
                                // println!("Return type lifetime : {:?}", &tgt_ty.lifetimes);

                                return None; // TODO
                            }
                            taint_analyzer.clear_taint();
                        }
                    }
                }
            }
        }
        None
    }

    fn check_arg_arg_violations<'a>(&self, func: &MirFunc<'tcx, 'a>) -> Option<Report> {

        let mut taint_analyzer = TaintAnalyzer::new(&func.mir_body);
        let mut alias_analyzer = AliasAnalyzer::new(&func.mir_body);

        for (inp_num1, inp1) in func.fn_sig.decl.inputs.iter().enumerate() {

            let inp1_subtypes = get_sub_types(inp1, &func.generic_bounds, self.tcx);
            let bounds1 = get_implicit_lifetime_bounds(inp1, &func.generic_bounds, self.tcx);

            for (inp_num2, inp2) in func.fn_sig.decl.inputs.iter().enumerate() {

                if inp_num1 == inp_num2 {continue;}

                let inp2_subtypes   = get_sub_types(inp2, &func.generic_bounds, self.tcx);
                let mut bounds2     = get_implicit_lifetime_bounds(inp2, &func.generic_bounds, self.tcx);

                bounds2.append(&mut bounds1.clone());
                bounds2.append(&mut func.lifetime_bounds.clone());

                for src_ty in inp1_subtypes.iter() {
                    for tgt_ty in inp2_subtypes.iter() {

                        if config::generic_matches_all {
                            // def_id `None` matches everything
                            if src_ty.def_id.is_some() && tgt_ty.def_id.is_some()
                                && (src_ty.def_id != tgt_ty.def_id)
                                && (!src_ty.is_closure) && (!tgt_ty.is_closure)
                            {
                                continue;
                            }
                        }
                        else {
                            // def_id `None` only matches `None`
                            if src_ty.def_id != tgt_ty.def_id
                                    && (!src_ty.is_closure) && (!tgt_ty.is_closure)
                            {
                                continue;
                            }
                        }
                        if checks::arg_arg_outlives(&src_ty, &tgt_ty, &bounds2) {

                            let source_place: Option<Place> = get_mir_value_from_hir_param(&func.params[inp_num1], &func.mir_body);
                            if source_place.is_none() {
                                continue;
                            }
                            let source_place = source_place.unwrap();
                            let target_place: Option<Place> = get_mir_value_from_hir_param(&func.params[inp_num2], &func.mir_body);
                            if target_place.is_none() {
                                continue;
                            }
                            let target_place = target_place.unwrap();
                            let source_field = get_first_field(&src_ty.projection);
                            let target_field = get_first_field(&tgt_ty.projection);

                            let dataflow_detected: bool;

                            if config::no_mir {
                                dataflow_detected = true;
                            }
                            else if config::alias_analysis {
                                dataflow_detected = alias_analyzer.check_alias(&source_place.local, &src_ty.projection,
                                                                               &target_place.local, &tgt_ty.projection, false);
                                alias_analyzer.reset();
                            }
                            else {
                                taint_analyzer.mark_taint(&source_place.local, source_field, false);
                                dataflow_detected = taint_analyzer.check_taint(&target_place.local, target_field);
                            }

                            if dataflow_detected {
                                // println!("---------- Potential use-after-free! ----------");
                                // print_span(*self.tcx, &func.fn_sig.span);
                                // println!("Arg {:?}, Arg {:?}", format_span(*self.tcx, &inp1.span), format_span(*self.tcx, &inp2.span));
                                // println!("The compatible types are {:?} , {:?}", src_ty.def_id, tgt_ty.def_id);
                                // println!("Arg {} Lifetime {:?}", inp_num1, &src_ty.lifetimes);
                                // println!("Arg {} Owner Lifetime : {:?}", inp_num2, &tgt_ty.lifetimes);
                                // println!("\n");
                                return None; // TODO
                            }
                            taint_analyzer.clear_taint();
                        }
                    }
                }
            }
        }
        None
    }

    pub fn analyze(mut self) {

        let mut reports: Vec<Report> = Vec::new();

        for mirfunc in fn_iter(&self.tcx) {

            // We don't need to check unsafe functions
            if mirfunc.fn_sig.header.unsafety == rustc_hir::Unsafety::Unsafe {
                continue;
            }
            let mut fname = mirfunc.func_name.clone();
            if fname.contains("::") {
                fname = fname.split("::").last().unwrap().to_string()
            }
            if config::filter {
                // Shallow filters for common patterns of false positives in trait implementations
                if fname == "clone" || mirfunc.impl_trait == "Clone" {
                    continue;
                }
                if mirfunc.impl_trait.contains("Iterator") && ((fname == "next") || (fname == "next_back")) {
                    continue;
                }
            }
            if mirfunc.func_name.contains(&config::debug_fn) {
                println!("Func name: {:?}", &mirfunc.func_name);
                self.debug_output(&mirfunc);
            }
            let mut report = self.check_arg_return_violations(&mirfunc);
            if let Some(mut report) = report {
                report.func_name = fname.clone();
                reports.push(report);
            }

            let mut report = self.check_arg_arg_violations(&mirfunc);
            if let Some(mut report) = report {
                report.func_name = fname.clone();
                reports.push(report);
            }
        }

        if reports.len() == 0 {
            progress_info!("Found no errors");
            return;
        }

        fs::remove_dir_all("yuga_reports/");
        fs::create_dir_all("yuga_reports/");

        let mut filename_repeat_count: HashMap<String, u32> = HashMap::new();

        for report in reports.iter() {

            let mut filename: String = format!("yuga_reports/{}-{}.html", report.func_name, report.error_type);

            match filename_repeat_count.get_mut(&filename) {
                Some(count) => {
                    filename = format!("yuga_reports/{}-{}-{}.html", report.func_name, report.error_type, count);
                    *count += 1;
                },
                None => {
                    filename_repeat_count.insert(filename.clone(), 1 as u32);
                }
            }
            fs::write(&filename, &report.html);
            progress_info!("Wrote report to {}", filename);
        }
        progress_info!("Found {} lifetime annotation bugs. Detailed reports can be found in ./yuga_reports", reports.len());
    }
}
