use phplumb_index::stubs::summarize_stub;
use phplumb_index::summary::{Declaration, Member, MemberKind, SymbolKind, summarize};

const HEADER: &str = "<?php\n\
use JetBrains\\PhpStorm\\Internal\\LanguageLevelTypeAware;\n\
use JetBrains\\PhpStorm\\Internal\\PhpStormStubsElementAvailable;\n";

fn stub(body: &str) -> String {
    format!("{HEADER}{body}")
}

fn class<'a>(declarations: &'a [Declaration], fqn: &str) -> &'a Declaration {
    declarations
        .iter()
        .find(|d| d.fqn == fqn && d.class.is_some())
        .unwrap_or_else(|| panic!("no class {fqn}"))
}

fn methods<'a>(declaration: &'a Declaration, name: &str) -> Vec<&'a Member> {
    declaration
        .class
        .as_ref()
        .unwrap()
        .members
        .iter()
        .filter(|m| m.kind == MemberKind::Method && m.name == name)
        .collect()
}

/// The `ty` that `summarize` gives a method `m` declared with `returns`.
fn declared_ty(returns: &str) -> Option<phplumb_index::phpdoc::TypeExpr> {
    let source = format!("<?php\nclass T {{ public function m(): {returns} {{}} }}\n");
    let summary = summarize(source.as_bytes());
    methods(class(&summary.declarations, "T"), "m")[0]
        .ty
        .clone()
}

#[test]
fn stub_members_keep_the_php_85_declaration() {
    let source = stub(
        "class D {\n\
         #[PhpStormStubsElementAvailable(from: '5.5', to: '8.2')]\n\
         public function modify($m) {}\n\
         #[PhpStormStubsElementAvailable(from: '8.3')]\n\
         public function modify($m) {}\n\
         #[PhpStormStubsElementAvailable(to: '7.4')]\n\
         public function gone() {}\n\
         }\n",
    );
    let second = source.rfind("modify").unwrap() as u32;
    let summary = summarize_stub(source.as_bytes());
    let d = class(&summary.declarations, "D");
    let modify = methods(d, "modify");
    assert_eq!(modify.len(), 1);
    assert_eq!(modify[0].name_start, second);
    assert!(methods(d, "gone").is_empty());
}

#[test]
fn stub_functions_keep_the_php_85_declaration() {
    let source = stub(
        "#[PhpStormStubsElementAvailable(from: '8.0')]\n\
         function f(): int {}\n\
         #[PhpStormStubsElementAvailable(from: '5.3', to: '7.4')]\n\
         function f() {}\n",
    );
    let first = source.find("function f").unwrap() as u32 + "function ".len() as u32;
    let summary = summarize_stub(source.as_bytes());
    let found: Vec<&Declaration> = summary
        .declarations
        .iter()
        .filter(|d| d.kind == SymbolKind::Function && d.fqn == "f")
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name_start, first);
}

#[test]
fn positional_availability_bounds_count() {
    let source = stub(
        "#[PhpStormStubsElementAvailable('8.6')]\n\
         function h() {}\n\
         function kept() {}\n",
    );
    let summary = summarize_stub(source.as_bytes());
    let names: Vec<&str> = summary
        .declarations
        .iter()
        .map(|d| d.fqn.as_str())
        .collect();
    assert_eq!(names, ["kept"]);
}

#[test]
fn language_level_types_pick_the_newest_version_up_to_85() {
    let source = stub(
        "class D {}\nclass E {}\nclass C {\n\
         #[LanguageLevelTypeAware(['8.4' => 'D', '8.6' => 'E'], default: 'static|false')]\n\
         public function m() {}\n\
         }\n",
    );
    let summary = summarize_stub(source.as_bytes());
    let m = methods(class(&summary.declarations, "C"), "m");
    assert_eq!(m[0].ty, declared_ty("D"));
}

#[test]
fn language_level_types_fall_back_to_default() {
    let source = stub(
        "class D {}\nclass E {}\nclass C {\n\
         #[LanguageLevelTypeAware(['8.6' => 'E'], default: 'D')]\n\
         public function m() {}\n\
         }\n",
    );
    let summary = summarize_stub(source.as_bytes());
    let m = methods(class(&summary.declarations, "C"), "m");
    assert_eq!(m[0].ty, declared_ty("D"));
}

#[test]
fn unmarked_stub_declarations_stay() {
    let source = stub("class K {\npublic function run(): K {}\n}\nfunction g(): K {}\n");
    let stubbed = summarize_stub(source.as_bytes());
    let plain = summarize(source.as_bytes());
    let shape = |summary: &[Declaration]| -> Vec<(String, u32, Option<String>)> {
        summary
            .iter()
            .flat_map(|d| {
                let own = (
                    d.fqn.clone(),
                    d.name_start,
                    format!("{:?}", d.returns).into(),
                );
                let members = d
                    .class
                    .iter()
                    .flat_map(|c| c.members.iter())
                    .map(|m| (m.name.clone(), m.name_start, format!("{:?}", m.ty).into()));
                std::iter::once(own).chain(members).collect::<Vec<_>>()
            })
            .collect()
    };
    assert_eq!(shape(&stubbed.declarations), shape(&plain.declarations));
}
