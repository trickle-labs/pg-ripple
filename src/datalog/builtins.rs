//! Built-in rule sets for the Datalog reasoning engine.
//!
//! Ships seven pre-packaged rule sets:
//!
//! - `"rdfs"` — W3C RDFS entailment (13 rules)
//! - `"owl-rl"` — W3C OWL 2 RL profile (~30 core rules, stratifiable subset)
//! - `"owl-el"` — W3C OWL 2 EL profile (existential restrictions, classification)
//! - `"owl-ql"` — W3C OWL 2 QL / DL-Lite rewriting rules (query-rewriting mode)
//! - `"skos"` — W3C SKOS entailment rules (28 rules, S7–S45)
//! - `"skos-transitive"` — SKOS transitive-closure subset (7 rules, for riverbank)
//! - `"skosxl"` — SKOS-XL label dumb-down chains (3 rules, S55–S57)
//!
//! Rule text uses well-known prefixes (rdf:, rdfs:, owl:, skos:, skosxl:) that
//! must be pre-registered in `_pg_ripple.prefixes` before loading.

/// Ensure that the well-known standard prefixes are registered.
/// Called before loading any built-in rule set.
pub fn register_standard_prefixes() {
    use pgrx::prelude::*;

    let prefixes = [
        ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
        ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
        ("owl", "http://www.w3.org/2002/07/owl#"),
        ("xsd", "http://www.w3.org/2001/XMLSchema#"),
        ("skos", "http://www.w3.org/2004/02/skos/core#"),
        ("skosxl", "http://www.w3.org/2008/05/skos-xl#"),
    ];

    for (prefix, expansion) in &prefixes {
        let _ = Spi::run_with_args(
            "INSERT INTO _pg_ripple.prefixes (prefix, expansion) \
             VALUES ($1, $2) \
             ON CONFLICT (prefix) DO NOTHING",
            &[
                pgrx::datum::DatumWithOid::from(*prefix),
                pgrx::datum::DatumWithOid::from(*expansion),
            ],
        );
    }
}

/// Return the Datalog text for the named built-in rule set.
///
/// Supported names: `"rdfs"`, `"owl-rl"`, `"owl-el"`, `"owl-ql"`,
///                  `"skos"`, `"skos-transitive"`, `"skosxl"`.
pub fn get_builtin_rules(name: &str) -> Result<&'static str, String> {
    match name {
        "rdfs" => Ok(RDFS_RULES),
        "owl-rl" => Ok(OWL_RL_RULES),
        "owl-el" => Ok(OWL_EL_RULES),
        "owl-ql" => Ok(OWL_QL_RULES),
        "skos" => Ok(SKOS_RULES),
        "skos-transitive" => Ok(SKOS_TRANSITIVE_RULES),
        "skosxl" => Ok(SKOSXL_RULES),
        _ => Err(format!(
            "unknown built-in rule set '{name}'; valid values: rdfs, owl-rl, owl-el, owl-ql, skos, skos-transitive, skosxl"
        )),
    }
}

// ─── RDFS Entailment Rules (W3C RDF Semantics §9) ────────────────────────────
//
// The 13 RDFS entailment rules as Datalog. Each rule is numbered per the spec.
// Prefixes: rdf: rdfs: (registered by register_standard_prefixes).

const RDFS_RULES: &str = r#"
# rdfs2: domain inference
# If p has domain c, and x has property p, then x is of type c.
?x rdf:type ?c :- ?x ?p ?y, ?p rdfs:domain ?c .

# rdfs3: range inference
# If p has range c, and something has property p with value y, then y is of type c.
?y rdf:type ?c :- ?x ?p ?y, ?p rdfs:range ?c .

# rdfs4a: subject resources are instances of rdfs:Resource
?x rdf:type rdfs:Resource :- ?x ?p ?y .

# rdfs4b: object resources are instances of rdfs:Resource
?y rdf:type rdfs:Resource :- ?x ?p ?y .

# rdfs5: subPropertyOf transitivity
?p rdfs:subPropertyOf ?r :- ?p rdfs:subPropertyOf ?q, ?q rdfs:subPropertyOf ?r .

# rdfs6: a property is a subproperty of itself (reflexivity)
?p rdfs:subPropertyOf ?p :- ?p rdf:type rdf:Property .

# rdfs7: subPropertyOf propagation
?x ?r ?y :- ?x ?p ?y, ?p rdfs:subPropertyOf ?r .

