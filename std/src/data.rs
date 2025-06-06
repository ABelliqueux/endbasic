// EndBASIC
// Copyright 2022 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! Commands to interact with the data provided by `DATA` statements.

use async_trait::async_trait;
use endbasic_core::ast::{ArgSep, ExprType, Value, VarRef};
use endbasic_core::compiler::{ArgSepSyntax, RepeatedSyntax, RepeatedTypeSyntax};
use endbasic_core::exec::{Clearable, Error, Machine, Result, Scope};
use endbasic_core::syms::{Callable, CallableMetadata, CallableMetadataBuilder};
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

/// Category description for all symbols provided by this module.
pub(crate) const CATEGORY: &str = "Data management";

struct ClearableIndex(Rc<RefCell<usize>>);

impl Clearable for ClearableIndex {
    fn reset_state(&self, _syms: &mut endbasic_core::syms::Symbols) {
        *self.0.borrow_mut() = 0;
    }
}

/// The `READ` command.
pub struct ReadCommand {
    metadata: CallableMetadata,
    index: Rc<RefCell<usize>>,
}

impl ReadCommand {
    /// Creates a new `READ` command.
    pub fn new(index: Rc<RefCell<usize>>) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("READ")
                .with_syntax(&[(
                    &[],
                    Some(&RepeatedSyntax {
                        name: Cow::Borrowed("vref"),
                        type_syn: RepeatedTypeSyntax::VariableRef,
                        sep: ArgSepSyntax::Exactly(ArgSep::Long),
                        require_one: true,
                        allow_missing: false,
                    }),
                )])
                .with_category(CATEGORY)
                .with_description(
                    "Extracts data values from DATA statements.
DATA statements can appear anywhere in the program and they register data values into an array of \
values.  READ is then used to extract values from this array into the provided variables in the \
order in which they were defined.  In other words: READ maintains an internal index into the data \
array that gets incremented by the number of provided variables every time it is executed.
The variable references in the vref1..vrefN list must match the types or be compatible with the \
values in the corresponding position of the data array.  Empty values in the data array can be \
specified by DATA, and those are converted into the default values for the relevant types: \
booleans are false, numbers are 0, and strings are empty.
Attempting to extract more values than are defined by DATA results in an \"out of data\" error.
The index that READ uses to extract DATA values can be reset by RESTORE and, more generally, by \
CLEAR.",
                )
                .build(),
            index,
        })
    }
}

#[async_trait(?Send)]
impl Callable for ReadCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, mut scope: Scope<'_>, machine: &mut Machine) -> Result<()> {
        debug_assert_ne!(0, scope.nargs());

        let mut vrefs = Vec::with_capacity(scope.nargs());
        while scope.nargs() > 0 {
            vrefs.push(scope.pop_varref_with_pos());
        }

        let mut index = self.index.borrow_mut();
        for (vname, vtype, pos) in vrefs {
            let datum = {
                let data = machine.get_data();
                debug_assert!(*index <= data.len());
                if *index == data.len() {
                    return Err(Error::InternalError(
                        pos,
                        format!("Out of data reading into {}", vname),
                    ));
                }

                match (vtype, &data[*index]) {
                    (_, Some(datum)) => datum.clone(),
                    (ExprType::Boolean, None) => Value::Boolean(false),
                    (ExprType::Double, None) => Value::Double(0.0),
                    (ExprType::Integer, None) => Value::Integer(0),
                    (ExprType::Text, None) => Value::Text("".to_owned()),
                }
            };
            *index += 1;

            let vref = VarRef::new(vname.to_string(), Some(vtype));
            machine
                .get_mut_symbols()
                .set_var(&vref, datum)
                .map_err(|e| Error::SyntaxError(pos, format!("{}", e)))?;
        }

        Ok(())
    }
}

/// The `RESTORE` command.
pub struct RestoreCommand {
    metadata: CallableMetadata,
    index: Rc<RefCell<usize>>,
}

impl RestoreCommand {
    /// Creates a new `RESTORE` command.
    pub fn new(index: Rc<RefCell<usize>>) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("RESTORE")
                .with_syntax(&[(&[], None)])
                .with_category(CATEGORY)
                .with_description(
                    "Resets the index of the data element to be returned.
This allows READ to re-return the same elements that were previously extracted from the array of \
values defined by DATA.",
                )
                .build(),
            index,
        })
    }
}

#[async_trait(?Send)]
impl Callable for RestoreCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, scope: Scope<'_>, _machine: &mut Machine) -> Result<()> {
        debug_assert_eq!(0, scope.nargs());
        *self.index.borrow_mut() = 0;
        Ok(())
    }
}

/// Instantiates all symbols in this module and adds them to the `machine`.
pub fn add_all(machine: &mut Machine) {
    let index = Rc::from(RefCell::from(0));
    machine.add_clearable(Box::from(ClearableIndex(index.clone())));
    machine.add_callable(ReadCommand::new(index.clone()));
    machine.add_callable(RestoreCommand::new(index));
}

#[cfg(test)]
mod tests {
    use crate::testutils::*;
    use endbasic_core::ast::Value;

    #[test]
    fn test_read_simple() {
        Tester::default()
            .run(
                r#"
            READ i: PRINT i
            READ i: PRINT i
            DATA 3, 5, -7
            READ i: PRINT i
            "#,
            )
            .expect_prints([" 3", " 5", "-7"])
            .expect_var("I", Value::Integer(-7))
            .check();
    }

