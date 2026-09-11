//! Shell escaping and persona-to-prompt text, shared by every registration
//! argument builder.

use skwad_core::Persona;

/// Escape a string for embedding inside a double-quoted shell argument:
/// backslash, double quote, dollar sign, backtick, and exclamation mark.
pub fn shell_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' | '"' | '$' | '`' | '!' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Build the "impersonate this persona" instruction text, or `None` when the
/// persona has no instructions.
pub fn persona_prompt(persona: Option<&Persona>) -> Option<String> {
    let persona = persona?;
    if persona.instructions.is_empty() {
        return None;
    }
    Some(format!(
        "You are asked to impersonate {} based on the following instructions: {}",
        persona.name, persona.instructions
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use skwad_core::{PersonaState, PersonaType};
    use uuid::Uuid;

    #[test]
    fn shell_escape_covers_every_special_character() {
        assert_eq!(shell_escape(r#"a\b"c$d`e!f"#), r#"a\\b\"c\$d\`e\!f"#);
        assert_eq!(shell_escape("plain text"), "plain text");
    }

    fn persona(instructions: &str) -> Persona {
        Persona {
            id: Uuid::nil(),
            name: "Ada".to_string(),
            instructions: instructions.to_string(),
            persona_type: PersonaType::User,
            state: PersonaState::Enabled,
        }
    }

    #[test]
    fn persona_prompt_populated() {
        let p = persona("Be terse.");
        let prompt = persona_prompt(Some(&p)).unwrap();
        assert!(prompt.contains("Ada"));
        assert!(prompt.contains("Be terse."));
    }

    #[test]
    fn persona_prompt_empty_instructions_is_none() {
        let p = persona("");
        assert_eq!(persona_prompt(Some(&p)), None);
    }

    #[test]
    fn persona_prompt_none_persona_is_none() {
        assert_eq!(persona_prompt(None), None);
    }
}
