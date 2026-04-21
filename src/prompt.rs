pub const SYSTEM: &str = "You are a Linux command assistant. Given a user intent and an <env> block, reply with the minimum commands needed, in the exact format:\n\n$ <command>\n<one-line purpose, under 80 chars>\n\nRules:\n- Multiple steps => multiple `$ ` blocks in order.\n- Each command MUST start with `$ ` on its own line; explanation is a single line right below.\n- No prose before or after. No markdown code fences. No extra blank lines.\n- Prefer non-destructive commands. Use sudo only if the env indicates it is needed and available.\n- Pick commands compatible with the distro, kernel, shell, and available tools given in <env>.\n- Keep commands copy-pasteable on one line where possible.\n- Answer in the user's language for the explanation line.";

const STDIN_LIMIT: usize = 4 * 1024;

pub fn build_user_message(env_block: &str, stdin: Option<&str>, question: &str) -> String {
    let mut s = String::new();
    s.push_str("<env>\n");
    s.push_str(env_block);
    s.push_str("</env>\n");
    if let Some(ctx) = stdin {
        let ctx = if ctx.len() > STDIN_LIMIT {
            let start = ctx.len() - STDIN_LIMIT;
            let slice = &ctx[start..];
            // avoid splitting a UTF-8 codepoint
            let adjusted = slice
                .char_indices()
                .next()
                .map(|(i, _)| &slice[i..])
                .unwrap_or(slice);
            adjusted
        } else {
            ctx
        };
        s.push_str("<stdin>\n");
        s.push_str(ctx.trim_end());
        s.push_str("\n</stdin>\n");
    }
    s.push_str("<ask>\n");
    s.push_str(question.trim());
    s.push_str("\n</ask>\n");
    s
}
