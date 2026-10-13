//! Type checking for VB6 symbols.
//!
//! The shared type model ([`TypeInfo`], [`VBType`]) lives in `vb6core`; this
//! module adds the [`TypeChecker`], which applies VB6 assignment and operation
//! rules on top of it.

pub use vb6core::types::{ArrayBound, TypeInfo, VBType};

use crate::error::{Result, SemanticError, SourceLocation};

/// Type checker for VB6 code
pub struct TypeChecker {
    // TODO: store type relationships, conversion rules, etc.
}

impl TypeChecker {
    /// Create a new type checker instance
    pub fn new() -> Self {
        Self {}
    }

    /// Check if an assignment is valid
    pub fn check_assignment(
        &self,
        target_type: &TypeInfo,
        source_type: &TypeInfo,
        _location: &SourceLocation,
    ) -> Result<()> {
        if source_type.can_assign_to(target_type) {
            Ok(())
        } else {
            Err(SemanticError::TypeMismatch {
                expected: target_type.to_string(),
                found: source_type.to_string(),
                location: _location.clone(),
            })
        }
    }

    /// Check if two types are compatible for an operation
    pub fn check_operation(
        &self,
        left_type: &TypeInfo,
        right_type: &TypeInfo,
        operation: &str,
        location: &SourceLocation,
    ) -> Result<TypeInfo> {
        let operator = operation.to_ascii_lowercase();
        if operation == "&"
            && (matches!(
                left_type.kind,
                VBType::Variant | VBType::String | VBType::Boolean | VBType::Date
            ) || self.is_numeric(&left_type.kind))
            && (matches!(
                right_type.kind,
                VBType::Variant | VBType::String | VBType::Boolean | VBType::Date
            ) || self.is_numeric(&right_type.kind))
        {
            return Ok(TypeInfo::string());
        }
        if matches!(left_type.kind, VBType::Variant) || matches!(right_type.kind, VBType::Variant) {
            return Ok(TypeInfo::variant());
        }

        let both_numeric = self.is_numeric(&left_type.kind) && self.is_numeric(&right_type.kind);
        if both_numeric
            && matches!(
                operator.as_str(),
                "+" | "-" | "*" | "/" | "\\" | "^" | "mod"
            )
        {
            return Ok(self.promote_numeric_types(left_type, right_type));
        }
        if operator == "+" && left_type.kind == VBType::String && right_type.kind == VBType::String
        {
            return Ok(TypeInfo::string());
        }
        if matches!(operator.as_str(), "=" | "<>" | "<" | ">" | "<=" | ">=")
            && (both_numeric || left_type.kind == right_type.kind)
        {
            return Ok(TypeInfo::new(VBType::Boolean));
        }
        if matches!(operator.as_str(), "and" | "or" | "xor" | "eqv" | "imp") {
            if both_numeric {
                return Ok(self.promote_numeric_types(left_type, right_type));
            }
            if left_type.kind == VBType::Boolean && right_type.kind == VBType::Boolean {
                return Ok(TypeInfo::new(VBType::Boolean));
            }
        }
        Err(SemanticError::InvalidOperation {
            message: format!(
                "operator `{operation}` is not supported for {} and {}",
                left_type, right_type
            ),
            location: location.clone(),
        })
    }

    fn is_numeric(&self, kind: &VBType) -> bool {
        matches!(
            kind,
            VBType::Integer
                | VBType::Long
                | VBType::Single
                | VBType::Double
                | VBType::Currency
                | VBType::Byte
        )
    }

    fn promote_numeric_types(&self, left: &TypeInfo, right: &TypeInfo) -> TypeInfo {
        // Promotion rules: Byte < Integer < Long < Single < Double < Currency
        use VBType::*;

        let promoted = match (&left.kind, &right.kind) {
            (Double, _) | (_, Double) => Double,
            (Currency, _) | (_, Currency) => Currency,
            (Single, _) | (_, Single) => Single,
            (Long, _) | (_, Long) => Long,
            (Integer, _) | (_, Integer) => Integer,
            (Byte, Byte) => Byte,
            _ => Variant,
        };

        TypeInfo::new(promoted)
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