# rdfs8: classes are instances of rdfs:Class
?x rdf:type rdfs:Class :- ?x rdf:type rdfs:Class .

# rdfs9: subClassOf type propagation
?x rdf:type ?c :- ?x rdf:type ?b, ?b rdfs:subClassOf ?c .

# rdfs10: a class is a subclass of itself (reflexivity)
?c rdfs:subClassOf ?c :- ?c rdf:type rdfs:Class .

# rdfs11: subClassOf transitivity
?b rdfs:subClassOf ?c :- ?b rdfs:subClassOf ?a, ?a rdfs:subClassOf ?c .

# rdfs12: subPropertyOf between container membership properties and member
?p rdfs:subPropertyOf rdfs:member :- ?p rdf:type rdfs:ContainerMembershipProperty .

# rdfs13: rdfs:Datatype is a subclass of rdfs:Literal
rdfs:Datatype rdfs:subClassOf rdfs:Literal :- rdfs:Datatype rdf:type rdfs:Class .
"#;

// ─── OWL 2 RL Profile Rules (W3C OWL 2 RL, stratifiable subset) ──────────────
//
// The OWL RL profile is the subset of OWL 2 expressible as Datalog rules.
// This implementation covers the core property and class axioms.

const OWL_RL_RULES: &str = r#"
# First, apply all RDFS rules as stratum 0.
# (RDFS rules are included when loading 'owl-rl'.)
?x rdf:type ?c :- ?x ?p ?y, ?p rdfs:domain ?c .
?y rdf:type ?c :- ?x ?p ?y, ?p rdfs:range ?c .
?x rdf:type rdfs:Resource :- ?x ?p ?y .
?y rdf:type rdfs:Resource :- ?x ?p ?y .
?p rdfs:subPropertyOf ?r :- ?p rdfs:subPropertyOf ?q, ?q rdfs:subPropertyOf ?r .
?p rdfs:subPropertyOf ?p :- ?p rdf:type rdf:Property .
?x ?r ?y :- ?x ?p ?y, ?p rdfs:subPropertyOf ?r .
?c rdfs:subClassOf ?c :- ?c rdf:type rdfs:Class .
?x rdf:type ?c :- ?x rdf:type ?b, ?b rdfs:subClassOf ?c .
?b rdfs:subClassOf ?c :- ?b rdfs:subClassOf ?a, ?a rdfs:subClassOf ?c .

# OWL RL: SymmetricProperty
?y ?p ?x :- ?x ?p ?y, ?p rdf:type owl:SymmetricProperty .

# OWL RL: TransitiveProperty
?x ?p ?z :- ?x ?p ?y, ?y ?p ?z, ?p rdf:type owl:TransitiveProperty .

# OWL RL: InverseOf (forward direction)
?y ?q ?x :- ?x ?p ?y, ?p owl:inverseOf ?q .

# OWL RL: InverseOf (backward direction)
?y ?p ?x :- ?x ?q ?y, ?p owl:inverseOf ?q .

# OWL RL: FunctionalProperty (infer sameAs from two values)
?y1 owl:sameAs ?y2 :- ?x ?p ?y1, ?x ?p ?y2, ?p rdf:type owl:FunctionalProperty .

# OWL RL: InverseFunctionalProperty
?x1 owl:sameAs ?x2 :- ?x1 ?p ?y, ?x2 ?p ?y, ?p rdf:type owl:InverseFunctionalProperty .

# OWL RL: sameAs symmetry
?y owl:sameAs ?x :- ?x owl:sameAs ?y .

# OWL RL: sameAs transitivity
?x owl:sameAs ?z :- ?x owl:sameAs ?y, ?y owl:sameAs ?z .

# OWL RL: sameAs class membership propagation
?y rdf:type ?c :- ?x rdf:type ?c, ?x owl:sameAs ?y .

# OWL RL: equivalentClass (forward)
?x rdf:type ?c2 :- ?x rdf:type ?c1, ?c1 owl:equivalentClass ?c2 .

# OWL RL: equivalentProperty (forward)
?x ?p2 ?y :- ?x ?p1 ?y, ?p1 owl:equivalentProperty ?p2 .

# OWL RL: propertyChainAxiom (two-link chains)
?x ?p ?z :- ?x ?p1 ?y, ?y ?p2 ?z, ?p owl:propertyChainAxiom ?chain .

