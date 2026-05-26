//! Nullability for annotated ARegex

use crate::types::ARegex;

pub fn nullable_bc(ri: &ARegex) -> bool {
    match ri {
        ARegex::Phi => false,
        ARegex::Eps(_) => true,
        ARegex::Lit(_, _) => false,
        ARegex::Alt(_, r1, r2) => nullable_bc(r1) || nullable_bc(r2),
        ARegex::Seq(_, r1, r2) => nullable_bc(r1) && nullable_bc(r2),
        ARegex::Star(_, _) => true,
    }
}

pub fn is_phi(ri: &ARegex) -> bool {
    match ri {
        ARegex::Phi => true,
        ARegex::Eps(_) => false,
        ARegex::Lit(_, _) => false,
        ARegex::Alt(_, r1, r2) => is_phi(r1) && is_phi(r2),
        ARegex::Seq(_, r1, r2) => is_phi(r1) || is_phi(r2),
        ARegex::Star(_, _) => false,
    }
}