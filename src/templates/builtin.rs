use super::Template;

/// Load all built-in templates embedded in the binary
pub fn load_builtin_templates() -> Vec<Template> {
    vec![
        include_template!("builtin/api-endpoint.yaml"),
        include_template!("builtin/react-component.yaml"),
        include_template!("builtin/cli-command.yaml"),
        include_template!("builtin/extract-function.yaml"),
        include_template!("builtin/clean-imports.yaml"),
        include_template!("builtin/add-tests.yaml"),
        include_template!("builtin/integration-test.yaml"),
        include_template!("builtin/fix-bug.yaml"),
        include_template!("builtin/add-docs.yaml"),
    ]
}

/// Macro to include and parse a YAML template file at compile time
macro_rules! include_template {
    ($path:expr) => {{
        let yaml = include_str!($path);
        serde_yaml::from_str(yaml).expect(concat!("Failed to parse ", $path))
    }};
}

use include_template;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_all_builtin_templates() {
        let templates = load_builtin_templates();
        assert_eq!(templates.len(), 9);
    }

    #[test]
    fn test_all_templates_have_names() {
        let templates = load_builtin_templates();
        for template in templates {
            assert!(!template.name.is_empty());
            assert!(!template.description.is_empty());
        }
    }

    #[test]
    fn test_template_names_are_unique() {
        let templates = load_builtin_templates();
        let mut names = std::collections::HashSet::new();
        for template in templates {
            assert!(
                names.insert(template.name.clone()),
                "Duplicate template name: {}",
                template.name
            );
        }
    }
}
