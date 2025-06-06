// EndBASIC
// Copyright 2021 Julio Merino
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

//! Array-related functions for EndBASIC.

use async_trait::async_trait;
use endbasic_core::ast::{ArgSep, ExprType, VarRef};
use endbasic_core::compiler::{
    ArgSepSyntax, RequiredRefSyntax, RequiredValueSyntax, SingularArgSyntax,
};
use endbasic_core::exec::{Error, Machine, Result, Scope};
use endbasic_core::syms::{
    Array, Callable, CallableMetadata, CallableMetadataBuilder, Symbol, Symbols,
};
use std::borrow::Cow;
use std::rc::Rc;

/// Category description for all symbols provided by this module.
const CATEGORY: &str = "Array functions";

/// Extracts the array reference and the dimension number from the list of arguments passed to
/// either `LBOUND` or `UBOUND`.
#[allow(clippy::needless_lifetimes)]
fn parse_bound_args<'a>(scope: &mut Scope<'_>, symbols: &'a Symbols) -> Result<(&'a Array, usize)> {
    let (arrayname, arraytype, arraypos) = scope.pop_varref_with_pos();

    let arrayref = VarRef::new(arrayname.to_string(), Some(arraytype));
    let array =
        match symbols.get(&arrayref).map_err(|e| Error::SyntaxError(arraypos, format!("{}", e)))? {
            Some(Symbol::Array(array)) => array,
            _ => unreachable!(),
        };

    if scope.nargs() == 1 {
        let (i, pos) = scope.pop_integer_with_pos();

        if i < 0 {
            return Err(Error::SyntaxError(pos, format!("Dimension {} must be positive", i)));
        }
        let i = i as usize;

        if i > array.dimensions().len() {
            return Err(Error::SyntaxError(
                pos,
                format!(
                    "Array {} has only {} dimensions but asked for {}",
                    arrayname,
                    array.dimensions().len(),
                    i,
                ),
            ));
        }
        Ok((array, i))
    } else {
        debug_assert_eq!(0, scope.nargs());

        if array.dimensions().len() > 1 {
            return Err(Error::SyntaxError(
                arraypos,
                "Requires a dimension for multidimensional arrays".to_owned(),
            ));
        }

        Ok((array, 1))
    }
}

/// The `LBOUND` function.
pub struct LboundFunction {
    metadata: CallableMetadata,
}

impl LboundFunction {
    /// Creates a new instance of the function.
    pub fn new() -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("LBOUND")
                .with_return_type(ExprType::Integer)
                .with_syntax(&[
                    (
                        &[SingularArgSyntax::RequiredRef(
                            RequiredRefSyntax {
                                name: Cow::Borrowed("array"),
                                require_array: true,
                                define_undefined: false,
                            },
                            ArgSepSyntax::End,
                        )],
                        None,
                    ),
                    (
                        &[
                            SingularArgSyntax::RequiredRef(
                                RequiredRefSyntax {
                                    name: Cow::Borrowed("array"),
                                    require_array: true,
                                    define_undefined: false,
                                },
                                ArgSepSyntax::Exactly(ArgSep::Long),
                            ),
                            SingularArgSyntax::RequiredValue(
                                RequiredValueSyntax {
                                    name: Cow::Borrowed("dimension"),
                                    vtype: ExprType::Integer,
                                },
                                ArgSepSyntax::End,
                            ),
                        ],
                        None,
                    ),
                ])
                .with_category(CATEGORY)
                .with_description(
                    "Returns the lower bound for the given dimension of the array.
The lower bound is the smallest available subscript that can be provided to array indexing \
operations.
For one-dimensional arrays, the dimension% is optional.  For multi-dimensional arrays, the \
dimension% is a 1-indexed integer.",
                )
                .build(),
        })
    }
}

#[async_trait(?Send)]
impl Callable for LboundFunction {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, mut scope: Scope<'_>, machine: &mut Machine) -> Result<()> {
        let (_array, _dim) = parse_bound_args(&mut scope, machine.get_symbols())?;
        scope.return_integer(0)
    }
}

/// The `UBOUND` function.
pub struct UboundFunction {
    metadata: CallableMetadata,
}

impl UboundFunction {
    /// Creates a new instance of the function.
    pub fn new() -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("UBOUND")
                .with_return_type(ExprType::Integer)
                .with_syntax(&[
                    (
                        &[SingularArgSyntax::RequiredRef(
                            RequiredRefSyntax {
                                name: Cow::Borrowed("array"),
                                require_array: true,
                                define_undefined: false,
                            },
                            ArgSepSyntax::End,
                        )],
                        None,
                    ),
                    (
                        &[
                            SingularArgSyntax::RequiredRef(
                                RequiredRefSyntax {
                                    name: Cow::Borrowed("array"),
                                    require_array: true,
                                    define_undefined: false,
                                },
                                ArgSepSyntax::Exactly(ArgSep::Long),
                            ),
                            SingularArgSyntax::RequiredValue(
                                RequiredValueSyntax {
                                    name: Cow::Borrowed("dimension"),
                                    vtype: ExprType::Integer,
                                },
                                ArgSepSyntax::End,
                            ),
                        ],
                        None,
                    ),
                ])
                .with_category(CATEGORY)
                .with_description(
                    "Returns the upper bound for the given dimension of the array.
The upper bound is the largest available subscript that can be provided to array indexing \
operations.
For one-dimensional arrays, the dimension% is optional.  For multi-dimensional arrays, the \
dimension% is a 1-indexed integer.",
                )
                .build(),
        })
    }
}

