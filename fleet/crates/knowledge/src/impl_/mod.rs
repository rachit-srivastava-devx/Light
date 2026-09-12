
mod bandit;
mod dedup;
mod embedding;
mod fusion;
mod gate_check;
mod ident;
mod item;
mod promote;
mod retrieve;
mod score;

pub use bandit::{pick_arm, ArmStats, NoArms, RandomSource};
pub use dedup::{write, DedupThreshold, NearestNeighborLookup, NewMemory, WriteDecision};
pub use embedding::{CosineSimilarity, DimensionMismatch, Embedding, EmptyEmbedding};
pub use fusion::RRF_K;
pub use gate_check::{check_added_line, PatternError, PatternMatcher};
pub use ident::{EmptyMemoryId, MemoryId, MemoryKind};
pub use item::{Importance, ImportanceOutOfRange, MemoryItem, Timestamp};
pub use promote::{promote_lesson, DiffPattern, PromotedLesson, PromotionRefusal, PromotionScope};
pub use retrieve::{retrieve, LexicalHit, LexicalSearch, RetrieveError, RetrievedItem, VectorHit, VectorSearch};
pub use score::{score, Relevance, Score, ScoreWeights, RECENCY_HALF_LIFE_SECS};
