use crate::analysis::lifetime::process::{MyLifetime, ShortLivedType};
use crate::analysis::lifetime::utils::{
    compare_lifetimes,
    MyProjection::{self, MyDeref, MyField},
    FieldInfo,
    get_drop_impl,
};
use crate::YugaConfig;
use rustc_hir::LifetimeName;
use rustc_middle::ty::TyCtxt;

pub fn arg_return_outlives( source_type:        &ShortLivedType,
                            target_type:        &ShortLivedType,
                            lifetime_bounds:    &Vec<(LifetimeName, LifetimeName)>,
                            tcx:                &TyCtxt<'_>,
                            config:             YugaConfig,
                       ) -> (bool, (Option<LifetimeName>, Option<LifetimeName>), (bool, bool)) {

    let source_lifetimes = &source_type.lifetimes;
    let target_lifetimes = &target_type.lifetimes;

    let mut src_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut src_is_raw = false;
    let mut src_needs_drop_impl = false;
    
    // The source lifetime is the one that is "closest" to the value
    // Note that this is a reverse iterator
    for src_lifetime in source_lifetimes.iter()
                                        .rev()
                                        .filter(|&x| !(x.is_refcell)) {
        // If the closest lifetime is a raw pointer, then we set src_is_raw = True
        if first && src_lifetime.is_raw {
            src_is_raw = true;
            if src_lifetime.name.is_none() {
                src_needs_drop_impl = true;
            }
        }
        first = false;

        if src_lifetime.name.is_some() {
            src_bounding_lt = src_lifetime.name;
            break;
        }
    }
    // If there are no source lifetimes, then we own the thing
    // and we can assume it'll live forever. No violation possible.
    if src_bounding_lt.is_none() || src_bounding_lt == Some(LifetimeName::Static) {
        return (false, (None, None), (false, false));
    }

    let mut tgt_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut tgt_is_raw = false;
    let mut tgt_needs_drop_impl = false;

    for tgt_lifetime in target_lifetimes.iter()
                                        .rev()
                                        .filter(|&x| !(x.is_refcell)) {
                                            
        if first && tgt_lifetime.is_raw {
            tgt_is_raw = true;
            if tgt_lifetime.name.is_none() {
                tgt_needs_drop_impl = true;
            }
        }
        first = false;

        if tgt_lifetime.name.is_some() {
            tgt_bounding_lt = tgt_lifetime.name;
            break;
        }
    }
    if (! src_is_raw) && (! tgt_is_raw) { return (false, (None, None), (false, false)); }
    if (src_is_raw && (! source_type.in_struct)) || (tgt_is_raw && (! target_type.in_struct)) { return (false, (None, None), (false, false)); }

    // If we are returning something that is neither a borrow nor raw pointer,
    // then this is not something we care about.
    if tgt_bounding_lt.is_none() && ! tgt_is_raw {
        return (false, (None, None), (false, false));
    }

    fn check_if_drop_impl_exists(ty: &ShortLivedType, tcx: &TyCtxt<'_>) -> bool {
        let mut break_point = 0;
        // Find the last Deref projection (corresponding to the raw pointer)
        for (i, proj) in ty.projection.iter().rev().enumerate() {
            if let proj = MyProjection::MyDeref { break; }
            break_point = i;
        }
        // Start from the next index in reverse order, looking for a field projection
        for proj in ty.projection.iter().rev().skip(break_point + 1) {
            match proj {
                MyProjection::MyField(field_info) => {
                    // Get the def_id of the struct corresponding to that field projection
                    if let Some(struct_def_id) = field_info.struct_def_id {
                        if get_drop_impl(struct_def_id, tcx).is_some() {
                            return true;
                        }
                    }
                },
                // The moment you hit another deref, stop
                MyProjection::MyDeref => break,
            }
        }
        false
    }
    
    if config.filter_by_drop_impl {
        if src_needs_drop_impl {
            src_needs_drop_impl = ! (check_if_drop_impl_exists(&source_type, &tcx));
        }
        if tgt_needs_drop_impl {
            tgt_needs_drop_impl = ! (check_if_drop_impl_exists(&source_type, &tcx));
        }
        if ! ((src_is_raw && !src_needs_drop_impl) || (tgt_is_raw && !tgt_needs_drop_impl)) {
            return (false, (None, None), (false, false));
        }
    }

    if tgt_bounding_lt.is_none() || tgt_bounding_lt == Some(LifetimeName::Static) {
        return (true, (src_bounding_lt, tgt_bounding_lt), (false, false));
    }
    if compare_lifetimes(&src_bounding_lt.unwrap(), &tgt_bounding_lt.unwrap()) {
        return (false, (None, None), (false, false));
    }
    else {
        if lifetime_bounds.iter() // Source should outlive the target
            .any(|&(x, y)| compare_lifetimes(&x, &src_bounding_lt.unwrap())
                        && compare_lifetimes(&y, &tgt_bounding_lt.unwrap()))
        { return (false, (None, None), (false, false)); }
    }
    (true, (src_bounding_lt, tgt_bounding_lt), (src_is_raw, tgt_is_raw))
}

