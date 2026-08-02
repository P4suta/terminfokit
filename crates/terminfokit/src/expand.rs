// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `tparm` expansion and padding events.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{ExpandError, ExpandErrorKind};

const MAX_OUTPUT: usize = 1024 * 1024;
const MAX_STEPS: usize = 65_536;
const MAX_STACK: usize = 1024;

/// Borrowed numeric or byte-string parameter supplied to tparm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param<'a> {
    /// Signed numeric parameter.
    Number(i64),
    /// Binary-safe string parameter.
    Bytes(&'a [u8]),
}

impl<'a> From<i64> for Param<'a> {
    fn from(value: i64) -> Self {
        Self::Number(value)
    }
}
impl<'a> From<&'a [u8]> for Param<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::Bytes(value)
    }
}
impl<'a> From<&'a str> for Param<'a> {
    fn from(value: &'a str) -> Self {
        Self::Bytes(value.as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Number(i64),
    Bytes(Vec<u8>),
}
impl Default for Value {
    fn default() -> Self {
        Self::Number(0)
    }
}

/// Resource limits for parameter expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpanderLimits {
    max_output: usize,
    max_steps: usize,
    max_stack: usize,
}

impl ExpanderLimits {
    /// Returns defensive limits for interactive terminal capabilities.
    pub const fn standard() -> Self {
        Self {
            max_output: MAX_OUTPUT,
            max_steps: MAX_STEPS,
            max_stack: MAX_STACK,
        }
    }

    /// Returns limits bounded only by address-space size.
    pub const fn unlimited() -> Self {
        Self {
            max_output: usize::MAX,
            max_steps: usize::MAX,
            max_stack: usize::MAX,
        }
    }

    /// Returns the maximum expanded byte count.
    pub const fn max_output(self) -> usize {
        self.max_output
    }

    /// Returns the maximum executed operator count.
    pub const fn max_steps(self) -> usize {
        self.max_steps
    }

    /// Returns the maximum value-stack depth.
    pub const fn max_stack(self) -> usize {
        self.max_stack
    }

    /// Replaces the expanded byte limit.
    pub const fn with_max_output(mut self, value: usize) -> Self {
        self.max_output = value;
        self
    }

    /// Replaces the executed operator limit.
    pub const fn with_max_steps(mut self, value: usize) -> Self {
        self.max_steps = value;
        self
    }

    /// Replaces the value-stack depth limit.
    pub const fn with_max_stack(mut self, value: usize) -> Self {
        self.max_stack = value;
        self
    }
}

impl Default for ExpanderLimits {
    fn default() -> Self {
        Self::standard()
    }
}

/// Parsed parameter program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    bytes: Vec<u8>,
    analysis: ProgramAnalysis,
}

impl Program {
    /// Validates and copies a parameter program.
    pub fn parse(bytes: &[u8]) -> Result<Self, ExpandError> {
        let analysis = analyze_program(bytes)?;
        Ok(Self {
            bytes: bytes.to_vec(),
            analysis,
        })
    }

    /// Returns the original validated bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns static program facts.
    pub const fn analyze(&self) -> &ProgramAnalysis {
        &self.analysis
    }
}

/// Static facts about a parameter program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProgramAnalysis {
    parameter_count: u8,
    uses_dynamic_variables: bool,
    uses_static_variables: bool,
}

impl ProgramAnalysis {
    /// Returns the highest referenced parameter count.
    pub const fn parameter_count(self) -> u8 {
        self.parameter_count
    }

    /// Reports use of call-local lower-case variables.
    pub const fn uses_dynamic_variables(self) -> bool {
        self.uses_dynamic_variables
    }

    /// Reports use of persistent upper-case variables.
    pub const fn uses_static_variables(self) -> bool {
        self.uses_static_variables
    }
}

