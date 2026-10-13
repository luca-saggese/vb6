use vb6parse::files::ModuleFile;
use vb6parse::io::SourceFile;
use vb6semantic::{SemanticAnalyzer, SemanticError, VBType};

fn analyze(code: &str) -> SemanticAnalyzer {
    let source = SourceFile::from_string("MxFeatures.bas", code);
    let (module, failures) = ModuleFile::parse(&source).unpack();
    assert!(
        failures.is_empty(),
        "vb6parse cannot represent this MX input: {failures:?}"
    );
    let module = module.expect("module should parse");
    let mut analyzer = SemanticAnalyzer::new();
    analyzer
        .analyze_module(&module)
        .expect("semantic module analysis should complete");
    analyzer
}

#[test]
fn as_auto_is_not_misreported_as_a_user_defined_type() {
    let analyzer = analyze(
        "Attribute VB_Name = \"MxFeatures\"\n\
        Option Explicit\n\
        Public count As Auto\n\
        Public Sub Consume(value As Auto)\n\
        End Sub\n",
    );

    let symbol = analyzer
        .scope_manager()
        .all_scopes()
        .into_iter()
        .find_map(|scope| scope.symbols.get("count"))
        .expect("declared symbol");
    assert_eq!(symbol.type_info.kind, VBType::Unknown);
    assert_eq!(
        analyzer
            .errors()
            .iter()
            .filter(|error| matches!(
                error,
                SemanticError::UnsupportedFeature { feature, reason, .. }
                    if feature == "MX S04 As Auto"
                        && reason.contains("does not infer declaration types")
            ))
            .count(),
        2,
        "module declarations and procedure parameters both report blocked inference"
    );
}

#[test]
fn option_strict_is_reported_as_blocked_without_expression_type_analysis() {
    let analyzer = analyze(
        "Attribute VB_Name = \"MxFeatures\"\n\
         Option Strict On\n\
         Public Sub Work()\n\
             Dim count As Long\n\
             count = \"not a number\"\n\
         End Sub\n",
    );

    assert!(analyzer.errors().iter().any(|error| matches!(
        error,
        SemanticError::UnsupportedFeature { feature, reason, .. }
            if feature == "MX S08 Option Strict"
                && reason.contains("does not type-check expression operands")
    )));
    assert!(
        !analyzer
            .errors()
            .iter()
            .any(|error| matches!(error, SemanticError::TypeMismatch { .. })),
        "Option Strict is not fully enforced until expression type checking exists"
    );
}