pub fn arg_return_mut ( source_type:        &ShortLivedType,
                        target_type:        &ShortLivedType,
                        lifetime_bounds:    &Vec<(LifetimeName, LifetimeName)>,
                        config:             YugaConfig,
                    ) -> (bool, (Option<LifetimeName>, Option<LifetimeName>)) {

    let source_lifetimes = &source_type.lifetimes;
    let target_lifetimes = &target_type.lifetimes;

    let mut src_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut src_is_raw = false;

    for src_lifetime in source_lifetimes.iter().rev() {
        if src_lifetime.is_refcell { continue; }
        if first {
            if ! src_lifetime.is_mut {
                return (false, (None, None));
            }
            src_is_raw = src_lifetime.is_raw;
            first = false;
            continue;
        }
        src_bounding_lt = src_lifetime.name;
        break;
    }
    if src_bounding_lt.is_none() || src_bounding_lt == Some(LifetimeName::Static) {
        return (false, (None, None));
    }
    let mut tgt_is_raw = false;
    for tgt_lifetime in target_lifetimes.iter().rev() {
        if tgt_lifetime.is_refcell { continue; }
        tgt_is_raw = tgt_lifetime.is_raw;
        break;
    }
    let mut tgt_bounding_lt: Option<LifetimeName> = None;
    for tgt_lifetime in target_lifetimes.iter() {
        if tgt_lifetime.is_refcell { continue; }
        if tgt_lifetime.name.is_some() {
            tgt_bounding_lt = tgt_lifetime.name;
        }
    }
    if !(src_is_raw && source_type.in_struct) { return (false, (None, None)); } // We want the source to be a raw pointer
    if tgt_is_raw { return (false, (None, None)); } // We want the target to be a normal borrow
    if tgt_bounding_lt.is_none() {
        return (false, (None, None));
    }
    if compare_lifetimes(&src_bounding_lt.unwrap(), &tgt_bounding_lt.unwrap()) {
        return (false, (None, None));
    }
    else {
        if lifetime_bounds.iter() // Source should outlive the target
            .any(|&(x, y)| compare_lifetimes(&x, &src_bounding_lt.unwrap())
                        && compare_lifetimes(&y, &tgt_bounding_lt.unwrap()))
        { return (false, (None, None)); }
    }
    (true, (src_bounding_lt, tgt_bounding_lt))
}

pub fn arg_arg_outlives(source_type:   &ShortLivedType,
                        target_type:   &ShortLivedType,
                        lifetime_bounds:    &Vec<(LifetimeName, LifetimeName)>,
                        config:         YugaConfig,
                    ) -> (bool, (Option<LifetimeName>, Option<LifetimeName>)) {

    let source_lifetimes = &source_type.lifetimes;
    let target_lifetimes = &target_type.lifetimes;

    let mut src_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut src_is_raw = false;

    // First get source bounding lifetime
    for src_lifetime in source_lifetimes.iter().rev() {

        if src_lifetime.is_refcell { continue; }
        if first && src_lifetime.is_raw { src_is_raw = true; }
        first = false;

        if src_lifetime.name.is_some() {
            src_bounding_lt = src_lifetime.name;
            break;
        }
    }
    if src_bounding_lt.is_none() || src_bounding_lt == Some(LifetimeName::Static) {
        return (false, (None, None));
    }

    let mut tgt_bounding_lt: Option<LifetimeName> = None;

    let mut check_mut = true;
    let mut viable = false;
    let mut tgt_is_raw = false;

    let mut first = true;

    for tgt_lifetime in target_lifetimes.iter().rev() {

        // Get target bounding lifetime, which is the one "closest" to the value
        if first {
            if (tgt_lifetime.is_refcell) {
                continue;
            }
            else {
                tgt_bounding_lt = tgt_lifetime.name;
                tgt_is_raw = tgt_lifetime.is_raw;
                first = false;
                continue;
            }
        }
        // Now check whether that borrow/pointer can actually be mutated
        // Note that this is not the same thing as whether the borrow/pointer is mutable
        // We're looking for cases like `&mut &T` or `&mut *const T` or `&mut *mut &T`
        if tgt_lifetime.is_refcell {
            // If we have a Refcell, then mutability is ensured.
            check_mut = false;
            continue;
        }
        if !check_mut {
            viable = true;
            break;
        }
        if check_mut && (! tgt_lifetime.is_mut) {
            viable = false;
            break;
        }
        viable = true;
    }
    if ! viable { return (false, (None, None)); }

    if (! src_is_raw) && (! tgt_is_raw) { return (false, (None, None)); }
    if (src_is_raw && (! source_type.in_struct)) || (tgt_is_raw && (! target_type.in_struct)) { return (false, (None, None)); }

    if tgt_bounding_lt.is_none() || tgt_bounding_lt == Some(LifetimeName::Static) {
        return (true, (src_bounding_lt, tgt_bounding_lt));
    }
    
    if compare_lifetimes(&src_bounding_lt.unwrap(), &tgt_bounding_lt.unwrap()) {
        return (false, (None, None));
    }
    else {
        if lifetime_bounds.iter() // Source should outlive the target
            .any(|&(x, y)| compare_lifetimes(&x, &src_bounding_lt.unwrap())
                        && compare_lifetimes(&y, &tgt_bounding_lt.unwrap()))
        { return (false, (None, None)); }
    }
    (true, (src_bounding_lt, tgt_bounding_lt))
}
