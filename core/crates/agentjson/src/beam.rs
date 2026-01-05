use std::collections::{HashMap, HashSet};

use crate::json::{parse_strict_json, quote_json_string};
use crate::lexer::{Token, TokenType, tolerant_lex};
use crate::types::{
    Candidate, CandidateDiagnostics, CandidateValidations, RepairAction, RepairOptions,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ContainerType {
    Object,
    Array,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Expect {
    KeyOrEnd,
    Colon,
    Value,
    ValueOrEnd,
    CommaOrEnd,
}

// Costs (initial defaults; tune with real data)
const COST_REMOVE_TRAILING_COMMA: f64 = 0.2;
const COST_CLOSE_CONTAINER: f64 = 0.5;
const COST_INSERT_MISSING_COMMA: f64 = 0.8;
const COST_INSERT_MISSING_COLON: f64 = 1.0;
const COST_CONVERT_SINGLE_QUOTES: f64 = 0.9;
const COST_WRAP_KEY: f64 = 1.1;
const COST_WRAP_VALUE: f64 = 1.5;
const COST_SKIP_GARBAGE: f64 = 1.2;
const COST_DELETE_TOKEN: f64 = 2.5;
const COST_CLOSE_OPEN_STRING: f64 = 3.0;
const COST_TRUNCATE_SUFFIX: f64 = 1.3;
const COST_SYNTHESIZE_VALUE: f64 = 2.5;
const COST_PY_LITERAL_MAP: f64 = 0.4;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Frame {
    typ: ContainerType,
    expect: Expect,
}

#[derive(Debug, Clone)]
struct State {
    i: usize,
    stack: Vec<Frame>,
    root_done: bool,
    out: Vec<String>,
    cost: f64,
    repairs: Vec<RepairAction>,
    repair_count: usize,
    garbage_skipped_bytes: usize,
    deleted_tokens: usize,
    inserted_tokens: usize,
    close_open_string_count: usize,
    dropped_spans: Vec<(usize, usize)>,
}

fn top(state: &State) -> Option<Frame> {
    state.stack.last().cloned()
}

fn set_top_expect(mut state: State, expect: Expect) -> State {
    if let Some(last) = state.stack.last_mut() {
        last.expect = expect;
    }
    state
}

fn append_out(mut state: State, piece: &str) -> State {
    state.out.push(piece.to_string());
    state
}

fn pop_trailing_comma(mut state: State) -> Option<State> {
    if state.out.last().map(|s| s.as_str()) != Some(",") {
        return None;
    }
    state.out.pop();
    Some(state)
}

#[derive(Debug, Clone, Copy, Default)]
struct RepairDelta {
    inserted_tokens: usize,
    deleted_tokens: usize,
    garbage_skipped_bytes: usize,
}

#[derive(Debug, Clone)]
struct RepairSpec<'a> {
    op: &'a str,
    cost_delta: f64,
    span: Option<(usize, usize)>,
    at: Option<usize>,
    token: Option<&'a str>,
    note: Option<String>,
    delta: RepairDelta,
    dropped_span: Option<(usize, usize)>,
}

impl<'a> RepairSpec<'a> {
    fn new(op: &'a str, cost_delta: f64) -> Self {
        Self {
            op,
            cost_delta,
            span: None,
            at: None,
            token: None,
            note: None,
            delta: RepairDelta::default(),
            dropped_span: None,
        }
    }

    fn span(mut self, span: (usize, usize)) -> Self {
        self.span = Some(span);
        self
    }

    fn at(mut self, at: usize) -> Self {
        self.at = Some(at);
        self
    }

    fn token(mut self, token: &'a str) -> Self {
        self.token = Some(token);
        self
    }

    fn note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }

    fn inserted_tokens(mut self, n: usize) -> Self {
        self.delta.inserted_tokens = n;
        self
    }

    fn deleted_tokens(mut self, n: usize) -> Self {
        self.delta.deleted_tokens = n;
        self
    }

    fn garbage_skipped_bytes(mut self, n: usize) -> Self {
        self.delta.garbage_skipped_bytes = n;
        self
    }

    fn dropped_span(mut self, span: (usize, usize)) -> Self {
        self.dropped_span = Some(span);
        self
    }
}

