//! Composable questionnaire controls.
//!
//! The behavior — answers, validation, navigation, focus and shortcuts — lives
//! in [`crate::tabs::questionnaire_base`]; this module is its skin. The public path
//! stays `gpui_component::questionnaire::*` for both halves.

mod components;

pub use components::*;

pub use crate::tabs::questionnaire_base::{
  QuestionnaireAnswer, QuestionnaireAnswerChange, QuestionnaireAnswers,
  QuestionnaireChoiceDefinition, QuestionnaireChoiceState, QuestionnaireEvent,
  QuestionnaireInputDefinition, QuestionnaireItemDefinition, QuestionnaireItemState,
  QuestionnaireItemStatus, QuestionnaireNavigationState, QuestionnaireProgressState,
  QuestionnaireSchemaError, QuestionnaireShortcutMode, QuestionnaireState, QuestionnaireSubmission,
  QuestionnaireSubmissionItem, QuestionnaireValidationContext, QuestionnaireValidationError,
  QuestionnaireValidator,
};

pub(crate) fn init(_: &mut gpui_kit::App) {}