fn analyze_program(bytes: &[u8]) -> Result<ProgramAnalysis, ExpandError> {
    let mut analysis = ProgramAnalysis::default();
    let mut implicit_parameters = 0u8;
    let mut index = 0usize;
    let mut condition_depth = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let offset = index;
        index += 1;
        let operator = *bytes
            .get(index)
            .ok_or_else(|| error(offset, ExpandErrorKind::TruncatedOperator))?;
        index += 1;
        match operator {
            b'?' => condition_depth = condition_depth.saturating_add(1),
            b't' | b'e' if condition_depth == 0 => {
                return Err(error(offset, ExpandErrorKind::UnbalancedConditional));
            }
            b';' => {
                condition_depth = condition_depth
                    .checked_sub(1)
                    .ok_or_else(|| error(offset, ExpandErrorKind::UnbalancedConditional))?;
            }
            b'p' => {
                let digit = *bytes
                    .get(index)
                    .ok_or_else(|| error(offset, ExpandErrorKind::TruncatedOperator))?;
                index += 1;
                let parameter = digit
                    .checked_sub(b'0')
                    .filter(|value| (1..=9).contains(value))
                    .ok_or_else(|| error(offset, ExpandErrorKind::InvalidParameter))?;
                analysis.parameter_count = analysis.parameter_count.max(parameter);
            }
            b'P' | b'g' => {
                let variable = *bytes
                    .get(index)
                    .ok_or_else(|| error(offset, ExpandErrorKind::TruncatedOperator))?;
                index += 1;
                match variable {
                    b'a'..=b'z' => analysis.uses_dynamic_variables = true,
                    b'A'..=b'Z' => analysis.uses_static_variables = true,
                    _ => return Err(error(offset, ExpandErrorKind::InvalidVariable)),
                }
            }
            b'{' => {
                let end = bytes[index..]
                    .iter()
                    .position(|byte| *byte == b'}')
                    .ok_or_else(|| error(offset, ExpandErrorKind::TruncatedOperator))?;
                index = index.saturating_add(end).saturating_add(1);
            }
            b'\'' => {
                if bytes.get(index.saturating_add(1)) != Some(&b'\'') {
                    return Err(error(offset, ExpandErrorKind::TruncatedOperator));
                }
                index = index.saturating_add(2);
            }
            b'2' | b'3'
                if bytes
                    .get(index)
                    .is_none_or(|next| !matches!(*next, b'd' | b'o' | b'x' | b'X' | b's')) =>
            {
                implicit_parameters = implicit_parameters.saturating_add(1);
            }
            b':' | b'#' | b'0'..=b'9' | b'.' | b' ' => {
                while bytes.get(index).is_some_and(|byte| {
                    matches!(*byte, b'#' | b'0'..=b'9' | b'.' | b' ' | b'-' | b'+')
                }) {
                    index += 1;
                }
                let conversion = *bytes
                    .get(index)
                    .ok_or_else(|| error(offset, ExpandErrorKind::TruncatedOperator))?;
                if !matches!(conversion, b'd' | b'o' | b'x' | b'X' | b's') {
                    return Err(error(offset, ExpandErrorKind::TruncatedOperator));
                }
                implicit_parameters = implicit_parameters.saturating_add(1);
                index += 1;
            }
            b'l' => implicit_parameters = implicit_parameters.saturating_add(1),
            b'd' | b'o' | b'x' | b'X' | b'c' | b's' => {
                implicit_parameters = implicit_parameters.saturating_add(1);
            }
            b'%' | b't' | b'e' | b'i' | b'+' | b'-' | b'*' | b'/' | b'm' | b'&' | b'|' | b'^'
            | b'=' | b'>' | b'<' | b'A' | b'O' | b'!' | b'~' | b'B' | b'D' | b'r' | b'n' => {}
            _ => return Err(error(offset, ExpandErrorKind::TruncatedOperator)),
        }
    }
    if condition_depth != 0 {
        return Err(error(bytes.len(), ExpandErrorKind::UnbalancedConditional));
    }
    if analysis.parameter_count == 0 {
        analysis.parameter_count = implicit_parameters.min(9);
    }
    Ok(analysis)
}

/// Expansion state. Upper-case variables persist between calls.
#[derive(Debug, Clone)]
pub struct Expander {
    static_variables: [Value; 26],
    stack: Vec<Value>,
    limits: ExpanderLimits,
}

impl Default for Expander {
    fn default() -> Self {
        Self::new()
    }
}

impl Expander {
    /// Creates expansion state with standard defensive limits.
    pub fn new() -> Self {
        Self {
            static_variables: core::array::from_fn(|_| Value::default()),
            stack: Vec::new(),
            limits: ExpanderLimits::standard(),
        }
    }