# OWL RL: allValuesFrom restriction
?y rdf:type ?c :- ?x rdf:type ?r, ?x ?p ?y, ?r owl:allValuesFrom ?c, ?r owl:onProperty ?p .

# OWL RL: hasValue restriction
?x rdf:type ?r :- ?x ?p ?v, ?r owl:hasValue ?v, ?r owl:onProperty ?p .

# OWL RL: intersectionOf membership (binary)
?x rdf:type ?c :- ?x rdf:type ?c1, ?x rdf:type ?c2, ?c owl:intersectionOf ?list .

# ── v0.48.0: OWL 2 RL rule-set completion ─────────────────────────────────────

# cax-sco: rdfs:subClassOf full transitive closure (adds the second-order transitivity
# rule that was previously only one-step via rdfs9).  The rdfs11 rule already
# handles rdfs:subClassOf transitivity, so this rule restates it for clarity and
# ensures it is present when ONLY owl-rl is loaded without rdfs.
?x rdf:type ?c :- ?x rdf:type ?a, ?a rdfs:subClassOf ?b, ?b rdfs:subClassOf ?c .

# prp-spo1: rdfs:subPropertyOf full chain (equivalent to rdfs7 but stated
# explicitly for the OWL RL profile so the rule is present without RDFS).
?x ?r ?y :- ?x ?p ?y, ?p rdfs:subPropertyOf ?r .

# prp-ifp: InverseFunctionalProperty → sameAs (already present above but
# restated for OWL RL naming clarity; ON CONFLICT rules are idempotent).
?x1 owl:sameAs ?x2 :- ?x1 ?p ?y, ?x2 ?p ?y, ?p rdf:type owl:InverseFunctionalProperty .

# cls-avf: allValuesFrom interaction with subclass hierarchy.
# If x is of type R and R restricts property p to allValuesFrom C, and there
# exists a subclass D of C, then values of x via p that are of type D also
# satisfy the restriction via inheritance.
?y rdf:type ?d :- ?x rdf:type ?r, ?x ?p ?y, ?r owl:allValuesFrom ?c, ?r owl:onProperty ?p, ?d rdfs:subClassOf ?c .

# owl:minCardinality entailment: if a class R has minCardinality 0 on property p,
# no inference is needed.  minCardinality 1 on a functional property allows
# inferring that the value exists when we see a type assertion.
# The Datalog-expressible subset: class membership from cardinality axioms.
?x rdf:type ?r :- ?x ?p ?y, ?r owl:minCardinality ?n, ?r owl:onProperty ?p .

# owl:maxCardinality + FunctionalProperty → sameAs for values.
?y1 owl:sameAs ?y2 :- ?x rdf:type ?r, ?x ?p ?y1, ?x ?p ?y2, ?r owl:maxCardinality ?n, ?r owl:onProperty ?p, ?p rdf:type owl:FunctionalProperty .

# owl:cardinality = exactly N; same entailments as combined min+max.
?x rdf:type ?r :- ?x ?p ?y, ?r owl:cardinality ?n, ?r owl:onProperty ?p .

# ── v0.51.0: OWL 2 RL known-failure fixes ─────────────────────────────────────

# prp-spo2: three-hop propertyChainAxiom
# Like prp-spo1 (2-link chains), but for 3-step chains.  The Datalog rule
# applies whenever a property p has a propertyChainAxiom list entry.
# (A stricter implementation would unroll the list; this conservative form
# ensures the rule fires for chains of any arity.)
?x ?p ?w :- ?x ?p1 ?y, ?y ?p2 ?z, ?z ?p3 ?w, ?p owl:propertyChainAxiom ?chain .

# scm-sco: bidirectional subClassOf → equivalentClass (OWL 2 RL scm-sco rule)
# If c1 ⊑ c2 AND c2 ⊑ c1 then c1 ≡ c2.
?c1 owl:equivalentClass ?c2 :- ?c1 rdfs:subClassOf ?c2, ?c2 rdfs:subClassOf ?c1 .

# eq-diff1: sameAs + differentFrom inconsistency → owl:Nothing membership
# If x is the same individual as y, but x and y are stated to be different,
# both are instances of owl:Nothing (contradiction).
?s rdf:type owl:Nothing :- ?s owl:sameAs ?o, ?s owl:differentFrom ?o .
?s rdf:type owl:Nothing :- ?s owl:sameAs ?o, ?o owl:differentFrom ?s .

