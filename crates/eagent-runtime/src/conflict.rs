use eagent_protocol::ids::TaskId;
use serde::{Deserialize, Serialize};

/// Kind of file mutation produced by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMutationKind {
    Create,
    Edit,
    Delete,
}

/// A file mutation produced by an agent during task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMutation {
    /// The file path relative to workspace root.
    pub path: String,
    /// What kind of mutation this is.
    pub kind: FileMutationKind,
    /// Full file content (for Create, or for Edit when providing the whole file).
    pub content: Option<String>,
    /// Unified diff (for Edit operations).
    pub diff: Option<String>,
    /// Which task produced this mutation.
    pub task_id: TaskId,
}

/// A conflict between two agents' file mutations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// The file path that is in conflict.
    pub path: String,
    /// The task that produced the first mutation.
    pub task_a: TaskId,
    /// The task that produced the conflicting mutation.
    pub task_b: TaskId,
    /// Human-readable description of the conflict.
    pub description: String,
}

/// Simplified conflict resolver for v1. Detects when two agents modify the
/// same file and flags it as a conflict. Non-conflicting mutations from
/// different agents are merged into a single list.
pub struct ConflictResolver;

impl ConflictResolver {
    /// Check if two sets of file mutations conflict (i.e., both touch the same file).
    pub fn check_conflicts(
        mutations_a: &[FileMutation],
        mutations_b: &[FileMutation],
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();

        for a in mutations_a {
            for b in mutations_b {
                if a.path == b.path {
                    let description = format!(
                        "Both tasks modified '{}': task {} ({:?}) vs task {} ({:?})",
                        a.path, a.task_id, a.kind, b.task_id, b.kind,
                    );
                    conflicts.push(Conflict {
                        path: a.path.clone(),
                        task_a: a.task_id,
                        task_b: b.task_id,
                        description,
                    });
                }
            }
        }

        conflicts
    }

    /// Attempt to merge multiple sets of file mutations.
    ///
    /// If any two sets touch the same file, this returns an error with all
    /// detected conflicts. Otherwise, returns the merged (flattened) list of
    /// mutations.
    pub fn merge_mutations(
        all_mutations: Vec<Vec<FileMutation>>,
    ) -> Result<Vec<FileMutation>, Vec<Conflict>> {
        let mut all_conflicts = Vec::new();

        // Check every pair of mutation sets for conflicts
        for i in 0..all_mutations.len() {
            for j in (i + 1)..all_mutations.len() {
                let conflicts =
                    Self::check_conflicts(&all_mutations[i], &all_mutations[j]);
                all_conflicts.extend(conflicts);
            }
        }

        if !all_conflicts.is_empty() {
            return Err(all_conflicts);
        }

        // No conflicts — flatten all mutations into one list
        let merged: Vec<FileMutation> = all_mutations.into_iter().flatten().collect();
        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mutation(path: &str, task_id: TaskId, kind: FileMutationKind) -> FileMutation {
        FileMutation {
            path: path.to_string(),
            kind,
            content: Some("content".into()),
            diff: None,
            task_id,
        }
    }

    #[test]
    fn no_conflicts_when_different_files() {
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let mutations_a = vec![make_mutation("src/foo.rs", task_a, FileMutationKind::Edit)];
        let mutations_b = vec![make_mutation("src/bar.rs", task_b, FileMutationKind::Edit)];

        let conflicts = ConflictResolver::check_conflicts(&mutations_a, &mutations_b);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn conflict_detected_on_same_file() {
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let mutations_a = vec![make_mutation("src/lib.rs", task_a, FileMutationKind::Edit)];
        let mutations_b = vec![make_mutation("src/lib.rs", task_b, FileMutationKind::Edit)];

        let conflicts = ConflictResolver::check_conflicts(&mutations_a, &mutations_b);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "src/lib.rs");
        assert_eq!(conflicts[0].task_a, task_a);
        assert_eq!(conflicts[0].task_b, task_b);
    }

    #[test]
    fn conflict_between_create_and_edit() {
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let mutations_a = vec![make_mutation("new_file.rs", task_a, FileMutationKind::Create)];
        let mutations_b = vec![make_mutation("new_file.rs", task_b, FileMutationKind::Edit)];

        let conflicts = ConflictResolver::check_conflicts(&mutations_a, &mutations_b);
        assert_eq!(conflicts.len(), 1);
    }

    #[test]
    fn merge_non_conflicting_succeeds() {
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let task_c = TaskId::new();
        let sets = vec![
            vec![make_mutation("a.rs", task_a, FileMutationKind::Create)],
            vec![make_mutation("b.rs", task_b, FileMutationKind::Edit)],
            vec![make_mutation("c.rs", task_c, FileMutationKind::Delete)],
        ];

        let merged = ConflictResolver::merge_mutations(sets).unwrap();
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn merge_conflicting_returns_error() {
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let sets = vec![
            vec![make_mutation("shared.rs", task_a, FileMutationKind::Edit)],
            vec![make_mutation("shared.rs", task_b, FileMutationKind::Edit)],
        ];

        let err = ConflictResolver::merge_mutations(sets).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].path, "shared.rs");
    }

    #[test]
    fn merge_empty_sets_succeeds() {
        let merged = ConflictResolver::merge_mutations(vec![]).unwrap();
        assert!(merged.is_empty());
    }

    #[test]
    fn multiple_conflicts_all_reported() {
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let mutations_a = vec![
            make_mutation("x.rs", task_a, FileMutationKind::Edit),
            make_mutation("y.rs", task_a, FileMutationKind::Edit),
        ];
        let mutations_b = vec![
            make_mutation("x.rs", task_b, FileMutationKind::Edit),
            make_mutation("y.rs", task_b, FileMutationKind::Create),
        ];

        let conflicts = ConflictResolver::check_conflicts(&mutations_a, &mutations_b);
        assert_eq!(conflicts.len(), 2);
    }
}