    /// Replaces defensive limits while preserving static variables.
    pub const fn with_limits(mut self, limits: ExpanderLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Returns active defensive limits.
    pub const fn limits(&self) -> ExpanderLimits {
        self.limits
    }

    /// Reuses caller storage and preserves it on failure.
    pub fn run_into(
        &mut self,
        program: &Program,
        params: &[Param<'_>],
        output: &mut Vec<u8>,
    ) -> Result<(), ExpandError> {
        let initial_len = output.len();
        let result = self.run(program.as_bytes(), params, output);
        if result.is_err() {
            output.truncate(initial_len);
        }
        result
    }

    fn run(
        &mut self,
        capability: &[u8],
        params: &[Param<'_>],
        output: &mut Vec<u8>,
    ) -> Result<(), ExpandError> {
        self.stack.clear();
        let mut values: [Value; 9] = core::array::from_fn(|_| Value::default());
        for (target, source) in values.iter_mut().zip(params.iter().take(9)) {
            *target = match source {
                Param::Number(value) => Value::Number(*value),
                Param::Bytes(value) => {
                    if value.len() > self.limits.max_output() {
                        return Err(error(0, ExpandErrorKind::OutputLimit));
                    }
                    let mut copy = Vec::new();
                    copy.try_reserve_exact(value.len())
                        .map_err(|_| error(0, ExpandErrorKind::OutputLimit))?;
                    copy.extend_from_slice(value);
                    Value::Bytes(copy)
                }
            };
        }
        let termcap_hack = !has_explicit_parameter(capability);
        if termcap_hack {
            for value in values[..params.len().min(9)].iter().rev() {
                push_stack(&mut self.stack, value.clone(), 0, self.limits.max_stack())?;
            }
        }
        let mut dynamic: [Value; 26] = core::array::from_fn(|_| Value::default());
        let mut conditions: Vec<Condition> = Vec::new();
        let mut active = true;
        let mut index = 0;
        let mut steps = 0usize;
        while index < capability.len() {
            steps = steps
                .checked_add(1)
                .ok_or_else(|| error(index, ExpandErrorKind::StepLimit))?;
            if steps > self.limits.max_steps() {
                return Err(error(index, ExpandErrorKind::StepLimit));
            }
            if capability[index] != b'%' {
                if active {
                    push_output(
                        output,
                        &[capability[index]],
                        index,
                        self.limits.max_output(),
                    )?;
                }
                index += 1;
                continue;
            }
            let operator_offset = index;
            index += 1;
            let operator = *capability
                .get(index)
                .ok_or_else(|| error(operator_offset, ExpandErrorKind::TruncatedOperator))?;
            index += 1;

            match operator {
                b'?' => {
                    conditions.push(Condition {
                        parent_active: active,
                        condition: false,
                        saw_then: false,
                    });
                }
                b't' => {
                    let frame = conditions.last_mut().ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::UnbalancedConditional)
                    })?;
                    if frame.saw_then {
                        return Err(error(
                            operator_offset,
                            ExpandErrorKind::UnbalancedConditional,
                        ));
                    }
                    frame.condition = if frame.parent_active {
                        pop_number(&mut self.stack, operator_offset)? != 0
                    } else {
                        false
                    };
                    frame.saw_then = true;
                    active = frame.parent_active && frame.condition;
                }
                b'e' => {
                    let frame = conditions.last().ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::UnbalancedConditional)
                    })?;
                    if !frame.saw_then {
                        return Err(error(
                            operator_offset,
                            ExpandErrorKind::UnbalancedConditional,
                        ));
                    }
                    active = frame.parent_active && !frame.condition;
                }
                b';' => {
                    let frame = conditions.pop().ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::UnbalancedConditional)
                    })?;
                    if !frame.saw_then {
                        return Err(error(
                            operator_offset,
                            ExpandErrorKind::UnbalancedConditional,
                        ));
                    }
                    active = frame.parent_active;
                }
                b'%' => {
                    if active {
                        push_output(output, b"%", operator_offset, self.limits.max_output())?;
                    }
                }
                b'p' => {
                    let digit = *capability.get(index).ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::TruncatedOperator)
                    })?;
                    index += 1;
                    if active {
                        let parameter = digit
                            .checked_sub(b'1')
                            .filter(|value| *value < 9)
                            .ok_or_else(|| {
                                error(operator_offset, ExpandErrorKind::InvalidParameter)
                            })?;
                        push_stack(
                            &mut self.stack,
                            values[usize::from(parameter)].clone(),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                }
                b'P' | b'g' => {
                    let variable = *capability.get(index).ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::TruncatedOperator)
                    })?;
                    index += 1;
                    if active {
                        let (array, variable_index) = variable_slot(
                            variable,
                            &mut dynamic,
                            &mut self.static_variables,
                            operator_offset,
                        )?;
                        if operator == b'P' {
                            array[variable_index] = pop(&mut self.stack, operator_offset)?;
                        } else {
                            push_stack(
                                &mut self.stack,
                                array[variable_index].clone(),
                                operator_offset,
                                self.limits.max_stack(),
                            )?;
                        }
                    }
                }
                b'{' => {
                    let start = index;
                    while capability.get(index).is_some_and(|byte| *byte != b'}') {
                        index += 1;
                    }
                    if capability.get(index) != Some(&b'}') {
                        return Err(error(operator_offset, ExpandErrorKind::TruncatedOperator));
                    }
                    if active {
                        let text = core::str::from_utf8(&capability[start..index])
                            .map_err(|_| error(operator_offset, ExpandErrorKind::InvalidNumber))?;
                        let value = text
                            .parse::<i64>()
                            .map_err(|_| error(operator_offset, ExpandErrorKind::InvalidNumber))?;
                        push_stack(
                            &mut self.stack,
                            Value::Number(value),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                    index += 1;
                }
                b'\'' => {
                    let value = *capability.get(index).ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::TruncatedOperator)
                    })?;
                    index += 1;
                    if capability.get(index) != Some(&b'\'') {
                        return Err(error(operator_offset, ExpandErrorKind::TruncatedOperator));
                    }
                    index += 1;
                    if active {
                        push_stack(
                            &mut self.stack,
                            Value::Number(i64::from(value)),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                }
                b'i' => {
                    if active {
                        increment(&mut values[0], operator_offset)?;
                        increment(&mut values[1], operator_offset)?;
                        if termcap_hack {
                            sync_implicit_parameters(&mut self.stack, &values, params.len().min(9));
                        }
                    }
                }
                b'l' => {
                    if active {
                        let value = pop_bytes(&mut self.stack, operator_offset)?;
                        push_stack(
                            &mut self.stack,
                            Value::Number(value.len() as i64),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                }
                b'+' | b'-' | b'*' | b'/' | b'm' | b'&' | b'|' | b'^' | b'=' | b'>' | b'<'
                | b'A' | b'O' => {
                    if active {
                        let right = pop_number(&mut self.stack, operator_offset)?;
                        let left = pop_number(&mut self.stack, operator_offset)?;
                        let value = match operator {
                            b'+' => left.wrapping_add(right),
                            b'-' => left.wrapping_sub(right),
                            b'*' => left.wrapping_mul(right),
                            b'/' => {
                                if right == 0 {
                                    return Err(error(
                                        operator_offset,
                                        ExpandErrorKind::DivideByZero,
                                    ));
                                }
                                left.wrapping_div(right)
                            }
                            b'm' => {
                                if right == 0 {
                                    return Err(error(
                                        operator_offset,
                                        ExpandErrorKind::DivideByZero,
                                    ));
                                }
                                left.wrapping_rem(right)
                            }
                            b'&' => left & right,
                            b'|' => left | right,
                            b'^' => left ^ right,
                            b'=' => i64::from(left == right),
                            b'>' => i64::from(left > right),
                            b'<' => i64::from(left < right),
                            b'A' => i64::from(left != 0 && right != 0),
                            b'O' => i64::from(left != 0 || right != 0),
                            _ => unreachable!(),
                        };
                        push_stack(
                            &mut self.stack,
                            Value::Number(value),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                }
                b'!' | b'~' => {
                    if active {
                        let value = pop_number(&mut self.stack, operator_offset)?;
                        push_stack(
                            &mut self.stack,
                            Value::Number(if operator == b'!' {
                                i64::from(value == 0)
                            } else {
                                !value
                            }),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                }
                b'B' | b'D' => {
                    if active {
                        let value = pop_number(&mut self.stack, operator_offset)?;
                        let transformed = if operator == b'B' {
                            (value / 10) * 16 + value % 10
                        } else {
                            value - 2 * (value % 16)
                        };
                        push_stack(
                            &mut self.stack,
                            Value::Number(transformed),
                            operator_offset,
                            self.limits.max_stack(),
                        )?;
                    }
                }
                b'r' => {
                    if active {
                        values.swap(0, 1);
                        if termcap_hack {
                            sync_implicit_parameters(&mut self.stack, &values, params.len().min(9));
                        }
                    }
                }
                b'n' => {
                    if active {
                        xor_parameter(&mut values[0], operator_offset)?;
                        xor_parameter(&mut values[1], operator_offset)?;
                        if termcap_hack {
                            sync_implicit_parameters(&mut self.stack, &values, params.len().min(9));
                        }
                    }
                }
                b'd' | b'o' | b'x' | b'X' | b'c' | b's' => {
                    if active {
                        format_value(
                            &mut self.stack,
                            operator,
                            "",
                            output,
                            operator_offset,
                            self.limits.max_output(),
                        )?;
                    }
                }
                b'2' | b'3'
                    if capability
                        .get(index)
                        .is_none_or(|next| !matches!(*next, b'd' | b'o' | b'x' | b'X' | b's')) =>
                {
                    if active {
                        let spec = if operator == b'2' { "02" } else { "03" };
                        format_value(
                            &mut self.stack,
                            b'd',
                            spec,
                            output,
                            operator_offset,
                            self.limits.max_output(),
                        )?;
                    }
                }
                b':' | b'#' | b'0'..=b'9' | b'.' | b' ' => {
                    let spec_start = index - 1;
                    while capability.get(index).is_some_and(|byte| {
                        matches!(*byte, b'#' | b'0'..=b'9' | b'.' | b' ' | b'-' | b'+')
                    }) {
                        index += 1;
                    }
                    let conversion = *capability.get(index).ok_or_else(|| {
                        error(operator_offset, ExpandErrorKind::TruncatedOperator)
                    })?;
                    index += 1;
                    if !matches!(conversion, b'd' | b'o' | b'x' | b'X' | b's') {
                        return Err(error(operator_offset, ExpandErrorKind::TruncatedOperator));
                    }
                    if active {
                        let mut spec = core::str::from_utf8(&capability[spec_start..index - 1])
                            .map_err(|_| {
                                error(operator_offset, ExpandErrorKind::TruncatedOperator)
                            })?;
                        if let Some(rest) = spec.strip_prefix(':') {
                            spec = rest;
                        }
                        format_value(
                            &mut self.stack,
                            conversion,
                            spec,
                            output,
                            operator_offset,
                            self.limits.max_output(),
                        )?;
                    }
                }
                _ => return Err(error(operator_offset, ExpandErrorKind::TruncatedOperator)),
            }
        }
        if !conditions.is_empty() {
            return Err(error(
                capability.len(),
                ExpandErrorKind::UnbalancedConditional,
            ));
        }
        Ok(())
    }
}

/// Expands once and discards static variables.
pub fn expand(capability: &[u8], params: &[Param<'_>]) -> Result<Vec<u8>, ExpandError> {
    let program = Program::parse(capability)?;
    let mut output = Vec::new();
    Expander::new().run_into(&program, params, &mut output)?;
    Ok(output)
}

/// Parsed padding duration and applicability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Padding {
    tenths_of_millisecond: u32,
    proportional: bool,
    mandatory: bool,
}

impl Padding {
    /// Returns the base delay in tenths of a millisecond.
    pub const fn tenths_of_millisecond(self) -> u32 {
        self.tenths_of_millisecond
    }

    /// Reports whether delay scales with affected lines.
    pub const fn proportional(self) -> bool {
        self.proportional
    }

    /// Reports whether flow control may suppress the delay.
    pub const fn mandatory(self) -> bool {
        self.mandatory
    }

    /// Computes an effective delay for runtime terminal conditions.
    pub fn effective(self, context: PaddingContext) -> Option<u64> {
        if !self.mandatory && (context.xon || context.baud < context.padding_baud_rate) {
            return None;
        }
        let lines = if self.proportional {
            context.affected_lines.max(1)
        } else {
            1
        };
        u64::from(self.tenths_of_millisecond).checked_mul(u64::from(lines))
    }
}

/// Runtime facts used to decide and scale padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaddingContext {
    xon: bool,
    baud: u32,
    padding_baud_rate: u32,
    affected_lines: u32,
}

impl PaddingContext {
    /// Creates a context which emits mandatory delays only.
    pub const fn new() -> Self {
        Self {
            xon: false,
            baud: u32::MAX,
            padding_baud_rate: 0,
            affected_lines: 1,
        }
    }

    /// Sets whether XON/XOFF flow control is active.
    pub const fn with_xon(mut self, value: bool) -> Self {
        self.xon = value;
        self
    }

    /// Sets the current output baud rate.
    pub const fn with_baud(mut self, value: u32) -> Self {
        self.baud = value;
        self
    }

    /// Sets the entry's padding baud-rate threshold.
    pub const fn with_padding_baud_rate(mut self, value: u32) -> Self {
        self.padding_baud_rate = value;
        self
    }

    /// Sets the line count for proportional delays.
    pub const fn with_affected_lines(mut self, value: u32) -> Self {
        self.affected_lines = value;
        self
    }
}

impl Default for PaddingContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Borrowed output segment produced by padding parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputEvent<'a> {
    /// Literal terminal bytes.
    Bytes(&'a [u8]),
    /// Parsed padding request.
    Delay(Padding),
}

/// Owned expansion output segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedOutputEvent {
    /// Owned literal terminal bytes.
    Bytes(Vec<u8>),
    /// Parsed padding request.
    Delay(Padding),
}

/// Expands a parameter program and separates padding requests.
pub fn expand_events(
    capability: &[u8],
    params: &[Param<'_>],
) -> Result<Vec<OwnedOutputEvent>, ExpandError> {
    let bytes = expand(capability, params)?;
    Ok(parse_padding(&bytes)
        .into_iter()
        .map(|event| match event {
            OutputEvent::Bytes(bytes) => OwnedOutputEvent::Bytes(bytes.to_vec()),
            OutputEvent::Delay(delay) => OwnedOutputEvent::Delay(delay),
        })
        .collect())
}

/// Splits already-expanded bytes into literal and padding events.
pub fn parse_padding(bytes: &[u8]) -> Vec<OutputEvent<'_>> {
    let mut events = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index + 2 < bytes.len() {
        if &bytes[index..index + 2] == b"$<"
            && let Some(relative_end) = bytes[index + 2..].iter().position(|byte| *byte == b'>')
        {
            let end = index + 2 + relative_end;
            if let Some(delay) = parse_delay(&bytes[index + 2..end]) {
                if start < index {
                    events.push(OutputEvent::Bytes(&bytes[start..index]));
                }
                events.push(OutputEvent::Delay(delay));
                index = end + 1;
                start = index;
                continue;
            }
        }
        index += 1;
    }
    if start < bytes.len() {
        events.push(OutputEvent::Bytes(&bytes[start..]));
    }
    events
}

fn parse_delay(value: &[u8]) -> Option<Padding> {
    let proportional = value.contains(&b'*');
    let mandatory = value.contains(&b'/');
    let numeric: Vec<u8> = value
        .iter()
        .copied()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b'.')
        .collect();
    if numeric.is_empty() {
        return None;
    }
    let text = core::str::from_utf8(&numeric).ok()?;
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let whole = whole.parse::<u32>().ok()?;
    let tenth = fraction
        .as_bytes()
        .first()
        .map_or(0, |byte| u32::from(byte - b'0'));
    Some(Padding {
        tenths_of_millisecond: whole.checked_mul(10)?.checked_add(tenth)?,
        proportional,
        mandatory,
    })
}

