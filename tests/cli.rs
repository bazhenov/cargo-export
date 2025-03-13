#[test]
fn foo() {
    trycmd::TestCases::new().case("tests/cmd/*.toml");
}
