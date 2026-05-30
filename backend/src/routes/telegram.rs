use anyhow::Result;
use teloxide::{prelude::*, types::ParseMode, utils::markdown};

use crate::state::AppState;

const TELEGRAM_FALLBACK_REPLY: &str =
    "我现在暂时无法处理这个问题，请稍后再试，或联系人工客服进一步确认。";
const TELEGRAM_MESSAGE_CHAR_LIMIT: usize = 4_000;

pub async fn run_long_polling(state: AppState) -> Result<()> {
    let bot = Bot::new(state.config.telegram_bot_token.clone());

    tracing::info!("telegram long polling started");

    teloxide::repl(bot, move |bot: Bot, message: Message| {
        let state = state.clone();

        async move { handle_message(bot, message, state).await }
    })
    .await;

    Ok(())
}

async fn handle_message(bot: Bot, message: Message, state: AppState) -> ResponseResult<()> {
    let Some(text) = message.text() else {
        tracing::debug!(chat_id = ?message.chat.id, "ignored non-text telegram update");
        return Ok(());
    };

    let question = text.trim();
    if question.is_empty() {
        tracing::debug!(chat_id = ?message.chat.id, "ignored empty telegram text message");
        return Ok(());
    }

    tracing::info!(
        chat_id = ?message.chat.id,
        collection = %state.config.chroma_collection,
        "received telegram text message"
    );

    let reply = match state.rag.answer_question(question).await {
        Ok(answer) => {
            tracing::info!(
                chat_id = ?message.chat.id,
                provider = %answer.provider,
                model = %answer.model,
                retrieval_count = answer.retrieval_count,
                "generated telegram RAG reply"
            );
            answer.final_answer
        }
        Err(error) => {
            tracing::error!(chat_id = ?message.chat.id, error = ?error, "telegram RAG query failed");
            TELEGRAM_FALLBACK_REPLY.to_string()
        }
    };

    for part in build_telegram_reply_messages(&strip_citation_markers(&reply)) {
        bot.send_message(message.chat.id, part)
            .parse_mode(ParseMode::MarkdownV2)
            .await?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum TelegramReplyBlock {
    BlankLine,
    CodeBlock { language: String, code: String },
    Heading(String),
    Quote(String),
    UnorderedListItem(String),
    OrderedListItem { index: String, text: String },
    Paragraph(String),
}

#[derive(Debug, Clone)]
enum InlineToken {
    Text(String),
    Strong(String),
    Code(String),
    Link { label: String, url: String },
}

pub fn strip_citation_markers(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            let start = result.len();
            result.push('[');
            let mut has_digit = false;

            while let Some(&next) = chars.peek() {
                if next == ']' {
                    chars.next();
                    if has_digit && chars.peek().map_or(true, |&c| c != '(') {
                        // [N] not followed by ( — strip it
                        result.truncate(start);
                    } else {
                        result.push(']');
                    }
                    break;
                } else if next.is_ascii_digit() {
                    has_digit = true;
                    result.push(next);
                    chars.next();
                } else {
                    result.push(next);
                    chars.next();
                }
            }
        } else {
            result.push(ch);
        }
    }

    // collapse multiple spaces left by stripped markers
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_space = false;
    for ch in result.chars() {
        if ch == ' ' {
            if !prev_space {
                collapsed.push(ch);
            }
            prev_space = true;
        } else {
            prev_space = false;
            collapsed.push(ch);
        }
    }

    collapsed.trim().to_string()
}

pub fn build_telegram_reply_messages(text: &str) -> Vec<String> {
    let blocks = parse_reply_blocks(text);
    let mut messages = Vec::new();
    let mut current = String::new();

    for block in blocks {
        for part in split_block_for_telegram(&block, TELEGRAM_MESSAGE_CHAR_LIMIT) {
            if current.is_empty() {
                current.push_str(&part);
                continue;
            }

            if rendered_len(&current) + 1 + rendered_len(&part) <= TELEGRAM_MESSAGE_CHAR_LIMIT {
                current.push('\n');
                current.push_str(&part);
            } else {
                if !current.is_empty() {
                    messages.push(current);
                }
                current = part;
            }
        }
    }

    if !current.is_empty() {
        messages.push(current);
    }

    if messages.is_empty() {
        vec![markdown::escape(TELEGRAM_FALLBACK_REPLY)]
    } else {
        messages
    }
}