#[derive(Debug, Clone, Copy)]
struct Condition {
    parent_active: bool,
    condition: bool,
    saw_then: bool,
}

fn variable_slot<'a>(
    name: u8,
    dynamic: &'a mut [Value; 26],
    static_variables: &'a mut [Value; 26],
    offset: usize,
) -> Result<(&'a mut [Value; 26], usize), ExpandError> {
    match name {
        b'a'..=b'z' => Ok((dynamic, usize::from(name - b'a'))),
        b'A'..=b'Z' => Ok((static_variables, usize::from(name - b'A'))),
        _ => Err(error(offset, ExpandErrorKind::InvalidVariable)),
    }
}
fn has_explicit_parameter(value: &[u8]) -> bool {
    let mut index = 0usize;
    while index + 1 < value.len() {
        if value[index] != b'%' {
            index += 1;
            continue;
        }
        if value[index + 1] == b'p' {
            return true;
        }
        index += 2;
    }
    false
}
fn sync_implicit_parameters(stack: &mut [Value], values: &[Value; 9], count: usize) {
    for (parameter, value) in values.iter().take(count.min(stack.len())).enumerate() {
        let stack_index = stack.len() - parameter - 1;
        stack[stack_index] = value.clone();
    }
}
fn increment(value: &mut Value, offset: usize) -> Result<(), ExpandError> {
    match value {
        Value::Number(value) => {
            *value = value.wrapping_add(1);
            Ok(())
        }
        Value::Bytes(_) => Err(error(offset, ExpandErrorKind::TypeMismatch)),
    }
}
fn xor_parameter(value: &mut Value, offset: usize) -> Result<(), ExpandError> {
    match value {
        Value::Number(value) => {
            *value ^= 0o140;
            Ok(())
        }
        Value::Bytes(_) => Err(error(offset, ExpandErrorKind::TypeMismatch)),
    }
}
fn push_stack(
    stack: &mut Vec<Value>,
    value: Value,
    offset: usize,
    max_stack: usize,
) -> Result<(), ExpandError> {
    if stack.len() >= max_stack {
        Err(error(offset, ExpandErrorKind::StackLimit))
    } else {
        stack.push(value);
        Ok(())
    }
}
fn pop(stack: &mut Vec<Value>, offset: usize) -> Result<Value, ExpandError> {
    stack
        .pop()
        .ok_or_else(|| error(offset, ExpandErrorKind::StackUnderflow))
}
fn pop_number(stack: &mut Vec<Value>, offset: usize) -> Result<i64, ExpandError> {
    match pop(stack, offset)? {
        Value::Number(value) => Ok(value),
        Value::Bytes(_) => Err(error(offset, ExpandErrorKind::TypeMismatch)),
    }
}
fn pop_bytes(stack: &mut Vec<Value>, offset: usize) -> Result<Vec<u8>, ExpandError> {
    match pop(stack, offset)? {
        Value::Bytes(value) => Ok(value),
        Value::Number(_) => Err(error(offset, ExpandErrorKind::TypeMismatch)),
    }
}

