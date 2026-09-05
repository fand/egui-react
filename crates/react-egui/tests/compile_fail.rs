//! Plan 2.4 / test 3-7: trybuild pins the wording of the macro errors and the
//! borrow errors users are expected to hit.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    t.pass("tests/ui/pass/*.rs");
}
