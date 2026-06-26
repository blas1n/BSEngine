use bevy_ecs::prelude::Component;

/// Root-level binding between a mesh entity and its bone hierarchy.
/// The animation system drives `Bone` entities referenced here each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Skeleton {
    /// Names of all bones, ordered by bone index.
    /// Must match the joints in the associated `SkinnedMesh` primitive.
    pub bone_names: Vec<String>,
    /// Per-bone parent index. `None` means the bone is a root bone.
    pub parent_indices: Vec<Option<usize>>,
    /// Root bone indices (bones with no parent).
    pub roots: Vec<usize>,
}

impl Skeleton {
    /// Build a skeleton from `(name, parent_index)` pairs.
    /// Pass `None` as parent for root bones.
    pub fn from_bones(bones: impl IntoIterator<Item = (impl Into<String>, Option<usize>)>) -> Self {
        let mut bone_names = Vec::new();
        let mut parent_indices = Vec::new();
        let mut roots = Vec::new();

        for (i, (name, parent)) in bones.into_iter().enumerate() {
            bone_names.push(name.into());
            parent_indices.push(parent);
            if parent.is_none() {
                roots.push(i);
            }
        }

        Self {
            bone_names,
            parent_indices,
            roots,
        }
    }

    /// Number of bones in this skeleton.
    pub fn bone_count(&self) -> usize {
        self.bone_names.len()
    }

    /// Returns the name of the bone at the given index, if valid.
    pub fn bone_name(&self, index: usize) -> Option<&str> {
        self.bone_names.get(index).map(|s| s.as_str())
    }

    /// Finds a bone index by name. Linear scan — call sparingly.
    pub fn find_bone(&self, name: &str) -> Option<usize> {
        self.bone_names.iter().position(|n| n == name)
    }

    /// Returns child indices for the given bone index.
    pub fn children_of(&self, parent_index: usize) -> impl Iterator<Item = usize> + '_ {
        self.parent_indices
            .iter()
            .enumerate()
            .filter_map(move |(i, p)| {
                if *p == Some(parent_index) {
                    Some(i)
                } else {
                    None
                }
            })
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self {
            bone_names: Vec::new(),
            parent_indices: Vec::new(),
            roots: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_skeleton() -> Skeleton {
        Skeleton::from_bones([
            ("root", None),
            ("spine", Some(0)),
            ("head", Some(1)),
            ("left_arm", Some(1)),
        ])
    }

    #[test]
    fn skeleton_bone_count() {
        let sk = build_test_skeleton();
        assert_eq!(sk.bone_count(), 4);
    }

    #[test]
    fn roots_identified() {
        let sk = build_test_skeleton();
        assert_eq!(sk.roots, vec![0]);
    }

    #[test]
    fn find_bone_by_name() {
        let sk = build_test_skeleton();
        assert_eq!(sk.find_bone("head"), Some(2));
        assert_eq!(sk.find_bone("nonexistent"), None);
    }

    #[test]
    fn children_of_spine() {
        let sk = build_test_skeleton();
        let children: Vec<usize> = sk.children_of(1).collect();
        assert!(children.contains(&2));
        assert!(children.contains(&3));
    }

    #[test]
    fn default_skeleton_empty() {
        let sk = Skeleton::default();
        assert_eq!(sk.bone_count(), 0);
        assert!(sk.roots.is_empty());
    }
}
