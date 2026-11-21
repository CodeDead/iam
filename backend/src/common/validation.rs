use std::collections::HashMap;

use mongodb::bson::oid::ObjectId;
use validator::{Validate, ValidationError};

use crate::common::errors::AppError;

pub fn parse_object_id(id: &str) -> Result<ObjectId, AppError> {
    ObjectId::parse_str(id.trim())
        .map_err(|_| AppError::bad_request("INVALID_ID", format!("Invalid resource ID: '{id}'")))
}

pub fn validate_dto<T: Validate>(dto: &T) -> Result<(), AppError> {
    dto.validate().map_err(|errors| {
        let mut details: HashMap<String, Vec<String>> = HashMap::new();
        for (field, field_errors) in errors.field_errors() {
            let messages: Vec<String> = field_errors
                .iter()
                .map(|err| {
                    err.message
                        .as_ref()
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| format!("Invalid value for {field} ({})", err.code))
                })
                .collect();
            details.insert(field.to_string(), messages);
        }
        AppError::validation("One or more input fields failed validation", details)
    })
}

pub fn validate_password_strength(password: &str) -> Result<(), ValidationError> {
    if password.len() < 8 || password.len() > 128 {
        let mut err = ValidationError::new("password_length");
        err.message = Some("Password must be between 8 and 128 characters".into());
        return Err(err);
    }

    let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && !c.is_whitespace());

    if !(has_upper && has_lower && has_digit && has_special) {
        let mut err = ValidationError::new("password_complexity");
        err.message = Some(
            "Password must contain at least one uppercase letter, lowercase letter, digit, and special character".into(),
        );
        return Err(err);
    }

    Ok(())
}

pub fn validate_permission_segment(segment: &str) -> Result<(), ValidationError> {
    if segment.is_empty()
        || segment.len() > 64
        || !segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        || !segment
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
    {
        let mut err = ValidationError::new("invalid_segment");
        err.message = Some(
            "Must start with a lowercase letter and contain only lowercase letters, digits, and underscores".into(),
        );
        return Err(err);
    }
    Ok(())
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}