fn add_repair(mut state: State, spec: RepairSpec<'_>) -> State {
    let mut action = RepairAction::new(spec.op, spec.cost_delta);
    action.span = spec.span;
    action.at = spec.at;
    action.token = spec.token.map(|s| s.to_string());
    action.note = spec.note;

    state.cost += spec.cost_delta;
    state.repairs.push(action);
    state.repair_count += 1;
    state.inserted_tokens += spec.delta.inserted_tokens;
    state.deleted_tokens += spec.delta.deleted_tokens;
    state.garbage_skipped_bytes += spec.delta.garbage_skipped_bytes;
    if spec.op == "close_open_string" {
        state.close_open_string_count += 1;
    }
    if let Some(ds) = spec.dropped_span {
        state.dropped_spans.push(ds);
    }
    state
}

fn advance(mut state: State, n: usize) -> State {
    state.i += n;
    state
}

fn is_value_start(token: &Token) -> bool {
    if token.typ == TokenType::Punct && (token.value == "{" || token.value == "[") {
        return true;
    }
    matches!(
        token.typ,
        TokenType::String | TokenType::Number | TokenType::Literal | TokenType::Ident
    )
}

fn is_key_start(token: &Token) -> bool {
    if token.typ == TokenType::String {
        return true;
    }
    matches!(token.typ, TokenType::Ident | TokenType::Literal)
}

fn complete_value_in_current_context(mut state: State) -> State {
    if state.stack.is_empty() {
        state.root_done = true;
        return state;
    }
    if let Some(top) = state.stack.last() {
        match (top.typ, top.expect) {
            (ContainerType::Object, Expect::Value) => {
                state = set_top_expect(state, Expect::CommaOrEnd)
            }
            (ContainerType::Array, Expect::ValueOrEnd) => {
                state = set_top_expect(state, Expect::CommaOrEnd)
            }
            _ => {}
        }
    }
    state
}

fn consume_container_open(mut state: State, token: &Token) -> Option<State> {
    if token.typ != TokenType::Punct {
        return None;
    }
    match token.value.as_str() {
        "{" => {
            state = append_out(state, "{");
            state.stack.push(Frame {
                typ: ContainerType::Object,
                expect: Expect::KeyOrEnd,
            });
            Some(advance(state, 1))
        }
        "[" => {
            state = append_out(state, "[");
            state.stack.push(Frame {
                typ: ContainerType::Array,
                expect: Expect::ValueOrEnd,
            });
            Some(advance(state, 1))
        }
        _ => None,
    }
}

fn consume_container_close(mut state: State, token: &Token) -> Option<State> {
    if token.typ != TokenType::Punct || state.stack.is_empty() {
        return None;
    }
    let top = state.stack.last().cloned()?;

    // If we are in KEY_OR_END/VALUE_OR_END and the output already ends with a comma,
    // accepting a close would produce an invalid trailing-comma JSON. Force a repair instead.
    if top.typ == ContainerType::Object
        && top.expect == Expect::KeyOrEnd
        && state.out.last().map(|s| s.as_str()) == Some(",")
    {
        return None;
    }
    if top.typ == ContainerType::Array
        && top.expect == Expect::ValueOrEnd
        && state.out.last().map(|s| s.as_str()) == Some(",")
    {
        return None;
    }

    if top.typ == ContainerType::Object
        && token.value == "}"
        && (top.expect == Expect::KeyOrEnd || top.expect == Expect::CommaOrEnd)
    {
        state = append_out(state, "}");
        state.stack.pop();
        state = advance(state, 1);
        return Some(complete_value_in_current_context(state));
    }
    if top.typ == ContainerType::Array
        && token.value == "]"
        && (top.expect == Expect::ValueOrEnd || top.expect == Expect::CommaOrEnd)
    {
        state = append_out(state, "]");
        state.stack.pop();
        state = advance(state, 1);
        return Some(complete_value_in_current_context(state));
    }
    None
}

