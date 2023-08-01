use crate::analysis::lifetime::process::{MyLifetime, ShortLivedType};
use crate::analysis::lifetime::utils::compare_lifetimes;
use rustc_hir::LifetimeName;

pub fn arg_return_outlives( source_type:   &ShortLivedType,
                            target_type:   &ShortLivedType,
                            lifetime_bounds:    &Vec<(LifetimeName, LifetimeName)>
                       ) -> bool {

    let source_lifetimes = &source_type.lifetimes;
    let target_lifetimes = &target_type.lifetimes;

    let mut src_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut src_is_raw = false;

    for src_lifetime in source_lifetimes.iter().rev() {

        if src_lifetime.is_refcell { continue; }
        if first && src_lifetime.is_raw { src_is_raw = true; }
        first = false;

        if src_lifetime.name.is_some() {
            src_bounding_lt = src_lifetime.name;
            break;
        }
    }
    // If there are no source lifetimes, then we own the thing
    // and we can assume it'll live forever. No violation possible.
    if src_bounding_lt.is_none() || src_bounding_lt == Some(LifetimeName::Static) {
        return false;
    }
    let mut tgt_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut tgt_is_raw = false;

    for tgt_lifetime in target_lifetimes.iter().rev() {

        if tgt_lifetime.is_refcell { continue; }
        if first && tgt_lifetime.is_raw { tgt_is_raw = true; }
        first = false;

        if tgt_lifetime.name.is_some() {
            tgt_bounding_lt = tgt_lifetime.name;
            break;
        }
    }
    if (! src_is_raw) && (! tgt_is_raw) { return false; }
    if (src_is_raw && (! source_type.in_struct)) || (tgt_is_raw && (! target_type.in_struct)) { return false; }

    // If we are returning something that is neither a borrow nor raw pointer,
    // then this is not something we care about.
    if tgt_bounding_lt.is_none() && ! tgt_is_raw {
        return false;
    }
    if tgt_bounding_lt.is_none() || tgt_bounding_lt == Some(LifetimeName::Static) {
        return true;
    }
    if compare_lifetimes(&src_bounding_lt.unwrap(), &tgt_bounding_lt.unwrap()) {
        return false;
    }
    else {
        if lifetime_bounds.iter() // Source should outlive the target
            .any(|&(x, y)| compare_lifetimes(&x, &src_bounding_lt.unwrap())
                        && compare_lifetimes(&y, &tgt_bounding_lt.unwrap()))
        { return false; }
    }
    true
}

pub fn arg_return_mut ( source_type:   &ShortLivedType,
                        target_type:   &ShortLivedType,
                        lifetime_bounds:    &Vec<(LifetimeName, LifetimeName)>
                    ) -> bool {

    let source_lifetimes = &source_type.lifetimes;
    let target_lifetimes = &target_type.lifetimes;

    let mut src_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut src_is_raw = false;

    for src_lifetime in source_lifetimes.iter().rev() {
        if src_lifetime.is_refcell { continue; }
        if first {
            if ! src_lifetime.is_mut {
                return false;
            }
            src_is_raw = src_lifetime.is_raw;
            first = false;
            continue;
        }
        src_bounding_lt = src_lifetime.name;
        break;
    }
    if src_bounding_lt.is_none() || src_bounding_lt == Some(LifetimeName::Static) {
        return false;
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
    if !(src_is_raw && source_type.in_struct) { return false; } // We want the source to be a raw pointer
    if tgt_is_raw { return false; } // We want the target to be a normal borrow
    if tgt_bounding_lt.is_none() {
        return false;
    }
    if compare_lifetimes(&src_bounding_lt.unwrap(), &tgt_bounding_lt.unwrap()) {
        return false;
    }
    else {
        if lifetime_bounds.iter() // Source should outlive the target
            .any(|&(x, y)| compare_lifetimes(&x, &src_bounding_lt.unwrap())
                        && compare_lifetimes(&y, &tgt_bounding_lt.unwrap()))
        { return false; }
    }
    true
}

pub fn arg_arg_outlives(source_type:   &ShortLivedType,
                        target_type:   &ShortLivedType,
                        lifetime_bounds:    &Vec<(LifetimeName, LifetimeName)>,
                    ) -> bool {

    let source_lifetimes = &source_type.lifetimes;
    let target_lifetimes = &target_type.lifetimes;

    let mut src_bounding_lt: Option<LifetimeName> = None;
    let mut first = true;
    let mut src_is_raw = false;

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
        return false;
    }

    let mut tgt_bounding_lt: Option<LifetimeName> = None;

    let mut check_mut = true;
    let mut viable = false;
    let mut tgt_is_raw = false;

    let mut first = true;

    for tgt_lifetime in target_lifetimes.iter().rev() {

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
        // If we have a Refcell, then mutability is ensured.
        if tgt_lifetime.is_refcell {
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
    if ! viable { return false; }


    if tgt_bounding_lt.is_none() || tgt_bounding_lt == Some(LifetimeName::Static) {
        return true;
    }
    if (! src_is_raw) && (! tgt_is_raw) { return false; }
    if (src_is_raw && (! source_type.in_struct)) || (tgt_is_raw && (! target_type.in_struct)) { return false; }

    if compare_lifetimes(&src_bounding_lt.unwrap(), &tgt_bounding_lt.unwrap()) {
        return false;
    }
    else {
        if lifetime_bounds.iter() // Source should outlive the target
            .any(|&(x, y)| compare_lifetimes(&x, &src_bounding_lt.unwrap())
                        && compare_lifetimes(&y, &tgt_bounding_lt.unwrap()))
        { return false; }
    }
    true
}
