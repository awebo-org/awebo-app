//! Trusted sandbox image registry.
//!
//! Only vetted images from official sources are allowed.
//! Each image has a display name, description, OCI reference, and category.

use std::fmt;

/// Category for grouping images in the picker UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageCategory {
    Base,
}

impl fmt::Display for ImageCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base => write!(f, "Base"),
        }
    }
}

/// A trusted sandbox image definition.
#[derive(Debug, Clone)]
pub struct SandboxImage {
    /// Unique identifier (used in config persistence).
    pub id: &'static str,
    /// Human-readable name shown in the picker.
    pub display_name: &'static str,
    /// Short description shown below the name.
    pub description: &'static str,
    /// OCI image reference (registry/repo:tag).
    pub oci_ref: &'static str,
    /// Category for UI grouping.
    pub category: ImageCategory,
    /// Default shell path inside the image.
    pub default_shell: &'static str,
    /// Default working directory inside the sandbox.
    pub default_workdir: &'static str,
}

/// Query: the full list of trusted images.
pub const IMAGES: &[SandboxImage] = &[
    SandboxImage {
        id: "alpine",
        display_name: "Alpine Linux",
        description: "Minimal 5MB Linux",
        oci_ref: "alpine:latest",
        category: ImageCategory::Base,
        default_shell: "/bin/sh",
        default_workdir: "/root",
    },
    SandboxImage {
        id: "node",
        display_name: "Node.js",
        description: "Node.js LTS with npm",
        oci_ref: "node:current-alpine",
        category: ImageCategory::Base,
        default_shell: "/bin/sh",
        default_workdir: "/root",
    },
];

/// Query: find an image by its unique id.
pub fn image_by_id(id: &str) -> Option<&'static SandboxImage> {
    IMAGES.iter().find(|img| img.id == id)
}

/// Query: images grouped by category (preserves order within each group).
#[cfg(test)]
pub fn images_by_category() -> Vec<(ImageCategory, Vec<&'static SandboxImage>)> {
    use ImageCategory::*;
    let categories = [Base];
    categories
        .iter()
        .map(|&cat| {
            let imgs: Vec<_> = IMAGES.iter().filter(|img| img.category == cat).collect();
            (cat, imgs)
        })
        .filter(|(_, imgs)| !imgs.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_images_have_unique_ids() {
        let mut ids: Vec<_> = IMAGES.iter().map(|i| i.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), IMAGES.len());
    }

    #[test]
    fn image_by_id_finds_alpine() {
        let img = image_by_id("alpine").unwrap();
        assert_eq!(img.display_name, "Alpine Linux");
    }

    #[test]
    fn image_by_id_returns_none_for_unknown() {
        assert!(image_by_id("nonexistent").is_none());
    }

    #[test]
    fn images_by_category_covers_all() {
        let grouped = images_by_category();
        let total: usize = grouped.iter().map(|(_, v)| v.len()).sum();
        assert_eq!(total, IMAGES.len());
    }
}