fn consume_punct(state: State, token: &Token) -> Option<State> {
    if token.typ != TokenType::Punct {
        return None;
    }

    // Root expects value-start; only { or [ are punct values.
    if state.stack.is_empty() && !state.root_done {
        return consume_container_open(state, token);
    }

    let top = top(&state)?;

    // Container open as a value
    if (top.expect == Expect::Value || top.expect == Expect::ValueOrEnd)
        && (token.value == "{" || token.value == "[")
    {
        return consume_container_open(state, token);
    }

    // Close container
    let closed = consume_container_close(state.clone(), token);
    if closed.is_some() {
        return closed;
    }

    // Comma / Colon
    if token.value == "," && top.expect == Expect::CommaOrEnd {
        let mut s = append_out(state, ",");
        if top.typ == ContainerType::Object {
            s = set_top_expect(s, Expect::KeyOrEnd);
        } else {
            s = set_top_expect(s, Expect::ValueOrEnd);
        }
        return Some(advance(s, 1));
    }

    if token.value == ":" && top.typ == ContainerType::Object && top.expect == Expect::Colon {
        let mut s = append_out(state, ":");
        s = set_top_expect(s, Expect::Value);
        return Some(advance(s, 1));
    }

    None
}

fn consume_key(state: State, token: &Token, opt: &RepairOptions) -> Option<State> {
    let top = top(&state)?;
    if top.typ != ContainerType::Object || top.expect != Expect::KeyOrEnd {
        return None;
    }

    if token.typ == TokenType::String {
        let close_open_count = state.close_open_string_count;
        let mut s2 = append_out(state, &quote_json_string(&token.value));
        s2 = advance(s2, 1);
        s2 = set_top_expect(s2, Expect::Colon);
        if token.quote == Some('\'') && opt.allow_single_quotes {
            s2 = add_repair(
                s2,
                RepairSpec::new(
                    "convert_single_to_double_quotes",
                    COST_CONVERT_SINGLE_QUOTES,
                )
                .span((token.start, token.end)),
            );
        }
        if !token.closed {
            if close_open_count >= opt.max_close_open_string {
                return None;
            }
            s2 = add_repair(
                s2,
                RepairSpec::new("close_open_string", COST_CLOSE_OPEN_STRING).at(token.end),
            );
        }
        return Some(s2);
    }

    if matches!(token.typ, TokenType::Ident | TokenType::Literal) && opt.allow_unquoted_keys {
        let mut s2 = append_out(state, &quote_json_string(&token.value));
        s2 = advance(s2, 1);
        s2 = set_top_expect(s2, Expect::Colon);
        s2 = add_repair(
            s2,
            RepairSpec::new("wrap_key_with_quotes", COST_WRAP_KEY).span((token.start, token.end)),
        );
        return Some(s2);
    }

    None
}

fn consume_value_primitive(state: State, token: &Token, opt: &RepairOptions) -> Option<State> {
    let expect_value = if state.stack.is_empty() && !state.root_done {
        true
    } else {
        match top(&state) {
            Some(t) => t.expect == Expect::Value || t.expect == Expect::ValueOrEnd,
            None => false,
        }
    };
    if !expect_value {
        return None;
    }

    if token.typ == TokenType::String {
        let close_open_count = state.close_open_string_count;
        let mut s2 = append_out(state, &quote_json_string(&token.value));
        s2 = advance(s2, 1);
        s2 = complete_value_in_current_context(s2);
        if token.quote == Some('\'') && opt.allow_single_quotes {
            s2 = add_repair(
                s2,
                RepairSpec::new(
                    "convert_single_to_double_quotes",
                    COST_CONVERT_SINGLE_QUOTES,
                )
                .span((token.start, token.end)),
            );
        }
        if !token.closed {
            if close_open_count >= opt.max_close_open_string {
                return None;
            }
            s2 = add_repair(
                s2,
                RepairSpec::new("close_open_string", COST_CLOSE_OPEN_STRING).at(token.end),
            );
        }
        return Some(s2);
    }

    if token.typ == TokenType::Number {
        let s2 = append_out(state, &token.value);
        let s2 = advance(s2, 1);
        return Some(complete_value_in_current_context(s2));
    }

    if token.typ == TokenType::Literal {
        let s2 = append_out(state, &token.value.to_ascii_lowercase());
        let s2 = advance(s2, 1);
        return Some(complete_value_in_current_context(s2));
    }

    if token.typ == TokenType::Ident {
        let v = token.value.clone();
        let low = v.to_ascii_lowercase();
        let mapped = match low.as_str() {
            "true" => Some("true"),
            "false" => Some("false"),
            "none" => Some("null"),
            "null" => Some("null"),
            "undefined" => Some("null"),
            _ => None,
        };
        if opt.allow_python_literals
            && let Some(mapped) = mapped
        {
            let mut s2 = append_out(state, mapped);
            s2 = advance(s2, 1);
            s2 = complete_value_in_current_context(s2);
            if !matches!(low.as_str(), "true" | "false" | "null") {
                s2 = add_repair(
                    s2,
                    RepairSpec::new("map_python_literal", COST_PY_LITERAL_MAP)
                        .span((token.start, token.end))
                        .note(format!("{v} -> {mapped}")),
                );
            }
            return Some(s2);
        }

        if opt.allow_unquoted_values {
            let mut s2 = append_out(state, &quote_json_string(&v));
            s2 = advance(s2, 1);
            s2 = complete_value_in_current_context(s2);
            s2 = add_repair(
                s2,
                RepairSpec::new("wrap_value_with_quotes", COST_WRAP_VALUE)
                    .span((token.start, token.end)),
            );
            return Some(s2);
        }
    }

    None
}

