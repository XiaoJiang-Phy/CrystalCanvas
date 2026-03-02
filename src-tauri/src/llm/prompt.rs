//! [Overview: LLM Prompt template definitions for context assembly]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::llm::context::CrystalContext;
use crate::llm::provider::ChatMessage;

pub const SYSTEM_PROMPT_TEMPLATE: &str = r#"
You are CrystalCanvas's modeling assistant. You MUST respond with a JSON 
object matching the CrystalCommand schema. Do NOT include any markdown formatting 
ticks (```json) around your output, and do not include any explanatory text 
outside the JSON block.

If the user's request is ambiguous, unsupported, or inherently dangerous, output:
{"action": "clarify", "params": {"question": "Your explanatory question here"}}

Available actions:
1. "delete_atoms" {"indices": [int, ...]}
2. "add_atom" {"element": "Si", "frac_pos": [0.0, 0.0, 0.0]}
3. "substitute" {"indices": [int, ...], "new_element": "Si"}
4. "cleave_slab" {"miller": [1, 0, 0], "layers": 3, "vacuum_a": 15.0} (vacuum range [5.0, 100.0])
5. "make_supercell" {"matrix": [[2, 0, 0], [0, 2, 0], [0, 0, 2]]} (Det > 0)
6. "export_file" {"format": "POSCAR"|"LAMMPS"|"QE", "path": "/path/to/file"}
7. "batch" {"commands": [{...}, {...}]}

Current Crystal State Summary:
"#;

pub fn build_messages(context: &CrystalContext, user_input: &str) -> Vec<ChatMessage> {
    let context_json = serde_json::to_string_pretty(context).unwrap_or_else(|_| "{}".to_string());
    let full_system_prompt = format!("{}{}", SYSTEM_PROMPT_TEMPLATE, context_json);

    vec![
        ChatMessage {
            role: "system".to_string(),
            content: full_system_prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: user_input.to_string(),
        },
    ]
}
