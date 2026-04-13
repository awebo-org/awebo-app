pub struct ModelInfo {
    pub name: &'static str,
    pub filename: &'static str,
    pub quant_label: &'static str,
    pub chat_template: &'static str,
    pub context_size: u32,
    /// Model family for filtering (e.g. "Google", "Meta", "OpenAI").
    pub family: &'static str,
    /// Human-readable parameter count (e.g. "3B", "20.9B MoE").
    pub params: &'static str,
    /// Approximate GGUF file size in bytes (for display before download).
    pub size_bytes: u64,
    /// HuggingFace repository id (e.g. "lmstudio-community/Phi-4-mini-instruct-GGUF").
    pub hf_repo: &'static str,
    /// Filename inside the HF repo (same as `filename` unless it differs).
    pub hf_filename: &'static str,
}

pub static MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "Gemma 4 E2B",
        filename: "gemma-4-E2B-it-Q8_0.gguf",
        quant_label: "Q8_0",
        chat_template: "gemma",
        context_size: 32768,
        family: "Google",
        params: "2B",
        size_bytes: 2_700_000_000,
        hf_repo: "lmstudio-community/gemma-4-E2B-it-GGUF",
        hf_filename: "gemma-4-E2B-it-Q8_0.gguf",
    },
    ModelInfo {
        name: "Gemma 4 E4B",
        filename: "gemma-4-E4B-it-Q4_K_M.gguf",
        quant_label: "Q4_K_M",
        chat_template: "gemma",
        context_size: 32768,
        family: "Google",
        params: "4B",
        size_bytes: 2_800_000_000,
        hf_repo: "lmstudio-community/gemma-4-E4B-it-GGUF",
        hf_filename: "gemma-4-E4B-it-Q4_K_M.gguf",
    },
    ModelInfo {
        name: "GPT-oss 20B",
        filename: "gpt-oss-20b-mxfp4.gguf",
        quant_label: "MXFP4",
        chat_template: "chatml",
        context_size: 131072,
        family: "OpenAI",
        params: "20.9B MoE",
        size_bytes: 12_100_000_000,
        hf_repo: "ggml-org/gpt-oss-20b-GGUF",
        hf_filename: "gpt-oss-20b-mxfp4.gguf",
    },
    ModelInfo {
        name: "GPT-oss 120B",
        filename: "gpt-oss-120b-MXFP4.gguf",
        quant_label: "MXFP4",
        chat_template: "chatml",
        context_size: 131072,
        family: "OpenAI",
        params: "116.8B MoE",
        size_bytes: 63_000_000_000,
        hf_repo: "ggml-org/gpt-oss-120b-GGUF",
        hf_filename: "gpt-oss-120b-MXFP4.gguf",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_not_empty() {
        assert!(!MODELS.is_empty());
    }

    #[test]
    fn expanded_registry_has_expected_models() {
        assert_eq!(MODELS.len(), 4);
    }

    #[test]
    fn all_models_have_names() {
        for m in MODELS {
            assert!(!m.name.is_empty());
        }
    }

    #[test]
    fn all_models_have_filenames() {
        for m in MODELS {
            assert!(!m.filename.is_empty());
            assert!(m.filename.ends_with(".gguf"));
        }
    }

    #[test]
    fn all_models_have_quant_labels() {
        for m in MODELS {
            assert!(!m.quant_label.is_empty());
        }
    }

    #[test]
    fn all_models_have_chat_templates() {
        for m in MODELS {
            assert!(!m.chat_template.is_empty());
        }
    }

    #[test]
    fn all_models_have_positive_context_size() {
        for m in MODELS {
            assert!(m.context_size > 0);
        }
    }

    #[test]
    fn all_models_have_family() {
        for m in MODELS {
            assert!(!m.family.is_empty());
        }
    }

    #[test]
    fn all_models_have_params() {
        for m in MODELS {
            assert!(!m.params.is_empty());
        }
    }

    #[test]
    fn all_models_have_positive_size() {
        for m in MODELS {
            assert!(m.size_bytes > 0);
        }
    }

    #[test]
    fn all_models_have_hf_repo() {
        for m in MODELS {
            assert!(!m.hf_repo.is_empty());
            assert!(m.hf_repo.contains('/'));
        }
    }

    #[test]
    fn all_models_have_hf_filename() {
        for m in MODELS {
            assert!(!m.hf_filename.is_empty());
            assert!(m.hf_filename.ends_with(".gguf"));
        }
    }

    #[test]
    fn model_names_unique() {
        let names: Vec<&str> = MODELS.iter().map(|m| m.name).collect();
        for (i, name) in names.iter().enumerate() {
            for (j, other) in names.iter().enumerate() {
                if i != j {
                    assert_ne!(name, other, "Duplicate model name: {}", name);
                }
            }
        }
    }

    #[test]
    fn model_filenames_unique() {
        let filenames: Vec<&str> = MODELS.iter().map(|m| m.filename).collect();
        for (i, f) in filenames.iter().enumerate() {
            for (j, other) in filenames.iter().enumerate() {
                if i != j {
                    assert_ne!(f, other, "Duplicate filename: {}", f);
                }
            }
        }
    }

    #[test]
    fn registry_includes_gpt_oss() {
        assert!(MODELS.iter().any(|m| m.name.contains("GPT-oss")));
    }

    #[test]
    fn registry_includes_gemma() {
        assert!(MODELS.iter().any(|m| m.family == "Google" && m.name.contains("Gemma")));
    }
}