fn split_block_for_telegram(block: &TelegramReplyBlock, limit: usize) -> Vec<String> {
    let rendered = render_reply_block(block);
    if rendered_len(&rendered) <= limit {
        return vec![rendered];
    }

    match block {
        TelegramReplyBlock::BlankLine => vec![String::new()],
        TelegramReplyBlock::CodeBlock { language, code } => {
            split_with_renderer(code, limit, |piece| render_code_block(piece, language))
        }
        TelegramReplyBlock::Heading(text) => split_with_renderer(text, limit, |piece| {
            markdown::bold(&markdown::escape(piece))
        }),
        TelegramReplyBlock::Quote(text) => split_inline_line(text, limit, ">"),
        TelegramReplyBlock::UnorderedListItem(text) => split_inline_line(text, limit, "• "),
        TelegramReplyBlock::OrderedListItem { index, text } => {
            split_inline_line(text, limit, &format!("{}\\. ", index))
        }
        TelegramReplyBlock::Paragraph(text) => split_inline_line(text, limit, ""),
    }
}

fn split_inline_line(text: &str, limit: usize, prefix: &str) -> Vec<String> {
    let tokens = tokenize_inline_markdown(text);
    let prefix_len = rendered_len(prefix);
    let token_limit = limit.saturating_sub(prefix_len).max(1);
    let mut parts = Vec::new();
    let mut current = prefix.to_string();

    for token in tokens {
        for piece in split_inline_token(&token, token_limit) {
            if rendered_len(&current) == prefix_len {
                current.push_str(&piece);
                continue;
            }

            if rendered_len(&current) + rendered_len(&piece) <= limit {
                current.push_str(&piece);
            } else {
                parts.push(current);
                current = prefix.to_string();
                current.push_str(&piece);
            }
        }
    }

    if rendered_len(&current) > prefix_len || (parts.is_empty() && prefix.is_empty()) {
        parts.push(current);
    }

    parts
}

fn split_inline_token(token: &InlineToken, limit: usize) -> Vec<String> {
    match token {
        InlineToken::Text(text) => split_with_renderer(text, limit, markdown::escape),
        InlineToken::Strong(text) => split_with_renderer(text, limit, |piece| {
            markdown::bold(&markdown::escape(piece))
        }),
        InlineToken::Code(text) => split_with_renderer(text, limit, markdown::code_inline),
        InlineToken::Link { label, url } => {
            let rendered = markdown::link(url, &markdown::escape(label));
            if rendered_len(&rendered) <= limit {
                vec![rendered]
            } else {
                let fallback = format!("[{}]({})", label, url);
                split_with_renderer(&fallback, limit, markdown::escape)
            }
        }
    }
}

fn split_with_renderer<F>(text: &str, limit: usize, render: F) -> Vec<String>
where
    F: Fn(&str) -> String,
{
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut parts = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        let mut candidate = current.clone();
        candidate.push(ch);

        if rendered_len(&render(&candidate)) <= limit {
            current = candidate;
            continue;
        }

        if current.is_empty() {
            parts.push(render(&ch.to_string()));
            continue;
        }

        parts.push(render(&current));
        current.clear();
        current.push(ch);
    }

    if !current.is_empty() {
        parts.push(render(&current));
    }

    parts
}

fn parse_reply_blocks(text: &str) -> Vec<TelegramReplyBlock> {
    let normalized = text.replace("\r\n", "\n");
    let mut blocks = Vec::new();
    let mut code_block_language = String::new();
    let mut code_block_lines = Vec::new();
    let mut in_code_block = false;

    for line in normalized.lines() {
        let trimmed = line.trim_start();

        if in_code_block {
            if trimmed.starts_with("```") {
                blocks.push(TelegramReplyBlock::CodeBlock {
                    language: code_block_language.clone(),
                    code: code_block_lines.join("\n"),
                });
                code_block_language.clear();
                code_block_lines.clear();
                in_code_block = false;
            } else {
                code_block_lines.push(line.to_string());
            }

            continue;
        }

        if let Some(language) = trimmed.strip_prefix("```") {
            code_block_language = language.trim().to_string();
            in_code_block = true;
            continue;
        }

        blocks.push(parse_line_block(line));
    }

    if in_code_block {
        blocks.push(TelegramReplyBlock::CodeBlock {
            language: code_block_language,
            code: code_block_lines.join("\n"),
        });
    }

    blocks
}