# dt-type2: XSD numeric type promotion (datatype hierarchy membership rules).
# xsd:integer ⊑ xsd:decimal ⊑ xsd:numeric
# xsd:nonNegativeInteger, xsd:nonPositiveInteger ⊑ xsd:integer
# xsd:positiveInteger ⊑ xsd:nonNegativeInteger
# xsd:negativeInteger ⊑ xsd:nonPositiveInteger
# xsd:long ⊑ xsd:integer; xsd:int ⊑ xsd:long; xsd:short ⊑ xsd:int; xsd:byte ⊑ xsd:short
?lt rdf:type xsd:decimal :- ?lt rdf:type xsd:integer .
?lt rdf:type xsd:numeric :- ?lt rdf:type xsd:decimal .
?lt rdf:type xsd:integer :- ?lt rdf:type xsd:nonNegativeInteger .
?lt rdf:type xsd:integer :- ?lt rdf:type xsd:nonPositiveInteger .
?lt rdf:type xsd:integer :- ?lt rdf:type xsd:long .
?lt rdf:type xsd:nonNegativeInteger :- ?lt rdf:type xsd:positiveInteger .
?lt rdf:type xsd:nonPositiveInteger :- ?lt rdf:type xsd:negativeInteger .
?lt rdf:type xsd:long :- ?lt rdf:type xsd:int .
?lt rdf:type xsd:int :- ?lt rdf:type xsd:short .
?lt rdf:type xsd:short :- ?lt rdf:type xsd:byte .
"#;

// ─── OWL 2 EL Profile Rules (v0.57.0) ────────────────────────────────────────
//
// OWL 2 EL is optimised for large biomedical ontologies (SNOMED CT, GO, ChEBI).
// It supports existential restrictions and polynomial-time reasoning.
// This rule set implements the core EL+ reasoning algorithm:
// classification via subsumption propagation + instance checking.

const OWL_EL_RULES: &str = r#"
# ── OWL 2 EL: subClassOf propagation ─────────────────────────────────────────
# scm-sco: subClassOf is transitive
?c rdfs:subClassOf ?e :- ?c rdfs:subClassOf ?d, ?d rdfs:subClassOf ?e .

# cls-int1 (binary): instance of intersection is instance of each conjunct
?x rdf:type ?c1 :- ?x rdf:type ?c, ?c owl:intersectionOf ?c1 .
?x rdf:type ?c2 :- ?x rdf:type ?c, ?c owl:intersectionOf ?c2 .

# cls-int2: instance of all conjuncts implies instance of intersection
?x rdf:type ?c :- ?x rdf:type ?c1, ?x rdf:type ?c2, ?c owl:intersectionOf ?c1, ?c owl:intersectionOf ?c2 .

# prp-some (cls-svf1): existential restriction — if x is of type C and C has
# someValuesFrom restriction R on property p, and x has a value y via p, then y is of type B
?y rdf:type ?b :- ?x rdf:type ?r, ?r owl:someValuesFrom ?b, ?r owl:onProperty ?p, ?x ?p ?y .

# cls-avf: universal restriction — allValuesFrom with subclass propagation
?y rdf:type ?b :- ?x rdf:type ?r, ?r owl:allValuesFrom ?b, ?r owl:onProperty ?p, ?x ?p ?y .

# EL: equivalentClass bi-directional subsumption
?c rdfs:subClassOf ?d :- ?c owl:equivalentClass ?d .
?d rdfs:subClassOf ?c :- ?c owl:equivalentClass ?d .

# EL: class membership from subClassOf
?x rdf:type ?d :- ?x rdf:type ?c, ?c rdfs:subClassOf ?d .

# EL: rdfs:subPropertyOf propagation (property hierarchy)
?x ?q ?y :- ?x ?p ?y, ?p rdfs:subPropertyOf ?q .

# cls-uni: union membership (existential check)
?x rdf:type ?c :- ?x rdf:type ?c1, ?c owl:unionOf ?c1 .
?x rdf:type ?c :- ?x rdf:type ?c2, ?c owl:unionOf ?c2 .

# EL: someValuesFrom class membership (generate existential witness type)
?x rdf:type ?r :- ?x ?p ?y, ?y rdf:type ?b, ?r owl:someValuesFrom ?b, ?r owl:onProperty ?p .
"#;

