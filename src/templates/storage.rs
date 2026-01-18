use anyhow::Result;
use directories::ProjectDirs;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::{Template, TemplateCategory};

/// Storage for templates (builtin, user, and project)
pub struct TemplateStorage {
    /// Built-in templates (embedded in binary)
    builtin: HashMap<String, Template>,
    /// User templates directory (~/.doodoori/templates/)
    user_dir: PathBuf,
    /// Project templates directory (.doodoori/templates/)
    project_dir: Option<PathBuf>,
}

#[allow(dead_code)]
impl TemplateStorage {
    /// Create a new template storage
    pub fn new() -> Result<Self> {
        // Load built-in templates
        let builtin = super::builtin::load_builtin_templates()
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();

        // User templates directory
        let user_dir = if let Some(proj_dirs) = ProjectDirs::from("", "", "doodoori") {
            proj_dirs.config_dir().join("templates")
        } else {
            // Fallback to home directory
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".doodoori")
                .join("templates")
        };

        // Create user directory if it doesn't exist
        if !user_dir.exists() {
            fs::create_dir_all(&user_dir)?;
        }

        // Project templates directory (if in a project)
        let project_dir = std::env::current_dir()
            .ok()
            .map(|cwd| cwd.join(".doodoori").join("templates"))
            .filter(|p| p.exists());

        Ok(Self {
            builtin,
            user_dir,
            project_dir,
        })
    }

    /// List all available templates (returns owned values)
    pub fn list(&self) -> Vec<Template> {
        let mut templates: Vec<Template> = self.builtin.values().cloned().collect();

        // Load user templates
        if let Ok(user_templates) = self.load_from_dir(&self.user_dir) {
            templates.extend(user_templates.into_values());
        }

        // Load project templates
        if let Some(ref project_dir) = self.project_dir {
            if let Ok(project_templates) = self.load_from_dir(project_dir) {
                templates.extend(project_templates.into_values());
            }
        }

        templates
    }

    /// Get a template by name (returns owned value)
    pub fn get(&self, name: &str) -> Option<Template> {
        // Check builtin first
        if let Some(template) = self.builtin.get(name) {
            return Some(template.clone());
        }

        // Check user templates
        if let Ok(user_templates) = self.load_from_dir(&self.user_dir) {
            if let Some(template) = user_templates.get(name) {
                return Some(template.clone());
            }
        }

        // Check project templates
        if let Some(ref project_dir) = self.project_dir {
            if let Ok(project_templates) = self.load_from_dir(project_dir) {
                if let Some(template) = project_templates.get(name) {
                    return Some(template.clone());
                }
            }
        }

        None
    }

    /// Save a user template
    pub fn save_user_template(&self, template: &Template) -> Result<()> {
        let file_path = self.user_dir.join(format!("{}.yaml", template.name));
        let yaml = serde_yaml::to_string(template)?;
        fs::write(file_path, yaml)?;
        Ok(())
    }

    /// Delete a user template
    pub fn delete_user_template(&self, name: &str) -> Result<()> {
        let file_path = self.user_dir.join(format!("{}.yaml", name));
        if !file_path.exists() {
            anyhow::bail!("Template not found: {}", name);
        }
        fs::remove_file(file_path)?;
        Ok(())
    }

    /// Filter templates by category
    pub fn filter_by_category(&self, category: TemplateCategory) -> Vec<Template> {
        self.list()
            .into_iter()
            .filter(|t| t.category == category)
            .collect()
    }

    /// Filter templates by tag
    pub fn filter_by_tag(&self, tag: &str) -> Vec<Template> {
        self.list()
            .into_iter()
            .filter(|t| t.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Load templates from a directory
    fn load_from_dir(&self, dir: &PathBuf) -> Result<HashMap<String, Template>> {
        let mut templates = HashMap::new();

        if !dir.exists() {
            return Ok(templates);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match self.load_template_file(&path) {
                    Ok(template) => {
                        templates.insert(template.name.clone(), template);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load template from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(templates)
    }

    /// Load a single template file
    fn load_template_file(&self, path: &PathBuf) -> Result<Template> {
        let contents = fs::read_to_string(path)?;
        let template: Template = serde_yaml::from_str(&contents)?;
        Ok(template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::TemplateVariable;

    #[test]
    fn test_storage_creation() {
        let storage = TemplateStorage::new();
        assert!(storage.is_ok());
    }

    #[test]
    fn test_list_builtin_templates() {
        let storage = TemplateStorage::new().unwrap();
        let templates = storage.list();
        // Should have at least the builtin templates
        assert!(!templates.is_empty());
    }

    #[test]
    fn test_get_builtin_template() {
        let storage = TemplateStorage::new().unwrap();
        // This will pass once we have builtin templates
        let template = storage.get("add-tests");
        assert!(template.is_some());
    }

    #[test]
    fn test_filter_by_category() {
        let storage = TemplateStorage::new().unwrap();
        let test_templates = storage.filter_by_category(TemplateCategory::Test);
        // Should have at least one test template
        assert!(!test_templates.is_empty());
    }

    #[test]
    fn test_filter_by_tag() {
        let storage = TemplateStorage::new().unwrap();
        let rust_templates = storage.filter_by_tag("rust");
        // Built-in templates should include rust tags
        assert!(!rust_templates.is_empty());
    }

    #[test]
    fn test_save_and_delete_user_template() {
        let storage = TemplateStorage::new().unwrap();

        let template = Template {
            name: "test-template-temp".to_string(),
            description: "Temporary test template".to_string(),
            category: TemplateCategory::Custom,
            prompt: "Test {var}".to_string(),
            variables: vec![TemplateVariable {
                name: "var".to_string(),
                description: "Test variable".to_string(),
                default: None,
                required: true,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec!["test".to_string()],
        };

        // Save
        let save_result = storage.save_user_template(&template);
        assert!(save_result.is_ok());

        // Verify file exists
        let file_path = storage.user_dir.join("test-template-temp.yaml");
        assert!(file_path.exists());

        // Delete
        let delete_result = storage.delete_user_template("test-template-temp");
        assert!(delete_result.is_ok());

        // Verify file is gone
        assert!(!file_path.exists());
    }

    #[test]
    fn test_delete_nonexistent_template() {
        let storage = TemplateStorage::new().unwrap();
        let result = storage.delete_user_template("nonexistent-template-xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Template not found"));
    }
}
