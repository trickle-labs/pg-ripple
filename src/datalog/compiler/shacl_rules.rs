//! Datalog SQL compiler: SHACL constraint check compilation (M15-13, v0.96.0).
//! Moved from datalog/compiler/mod.rs.

use crate::datalog::{BodyLiteral, Rule, Term};

use super::{
    VarMap, build_join_cond, compare_op_sql, const_sql, render_comparison_term, vp_read_expr,
};

// ─── Constraint check compiler ────────────────────────────────────────────────

/// Compile a constraint rule (empty head) to a `SELECT EXISTS (…) AS violated`.
pub fn compile_constraint_check(rule: &Rule) -> Result<String, String> {
    if rule.head.is_some() {
        return Err("not a constraint rule".to_owned());
    }

    let mut from_clauses: Vec<String> = Vec::new();
    let mut where_clauses: Vec<String> = Vec::new();
    let mut var_map = VarMap::default();
    let mut atom_idx = 0usize;

    for lit in &rule.body {
        match lit {
            BodyLiteral::Positive(atom) => {
                let alias = format!("t{atom_idx}");
                let pred_id = match &atom.p {
                    Term::Const(id) => *id,
                    _ => return Err("variable predicate in constraint body".to_owned()),
                };

                if let Term::Var(v) = &atom.s {
                    var_map.bind(v, &alias, "s");
                } else if let Term::Const(c) = &atom.s {
                    where_clauses.push(format!("{alias}.s = {}", const_sql(*c)));
                }
                if let Term::Var(v) = &atom.o {
                    var_map.bind(v, &alias, "o");
                } else if let Term::Const(c) = &atom.o {
                    where_clauses.push(format!("{alias}.o = {}", const_sql(*c)));
                }

                if atom_idx == 0 {
                    from_clauses.push(format!("{} {alias}", vp_read_expr(pred_id)));
                } else {
                    let join_cond = build_join_cond(&alias, atom, &var_map);
                    if join_cond.is_empty() {
                        from_clauses.push(format!("{} {alias}", vp_read_expr(pred_id)));
                    } else {
                        from_clauses.push(format!(
                            "JOIN {} {alias} ON {}",
                            vp_read_expr(pred_id),
                            join_cond
                        ));
                    }
                }
                atom_idx += 1;
            }
            BodyLiteral::Compare(lhs, op, rhs) => {
                let l = render_comparison_term(lhs, &var_map);
                let r = render_comparison_term(rhs, &var_map);
                let op_str = compare_op_sql(op);
                where_clauses.push(format!("{l} {op_str} {r}"));
            }
            _ => {}
        }
    }

    if from_clauses.is_empty() {
        return Ok("SELECT FALSE AS violated".to_owned());
    }

    let from_str = from_clauses.join("\n");
    let where_str = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    Ok(format!(
        "SELECT EXISTS (\n\
             SELECT 1 FROM {from_str}\n\
             {where_str}\n\
         ) AS violated"
    ))
}