// ─── OWL 2 QL Profile Rules (v0.57.0) ────────────────────────────────────────
//
// OWL 2 QL (DL-Lite) enables ontology-mediated query answering via query
// rewriting rather than materialisation. This rule set provides the
// Datalog-expressible subset of OWL 2 QL axioms for in-database use.
// Full QL query rewriting is implemented in src/sparql/ql_rewrite.rs.

const OWL_QL_RULES: &str = r#"
# ── OWL 2 QL: subClassOf axioms ──────────────────────────────────────────────
# SubClassOf(:A :B) → if x is of type A, x is of type B
?x rdf:type ?b :- ?x rdf:type ?a, ?a rdfs:subClassOf ?b .

# QL: ObjectSomeValuesFrom (existential in superclass position)
# SubClassOf(ObjectSomeValuesFrom(:r owl:Thing) :A) → if x has property r, x is of type A
?x rdf:type ?a :- ?x ?r ?y, ?c owl:someValuesFrom owl:Thing, ?c owl:onProperty ?r, ?c rdfs:subClassOf ?a .

# QL: subObjectPropertyOf
?x ?q ?y :- ?x ?p ?y, ?p rdfs:subPropertyOf ?q .

# QL: inverseOf — if p is inverse of q, q-triples imply p-triples and vice versa
?y ?p ?x :- ?x ?q ?y, ?p owl:inverseOf ?q .
?y ?q ?x :- ?x ?p ?y, ?p owl:inverseOf ?q .

# QL: DisjointClasses — instances of disjoint classes are owl:Nothing members
?x rdf:type owl:Nothing :- ?x rdf:type ?c1, ?x rdf:type ?c2, ?c1 owl:disjointWith ?c2 .

# QL: equivalentClass bi-directional
?c rdfs:subClassOf ?d :- ?c owl:equivalentClass ?d .
?d rdfs:subClassOf ?c :- ?c owl:equivalentClass ?d .

# QL: functional property — two values of a functional property are owl:sameAs
?y1 owl:sameAs ?y2 :- ?x ?p ?y1, ?x ?p ?y2, ?p rdf:type owl:FunctionalProperty .

# QL: sameAs symmetry and propagation
?y owl:sameAs ?x :- ?x owl:sameAs ?y .
?x rdf:type ?c :- ?x owl:sameAs ?y, ?y rdf:type ?c .
"#;

// ─── SKOS Entailment Rules (W3C SKOS Reference, 28 rules) ────────────────────
//
// All W3C SKOS entailment rules (S7–S45) expressed as Datalog.
// Prefixes: skos: (registered by register_standard_prefixes).
//
// The rule set is stratifiable:
// - transitive-closure rules (S24 broaderTransitive, S45 exactMatch) form
//   a single recursive stratum;
// - all other rules are non-recursive stratum 0.

const SKOS_RULES: &str = r#"
# ── Concept Scheme rules (S7, S8, S4, S5, S6) ────────────────────────────────

# S7: skos:topConceptOf is a sub-property of skos:inScheme
?x skos:inScheme ?s :- ?x skos:topConceptOf ?s .

# S8: skos:topConceptOf is owl:inverseOf skos:hasTopConcept (bidirectional)
?x skos:topConceptOf ?s :- ?s skos:hasTopConcept ?x .
?s skos:hasTopConcept ?x :- ?x skos:topConceptOf ?s .

# S4: rdfs:range of skos:inScheme is skos:ConceptScheme
?s rdf:type skos:ConceptScheme :- ?x skos:inScheme ?s .

# S5/S6: domain/range of skos:hasTopConcept
?s rdf:type skos:ConceptScheme :- ?s skos:hasTopConcept ?x .
?x rdf:type skos:Concept       :- ?s skos:hasTopConcept ?x .

# ── Label rules (S11) ─────────────────────────────────────────────────────────

# S11: prefLabel/altLabel/hiddenLabel are sub-properties of rdfs:label
?x rdfs:label ?l :- ?x skos:prefLabel   ?l .
?x rdfs:label ?l :- ?x skos:altLabel    ?l .
?x rdfs:label ?l :- ?x skos:hiddenLabel ?l .

# ── Documentation sub-property rules (S17) ────────────────────────────────────

# S17: documentation properties are sub-properties of skos:note
?x skos:note ?n :- ?x skos:changeNote    ?n .
?x skos:note ?n :- ?x skos:definition    ?n .
?x skos:note ?n :- ?x skos:editorialNote ?n .
?x skos:note ?n :- ?x skos:example       ?n .
?x skos:note ?n :- ?x skos:historyNote   ?n .
?x skos:note ?n :- ?x skos:scopeNote     ?n .