#[async_trait(?Send)]
impl Callable for UboundFunction {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, mut scope: Scope<'_>, machine: &mut Machine) -> Result<()> {
        let (array, dim) = parse_bound_args(&mut scope, machine.get_symbols())?;
        scope.return_integer((array.dimensions()[dim - 1] - 1) as i32)
    }
}

/// Adds all symbols provided by this module to the given `machine`.
pub fn add_all(machine: &mut Machine) {
    machine.add_callable(LboundFunction::new());
    machine.add_callable(UboundFunction::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::*;

    /// Validates error handling of `LBOUND` and `UBOUND` as given in `func`.
    fn do_bound_errors_test(func: &str) {
        Tester::default()
            .run(format!("DIM x(2): result = {}()", func))
            .expect_compilation_err(format!(
                "1:20: {} expected <array> | <array, dimension%>",
                func
            ))
            .check();

        Tester::default()
            .run(format!("DIM x(2): result = {}(x, 1, 2)", func))
            .expect_compilation_err(format!(
                "1:20: {} expected <array> | <array, dimension%>",
                func
            ))
            .check();

        Tester::default()
            .run(format!("DIM x(2): result = {}(x, -1)", func))
            .expect_err("1:30: Dimension -1 must be positive")
            .expect_array("x", ExprType::Integer, &[2], vec![])
            .check();

        Tester::default()
            .run(format!("DIM x(2): result = {}(x, TRUE)", func))
            .expect_compilation_err("1:30: BOOLEAN is not a number")
            .check();

        Tester::default()
            .run(format!("i = 0: result = {}(i)", func))
            .expect_compilation_err("1:24: Requires a reference, not a value")
            .check();

        Tester::default()
            .run(format!("result = {}(3)", func))
            .expect_compilation_err("1:17: Requires a reference, not a value")
            .check();

        Tester::default()
            .run(format!("i = 0: result = {}(i)", func))
            .expect_compilation_err("1:24: Requires a reference, not a value")
            .check();

        Tester::default()
            .run(format!("DIM i(3) AS BOOLEAN: result = {}(i$)", func))
            .expect_compilation_err("1:38: Incompatible type annotation in i$ reference")
            .check();

        Tester::default()
            .run(format!("result = {}(x)", func))
            .expect_compilation_err("1:17: Undefined symbol X")
            .check();

        Tester::default()
            .run(format!("DIM x(2, 3, 4): result = {}(x)", func))
            .expect_err("1:33: Requires a dimension for multidimensional arrays")
            .expect_array("x", ExprType::Integer, &[2, 3, 4], vec![])
            .check();

        Tester::default()
            .run(format!("DIM x(2, 3, 4): result = {}(x, 5)", func))
            .expect_err("1:36: Array X has only 3 dimensions but asked for 5")
            .expect_array("x", ExprType::Integer, &[2, 3, 4], vec![])
            .check();
    }

    #[test]
    fn test_lbound_ok() {
        Tester::default()
            .run("DIM x(10): result = LBOUND(x)")
            .expect_var("result", 0i32)
            .expect_array("x", ExprType::Integer, &[10], vec![])
            .check();

        Tester::default()
            .run("DIM x(10, 20): result = LBOUND(x, 1)")
            .expect_var("result", 0i32)
            .expect_array("x", ExprType::Integer, &[10, 20], vec![])
            .check();

        Tester::default()
            .run("DIM x(10, 20): result = LBOUND(x, 2.1)")
            .expect_var("result", 0i32)
            .expect_array("x", ExprType::Integer, &[10, 20], vec![])
            .check();
    }

    #[test]
    fn test_lbound_errors() {
        do_bound_errors_test("LBOUND");
    }

    #[test]
    fn test_ubound_ok() {
        Tester::default()
            .run("DIM x(10): result = UBOUND(x)")
            .expect_var("result", 9i32)
            .expect_array("x", ExprType::Integer, &[10], vec![])
            .check();

        Tester::default()
            .run("DIM x(10, 20): result = UBOUND(x, 1)")
            .expect_var("result", 9i32)
            .expect_array("x", ExprType::Integer, &[10, 20], vec![])
            .check();

        Tester::default()
            .run("DIM x(10, 20): result = UBOUND(x, 2.1)")
            .expect_var("result", 19i32)
            .expect_array("x", ExprType::Integer, &[10, 20], vec![])
            .check();
    }

    #[test]
    fn test_ubound_errors() {
        do_bound_errors_test("UBOUND");
    }

    #[test]
    fn test_bound_integration() {
        Tester::default()
            .run("DIM x(5): FOR i = LBOUND(x) TO UBOUND(x): x(i) = i * 2: NEXT")
            .expect_var("i", 5i32)
            .expect_array_simple(
                "x",
                ExprType::Integer,
                vec![0i32.into(), 2i32.into(), 4i32.into(), 6i32.into(), 8i32.into()],
            )
            .check();
    }
}