fn parse_line_block(line: &str) -> TelegramReplyBlock {
    if line.trim().is_empty() {
        return TelegramReplyBlock::BlankLine;
    }

    let trimmed = line.trim_start();

    if let Some(heading) = parse_heading(trimmed) {
        return TelegramReplyBlock::Heading(heading.to_string());
    }

    if let Some(quoted) = trimmed.strip_prefix("> ") {
        return TelegramReplyBlock::Quote(quoted.to_string());
    }

    if let Some(list_item) = parse_unordered_list_item(trimmed) {
        return TelegramReplyBlock::UnorderedListItem(list_item.to_string());
    }

    if let Some((index, list_item)) = parse_ordered_list_item(trimmed) {
        return TelegramReplyBlock::OrderedListItem {
            index: index.to_string(),
            text: list_item.to_string(),
        };
    }

    TelegramReplyBlock::Paragraph(trimmed.to_string())
}

fn render_reply_block(block: &TelegramReplyBlock) -> String {
    match block {
        TelegramReplyBlock::BlankLine => String::new(),
        TelegramReplyBlock::CodeBlock { language, code } => render_code_block(code, language),
        TelegramReplyBlock::Heading(text) => markdown::bold(&markdown::escape(text)),
        TelegramReplyBlock::Quote(text) => markdown::blockquote(&render_inline_markdown(text)),
        TelegramReplyBlock::UnorderedListItem(text) => {
            format!("• {}", render_inline_markdown(text))
        }
        TelegramReplyBlock::OrderedListItem { index, text } => {
            format!("{}\\. {}", index, render_inline_markdown(text))
        }
        TelegramReplyBlock::Paragraph(text) => render_inline_markdown(text),
    }
}

fn render_code_block(code: &str, language: &str) -> String {
    if language.is_empty() {
        markdown::code_block(code)
    } else {
        markdown::code_block_with_lang(code, language)
    }
}

fn tokenize_inline_markdown(text: &str) -> Vec<InlineToken> {
    let mut tokens = Vec::new();
    let mut plain_text = String::new();
    let mut index = 0;

    while index < text.len() {
        let rest = &text[index..];

        if let Some((label, url, consumed)) = parse_markdown_link(rest) {
            flush_plain_text(&mut tokens, &mut plain_text);
            tokens.push(InlineToken::Link {
                label: label.to_string(),
                url: url.to_string(),
            });
            index += consumed;
            continue;
        }

        if let Some((content, consumed)) = parse_strong_span(rest) {
            flush_plain_text(&mut tokens, &mut plain_text);
            tokens.push(InlineToken::Strong(content.to_string()));
            index += consumed;
            continue;
        }

        if let Some((content, consumed)) = parse_inline_code_span(rest) {
            flush_plain_text(&mut tokens, &mut plain_text);
            tokens.push(InlineToken::Code(content.to_string()));
            index += consumed;
            continue;
        }

        let ch = rest.chars().next().expect("inline text should have a char");
        plain_text.push(ch);
        index += ch.len_utf8();
    }

    flush_plain_text(&mut tokens, &mut plain_text);
    tokens
}

fn flush_plain_text(tokens: &mut Vec<InlineToken>, plain_text: &mut String) {
    if plain_text.is_empty() {
        return;
    }

    tokens.push(InlineToken::Text(std::mem::take(plain_text)));
}

fn render_inline_markdown(text: &str) -> String {
    tokenize_inline_markdown(text)
        .into_iter()
        .map(|token| match token {
            InlineToken::Text(text) => markdown::escape(&text),
            InlineToken::Strong(text) => markdown::bold(&markdown::escape(&text)),
            InlineToken::Code(text) => markdown::code_inline(&text),
            InlineToken::Link { label, url } => markdown::link(&url, &markdown::escape(&label)),
        })
        .collect::<String>()
}

fn parse_heading(line: &str) -> Option<&str> {
    let level = line.chars().take_while(|ch| *ch == '#').count();

    if level == 0 {
        return None;
    }

    let marker_len = line
        .char_indices()
        .nth(level)
        .map(|(idx, _)| idx)
        .unwrap_or(line.len());
    line[marker_len..].strip_prefix(' ').map(str::trim)
}