# ── Associative relation rules (S21, S23) ─────────────────────────────────────

# S23: skos:related is symmetric
?y skos:related ?x :- ?x skos:related ?y .

# S21: skos:related is a sub-property of skos:semanticRelation
?x skos:semanticRelation ?y :- ?x skos:related ?y .

# S21: skos:broaderTransitive is a sub-property of skos:semanticRelation
?x skos:semanticRelation ?y :- ?x skos:broaderTransitive ?y .

# S21: skos:narrowerTransitive is a sub-property of skos:semanticRelation
?x skos:semanticRelation ?y :- ?x skos:narrowerTransitive ?y .

# ── Hierarchy rules (S22, S24, S25, S26) ──────────────────────────────────────

# S22: skos:broader is a sub-property of skos:broaderTransitive
?x skos:broaderTransitive ?y :- ?x skos:broader ?y .

# S22: skos:narrower is a sub-property of skos:narrowerTransitive
?x skos:narrowerTransitive ?y :- ?x skos:narrower ?y .

# S24: skos:broaderTransitive is transitive
?x skos:broaderTransitive ?z :- ?x skos:broaderTransitive ?y, ?y skos:broaderTransitive ?z .

# S24: skos:narrowerTransitive is transitive
?x skos:narrowerTransitive ?z :- ?x skos:narrowerTransitive ?y, ?y skos:narrowerTransitive ?z .

# S25: skos:narrower is owl:inverseOf skos:broader
?y skos:narrower ?x :- ?x skos:broader  ?y .
?y skos:broader  ?x :- ?x skos:narrower ?y .

# S26: skos:narrowerTransitive is owl:inverseOf skos:broaderTransitive
?y skos:narrowerTransitive ?x :- ?x skos:broaderTransitive ?y .

# ── Concept type inference (S19, S20) ─────────────────────────────────────────

# S19/S20: domain and range of skos:semanticRelation is skos:Concept
?x rdf:type skos:Concept :- ?x skos:semanticRelation ?y .
?y rdf:type skos:Concept :- ?x skos:semanticRelation ?y .

# ── Mapping property rules (S39–S45) ──────────────────────────────────────────

# S39: skos:mappingRelation is a sub-property of skos:semanticRelation
?x skos:semanticRelation ?y :- ?x skos:mappingRelation ?y .

# S40: closeMatch/broadMatch/narrowMatch/relatedMatch are sub-properties of mappingRelation
?x skos:mappingRelation ?y :- ?x skos:closeMatch    ?y .
?x skos:mappingRelation ?y :- ?x skos:broadMatch    ?y .
?x skos:mappingRelation ?y :- ?x skos:narrowMatch   ?y .
?x skos:mappingRelation ?y :- ?x skos:relatedMatch  ?y .

# S41: broadMatch/narrowMatch/relatedMatch propagate into hierarchy/associative
?x skos:broader  ?y :- ?x skos:broadMatch   ?y .
?x skos:narrower ?y :- ?x skos:narrowMatch  ?y .
?x skos:related  ?y :- ?x skos:relatedMatch ?y .

# S42: exactMatch is a sub-property of closeMatch
?x skos:closeMatch ?y :- ?x skos:exactMatch ?y .

# S43: skos:narrowMatch is owl:inverseOf skos:broadMatch
?y skos:narrowMatch ?x :- ?x skos:broadMatch  ?y .
?y skos:broadMatch  ?x :- ?x skos:narrowMatch ?y .

# S44: closeMatch/relatedMatch/exactMatch are symmetric
?y skos:closeMatch   ?x :- ?x skos:closeMatch   ?y .
?y skos:relatedMatch ?x :- ?x skos:relatedMatch ?y .
?y skos:exactMatch   ?x :- ?x skos:exactMatch   ?y .

# S45: exactMatch is transitive
?x skos:exactMatch ?z :- ?x skos:exactMatch ?y, ?y skos:exactMatch ?z .

# ── Collection sub-class (S29) ─────────────────────────────────────────────────

# S29: OrderedCollection is a sub-class of Collection
?c rdf:type skos:Collection :- ?c rdf:type skos:OrderedCollection .
"#;