    #[test]
    fn test_read_multiple() {
        Tester::default()
            .run(
                r#"
            READ i, j, k, i
            DATA 3, 5, 7, 6
            "#,
            )
            .expect_var("I", Value::Integer(6))
            .expect_var("J", Value::Integer(5))
            .expect_var("K", Value::Integer(7))
            .check();
    }

    #[test]
    fn test_read_defaults_with_annotations() {
        Tester::default()
            .run(r#"DATA , , , ,: READ a, b?, d#, i%, s$"#)
            .expect_var("a", Value::Integer(0))
            .expect_var("b", Value::Boolean(false))
            .expect_var("d", Value::Double(0.0))
            .expect_var("i", Value::Integer(0))
            .expect_var("s", Value::Text("".to_owned()))
            .check();
    }

    #[test]
    fn test_read_defaults_without_annotations() {
        Tester::default()
            .run(
                r#"
            DIM b AS BOOLEAN
            DIM d AS DOUBLE
            DIM i AS INTEGER
            DIM s AS STRING
            DATA , , , ,
            READ a, b, d, i, s
            "#,
            )
            .expect_var("a", Value::Integer(0))
            .expect_var("b", Value::Boolean(false))
            .expect_var("d", Value::Double(0.0))
            .expect_var("i", Value::Integer(0))
            .expect_var("s", Value::Text("".to_owned()))
            .check();
    }

    #[test]
    fn test_read_double_to_integer() {
        Tester::default().run(r#"DATA 5.6: READ i%"#).expect_var("i", Value::Integer(6)).check();
    }

    #[test]
    fn test_read_integer_to_double() {
        Tester::default().run(r#"DATA 5: READ d#"#).expect_var("d", Value::Double(5.0)).check();
    }

    #[test]
    fn test_read_out_of_data() {
        Tester::default()
            .run(r#"DATA 5: READ i: READ j"#)
            .expect_err("1:22: Out of data reading into J")
            .expect_var("I", Value::Integer(5))
            .check();
    }

    #[test]
    fn test_read_clear_on_run() {
        Tester::default()
            .set_program(None, "DATA 1: READ i: PRINT i")
            .run(r#"RUN: RUN"#)
            .expect_clear()
            .expect_prints([" 1"])
            .expect_clear()
            .expect_prints([" 1"])
            .expect_var("I", Value::Integer(1))
            .expect_program(None as Option<String>, "DATA 1: READ i: PRINT i")
            .check();
    }

    #[test]
    fn test_read_index_remains_out_of_bounds() {
        let mut t = Tester::default();
        t.run(r#"DATA 1: READ i, j"#)
            .expect_var("i", Value::Integer(1))
            .expect_err("1:17: Out of data reading into J")
            .check();

        // This represents a second invocation in the REPL, which in principle should work to avoid
        // surprises but currently doesn't due to the fact that we maintain the index outside of the
        // machine and `machine.exec()` cannot clear it upfront.  Note how the read into `i` picks
        // up the second value, not the first one, because the `DATA` is only [1, 2], NOT [1, 1, 2],
        // but the index is still 1, not 0.  This is kind of intentional though, because adding
        // extra hooks into `machine.exec()` just for this single use case seems overkill.
        t.run(r#"DATA 1, 2: READ i, j"#)
            .expect_var("i", Value::Integer(2))
            .expect_err("1:20: Out of data reading into J")
            .check();

        // Running `CLEAR` explicitly should resolve the issue described above and give us the
        // expected behavior.
        t.run(r#"CLEAR"#).expect_clear().check();
        t.run(r#"DATA 1, 2: READ i, j"#)
            .expect_clear()
            .expect_var("i", Value::Integer(1))
            .expect_var("j", Value::Integer(2))
            .check();
    }

    #[test]
    fn test_read_errors() {
        check_stmt_compilation_err("1:1: READ expected vref1[, .., vrefN]", "READ");
        check_stmt_compilation_err("1:6: Requires a reference, not a value", "READ 3");
        check_stmt_compilation_err("1:1: READ expected vref1[, .., vrefN]", "READ i; j");

        check_stmt_err(
            "1:16: Cannot assign value of type STRING to variable of type INTEGER",
            "DATA \"x\": READ i",
        );
        check_stmt_err(
            "1:18: Cannot assign value of type BOOLEAN to variable of type INTEGER",
            "DATA FALSE: READ s%",
        );
    }

    #[test]
    fn test_restore_nothing() {
        Tester::default().run("RESTORE").check();
    }

    #[test]
    fn test_restore_before_read() {
        Tester::default()
            .run(
                r#"
            DATA 3, 5, 7
            RESTORE
            READ i: PRINT i
            READ i: PRINT i
            "#,
            )
            .expect_prints([" 3", " 5"])
            .expect_var("I", Value::Integer(5))
            .check();
    }

    #[test]
    fn test_restore_after_read() {
        Tester::default()
            .run(
                r#"
            DATA 3, -5, 7
            READ i: PRINT i
            READ i: PRINT i
            RESTORE
            READ i: PRINT i
            "#,
            )
            .expect_prints([" 3", "-5", " 3"])
            .expect_var("I", Value::Integer(3))
            .check();
    }

    #[test]
    fn test_restore_errors() {
        check_stmt_compilation_err("1:1: RESTORE expected no arguments", "RESTORE 123");
    }
}