fn format_value(
    stack: &mut Vec<Value>,
    conversion: u8,
    spec: &str,
    output: &mut Vec<u8>,
    offset: usize,
    max_output: usize,
) -> Result<(), ExpandError> {
    let flags = FormatSpec::parse(spec);
    let remaining = max_output.saturating_sub(output.len());
    if flags.width > remaining || flags.precision.is_some_and(|value| value > remaining) {
        return Err(error(offset, ExpandErrorKind::OutputLimit));
    }
    let bytes = if conversion == b's' {
        let mut value = pop_bytes(stack, offset)?;
        if let Some(precision) = flags.precision {
            value.truncate(precision);
        }
        pad(value, flags, false, offset)?
    } else if conversion == b'c' {
        vec![pop_number(stack, offset)? as u8]
    } else {
        let value = pop_number(stack, offset)?;
        let (negative, magnitude) = if value < 0 {
            (true, value.unsigned_abs())
        } else {
            (false, value as u64)
        };
        let (mut digits, prefix) = match conversion {
            b'd' => (
                format!("{magnitude}"),
                if negative {
                    "-"
                } else if flags.plus {
                    "+"
                } else if flags.space {
                    " "
                } else {
                    ""
                },
            ),
            b'o' => (
                format!("{magnitude:o}"),
                if flags.alternate && magnitude != 0 {
                    "0"
                } else {
                    ""
                },
            ),
            b'x' => (
                format!("{magnitude:x}"),
                if flags.alternate && magnitude != 0 {
                    "0x"
                } else {
                    ""
                },
            ),
            b'X' => (
                format!("{magnitude:X}"),
                if flags.alternate && magnitude != 0 {
                    "0X"
                } else {
                    ""
                },
            ),
            _ => return Err(error(offset, ExpandErrorKind::TruncatedOperator)),
        };
        if let Some(precision) = flags.precision
            && digits.len() < precision
        {
            let zeros = precision - digits.len();
            let mut padded = alloc::string::String::new();
            padded
                .try_reserve_exact(precision)
                .map_err(|_| error(offset, ExpandErrorKind::OutputLimit))?;
            padded.extend(core::iter::repeat_n('0', zeros));
            padded.push_str(&digits);
            digits = padded;
        }
        let value_len = prefix
            .len()
            .checked_add(digits.len())
            .ok_or_else(|| error(offset, ExpandErrorKind::OutputLimit))?;
        if value_len > remaining {
            return Err(error(offset, ExpandErrorKind::OutputLimit));
        }
        let mut value = Vec::new();
        value
            .try_reserve_exact(value_len)
            .map_err(|_| error(offset, ExpandErrorKind::OutputLimit))?;
        value.extend_from_slice(prefix.as_bytes());
        value.extend_from_slice(digits.as_bytes());
        pad(value, flags, true, offset)?
    };
    push_output(output, &bytes, offset, max_output)
}

