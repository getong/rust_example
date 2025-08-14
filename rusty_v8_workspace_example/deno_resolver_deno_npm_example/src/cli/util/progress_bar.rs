use crate::cli::http_util::UpdateGuard;

#[derive(Clone, Debug)]
pub struct ProgressBar;

#[derive(Debug)]
pub enum ProgressBarStyle {
    TextOnly,
}

#[derive(Debug)]
pub enum ProgressMessagePrompt {
    Initialize,
}

impl ProgressBar {
    pub fn new(_style: ProgressBarStyle) -> Self {
        Self
    }
    
    pub fn update(&self, _msg: &str) -> UpdateGuard {
        UpdateGuard
    }
    
    pub fn update_with_prompt(&self, _prompt: ProgressMessagePrompt, _msg: &str) -> UpdateGuard {
        UpdateGuard
    }
}