fn try_consume(state: State, token: &Token, opt: &RepairOptions) -> Option<State> {
    if token.typ == TokenType::Eof {
        if state.stack.is_empty() && state.root_done {
            return Some(advance(state, 1));
        }
        return None;
    }

    if let Some(s) = consume_punct(state.clone(), token) {
        return Some(s);
    }
    if let Some(s) = consume_key(state.clone(), token, opt) {
        return Some(s);
    }
    if let Some(s) = consume_value_primitive(state, token, opt) {
        return Some(s);
    }
    None
}

fn repair_remove_trailing_comma_before_end(state: State, token: &Token) -> Option<State> {
    if token.typ != TokenType::Punct || !(token.value == "}" || token.value == "]") {
        return None;
    }
    let top = top(&state)?;
    if top.typ == ContainerType::Object && token.value == "}" && top.expect == Expect::KeyOrEnd {
        let mut popped = pop_trailing_comma(state)?;
        popped = set_top_expect(popped, Expect::CommaOrEnd);
        popped = add_repair(
            popped,
            RepairSpec::new("remove_trailing_comma", COST_REMOVE_TRAILING_COMMA).at(token.start),
        );
        return Some(popped);
    }
    if top.typ == ContainerType::Array && token.value == "]" && top.expect == Expect::ValueOrEnd {
        let mut popped = pop_trailing_comma(state)?;
        popped = set_top_expect(popped, Expect::CommaOrEnd);
        popped = add_repair(
            popped,
            RepairSpec::new("remove_trailing_comma", COST_REMOVE_TRAILING_COMMA).at(token.start),
        );
        return Some(popped);
    }
    None
}

fn repair_insert_missing_comma(state: State, token: &Token) -> Option<State> {
    let top = top(&state)?;
    if top.expect != Expect::CommaOrEnd {
        return None;
    }
    if token.typ == TokenType::Punct && (token.value == "}" || token.value == "]") {
        return None;
    }

    // Context-weighted cost: clearer boundaries are cheaper.
    let cost = if token.typ == TokenType::String
        || (token.typ == TokenType::Punct && (token.value == "{" || token.value == "["))
    {
        0.7
    } else if token.typ == TokenType::Ident {
        1.0
    } else {
        COST_INSERT_MISSING_COMMA
    };

    if top.typ == ContainerType::Array && is_value_start(token) {
        let mut s = append_out(state, ",");
        s = set_top_expect(s, Expect::ValueOrEnd);
        s = add_repair(
            s,
            RepairSpec::new("insert_missing_comma", cost)
                .at(token.start)
                .token(",")
                .inserted_tokens(1),
        );
        return Some(s);
    }
    if top.typ == ContainerType::Object && is_key_start(token) {
        let mut s = append_out(state, ",");
        s = set_top_expect(s, Expect::KeyOrEnd);
        s = add_repair(
            s,
            RepairSpec::new("insert_missing_comma", cost)
                .at(token.start)
                .token(",")
                .inserted_tokens(1),
        );
        return Some(s);
    }
    None
}

fn repair_insert_missing_colon(state: State, token: &Token) -> Option<State> {
    let top = top(&state)?;
    if top.typ != ContainerType::Object || top.expect != Expect::Colon {
        return None;
    }
    if token.typ == TokenType::Punct && token.value == ":" {
        return None;
    }
    if is_value_start(token)
        || (token.typ == TokenType::Punct && (token.value == "{" || token.value == "["))
    {
        let mut s = append_out(state, ":");
        s = set_top_expect(s, Expect::Value);
        s = add_repair(
            s,
            RepairSpec::new("insert_missing_colon", COST_INSERT_MISSING_COLON)
                .at(token.start)
                .token(":")
                .inserted_tokens(1),
        );
        return Some(s);
    }
    None
}

