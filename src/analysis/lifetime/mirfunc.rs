use rustc_hir::{def_id::DefId, BodyId, Param, Ty, Mutability, FnSig};
use rustc_hir::LifetimeName;
use rustc_hir::ParamName::{Plain, Fresh};

use rustc_span::Span;
use rustc_middle::mir::{Operand, Statement, StatementKind, Rvalue, VarDebugInfo, Place, Local, PlaceElem};
use rustc_middle::hir::map::Map;

use std::collections::HashMap;
use rustc_middle::mir::Body;

pub struct MirFunc<'tcx, 'a> {
	pub fn_sig: 			&'a FnSig<'tcx>,
	pub body_span: 			Span,
	pub func_name: 			String,
	pub impl_trait: 		String,
	pub params: 			&'a [Param<'tcx>],
	pub generic_bounds: 	HashMap<DefId, rustc_hir::GenericBounds<'tcx>>,
	pub lifetime_bounds: 	Vec<(LifetimeName, LifetimeName)>,
	pub mir_body: 			&'a Body<'tcx>
}