fn parse_unordered_list_item(line: &str) -> Option<&str> {
    ["- ", "* ", "+ "]
        .into_iter()
        .find_map(|prefix| line.strip_prefix(prefix).map(str::trim))
}

fn parse_ordered_list_item(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut digits_end = 0;

    while digits_end < bytes.len() && bytes[digits_end].is_ascii_digit() {
        digits_end += 1;
    }

    if digits_end == 0 || !line[digits_end..].starts_with(". ") {
        return None;
    }

    Some((&line[..digits_end], line[digits_end + 2..].trim()))
}

fn parse_markdown_link(text: &str) -> Option<(&str, &str, usize)> {
    if !text.starts_with('[') {
        return None;
    }

    let label_end = text[1..].find(']')? + 1;
    let after_label = &text[label_end + 1..];
    let after_open = after_label.strip_prefix("(")?;
    let url_end = after_open.find(')')?;

    Some((
        &text[1..label_end],
        &after_open[..url_end],
        label_end + 3 + url_end,
    ))
}

fn parse_strong_span(text: &str) -> Option<(&str, usize)> {
    for delimiter in ["**", "__"] {
        if !text.starts_with(delimiter) {
            continue;
        }

        let end = text[delimiter.len()..].find(delimiter)?;
        let content_start = delimiter.len();
        let content_end = content_start + end;
        let consumed = content_end + delimiter.len();

        return Some((&text[content_start..content_end], consumed));
    }

    None
}

fn parse_inline_code_span(text: &str) -> Option<(&str, usize)> {
    if !text.starts_with('`') {
        return None;
    }

    let end = text[1..].find('`')? + 1;
    Some((&text[1..end], end + 1))
}

fn rendered_len(text: &str) -> usize {
    text.chars().count()
}

#[cfg(test)]
mod tests {
    use super::{build_telegram_reply_messages, TELEGRAM_MESSAGE_CHAR_LIMIT};

    fn format_for_test(text: &str) -> String {
        build_telegram_reply_messages(text).join("\n")
    }

    #[test]
    fn format_reply_for_telegram_renders_basic_markdown() {
        let reply = "# 标题\n\n结论 **重点** [1]\n- 第一项\n2. 第二项\n> 保留原文\n`cargo test`\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```";

        let rendered = format_for_test(reply);

        assert!(rendered.contains("*标题*"));
        assert!(rendered.contains("结论 *重点* \\[1\\]"));
        assert!(rendered.contains("• 第一项"));
        assert!(rendered.contains("2\\. 第二项"));
        assert!(rendered.contains(">保留原文"));
        assert!(rendered.contains("`cargo test`"));
        assert!(rendered.contains("```rust\nfn main() {\n    println!(\"hi\");\n}\n```"));
    }

    #[test]
    fn format_reply_for_telegram_escapes_plain_telegram_markdown_chars() {
        let rendered = format_for_test("_a_ *b* [1] (test)!");

        assert_eq!(rendered, r"\_a\_ \*b\* \[1\] \(test\)\!");
    }

    #[test]
    fn format_reply_for_telegram_preserves_markdown_links() {
        let rendered = format_for_test("[帮助中心](https://example.com/docs?id=1)");

        assert_eq!(rendered, "[帮助中心](https://example.com/docs?id=1)");
    }

    #[test]
    fn build_telegram_reply_messages_splits_long_plain_text() {
        let reply = "A".repeat(TELEGRAM_MESSAGE_CHAR_LIMIT + 120);

        let parts = build_telegram_reply_messages(&reply);

        assert_eq!(parts.len(), 2);
        assert!(parts
            .iter()
            .all(|part| part.chars().count() <= TELEGRAM_MESSAGE_CHAR_LIMIT));
    }

    #[test]
    fn build_telegram_reply_messages_splits_long_code_block() {
        let reply = format!(
            "```\n{}\n```",
            "x".repeat(TELEGRAM_MESSAGE_CHAR_LIMIT + 120)
        );

        let parts = build_telegram_reply_messages(&reply);

        assert!(parts.len() >= 2);
        assert!(parts.iter().all(|part| part.starts_with("```")));
        assert!(parts.iter().all(|part| part.ends_with("```")));
        assert!(parts
            .iter()
            .all(|part| part.chars().count() <= TELEGRAM_MESSAGE_CHAR_LIMIT));
    }
}