fn repair_skip_garbage(state: State, token: &Token, opt: &RepairOptions) -> Option<State> {
    if token.typ != TokenType::Garbage {
        return None;
    }
    let tok_len = token.end.saturating_sub(token.start);
    if state.garbage_skipped_bytes + tok_len > opt.max_garbage_skip_bytes {
        return None;
    }
    let cost = COST_SKIP_GARBAGE + (0.0002 * (tok_len as f64));
    let mut s = advance(state, 1);
    s = add_repair(
        s,
        RepairSpec::new("skip_garbage", cost)
            .span((token.start, token.end))
            .garbage_skipped_bytes(tok_len),
    );
    Some(s)
}

fn repair_delete_unexpected(state: State, token: &Token, opt: &RepairOptions) -> Option<State> {
    if token.typ == TokenType::Eof {
        return None;
    }
    if state.deleted_tokens >= opt.max_deleted_tokens {
        return None;
    }
    let mut s = advance(state, 1);
    s = add_repair(
        s,
        RepairSpec::new("delete_unexpected_token", COST_DELETE_TOKEN)
            .span((token.start, token.end))
            .deleted_tokens(1),
    );
    Some(s)
}

fn repair_truncate_suffix(
    state: State,
    token: &Token,
    text_len: usize,
    eof_index: usize,
) -> Option<State> {
    if state.out.is_empty() {
        return None;
    }
    if token.typ == TokenType::Eof {
        return None;
    }
    // Truncation is a "partial success" escape hatch; restrict it to clearly non-structural tokens
    // so we don't prematurely cut off valid JSON at commas/colons/brackets.
    if !matches!(token.typ, TokenType::Garbage | TokenType::Ident) {
        return None;
    }
    let dropped = text_len.saturating_sub(token.start);
    let cost = COST_TRUNCATE_SUFFIX + (0.00005 * (dropped as f64));
    let mut s = state;
    s.i = eof_index;
    s = add_repair(
        s,
        RepairSpec::new("truncate_suffix", cost)
            .span((token.start, text_len))
            .dropped_span((token.start, text_len)),
    );
    Some(s)
}

fn repair_synthesize_missing_value(state: State, token: &Token) -> Option<State> {
    let expect_value = if state.stack.is_empty() && !state.root_done {
        true
    } else {
        match top(&state) {
            Some(t) => t.expect == Expect::Value || t.expect == Expect::ValueOrEnd,
            None => false,
        }
    };
    if !expect_value {
        return None;
    }
    let can_synth = token.typ == TokenType::Eof
        || (token.typ == TokenType::Punct
            && (token.value == "," || token.value == "}" || token.value == "]"));
    if can_synth {
        let mut s = append_out(state, "null");
        s = add_repair(
            s,
            RepairSpec::new("synthesize_missing_value", COST_SYNTHESIZE_VALUE)
                .at(token.start)
                .token("null")
                .inserted_tokens(1),
        );
        s = complete_value_in_current_context(s);
        return Some(s);
    }
    None
}

fn repair_close_one_container_at_eof(state: State, token: &Token) -> Option<State> {
    if token.typ != TokenType::Eof || state.stack.is_empty() {
        return None;
    }
    let mut top = state.stack.last().cloned()?;
    let mut s = state;

    if top.typ == ContainerType::Object
        && top.expect == Expect::KeyOrEnd
        && let Some(mut popped) = pop_trailing_comma(s.clone())
    {
        popped = set_top_expect(popped, Expect::CommaOrEnd);
        popped = add_repair(
            popped,
            RepairSpec::new("remove_trailing_comma", COST_REMOVE_TRAILING_COMMA).at(token.start),
        );
        s = popped;
        top = s.stack.last().cloned()?;
    }
    if top.typ == ContainerType::Array
        && top.expect == Expect::ValueOrEnd
        && let Some(mut popped) = pop_trailing_comma(s.clone())
    {
        popped = set_top_expect(popped, Expect::CommaOrEnd);
        popped = add_repair(
            popped,
            RepairSpec::new("remove_trailing_comma", COST_REMOVE_TRAILING_COMMA).at(token.start),
        );
        s = popped;
        top = s.stack.last().cloned()?;
    }

    let closer = if top.typ == ContainerType::Object {
        "}"
    } else {
        "]"
    };
    s = append_out(s, closer);
    s.stack.pop();
    s = add_repair(
        s,
        RepairSpec::new("insert_missing_closer", COST_CLOSE_CONTAINER)
            .at(token.start)
            .token(closer)
            .inserted_tokens(1),
    );
    s = complete_value_in_current_context(s);
    Some(s)
}