#[derive(Debug, Clone, Copy, Default)]
struct FormatSpec {
    width: usize,
    precision: Option<usize>,
    left: bool,
    zero: bool,
    plus: bool,
    space: bool,
    alternate: bool,
}
impl FormatSpec {
    fn parse(mut value: &str) -> Self {
        let mut result = Self::default();
        loop {
            match value.as_bytes().first() {
                Some(b'-') => result.left = true,
                Some(b'0') => result.zero = true,
                Some(b'+') => result.plus = true,
                Some(b' ') => result.space = true,
                Some(b'#') => result.alternate = true,
                _ => break,
            }
            value = &value[1..];
        }
        let width_len = value.bytes().take_while(u8::is_ascii_digit).count();
        if width_len != 0 {
            result.width = value[..width_len].bytes().fold(0usize, |sum, byte| {
                sum.saturating_mul(10)
                    .saturating_add(usize::from(byte - b'0'))
            });
            value = &value[width_len..];
        }
        if let Some(rest) = value.strip_prefix('.') {
            result.precision = Some(rest.bytes().take_while(u8::is_ascii_digit).fold(
                0usize,
                |sum, byte| {
                    sum.saturating_mul(10)
                        .saturating_add(usize::from(byte - b'0'))
                },
            ));
        }
        result
    }
}
fn pad(
    mut value: Vec<u8>,
    spec: FormatSpec,
    numeric: bool,
    offset: usize,
) -> Result<Vec<u8>, ExpandError> {
    if value.len() >= spec.width {
        return Ok(value);
    }
    let count = spec.width - value.len();
    if spec.left {
        value
            .try_reserve_exact(count)
            .map_err(|_| error(offset, ExpandErrorKind::OutputLimit))?;
        value.extend(core::iter::repeat_n(b' ', count));
        return Ok(value);
    }
    let byte = if numeric && spec.zero && spec.precision.is_none() {
        b'0'
    } else {
        b' '
    };
    let prefix_len = if byte == b'0'
        && value
            .first()
            .is_some_and(|first| matches!(*first, b'+' | b'-' | b' '))
    {
        1
    } else if byte == b'0' && value.starts_with(b"0x") || byte == b'0' && value.starts_with(b"0X") {
        2
    } else {
        0
    };
    if prefix_len != 0 {
        let tail = value.split_off(prefix_len);
        let mut result = value;
        result
            .try_reserve_exact(count.saturating_add(tail.len()))
            .map_err(|_| error(offset, ExpandErrorKind::OutputLimit))?;
        result.extend(core::iter::repeat_n(byte, count));
        result.extend(tail);
        Ok(result)
    } else {
        let mut result = Vec::new();
        result
            .try_reserve_exact(spec.width)
            .map_err(|_| error(offset, ExpandErrorKind::OutputLimit))?;
        result.extend(core::iter::repeat_n(byte, count));
        result.extend(value);
        Ok(result)
    }
}
fn push_output(
    output: &mut Vec<u8>,
    bytes: &[u8],
    offset: usize,
    max_output: usize,
) -> Result<(), ExpandError> {
    if output.len().saturating_add(bytes.len()) > max_output {
        Err(error(offset, ExpandErrorKind::OutputLimit))
    } else {
        output
            .try_reserve(bytes.len())
            .map_err(|_| error(offset, ExpandErrorKind::OutputLimit))?;
        output.extend_from_slice(bytes);
        Ok(())
    }
}
const fn error(offset: usize, kind: ExpandErrorKind) -> ExpandError {
    ExpandError::new(offset, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cup_and_conditionals() {
        assert_eq!(
            expand(
                b"\x1b[%i%p1%d;%p2%02dH",
                &[Param::Number(3), Param::Number(7)]
            )
            .unwrap(),
            b"\x1b[4;08H"
        );
        assert_eq!(
            expand(b"%?%p1%{8}%<%tlow%ehigh%;", &[Param::Number(4)]).unwrap(),
            b"low"
        );
    }

    #[test]
    fn variables_and_strings() {
        let mut expander = Expander::new();
        let first = Program::parse(b"%{42}%PA%gA%d").unwrap();
        let second = Program::parse(b"%gA%{1}%+%d").unwrap();
        let mut output = Vec::new();
        expander.run_into(&first, &[], &mut output).unwrap();
        assert_eq!(output, b"42");
        output.clear();
        expander.run_into(&second, &[], &mut output).unwrap();
        assert_eq!(output, b"43");
        assert_eq!(
            expand(b"%p1%l%d:%p1%.3s", &[Param::Bytes(b"abcdef")]).unwrap(),
            b"6:abc"
        );
    }

    #[test]
    fn implicit_termcap_parameters_follow_ncurses_tparm_compatibility() {
        let program = Program::parse(b"%d;%d").unwrap();
        assert_eq!(program.analyze().parameter_count(), 2);
        assert_eq!(
            expand(b"%d;%d", &[Param::Number(3), Param::Number(4)]).unwrap(),
            b"3;4"
        );
        assert_eq!(
            expand(b"%r%d;%d", &[Param::Number(3), Param::Number(4)]).unwrap(),
            b"4;3"
        );
        assert_eq!(
            expand(b"%i%d;%d", &[Param::Number(3), Param::Number(4)]).unwrap(),
            b"4;5"
        );
        assert_eq!(
            expand(b"%n%d", &[Param::Number(1), Param::Number(2)]).unwrap(),
            b"97"
        );
        assert_eq!(expand(b"%B%d", &[Param::Number(42)]).unwrap(), b"66");
        assert_eq!(expand(b"%D%d", &[Param::Number(31)]).unwrap(), b"1");
        assert_eq!(expand(b"%2", &[Param::Number(7)]).unwrap(), b"07");
    }

    #[test]
    fn exposes_padding_events() {
        assert_eq!(
            parse_padding(b"a$<12.5*/>b"),
            vec![
                OutputEvent::Bytes(b"a"),
                OutputEvent::Delay(Padding {
                    tenths_of_millisecond: 125,
                    proportional: true,
                    mandatory: true
                }),
                OutputEvent::Bytes(b"b")
            ]
        );
    }

    #[test]
    fn program_analysis_and_limits_are_enforced() {
        let program = Program::parse(b"%p9%{1}%+%d").unwrap();
        assert_eq!(program.analyze().parameter_count(), 9);
        let mut expander = Expander::new().with_limits(
            ExpanderLimits::standard()
                .with_max_output(2)
                .with_max_steps(64),
        );
        let mut output = Vec::new();
        let error = expander
            .run_into(&Program::parse(b"abcd").unwrap(), &[], &mut output)
            .unwrap_err();
        assert_eq!(error.kind(), ExpandErrorKind::OutputLimit);
        assert!(output.is_empty());
    }

    #[test]
    fn oversized_printf_fields_fail_before_allocating() {
        let mut expander = Expander::new().with_limits(
            ExpanderLimits::standard()
                .with_max_output(32)
                .with_max_steps(64),
        );
        for source in [
            b"%p1%999999999999999999999999d".as_slice(),
            b"%p1%.999999999999999999999999d".as_slice(),
        ] {
            let program = Program::parse(source).unwrap();
            let mut output = Vec::new();
            let error = expander
                .run_into(&program, &[Param::Number(7)], &mut output)
                .unwrap_err();
            assert_eq!(error.kind(), ExpandErrorKind::OutputLimit);
            assert!(output.is_empty());
        }
    }
}