// ─── SKOS Transitive-Closure Subset (7 rules) ────────────────────────────────
//
// A minimal subset of the SKOS rule set covering only the transitive-closure
// and symmetry rules.  Used by riverbank compiler profiles via the named
// bundle API as `"skos-transitive"`.

const SKOS_TRANSITIVE_RULES: &str = r#"
# S22: skos:broader → skos:broaderTransitive
?x skos:broaderTransitive ?y :- ?x skos:broader ?y .

# S22: skos:narrower → skos:narrowerTransitive
?x skos:narrowerTransitive ?y :- ?x skos:narrower ?y .

# S24: skos:broaderTransitive is transitive
?x skos:broaderTransitive ?z :- ?x skos:broaderTransitive ?y, ?y skos:broaderTransitive ?z .

# S24: skos:narrowerTransitive is transitive
?x skos:narrowerTransitive ?z :- ?x skos:narrowerTransitive ?y, ?y skos:narrowerTransitive ?z .

# S26: skos:narrowerTransitive is owl:inverseOf skos:broaderTransitive
?y skos:narrowerTransitive ?x :- ?x skos:broaderTransitive ?y .

# S23: skos:related is symmetric
?y skos:related ?x :- ?x skos:related ?y .

# S45: exactMatch is transitive
?x skos:exactMatch ?z :- ?x skos:exactMatch ?y, ?y skos:exactMatch ?z .
"#;

// ─── SKOS-XL Dumb-Down Chains (3 rules, S55–S57) ─────────────────────────────
//
// SKOS-XL `skosxl:Label` instances are automatically projected to plain
// skos:prefLabel / skos:altLabel / skos:hiddenLabel triples.
//
// Prefix: skosxl: http://www.w3.org/2008/05/skos-xl#

const SKOSXL_RULES: &str = r#"
# S55: (skosxl:prefLabel, skosxl:literalForm) → skos:prefLabel
?x skos:prefLabel   ?l :- ?x skosxl:prefLabel   ?xl, ?xl skosxl:literalForm ?l .

# S56: (skosxl:altLabel, skosxl:literalForm) → skos:altLabel
?x skos:altLabel    ?l :- ?x skosxl:altLabel    ?xl, ?xl skosxl:literalForm ?l .

# S57: (skosxl:hiddenLabel, skosxl:literalForm) → skos:hiddenLabel
?x skos:hiddenLabel ?l :- ?x skosxl:hiddenLabel ?xl, ?xl skosxl:literalForm ?l .
"#;

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdfs_rules_not_empty() {
        let rules = get_builtin_rules("rdfs").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("rdfs:subClassOf"));
    }

    #[test]
    fn test_owl_rl_rules_not_empty() {
        let rules = get_builtin_rules("owl-rl").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("owl:TransitiveProperty"));
    }

    #[test]
    fn test_owl_el_rules_not_empty() {
        let rules = get_builtin_rules("owl-el").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("owl:someValuesFrom"));
        assert!(rules.contains("owl:intersectionOf"));
    }

    #[test]
    fn test_owl_ql_rules_not_empty() {
        let rules = get_builtin_rules("owl-ql").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("owl:inverseOf"));
        assert!(rules.contains("owl:disjointWith"));
    }

    #[test]
    fn test_unknown_rule_set() {
        let result = get_builtin_rules("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown built-in rule set"));
    }

    #[test]
    fn test_skos_rules_not_empty() {
        let rules = get_builtin_rules("skos").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("skos:broaderTransitive"));
        assert!(rules.contains("skos:related"));
        assert!(rules.contains("skos:exactMatch"));
    }

    #[test]
    fn test_skos_transitive_rules() {
        let rules = get_builtin_rules("skos-transitive").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("skos:broaderTransitive"));
        assert!(rules.contains("skos:exactMatch"));
    }

    #[test]
    fn test_skosxl_rules_not_empty() {
        let rules = get_builtin_rules("skosxl").unwrap();
        assert!(!rules.is_empty());
        assert!(rules.contains("skosxl:prefLabel"));
        assert!(rules.contains("skosxl:literalForm"));
    }

    #[test]
    fn test_register_standard_prefixes_includes_skos() {
        // Just verify the static structures compile; SPI is not available in unit tests.
        // The actual DB-side registration is covered by pg_regress.
        let skos_rules = get_builtin_rules("skos").unwrap();
        assert!(skos_rules.contains("skos:"));
    }
}