fn expand_repairs(
    state: State,
    token: &Token,
    opt: &RepairOptions,
    text_len: usize,
    eof_index: usize,
    next_token: Option<&Token>,
) -> Vec<State> {
    if state.repair_count >= opt.max_repairs {
        return Vec::new();
    }

    let mut out: Vec<State> = Vec::new();

    if let Some(s) = repair_remove_trailing_comma_before_end(state.clone(), token) {
        out.push(s);
    }
    if let Some(s) = repair_insert_missing_comma(state.clone(), token) {
        out.push(s);
    }
    if let Some(s) = repair_insert_missing_colon(state.clone(), token) {
        out.push(s);
    }
    if let Some(s) = repair_synthesize_missing_value(state.clone(), token) {
        out.push(s);
    }
    if let Some(s) = repair_close_one_container_at_eof(state.clone(), token) {
        out.push(s);
    }
    if let Some(s) = repair_skip_garbage(state.clone(), token, opt) {
        out.push(s);
    }

    if opt.partial_ok {
        let mut allow_truncate = true;
        // Avoid truncating at an IDENT that is very likely a real (unquoted) key: IDENT followed by ':'.
        if token.typ == TokenType::Ident
            && let Some(top) = top(&state)
            && top.typ == ContainerType::Object
            && top.expect == Expect::KeyOrEnd
            && let Some(nt) = next_token
            && nt.typ == TokenType::Punct
            && nt.value == ":"
        {
            allow_truncate = false;
        }
        if allow_truncate
            && let Some(s) = repair_truncate_suffix(state.clone(), token, text_len, eof_index)
        {
            out.push(s);
        }
    }

    // Last resort: only if we haven't found a better idea.
    if out.is_empty()
        && let Some(s) = repair_delete_unexpected(state, token, opt)
    {
        out.push(s);
    }

    out
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Signature {
    i: usize,
    root_done: bool,
    stack: Vec<Frame>,
    tail: String,
}

fn tail_signature(out: &[String]) -> String {
    let mut joined = String::new();
    let start = out.len().saturating_sub(8);
    for s in &out[start..] {
        joined.push_str(s);
    }
    if joined.len() <= 64 {
        return joined;
    }
    let mut idx = joined.len() - 64;
    while idx < joined.len() && !joined.is_char_boundary(idx) {
        idx += 1;
    }
    joined[idx..].to_string()
}

fn signature(state: &State) -> Signature {
    Signature {
        i: state.i,
        root_done: state.root_done,
        stack: state.stack.clone(),
        tail: tail_signature(&state.out),
    }
}

const FNV_OFFSET_BASIS_U64: u64 = 14695981039346656037;
const FNV_PRIME_U64: u64 = 1099511628211;

fn fnv1a_u64(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME_U64);
    }
    h
}

fn fnv1a_u64_mix_u64(h: u64, x: u64) -> u64 {
    fnv1a_u64(h, &x.to_le_bytes())
}

fn stable_fingerprint(sig: &Signature, seed: u64) -> u64 {
    let mut h = FNV_OFFSET_BASIS_U64 ^ seed;
    h = fnv1a_u64_mix_u64(h, sig.i as u64);
    h = fnv1a_u64(h, &[sig.root_done as u8]);
    h = fnv1a_u64_mix_u64(h, sig.stack.len() as u64);
    for f in &sig.stack {
        let typ = match f.typ {
            ContainerType::Object => 1u8,
            ContainerType::Array => 2u8,
        };
        let expect = match f.expect {
            Expect::KeyOrEnd => 1u8,
            Expect::Colon => 2u8,
            Expect::Value => 3u8,
            Expect::ValueOrEnd => 4u8,
            Expect::CommaOrEnd => 5u8,
        };
        h = fnv1a_u64(h, &[typ, expect]);
    }
    h = fnv1a_u64(h, sig.tail.as_bytes());
    h
}

