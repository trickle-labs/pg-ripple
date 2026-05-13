//! pgvector embedding, RAG, and hybrid search integration (v0.27.0+).
//!
//! # Module layout (v0.114.0)
//!
//! | Sub-module | Contents |
//! |---|---|
//! | `index`   | Runtime checks, API client, store/similar/embed/refresh, SPARQL translation |
//! | `hybrid`  | Hybrid RRF search combining SPARQL and vector results |
//! | `rag`     | RAG infrastructure: list_models, add_triples, contextualize, rag_retrieve |

pub mod hybrid;
pub mod index;
pub mod rag;

// Public re-exports (keep crate-visibility for pub(crate) items)
pub use hybrid::hybrid_search;
pub(crate) use index::{
    call_embedding_api_pub, embed_entities, has_pgvector, refresh_embeddings,
    similar_entities, sql_for_pg_similar, store_embedding,
};
pub use rag::{add_embedding_triples, contextualize_entity, list_embedding_models, rag_retrieve};
