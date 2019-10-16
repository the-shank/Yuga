use rustc::mir;
use rustc::ty::{self, Instance, TyCtxt};

pub trait TyCtxtExt<'tcx> {
    fn find_fn(&self, instance: Instance<'tcx>) -> Option<&'tcx mir::Body<'tcx>>;
}

// TODO: use more fine-grained error handling than returning None
impl<'tcx> TyCtxtExt<'tcx> for TyCtxt<'tcx> {
    /// Try to find MIR function body with given Instance
    /// this is a combined version of MIRI's find_fn + Rust InterpCx's load_mir
    fn find_fn(&self, instance: Instance<'tcx>) -> Option<&'tcx mir::Body<'tcx>> {
        // https://github.com/rust-lang/miri/blob/1037f69bf6dcf73dfbe06453336eeae61ba7c51f/src/shims/mod.rs#L14-L55
        // TODO: apply hooks in rustc MIR evaluator

        // currently we don't handle any foreign item
        if self.is_foreign_item(instance.def_id()) {
            println!("Unsupported foreign item: {:?}", &instance);
            return None;
        }

        // https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_mir/interpret/eval_context.rs.html#293-318
        let did = instance.def.def_id();
        if did.is_local()
            && self.has_typeck_tables(did)
            && self.typeck_tables_of(did).tainted_by_errors
        {
            // type check failure
            println!("Type check failed for an item: {:?}", &instance);
            return None;
        }

        match instance.def {
            ty::InstanceDef::Item(_) => {
                if self.is_mir_available(did) {
                    Some(self.optimized_mir(did))
                } else {
                    // MIR is not available
                    println!("No MIR for an item: {:?}", &instance);
                    None
                }
            }
            _ => Some(self.instance_mir(instance.def)),
        }
    }
}