fn state_fingerprint(state: &State, seed: u64) -> u64 {
    let sig = signature(state);
    let mut h = stable_fingerprint(&sig, seed);
    h = fnv1a_u64_mix_u64(h, state.repairs.len() as u64);
    for r in &state.repairs {
        h = fnv1a_u64(h, r.op.as_bytes());
        if let Some((s, e)) = r.span {
            h = fnv1a_u64_mix_u64(h, s as u64);
            h = fnv1a_u64_mix_u64(h, e as u64);
        } else {
            h = fnv1a_u64_mix_u64(h, u64::MAX);
            h = fnv1a_u64_mix_u64(h, u64::MAX);
        }
        h = fnv1a_u64_mix_u64(h, r.at.unwrap_or(usize::MAX) as u64);
        if let Some(tok) = r.token.as_ref() {
            h = fnv1a_u64(h, tok.as_bytes());
        }
        h = fnv1a_u64_mix_u64(h, r.cost_delta.to_bits());
        if let Some(note) = r.note.as_ref() {
            h = fnv1a_u64(h, note.as_bytes());
        }
    }
    h
}

#[derive(Clone, Copy)]
struct PruneKey {
    cost: f64,
    repair_count: usize,
    i: usize,
    fp: u64,
}

impl PruneKey {
    fn cmp(self, other: Self) -> std::cmp::Ordering {
        let c = self.cost.total_cmp(&other.cost);
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        let c2 = self.repair_count.cmp(&other.repair_count);
        if c2 != std::cmp::Ordering::Equal {
            return c2;
        }
        let c3 = self.i.cmp(&other.i);
        if c3 != std::cmp::Ordering::Equal {
            return c3;
        }
        self.fp.cmp(&other.fp)
    }
}

fn make_prune_key(state: &State, seed: u64) -> PruneKey {
    PruneKey {
        cost: state.cost,
        repair_count: state.repair_count,
        i: state.i,
        fp: state_fingerprint(state, seed),
    }
}

fn prune(states: Vec<State>, beam_width: usize, seed: u64) -> Vec<State> {
    let mut best: HashMap<Signature, (PruneKey, State)> = HashMap::new();
    for s in states {
        let sig = signature(&s);
        let key = make_prune_key(&s, seed);
        let replace = match best.get(&sig) {
            None => true,
            Some((prev_key, _)) => key.cmp(*prev_key) == std::cmp::Ordering::Less,
        };
        if replace {
            best.insert(sig, (key, s));
        }
    }
    let mut out: Vec<(PruneKey, State)> = best.into_values().collect();
    out.sort_by(|(a, _), (b, _)| a.cmp(*b));
    out.truncate(beam_width);
    out.into_iter().map(|(_, s)| s).collect()
}

fn is_finished(state: &State, token: &Token) -> bool {
    state.root_done && state.stack.is_empty() && token.typ == TokenType::Eof
}

pub fn probabilistic_repair(
    extracted_text: &str,
    opt: &RepairOptions,
    base_repairs: &[RepairAction],
) -> Vec<Candidate> {
    let tokens = tolerant_lex(extracted_text, opt.allow_single_quotes);
    if tokens.is_empty() {
        return Vec::new();
    }
    let eof_index = tokens.len() - 1;

    let base_cost: f64 = base_repairs.iter().map(|a| a.cost_delta).sum();
    let init = State {
        i: 0,
        stack: Vec::new(),
        root_done: false,
        out: Vec::new(),
        cost: base_cost,
        repairs: base_repairs.to_vec(),
        repair_count: 0,
        garbage_skipped_bytes: 0,
        deleted_tokens: 0,
        inserted_tokens: 0,
        close_open_string_count: 0,
        dropped_spans: Vec::new(),
    };

    let mut beam: Vec<State> = vec![init];
    let mut finals: Vec<State> = Vec::new();

    let max_steps = std::cmp::max(64usize, tokens.len() * 4);
    for _ in 0..max_steps {
        if beam.is_empty() {
            break;
        }
        let mut next_states: Vec<State> = Vec::new();
        for s in beam.iter() {
            if s.i >= tokens.len() {
                continue;
            }
            let tok = &tokens[s.i];
            if is_finished(s, tok) {
                finals.push(s.clone());
                continue;
            }

            if s.root_done
                && s.stack.is_empty()
                && tok.typ != TokenType::Eof
                && (tok.typ == TokenType::Garbage || tok.typ == TokenType::Ident)
            {
                let tok_len = tok.end.saturating_sub(tok.start);
                if s.garbage_skipped_bytes + tok_len > opt.max_garbage_skip_bytes {
                    continue;
                }
                let cost = 0.3 + (0.0002 * (tok_len as f64));
                let mut s2 = advance(s.clone(), 1);
                s2 = add_repair(
                    s2,
                    RepairSpec::new("skip_suffix", cost)
                        .span((tok.start, tok.end))
                        .garbage_skipped_bytes(tok_len),
                );
                next_states.push(s2);
                continue;
            }

            let consumed = try_consume(s.clone(), tok, opt);
            if let Some(consumed) = consumed {
                let strict_consume = consumed.cost == s.cost
                    && consumed.repair_count == s.repair_count
                    && consumed.repairs.len() == s.repairs.len();
                next_states.push(consumed);
                if strict_consume {
                    continue;
                }
            }

            let next_tok = if s.i + 1 < tokens.len() {
                Some(&tokens[s.i + 1])
            } else {
                None
            };
            next_states.extend(expand_repairs(
                s.clone(),
                tok,
                opt,
                extracted_text.len(),
                eof_index,
                next_tok,
            ));
        }

        beam = prune(next_states, opt.beam_width, opt.deterministic_seed);
        if finals.len() >= opt.top_k.saturating_mul(3) {
            break;
        }
    }

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut seen_norm: HashSet<String> = HashSet::new();
    let seed = opt.deterministic_seed;
    let mut finals_keyed: Vec<(PruneKey, State)> = finals
        .into_iter()
        .map(|s| {
            let k = make_prune_key(&s, seed);
            (k, s)
        })
        .collect();
    finals_keyed.sort_by(|(a, _), (b, _)| a.cmp(*b));
    for (_k, s) in finals_keyed {
        let norm = s.out.join("").trim().to_string();
        if norm.is_empty() {
            continue;
        }
        if seen_norm.contains(&norm) {
            continue;
        }
        let value = match parse_strict_json(&norm) {
            Ok(v) => v,
            Err(_) => continue,
        };
        seen_norm.insert(norm.clone());
        let cost = s.cost;
        let confidence = (-opt.confidence_alpha * cost).exp();
        let diagnostics = CandidateDiagnostics {
            garbage_skipped_bytes: s.garbage_skipped_bytes,
            deleted_tokens: s.deleted_tokens,
            inserted_tokens: s.inserted_tokens,
            close_open_string_count: s.close_open_string_count,
            beam_width: Some(opt.beam_width),
            max_repairs: Some(opt.max_repairs),
        };
        candidates.push(Candidate {
            candidate_id: candidates.len(),
            value: Some(value),
            normalized_json: Some(norm),
            ir: None,
            confidence,
            cost,
            repairs: s.repairs,
            validations: CandidateValidations {
                strict_json_parse: true,
                schema_match: None,
            },
            diagnostics,
            dropped_spans: s.dropped_spans,
        });
        if candidates.len() >= opt.top_k {
            break;
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_is_deterministic_with_seed() {
        let base = State {
            i: 0,
            stack: Vec::new(),
            root_done: false,
            out: vec!["x".to_string()],
            cost: 0.0,
            repairs: Vec::new(),
            repair_count: 0,
            garbage_skipped_bytes: 0,
            deleted_tokens: 0,
            inserted_tokens: 0,
            close_open_string_count: 0,
            dropped_spans: Vec::new(),
        };

        let mut s1 = base.clone();
        s1.out = vec!["1".to_string()];
        let mut s2 = base.clone();
        s2.out = vec!["2".to_string()];

        let o1 = prune(vec![s1.clone(), s2.clone()], 2, 123);
        let o2 = prune(vec![s2, s1], 2, 123);

        assert_eq!(o1.len(), 2);
        assert_eq!(o2.len(), 2);
        assert_eq!(o1[0].out[0], o2[0].out[0]);
        assert_eq!(o1[1].out[0], o2[1].out[0]);
    }
}
