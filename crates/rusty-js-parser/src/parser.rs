
use crate::lexer::{LexError, Lexer, LexerGoal};
use crate::token::{Punct, TemplatePart, Token, TokenKind};
use rusty_js_ast::{
    BindingIdentifier, DefaultExportBody, ExportDeclaration, ExportEntry, ExportImportName,
    ExportSpecifier, Expr, ImportAttribute, ImportDeclaration, ImportEntry, ImportName,
    ImportSpecifier, Module, ModuleExportName, ModuleItem, ModuleSpecifier, Span, Stmt,
    VariableKind,
};

#[derive(Debug, Clone)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseGoal {
    Script,
    Module,
    Untagged,
}

pub mod parse_profile {
    use std::cell::{Cell, RefCell};
    use std::time::Instant;

    #[derive(Default)]
    struct Stats {
        bytes: usize,
        module_count: u64,
        module_ns: u64,
        statement_count: u64,
        statement_ns: u64,
        expression_count: u64,
        expression_ns: u64,
        function_body_count: u64,
        function_body_ns: u64,
        class_body_count: u64,
        class_body_ns: u64,
        stmt_var_count: u64,
        stmt_var_ns: u64,
        stmt_function_count: u64,
        stmt_function_ns: u64,
        stmt_expression_count: u64,
        stmt_expression_ns: u64,
        stmt_block_count: u64,
        stmt_block_ns: u64,
        stmt_control_count: u64,
        stmt_control_ns: u64,
        stmt_label_count: u64,
        stmt_label_ns: u64,
        expr_conditional_count: u64,
        expr_conditional_ns: u64,
        expr_binary_count: u64,
        expr_binary_ns: u64,
        expr_lhs_count: u64,
        expr_lhs_ns: u64,
        expr_primary_count: u64,
        expr_primary_ns: u64,
        fn_body_loop_count: u64,
        fn_body_loop_ns: u64,
        fn_body_bound_names_count: u64,
        fn_body_bound_names_ns: u64,
        fn_body_desugar_count: u64,
        fn_body_desugar_ns: u64,
        lhs_base_count: u64,
        lhs_base_ns: u64,
        lhs_cont_count: u64,
        lhs_cont_ns: u64,
        lhs_member_count: u64,
        lhs_member_ns: u64,
        lhs_computed_count: u64,
        lhs_computed_ns: u64,
        lhs_call_count: u64,
        lhs_call_ns: u64,
        lhs_template_count: u64,
        lhs_template_ns: u64,
        primary_ident_count: u64,
        primary_ident_ns: u64,
        primary_literal_count: u64,
        primary_literal_ns: u64,
        primary_fn_class_count: u64,
        primary_fn_class_ns: u64,
        primary_function_count: u64,
        primary_function_ns: u64,
        primary_class_count: u64,
        primary_class_ns: u64,
        primary_object_array_count: u64,
        primary_object_array_ns: u64,
        primary_paren_template_count: u64,
        primary_paren_template_ns: u64,
        primary_paren_count: u64,
        primary_paren_ns: u64,
        primary_template_count: u64,
        primary_template_ns: u64,
        primary_other_count: u64,
        primary_other_ns: u64,
        call_args_count: u64,
        call_args_ns: u64,
        paren_inner_count: u64,
        paren_inner_ns: u64,
        paren_inner_family_count: [u64; 7],
        paren_inner_family_ns: [u64; 7],
        paren_conditional_count: u64,
        paren_conditional_ns: u64,
        paren_binary_count: u64,
        paren_binary_ns: u64,
        paren_lhs_count: u64,
        paren_lhs_ns: u64,
        paren_primary_count: u64,
        paren_primary_ns: u64,
        fn_expr_params_count: u64,
        fn_expr_params_ns: u64,
        fn_expr_body_count: u64,
        fn_expr_body_ns: u64,
        fn_expr_revalidate_count: u64,
        fn_expr_revalidate_ns: u64,
        class_expr_super_count: u64,
        class_expr_super_ns: u64,
        class_expr_body_count: u64,
        class_expr_body_ns: u64,
        fn_body_loop_top_count: u64,
        fn_body_loop_top_ns: u64,
        fn_body_loop_nested_count: u64,
        fn_body_loop_nested_ns: u64,
        fn_body_top_stmt_family_count: [u64; 5],
        fn_body_top_stmt_family_ns: [u64; 5],
        fn_body_top_var_decl_phase_count: [u64; 4],
        fn_body_top_var_decl_phase_ns: [u64; 4],
        fn_body_top_fn_decl_phase_count: [u64; 6],
        fn_body_top_fn_decl_phase_ns: [u64; 6],
        fn_body_top_function_family_count: [u64; 4],
        fn_body_top_function_family_ns: [u64; 4],
        fn_body_top_class_decl_phase_count: [u64; 4],
        fn_body_top_class_decl_phase_ns: [u64; 4],
        fn_body_top_var_init_family_count: [u64; 6],
        fn_body_top_var_init_family_ns: [u64; 6],
        fn_body_top_var_init_expr_family_count: [u64; 6],
        fn_body_top_var_init_expr_family_ns: [u64; 6],
        fn_body_top_class_member_family_count: [u64; 4],
        fn_body_top_class_member_family_ns: [u64; 4],
        fn_body_top_class_method_phase_count: [u64; 5],
        fn_body_top_class_method_phase_ns: [u64; 5],
        fn_body_top_class_method_body_stmt_family_count: [u64; 5],
        fn_body_top_class_method_body_stmt_family_ns: [u64; 5],
        fn_body_top_class_method_body_control_family_count: [u64; 6],
        fn_body_top_class_method_body_control_family_ns: [u64; 6],
        fn_body_top_class_method_body_if_phase_count: [u64; 4],
        fn_body_top_class_method_body_if_phase_ns: [u64; 4],
        fn_body_top_class_method_body_if_consequent_family_count: [u64; 5],
        fn_body_top_class_method_body_if_consequent_family_ns: [u64; 5],
        fn_body_top_class_method_body_if_alternate_family_count: [u64; 5],
        fn_body_top_class_method_body_if_alternate_family_ns: [u64; 5],
        fn_body_top_class_method_body_if_consequent_block_stmt_count: [u64; 5],
        fn_body_top_class_method_body_if_consequent_block_stmt_ns: [u64; 5],
        fn_body_top_class_method_body_if_alternate_block_stmt_count: [u64; 5],
        fn_body_top_class_method_body_if_alternate_block_stmt_ns: [u64; 5],
        fn_body_top_class_method_body_if_consequent_block_control_count: [u64; 6],
        fn_body_top_class_method_body_if_consequent_block_control_ns: [u64; 6],
        fn_body_top_class_method_body_if_alternate_block_control_count: [u64; 6],
        fn_body_top_class_method_body_if_alternate_block_control_ns: [u64; 6],
        fn_body_top_class_method_body_if_consequent_block_if_phase_count: [u64; 4],
        fn_body_top_class_method_body_if_consequent_block_if_phase_ns: [u64; 4],
        fn_body_top_class_method_body_if_alternate_block_if_phase_count: [u64; 4],
        fn_body_top_class_method_body_if_alternate_block_if_phase_ns: [u64; 4],
        fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count: [u64; 5],
        fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns: [u64; 5],
        fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count: [u64; 5],
        fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns: [u64; 5],
        fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count: [u64; 5],
        fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns: [u64; 5],
        fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count: [u64; 5],
        fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns: [u64; 5],
        nested_stmt_var_count: u64,
        nested_stmt_var_ns: u64,
        nested_stmt_var_exclusive_ns: u64,
        nested_stmt_function_count: u64,
        nested_stmt_function_ns: u64,
        nested_stmt_function_exclusive_ns: u64,
        nested_stmt_expression_count: u64,
        nested_stmt_expression_ns: u64,
        nested_stmt_block_count: u64,
        nested_stmt_block_ns: u64,
        nested_stmt_control_count: u64,
        nested_stmt_control_ns: u64,
        nested_control_if_count: u64,
        nested_control_if_ns: u64,
        nested_control_for_count: u64,
        nested_control_for_ns: u64,
        nested_control_loop_count: u64,
        nested_control_loop_ns: u64,
        nested_control_switch_try_with_count: u64,
        nested_control_switch_try_with_ns: u64,
        nested_control_return_throw_count: u64,
        nested_control_return_throw_ns: u64,
        nested_control_break_continue_count: u64,
        nested_control_break_continue_ns: u64,
        nested_if_test_count: u64,
        nested_if_test_ns: u64,
        nested_if_consequent_count: u64,
        nested_if_consequent_ns: u64,
        nested_if_alternate_count: u64,
        nested_if_alternate_ns: u64,
        nested_if_close_count: u64,
        nested_if_close_ns: u64,
        nested_if_consequent_family_count: [u64; 5],
        nested_if_consequent_family_ns: [u64; 5],
        nested_if_alternate_family_count: [u64; 5],
        nested_if_alternate_family_ns: [u64; 5],
        nested_if_consequent_block_stmt_count: [u64; 5],
        nested_if_consequent_block_stmt_ns: [u64; 5],
        nested_if_alternate_block_stmt_count: [u64; 5],
        nested_if_alternate_block_stmt_ns: [u64; 5],
        nested_if_consequent_block_control_count: [u64; 6],
        nested_if_consequent_block_control_ns: [u64; 6],
        nested_if_alternate_block_control_count: [u64; 6],
        nested_if_alternate_block_control_ns: [u64; 6],
        nested_if_consequent_block_if_phase_count: [u64; 4],
        nested_if_consequent_block_if_phase_ns: [u64; 4],
        nested_if_alternate_block_if_phase_count: [u64; 4],
        nested_if_alternate_block_if_phase_ns: [u64; 4],
        nested_if_consequent_block_if_consequent_family_count: [u64; 5],
        nested_if_consequent_block_if_consequent_family_ns: [u64; 5],
        nested_if_consequent_block_if_alternate_family_count: [u64; 5],
        nested_if_consequent_block_if_alternate_family_ns: [u64; 5],
        nested_if_alternate_block_if_consequent_family_count: [u64; 5],
        nested_if_alternate_block_if_consequent_family_ns: [u64; 5],
        nested_if_alternate_block_if_alternate_family_count: [u64; 5],
        nested_if_alternate_block_if_alternate_family_ns: [u64; 5],
        nested_if_consequent_block_if_consequent_block_stmt_count: [u64; 5],
        nested_if_consequent_block_if_consequent_block_stmt_ns: [u64; 5],
        nested_if_alternate_block_if_consequent_block_stmt_count: [u64; 5],
        nested_if_alternate_block_if_consequent_block_stmt_ns: [u64; 5],
        nested_if_consequent_block_if_consequent_block_control_count: [u64; 6],
        nested_if_consequent_block_if_consequent_block_control_ns: [u64; 6],
        nested_if_alternate_block_if_consequent_block_control_count: [u64; 6],
        nested_if_alternate_block_if_consequent_block_control_ns: [u64; 6],
        nested_if_consequent_block_if_consequent_block_if_phase_count: [u64; 4],
        nested_if_consequent_block_if_consequent_block_if_phase_ns: [u64; 4],
        nested_if_alternate_block_if_consequent_block_if_phase_count: [u64; 4],
        nested_if_alternate_block_if_consequent_block_if_phase_ns: [u64; 4],
        nested_stmt_depth2_count: [u64; 5],
        nested_stmt_depth2_ns: [u64; 5],
        nested_stmt_depth3_count: [u64; 5],
        nested_stmt_depth3_ns: [u64; 5],
        nested_var_target_count: u64,
        nested_var_target_ns: u64,
        nested_var_no_let_count: u64,
        nested_var_no_let_ns: u64,
        nested_var_init_count: u64,
        nested_var_init_ns: u64,
        nested_var_init_exclusive_ns: u64,
        nested_var_init_assign_count: u64,
        nested_var_init_assign_ns: u64,
        nested_var_init_expr_count: u64,
        nested_var_init_expr_ns: u64,
        bump_fetch_count: u64,
        bump_fetch_ns: u64,
        bump_goal_count: u64,
        bump_goal_ns: u64,
        lexer_trivia_count: u64,
        lexer_trivia_ns: u64,
        lexer_punct_count: u64,
        lexer_punct_ns: u64,
        lexer_ident_count: u64,
        lexer_ident_ns: u64,
        lexer_private_ident_count: u64,
        lexer_private_ident_ns: u64,
        lexer_numeric_count: u64,
        lexer_numeric_ns: u64,
        lexer_string_count: u64,
        lexer_string_ns: u64,
        lexer_template_count: u64,
        lexer_template_ns: u64,
        lexer_regex_count: u64,
        lexer_regex_ns: u64,
        lexer_hashbang_count: u64,
        lexer_hashbang_ns: u64,
        lexer_eof_count: u64,
        lexer_eof_ns: u64,
        lexer_string_scan_count: u64,
        lexer_string_scan_ns: u64,
        lexer_string_convert_count: u64,
        lexer_string_convert_ns: u64,
        lexer_string_token_count: u64,
        lexer_string_token_ns: u64,
        lexer_string_escape_count: u64,
        lexer_string_escape_ns: u64,
        lexer_string_no_escape_count: u64,
        lexer_string_no_escape_ns: u64,
        lexer_string_no_escape_ascii_count: u64,
        lexer_string_no_escape_ascii_ns: u64,
        lexer_string_no_escape_non_ascii_count: u64,
        lexer_string_no_escape_non_ascii_ns: u64,
        lexer_string_no_escape_decode_count: u64,
        lexer_string_no_escape_decode_ns: u64,
        lexer_string_no_escape_marker_count: u64,
        lexer_string_no_escape_marker_ns: u64,
        lexer_string_no_escape_push_count: u64,
        lexer_string_no_escape_push_ns: u64,
        lexer_string_no_escape_advance_count: u64,
        lexer_string_no_escape_advance_ns: u64,
        nested_var_init_fn_class_count: u64,
        nested_var_init_fn_class_ns: u64,
        nested_var_init_paren_count: u64,
        nested_var_init_paren_ns: u64,
        nested_var_init_object_array_count: u64,
        nested_var_init_object_array_ns: u64,
        nested_var_init_ident_count: u64,
        nested_var_init_ident_ns: u64,
        nested_var_init_literal_count: u64,
        nested_var_init_literal_ns: u64,
        nested_var_init_other_count: u64,
        nested_var_init_other_ns: u64,
        nested_var_init_direct_fn_class_count: u64,
        nested_var_init_direct_fn_class_ns: u64,
        nested_var_init_direct_paren_count: u64,
        nested_var_init_direct_paren_ns: u64,
        nested_var_init_direct_object_array_count: u64,
        nested_var_init_direct_object_array_ns: u64,
        nested_var_init_direct_ident_count: u64,
        nested_var_init_direct_ident_ns: u64,
        nested_var_init_direct_literal_count: u64,
        nested_var_init_direct_literal_ns: u64,
        nested_var_init_direct_other_count: u64,
        nested_var_init_direct_other_ns: u64,
        nested_var_init_depth3_count: [u64; 6],
        nested_var_init_depth3_ns: [u64; 6],
        nested_var_init_depth3_object_array_count: [u64; 2],
        nested_var_init_depth3_object_array_ns: [u64; 2],
        nested_var_init_depth3_object_prop_count: [u64; 7],
        nested_var_init_depth3_object_prop_ns: [u64; 7],
        nested_var_init_depth3_object_colon_count: [u64; 3],
        nested_var_init_depth3_object_colon_ns: [u64; 3],
        nested_var_init_depth3_object_colon_value_count: [u64; 6],
        nested_var_init_depth3_object_colon_value_ns: [u64; 6],
        nested_var_finish_count: u64,
        nested_var_finish_ns: u64,
        nested_var_depth3_count: [u64; 4],
        nested_var_depth3_ns: [u64; 4],
        nested_fn_decl_name_count: u64,
        nested_fn_decl_name_ns: u64,
        nested_fn_decl_params_count: u64,
        nested_fn_decl_params_ns: u64,
        nested_fn_decl_dups_count: u64,
        nested_fn_decl_dups_ns: u64,
        nested_fn_decl_body_count: u64,
        nested_fn_decl_body_ns: u64,
        nested_fn_decl_body_exclusive_ns: u64,
        nested_fn_decl_revalidate_count: u64,
        nested_fn_decl_revalidate_ns: u64,
        nested_fn_decl_finish_count: u64,
        nested_fn_decl_finish_ns: u64,
        nested_fn_decl_depth2_count: [u64; 6],
        nested_fn_decl_depth2_ns: [u64; 6],
        nested_fn_body_loop_count: u64,
        nested_fn_body_loop_ns: u64,
        nested_fn_body_loop_depth2_count: u64,
        nested_fn_body_loop_depth2_ns: u64,
        nested_fn_body_loop_depth3_count: u64,
        nested_fn_body_loop_depth3_ns: u64,
        nested_fn_body_loop_depth4_plus_count: u64,
        nested_fn_body_loop_depth4_plus_ns: u64,
        nested_fn_body_bound_names_count: u64,
        nested_fn_body_bound_names_ns: u64,
        nested_fn_body_close_count: u64,
        nested_fn_body_close_ns: u64,
        nested_fn_body_restore_count: u64,
        nested_fn_body_restore_ns: u64,
        nested_fn_body_desugar_count: u64,
        nested_fn_body_desugar_ns: u64,
        nested_assign_async_count: u64,
        nested_assign_async_ns: u64,
        nested_assign_arrow_count: u64,
        nested_assign_arrow_ns: u64,
        nested_assign_conditional_count: u64,
        nested_assign_conditional_ns: u64,
        nested_assign_conditional_exclusive_ns: u64,
        nested_assign_rhs_count: u64,
        nested_assign_rhs_ns: u64,
        nested_assign_cover_count: u64,
        nested_assign_cover_ns: u64,
        top_var_init_ident_assign_count: [u64; 6],
        top_var_init_ident_assign_ns: [u64; 6],
        top_var_init_ident_arrow_count: [u64; 5],
        top_var_init_ident_arrow_ns: [u64; 5],
        top_var_init_ident_arrow_expr_assign_count: [u64; 5],
        top_var_init_ident_arrow_expr_assign_ns: [u64; 5],
        top_var_init_ident_arrow_expr_cond_count: [u64; 4],
        top_var_init_ident_arrow_expr_cond_ns: [u64; 4],
        top_var_init_ident_arrow_expr_cond_binary_count: [u64; 5],
        top_var_init_ident_arrow_expr_cond_binary_ns: [u64; 5],
        top_var_init_ident_arrow_expr_cond_unary_count: [u64; 5],
        top_var_init_ident_arrow_expr_cond_unary_ns: [u64; 5],
        top_var_init_ident_arrow_expr_cond_postfix_count: [u64; 3],
        top_var_init_ident_arrow_expr_cond_postfix_ns: [u64; 3],
        top_var_init_ident_arrow_expr_cond_lhs_count: [u64; 3],
        top_var_init_ident_arrow_expr_cond_lhs_ns: [u64; 3],
        top_var_init_ident_arrow_expr_cond_primary_count: [u64; 6],
        top_var_init_ident_arrow_expr_cond_primary_ns: [u64; 6],
        top_var_init_ident_arrow_expr_cond_fnclass_count: [u64; 7],
        top_var_init_ident_arrow_expr_cond_fnclass_ns: [u64; 7],
        top_var_init_ident_arrow_expr_cond_classbody_count: [u64; 4],
        top_var_init_ident_arrow_expr_cond_classbody_ns: [u64; 4],
        top_var_init_ident_arrow_expr_cond_class_member_count: [u64; 4],
        top_var_init_ident_arrow_expr_cond_class_member_ns: [u64; 4],
        top_var_init_ident_arrow_expr_cond_class_method_count: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_ns: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_body_control_count: [u64; 6],
        top_var_init_ident_arrow_expr_cond_class_method_body_control_ns: [u64; 6],
        top_var_init_ident_arrow_expr_cond_class_method_body_if_count: [u64; 4],
        top_var_init_ident_arrow_expr_cond_class_method_body_if_ns: [u64; 4],
        top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count: [u64; 5],
        top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns: [u64; 5],
    }

    thread_local! {
        static DEPTH: Cell<u32> = const { Cell::new(0) };
        static FUNCTION_BODY_DEPTH: Cell<u32> = const { Cell::new(0) };
        static TOP_CLASS_METHOD_BODY_DEPTH: Cell<u32> = const { Cell::new(0) };
        static PAREN_DEPTH: Cell<u32> = const { Cell::new(0) };
        static DEPTH3_OBJECT_INIT_DEPTH: Cell<u32> = const { Cell::new(0) };
        static TOP_VAR_INIT_IDENT_DEPTH: Cell<u32> = const { Cell::new(0) };
        static TOP_VAR_INIT_IDENT_ASSIGN_DEPTH: Cell<u32> = const { Cell::new(0) };
        static IF_BRANCH_BLOCK_BRANCH: Cell<u8> = const { Cell::new(0) };
        static IF_BRANCH_BLOCK_PARSE_DEPTH: Cell<u32> = const { Cell::new(0) };
        static IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH: Cell<u8> = const { Cell::new(0) };
        static IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
        static TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH: Cell<u8> = const { Cell::new(0) };
        static TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
        static SOURCE_LABEL: RefCell<String> = const { RefCell::new(String::new()) };
        static EXCLUSIVE_STACK: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
        static STATS: RefCell<Stats> = RefCell::new(Stats::default());
    }

    pub(crate) fn enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var_os("CRUFT_PARSER_PROFILE").is_some())
    }

    pub struct SourceLabelGuard {
        previous: String,
    }

    impl SourceLabelGuard {
        pub fn new(label: &str) -> Self {
            let previous = SOURCE_LABEL.with(|source_label| {
                let mut source_label = source_label.borrow_mut();
                std::mem::replace(&mut *source_label, label.to_string())
            });
            Self { previous }
        }
    }

    impl Drop for SourceLabelGuard {
        fn drop(&mut self) {
            SOURCE_LABEL.with(|source_label| {
                *source_label.borrow_mut() = std::mem::take(&mut self.previous);
            });
        }
    }

    fn min_bytes() -> usize {
        std::env::var("CRUFT_PARSER_PROFILE_MIN_BYTES")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(0)
    }

    pub(crate) struct RootGuard {
        active: bool,
        root: bool,
    }

    impl RootGuard {
        pub(crate) fn new(bytes: usize) -> Self {
            if !enabled() {
                return Self {
                    active: false,
                    root: false,
                };
            }
            let root = DEPTH.with(|depth| {
                let current = depth.get();
                depth.set(current + 1);
                current == 0
            });
            if root {
                STATS.with(|stats| {
                    let mut stats = stats.borrow_mut();
                    *stats = Stats::default();
                    stats.bytes = bytes;
                });
            }
            Self { active: true, root }
        }
    }

    impl Drop for RootGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            if self.root {
                STATS.with(|stats| {
                    let stats = stats.borrow();
                    if stats.bytes < min_bytes() {
                        return;
                    }
                    let source_label = SOURCE_LABEL.with(|source_label| source_label.borrow().clone());
                    eprintln!(
                        "[parser-profile] source_url={} bytes={} module={}:{}ns statement={}:{}ns expression={}:{}ns function_body={}:{}ns class_body={}:{}ns",
                        source_label,
                        stats.bytes,
                        stats.module_count,
                        stats.module_ns,
                        stats.statement_count,
                        stats.statement_ns,
                        stats.expression_count,
                        stats.expression_ns,
                        stats.function_body_count,
                        stats.function_body_ns,
                        stats.class_body_count,
                        stats.class_body_ns
                    );
                    eprintln!(
                        "[parser-profile-stmt] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns label={}:{}ns",
                        stats.stmt_var_count,
                        stats.stmt_var_ns,
                        stats.stmt_function_count,
                        stats.stmt_function_ns,
                        stats.stmt_expression_count,
                        stats.stmt_expression_ns,
                        stats.stmt_block_count,
                        stats.stmt_block_ns,
                        stats.stmt_control_count,
                        stats.stmt_control_ns,
                        stats.stmt_label_count,
                        stats.stmt_label_ns
                    );
                    eprintln!(
                        "[parser-profile-expr] conditional={}:{}ns binary={}:{}ns lhs={}:{}ns primary={}:{}ns",
                        stats.expr_conditional_count,
                        stats.expr_conditional_ns,
                        stats.expr_binary_count,
                        stats.expr_binary_ns,
                        stats.expr_lhs_count,
                        stats.expr_lhs_ns,
                        stats.expr_primary_count,
                        stats.expr_primary_ns
                    );
                    eprintln!(
                        "[parser-profile-fn-body] loop={}:{}ns bound_names={}:{}ns desugar={}:{}ns",
                        stats.fn_body_loop_count,
                        stats.fn_body_loop_ns,
                        stats.fn_body_bound_names_count,
                        stats.fn_body_bound_names_ns,
                        stats.fn_body_desugar_count,
                        stats.fn_body_desugar_ns
                    );
                    eprintln!(
                        "[parser-profile-lhs] base={}:{}ns cont={}:{}ns member={}:{}ns computed={}:{}ns call={}:{}ns template={}:{}ns",
                        stats.lhs_base_count,
                        stats.lhs_base_ns,
                        stats.lhs_cont_count,
                        stats.lhs_cont_ns,
                        stats.lhs_member_count,
                        stats.lhs_member_ns,
                        stats.lhs_computed_count,
                        stats.lhs_computed_ns,
                        stats.lhs_call_count,
                        stats.lhs_call_ns,
                        stats.lhs_template_count,
                        stats.lhs_template_ns
                    );
                    eprintln!(
                        "[parser-profile-primary] ident={}:{}ns literal={}:{}ns fn_class={}:{}ns object_array={}:{}ns paren_template={}:{}ns other={}:{}ns",
                        stats.primary_ident_count,
                        stats.primary_ident_ns,
                        stats.primary_literal_count,
                        stats.primary_literal_ns,
                        stats.primary_fn_class_count,
                        stats.primary_fn_class_ns,
                        stats.primary_object_array_count,
                        stats.primary_object_array_ns,
                        stats.primary_paren_template_count,
                        stats.primary_paren_template_ns,
                        stats.primary_other_count,
                        stats.primary_other_ns
                    );
                    eprintln!(
                        "[parser-profile-primary-split] function={}:{}ns class={}:{}ns paren={}:{}ns template={}:{}ns",
                        stats.primary_function_count,
                        stats.primary_function_ns,
                        stats.primary_class_count,
                        stats.primary_class_ns,
                        stats.primary_paren_count,
                        stats.primary_paren_ns,
                        stats.primary_template_count,
                        stats.primary_template_ns
                    );
                    eprintln!(
                        "[parser-profile-call-paren] call_args={}:{}ns paren_inner={}:{}ns",
                        stats.call_args_count,
                        stats.call_args_ns,
                        stats.paren_inner_count,
                        stats.paren_inner_ns
                    );
                    eprintln!(
                        "[parser-profile-paren-inner-family] function={}:{}ns class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.paren_inner_family_count[0],
                        stats.paren_inner_family_ns[0],
                        stats.paren_inner_family_count[1],
                        stats.paren_inner_family_ns[1],
                        stats.paren_inner_family_count[2],
                        stats.paren_inner_family_ns[2],
                        stats.paren_inner_family_count[3],
                        stats.paren_inner_family_ns[3],
                        stats.paren_inner_family_count[4],
                        stats.paren_inner_family_ns[4],
                        stats.paren_inner_family_count[5],
                        stats.paren_inner_family_ns[5],
                        stats.paren_inner_family_count[6],
                        stats.paren_inner_family_ns[6]
                    );
                    eprintln!(
                        "[parser-profile-paren-expr] conditional={}:{}ns binary={}:{}ns lhs={}:{}ns primary={}:{}ns",
                        stats.paren_conditional_count,
                        stats.paren_conditional_ns,
                        stats.paren_binary_count,
                        stats.paren_binary_ns,
                        stats.paren_lhs_count,
                        stats.paren_lhs_ns,
                        stats.paren_primary_count,
                        stats.paren_primary_ns
                    );
                    eprintln!(
                        "[parser-profile-fn-class] fn_params={}:{}ns fn_body={}:{}ns fn_revalidate={}:{}ns class_super={}:{}ns class_body={}:{}ns",
                        stats.fn_expr_params_count,
                        stats.fn_expr_params_ns,
                        stats.fn_expr_body_count,
                        stats.fn_expr_body_ns,
                        stats.fn_expr_revalidate_count,
                        stats.fn_expr_revalidate_ns,
                        stats.class_expr_super_count,
                        stats.class_expr_super_ns,
                        stats.class_expr_body_count,
                        stats.class_expr_body_ns
                    );
                    eprintln!(
                        "[parser-profile-fn-body-depth] top_loop={}:{}ns nested_loop={}:{}ns",
                        stats.fn_body_loop_top_count,
                        stats.fn_body_loop_top_ns,
                        stats.fn_body_loop_nested_count,
                        stats.fn_body_loop_nested_ns
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-stmt] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns",
                        stats.fn_body_top_stmt_family_count[0],
                        stats.fn_body_top_stmt_family_ns[0],
                        stats.fn_body_top_stmt_family_count[1],
                        stats.fn_body_top_stmt_family_ns[1],
                        stats.fn_body_top_stmt_family_count[2],
                        stats.fn_body_top_stmt_family_ns[2],
                        stats.fn_body_top_stmt_family_count[3],
                        stats.fn_body_top_stmt_family_ns[3],
                        stats.fn_body_top_stmt_family_count[4],
                        stats.fn_body_top_stmt_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-decl] target={}:{}ns no_let={}:{}ns init={}:{}ns finish={}:{}ns",
                        stats.fn_body_top_var_decl_phase_count[0],
                        stats.fn_body_top_var_decl_phase_ns[0],
                        stats.fn_body_top_var_decl_phase_count[1],
                        stats.fn_body_top_var_decl_phase_ns[1],
                        stats.fn_body_top_var_decl_phase_count[2],
                        stats.fn_body_top_var_decl_phase_ns[2],
                        stats.fn_body_top_var_decl_phase_count[3],
                        stats.fn_body_top_var_decl_phase_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-fn-decl] name={}:{}ns params={}:{}ns dups={}:{}ns body={}:{}ns revalidate={}:{}ns finish={}:{}ns",
                        stats.fn_body_top_fn_decl_phase_count[0],
                        stats.fn_body_top_fn_decl_phase_ns[0],
                        stats.fn_body_top_fn_decl_phase_count[1],
                        stats.fn_body_top_fn_decl_phase_ns[1],
                        stats.fn_body_top_fn_decl_phase_count[2],
                        stats.fn_body_top_fn_decl_phase_ns[2],
                        stats.fn_body_top_fn_decl_phase_count[3],
                        stats.fn_body_top_fn_decl_phase_ns[3],
                        stats.fn_body_top_fn_decl_phase_count[4],
                        stats.fn_body_top_fn_decl_phase_ns[4],
                        stats.fn_body_top_fn_decl_phase_count[5],
                        stats.fn_body_top_fn_decl_phase_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-function-family] function={}:{}ns async_function={}:{}ns class={}:{}ns decorated_class={}:{}ns",
                        stats.fn_body_top_function_family_count[0],
                        stats.fn_body_top_function_family_ns[0],
                        stats.fn_body_top_function_family_count[1],
                        stats.fn_body_top_function_family_ns[1],
                        stats.fn_body_top_function_family_count[2],
                        stats.fn_body_top_function_family_ns[2],
                        stats.fn_body_top_function_family_count[3],
                        stats.fn_body_top_function_family_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-decl] name={}:{}ns super={}:{}ns body={}:{}ns finish={}:{}ns",
                        stats.fn_body_top_class_decl_phase_count[0],
                        stats.fn_body_top_class_decl_phase_ns[0],
                        stats.fn_body_top_class_decl_phase_count[1],
                        stats.fn_body_top_class_decl_phase_ns[1],
                        stats.fn_body_top_class_decl_phase_count[2],
                        stats.fn_body_top_class_decl_phase_ns[2],
                        stats.fn_body_top_class_decl_phase_count[3],
                        stats.fn_body_top_class_decl_phase_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-family] fn_class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.fn_body_top_var_init_family_count[0],
                        stats.fn_body_top_var_init_family_ns[0],
                        stats.fn_body_top_var_init_family_count[1],
                        stats.fn_body_top_var_init_family_ns[1],
                        stats.fn_body_top_var_init_family_count[2],
                        stats.fn_body_top_var_init_family_ns[2],
                        stats.fn_body_top_var_init_family_count[3],
                        stats.fn_body_top_var_init_family_ns[3],
                        stats.fn_body_top_var_init_family_count[4],
                        stats.fn_body_top_var_init_family_ns[4],
                        stats.fn_body_top_var_init_family_count[5],
                        stats.fn_body_top_var_init_family_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-expr-family] fn_class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.fn_body_top_var_init_expr_family_count[0],
                        stats.fn_body_top_var_init_expr_family_ns[0],
                        stats.fn_body_top_var_init_expr_family_count[1],
                        stats.fn_body_top_var_init_expr_family_ns[1],
                        stats.fn_body_top_var_init_expr_family_count[2],
                        stats.fn_body_top_var_init_expr_family_ns[2],
                        stats.fn_body_top_var_init_expr_family_count[3],
                        stats.fn_body_top_var_init_expr_family_ns[3],
                        stats.fn_body_top_var_init_expr_family_count[4],
                        stats.fn_body_top_var_init_expr_family_ns[4],
                        stats.fn_body_top_var_init_expr_family_count[5],
                        stats.fn_body_top_var_init_expr_family_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-member] static_block={}:{}ns accessor={}:{}ns method={}:{}ns field={}:{}ns",
                        stats.fn_body_top_class_member_family_count[0],
                        stats.fn_body_top_class_member_family_ns[0],
                        stats.fn_body_top_class_member_family_count[1],
                        stats.fn_body_top_class_member_family_ns[1],
                        stats.fn_body_top_class_member_family_count[2],
                        stats.fn_body_top_class_member_family_ns[2],
                        stats.fn_body_top_class_member_family_count[3],
                        stats.fn_body_top_class_member_family_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method] params={}:{}ns body={}:{}ns kind_validate={}:{}ns strict_revalidate={}:{}ns push={}:{}ns",
                        stats.fn_body_top_class_method_phase_count[0],
                        stats.fn_body_top_class_method_phase_ns[0],
                        stats.fn_body_top_class_method_phase_count[1],
                        stats.fn_body_top_class_method_phase_ns[1],
                        stats.fn_body_top_class_method_phase_count[2],
                        stats.fn_body_top_class_method_phase_ns[2],
                        stats.fn_body_top_class_method_phase_count[3],
                        stats.fn_body_top_class_method_phase_ns[3],
                        stats.fn_body_top_class_method_phase_count[4],
                        stats.fn_body_top_class_method_phase_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-stmt] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns",
                        stats.fn_body_top_class_method_body_stmt_family_count[0],
                        stats.fn_body_top_class_method_body_stmt_family_ns[0],
                        stats.fn_body_top_class_method_body_stmt_family_count[1],
                        stats.fn_body_top_class_method_body_stmt_family_ns[1],
                        stats.fn_body_top_class_method_body_stmt_family_count[2],
                        stats.fn_body_top_class_method_body_stmt_family_ns[2],
                        stats.fn_body_top_class_method_body_stmt_family_count[3],
                        stats.fn_body_top_class_method_body_stmt_family_ns[3],
                        stats.fn_body_top_class_method_body_stmt_family_count[4],
                        stats.fn_body_top_class_method_body_stmt_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-control] if={}:{}ns for={}:{}ns loop={}:{}ns switch_try_with={}:{}ns return_throw={}:{}ns break_continue={}:{}ns",
                        stats.fn_body_top_class_method_body_control_family_count[0],
                        stats.fn_body_top_class_method_body_control_family_ns[0],
                        stats.fn_body_top_class_method_body_control_family_count[1],
                        stats.fn_body_top_class_method_body_control_family_ns[1],
                        stats.fn_body_top_class_method_body_control_family_count[2],
                        stats.fn_body_top_class_method_body_control_family_ns[2],
                        stats.fn_body_top_class_method_body_control_family_count[3],
                        stats.fn_body_top_class_method_body_control_family_ns[3],
                        stats.fn_body_top_class_method_body_control_family_count[4],
                        stats.fn_body_top_class_method_body_control_family_ns[4],
                        stats.fn_body_top_class_method_body_control_family_count[5],
                        stats.fn_body_top_class_method_body_control_family_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-if-phase] test={}:{}ns consequent={}:{}ns alternate={}:{}ns close={}:{}ns",
                        stats.fn_body_top_class_method_body_if_phase_count[0],
                        stats.fn_body_top_class_method_body_if_phase_ns[0],
                        stats.fn_body_top_class_method_body_if_phase_count[1],
                        stats.fn_body_top_class_method_body_if_phase_ns[1],
                        stats.fn_body_top_class_method_body_if_phase_count[2],
                        stats.fn_body_top_class_method_body_if_phase_ns[2],
                        stats.fn_body_top_class_method_body_if_phase_count[3],
                        stats.fn_body_top_class_method_body_if_phase_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-if-substmt] consequent_var={}:{}ns consequent_function={}:{}ns consequent_expression={}:{}ns consequent_block={}:{}ns consequent_control={}:{}ns alternate_var={}:{}ns alternate_function={}:{}ns alternate_expression={}:{}ns alternate_block={}:{}ns alternate_control={}:{}ns",
                        stats.fn_body_top_class_method_body_if_consequent_family_count[0],
                        stats.fn_body_top_class_method_body_if_consequent_family_ns[0],
                        stats.fn_body_top_class_method_body_if_consequent_family_count[1],
                        stats.fn_body_top_class_method_body_if_consequent_family_ns[1],
                        stats.fn_body_top_class_method_body_if_consequent_family_count[2],
                        stats.fn_body_top_class_method_body_if_consequent_family_ns[2],
                        stats.fn_body_top_class_method_body_if_consequent_family_count[3],
                        stats.fn_body_top_class_method_body_if_consequent_family_ns[3],
                        stats.fn_body_top_class_method_body_if_consequent_family_count[4],
                        stats.fn_body_top_class_method_body_if_consequent_family_ns[4],
                        stats.fn_body_top_class_method_body_if_alternate_family_count[0],
                        stats.fn_body_top_class_method_body_if_alternate_family_ns[0],
                        stats.fn_body_top_class_method_body_if_alternate_family_count[1],
                        stats.fn_body_top_class_method_body_if_alternate_family_ns[1],
                        stats.fn_body_top_class_method_body_if_alternate_family_count[2],
                        stats.fn_body_top_class_method_body_if_alternate_family_ns[2],
                        stats.fn_body_top_class_method_body_if_alternate_family_count[3],
                        stats.fn_body_top_class_method_body_if_alternate_family_ns[3],
                        stats.fn_body_top_class_method_body_if_alternate_family_count[4],
                        stats.fn_body_top_class_method_body_if_alternate_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-if-block-stmt] consequent_var={}:{}ns consequent_function={}:{}ns consequent_expression={}:{}ns consequent_block={}:{}ns consequent_control={}:{}ns alternate_var={}:{}ns alternate_function={}:{}ns alternate_expression={}:{}ns alternate_block={}:{}ns alternate_control={}:{}ns",
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_count[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_ns[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_count[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_ns[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_count[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_ns[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_count[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_ns[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_count[4],
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_ns[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_count[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_ns[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_count[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_ns[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_count[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_ns[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_count[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_ns[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_count[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-if-block-control] consequent_if={}:{}ns consequent_for={}:{}ns consequent_loop={}:{}ns consequent_switch_try_with={}:{}ns consequent_return_throw={}:{}ns consequent_break_continue={}:{}ns alternate_if={}:{}ns alternate_for={}:{}ns alternate_loop={}:{}ns alternate_switch_try_with={}:{}ns alternate_return_throw={}:{}ns alternate_break_continue={}:{}ns",
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count[4],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[4],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count[5],
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[5],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count[5],
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-if-block-if-phase] consequent_test={}:{}ns consequent_consequent={}:{}ns consequent_alternate={}:{}ns consequent_close={}:{}ns alternate_test={}:{}ns alternate_consequent={}:{}ns alternate_alternate={}:{}ns alternate_close={}:{}ns",
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_count[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_ns[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_count[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_ns[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_count[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_ns[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_count[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_ns[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_count[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_ns[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_count[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_ns[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_count[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_ns[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_count[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-class-method-body-if-block-if-substmt] consequent_consequent_var={}:{}ns consequent_consequent_function={}:{}ns consequent_consequent_expression={}:{}ns consequent_consequent_block={}:{}ns consequent_consequent_control={}:{}ns consequent_alternate_var={}:{}ns consequent_alternate_function={}:{}ns consequent_alternate_expression={}:{}ns consequent_alternate_block={}:{}ns consequent_alternate_control={}:{}ns alternate_consequent_var={}:{}ns alternate_consequent_function={}:{}ns alternate_consequent_expression={}:{}ns alternate_consequent_block={}:{}ns alternate_consequent_control={}:{}ns alternate_alternate_var={}:{}ns alternate_alternate_function={}:{}ns alternate_alternate_expression={}:{}ns alternate_alternate_block={}:{}ns alternate_alternate_control={}:{}ns",
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count[4],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns[4],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns[0],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns[1],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns[2],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns[3],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count[4],
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns[0],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns[1],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns[2],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns[3],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count[4],
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-stmt] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns",
                        stats.nested_stmt_var_count,
                        stats.nested_stmt_var_ns,
                        stats.nested_stmt_function_count,
                        stats.nested_stmt_function_ns,
                        stats.nested_stmt_expression_count,
                        stats.nested_stmt_expression_ns,
                        stats.nested_stmt_block_count,
                        stats.nested_stmt_block_ns,
                        stats.nested_stmt_control_count,
                        stats.nested_stmt_control_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-control] if={}:{}ns for={}:{}ns loop={}:{}ns switch_try_with={}:{}ns return_throw={}:{}ns break_continue={}:{}ns",
                        stats.nested_control_if_count,
                        stats.nested_control_if_ns,
                        stats.nested_control_for_count,
                        stats.nested_control_for_ns,
                        stats.nested_control_loop_count,
                        stats.nested_control_loop_ns,
                        stats.nested_control_switch_try_with_count,
                        stats.nested_control_switch_try_with_ns,
                        stats.nested_control_return_throw_count,
                        stats.nested_control_return_throw_ns,
                        stats.nested_control_break_continue_count,
                        stats.nested_control_break_continue_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-if-phase] test={}:{}ns consequent={}:{}ns alternate={}:{}ns close={}:{}ns",
                        stats.nested_if_test_count,
                        stats.nested_if_test_ns,
                        stats.nested_if_consequent_count,
                        stats.nested_if_consequent_ns,
                        stats.nested_if_alternate_count,
                        stats.nested_if_alternate_ns,
                        stats.nested_if_close_count,
                        stats.nested_if_close_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-if-substmt] consequent_var={}:{}ns consequent_function={}:{}ns consequent_expression={}:{}ns consequent_block={}:{}ns consequent_control={}:{}ns alternate_var={}:{}ns alternate_function={}:{}ns alternate_expression={}:{}ns alternate_block={}:{}ns alternate_control={}:{}ns",
                        stats.nested_if_consequent_family_count[0],
                        stats.nested_if_consequent_family_ns[0],
                        stats.nested_if_consequent_family_count[1],
                        stats.nested_if_consequent_family_ns[1],
                        stats.nested_if_consequent_family_count[2],
                        stats.nested_if_consequent_family_ns[2],
                        stats.nested_if_consequent_family_count[3],
                        stats.nested_if_consequent_family_ns[3],
                        stats.nested_if_consequent_family_count[4],
                        stats.nested_if_consequent_family_ns[4],
                        stats.nested_if_alternate_family_count[0],
                        stats.nested_if_alternate_family_ns[0],
                        stats.nested_if_alternate_family_count[1],
                        stats.nested_if_alternate_family_ns[1],
                        stats.nested_if_alternate_family_count[2],
                        stats.nested_if_alternate_family_ns[2],
                        stats.nested_if_alternate_family_count[3],
                        stats.nested_if_alternate_family_ns[3],
                        stats.nested_if_alternate_family_count[4],
                        stats.nested_if_alternate_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-stmt] consequent_var={}:{}ns consequent_function={}:{}ns consequent_expression={}:{}ns consequent_block={}:{}ns consequent_control={}:{}ns alternate_var={}:{}ns alternate_function={}:{}ns alternate_expression={}:{}ns alternate_block={}:{}ns alternate_control={}:{}ns",
                        stats.nested_if_consequent_block_stmt_count[0],
                        stats.nested_if_consequent_block_stmt_ns[0],
                        stats.nested_if_consequent_block_stmt_count[1],
                        stats.nested_if_consequent_block_stmt_ns[1],
                        stats.nested_if_consequent_block_stmt_count[2],
                        stats.nested_if_consequent_block_stmt_ns[2],
                        stats.nested_if_consequent_block_stmt_count[3],
                        stats.nested_if_consequent_block_stmt_ns[3],
                        stats.nested_if_consequent_block_stmt_count[4],
                        stats.nested_if_consequent_block_stmt_ns[4],
                        stats.nested_if_alternate_block_stmt_count[0],
                        stats.nested_if_alternate_block_stmt_ns[0],
                        stats.nested_if_alternate_block_stmt_count[1],
                        stats.nested_if_alternate_block_stmt_ns[1],
                        stats.nested_if_alternate_block_stmt_count[2],
                        stats.nested_if_alternate_block_stmt_ns[2],
                        stats.nested_if_alternate_block_stmt_count[3],
                        stats.nested_if_alternate_block_stmt_ns[3],
                        stats.nested_if_alternate_block_stmt_count[4],
                        stats.nested_if_alternate_block_stmt_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-control] consequent_if={}:{}ns consequent_for={}:{}ns consequent_loop={}:{}ns consequent_switch_try_with={}:{}ns consequent_return_throw={}:{}ns consequent_break_continue={}:{}ns alternate_if={}:{}ns alternate_for={}:{}ns alternate_loop={}:{}ns alternate_switch_try_with={}:{}ns alternate_return_throw={}:{}ns alternate_break_continue={}:{}ns",
                        stats.nested_if_consequent_block_control_count[0],
                        stats.nested_if_consequent_block_control_ns[0],
                        stats.nested_if_consequent_block_control_count[1],
                        stats.nested_if_consequent_block_control_ns[1],
                        stats.nested_if_consequent_block_control_count[2],
                        stats.nested_if_consequent_block_control_ns[2],
                        stats.nested_if_consequent_block_control_count[3],
                        stats.nested_if_consequent_block_control_ns[3],
                        stats.nested_if_consequent_block_control_count[4],
                        stats.nested_if_consequent_block_control_ns[4],
                        stats.nested_if_consequent_block_control_count[5],
                        stats.nested_if_consequent_block_control_ns[5],
                        stats.nested_if_alternate_block_control_count[0],
                        stats.nested_if_alternate_block_control_ns[0],
                        stats.nested_if_alternate_block_control_count[1],
                        stats.nested_if_alternate_block_control_ns[1],
                        stats.nested_if_alternate_block_control_count[2],
                        stats.nested_if_alternate_block_control_ns[2],
                        stats.nested_if_alternate_block_control_count[3],
                        stats.nested_if_alternate_block_control_ns[3],
                        stats.nested_if_alternate_block_control_count[4],
                        stats.nested_if_alternate_block_control_ns[4],
                        stats.nested_if_alternate_block_control_count[5],
                        stats.nested_if_alternate_block_control_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-if-phase] consequent_test={}:{}ns consequent_consequent={}:{}ns consequent_alternate={}:{}ns consequent_close={}:{}ns alternate_test={}:{}ns alternate_consequent={}:{}ns alternate_alternate={}:{}ns alternate_close={}:{}ns",
                        stats.nested_if_consequent_block_if_phase_count[0],
                        stats.nested_if_consequent_block_if_phase_ns[0],
                        stats.nested_if_consequent_block_if_phase_count[1],
                        stats.nested_if_consequent_block_if_phase_ns[1],
                        stats.nested_if_consequent_block_if_phase_count[2],
                        stats.nested_if_consequent_block_if_phase_ns[2],
                        stats.nested_if_consequent_block_if_phase_count[3],
                        stats.nested_if_consequent_block_if_phase_ns[3],
                        stats.nested_if_alternate_block_if_phase_count[0],
                        stats.nested_if_alternate_block_if_phase_ns[0],
                        stats.nested_if_alternate_block_if_phase_count[1],
                        stats.nested_if_alternate_block_if_phase_ns[1],
                        stats.nested_if_alternate_block_if_phase_count[2],
                        stats.nested_if_alternate_block_if_phase_ns[2],
                        stats.nested_if_alternate_block_if_phase_count[3],
                        stats.nested_if_alternate_block_if_phase_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-if-branch] consequent_consequent_var={}:{}ns consequent_consequent_function={}:{}ns consequent_consequent_expression={}:{}ns consequent_consequent_block={}:{}ns consequent_consequent_control={}:{}ns consequent_alternate_var={}:{}ns consequent_alternate_function={}:{}ns consequent_alternate_expression={}:{}ns consequent_alternate_block={}:{}ns consequent_alternate_control={}:{}ns alternate_consequent_var={}:{}ns alternate_consequent_function={}:{}ns alternate_consequent_expression={}:{}ns alternate_consequent_block={}:{}ns alternate_consequent_control={}:{}ns alternate_alternate_var={}:{}ns alternate_alternate_function={}:{}ns alternate_alternate_expression={}:{}ns alternate_alternate_block={}:{}ns alternate_alternate_control={}:{}ns",
                        stats.nested_if_consequent_block_if_consequent_family_count[0],
                        stats.nested_if_consequent_block_if_consequent_family_ns[0],
                        stats.nested_if_consequent_block_if_consequent_family_count[1],
                        stats.nested_if_consequent_block_if_consequent_family_ns[1],
                        stats.nested_if_consequent_block_if_consequent_family_count[2],
                        stats.nested_if_consequent_block_if_consequent_family_ns[2],
                        stats.nested_if_consequent_block_if_consequent_family_count[3],
                        stats.nested_if_consequent_block_if_consequent_family_ns[3],
                        stats.nested_if_consequent_block_if_consequent_family_count[4],
                        stats.nested_if_consequent_block_if_consequent_family_ns[4],
                        stats.nested_if_consequent_block_if_alternate_family_count[0],
                        stats.nested_if_consequent_block_if_alternate_family_ns[0],
                        stats.nested_if_consequent_block_if_alternate_family_count[1],
                        stats.nested_if_consequent_block_if_alternate_family_ns[1],
                        stats.nested_if_consequent_block_if_alternate_family_count[2],
                        stats.nested_if_consequent_block_if_alternate_family_ns[2],
                        stats.nested_if_consequent_block_if_alternate_family_count[3],
                        stats.nested_if_consequent_block_if_alternate_family_ns[3],
                        stats.nested_if_consequent_block_if_alternate_family_count[4],
                        stats.nested_if_consequent_block_if_alternate_family_ns[4],
                        stats.nested_if_alternate_block_if_consequent_family_count[0],
                        stats.nested_if_alternate_block_if_consequent_family_ns[0],
                        stats.nested_if_alternate_block_if_consequent_family_count[1],
                        stats.nested_if_alternate_block_if_consequent_family_ns[1],
                        stats.nested_if_alternate_block_if_consequent_family_count[2],
                        stats.nested_if_alternate_block_if_consequent_family_ns[2],
                        stats.nested_if_alternate_block_if_consequent_family_count[3],
                        stats.nested_if_alternate_block_if_consequent_family_ns[3],
                        stats.nested_if_alternate_block_if_consequent_family_count[4],
                        stats.nested_if_alternate_block_if_consequent_family_ns[4],
                        stats.nested_if_alternate_block_if_alternate_family_count[0],
                        stats.nested_if_alternate_block_if_alternate_family_ns[0],
                        stats.nested_if_alternate_block_if_alternate_family_count[1],
                        stats.nested_if_alternate_block_if_alternate_family_ns[1],
                        stats.nested_if_alternate_block_if_alternate_family_count[2],
                        stats.nested_if_alternate_block_if_alternate_family_ns[2],
                        stats.nested_if_alternate_block_if_alternate_family_count[3],
                        stats.nested_if_alternate_block_if_alternate_family_ns[3],
                        stats.nested_if_alternate_block_if_alternate_family_count[4],
                        stats.nested_if_alternate_block_if_alternate_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-if-consequent-block-stmt] consequent_var={}:{}ns consequent_function={}:{}ns consequent_expression={}:{}ns consequent_block={}:{}ns consequent_control={}:{}ns alternate_var={}:{}ns alternate_function={}:{}ns alternate_expression={}:{}ns alternate_block={}:{}ns alternate_control={}:{}ns",
                        stats.nested_if_consequent_block_if_consequent_block_stmt_count[0],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_ns[0],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_count[1],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_ns[1],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_count[2],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_ns[2],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_count[3],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_ns[3],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_count[4],
                        stats.nested_if_consequent_block_if_consequent_block_stmt_ns[4],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_count[0],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_ns[0],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_count[1],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_ns[1],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_count[2],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_ns[2],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_count[3],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_ns[3],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_count[4],
                        stats.nested_if_alternate_block_if_consequent_block_stmt_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-if-consequent-block-control] consequent_if={}:{}ns consequent_for={}:{}ns consequent_loop={}:{}ns consequent_switch_try_with={}:{}ns consequent_return_throw={}:{}ns consequent_break_continue={}:{}ns alternate_if={}:{}ns alternate_for={}:{}ns alternate_loop={}:{}ns alternate_switch_try_with={}:{}ns alternate_return_throw={}:{}ns alternate_break_continue={}:{}ns",
                        stats.nested_if_consequent_block_if_consequent_block_control_count[0],
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[0],
                        stats.nested_if_consequent_block_if_consequent_block_control_count[1],
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[1],
                        stats.nested_if_consequent_block_if_consequent_block_control_count[2],
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[2],
                        stats.nested_if_consequent_block_if_consequent_block_control_count[3],
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[3],
                        stats.nested_if_consequent_block_if_consequent_block_control_count[4],
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[4],
                        stats.nested_if_consequent_block_if_consequent_block_control_count[5],
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[5],
                        stats.nested_if_alternate_block_if_consequent_block_control_count[0],
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[0],
                        stats.nested_if_alternate_block_if_consequent_block_control_count[1],
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[1],
                        stats.nested_if_alternate_block_if_consequent_block_control_count[2],
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[2],
                        stats.nested_if_alternate_block_if_consequent_block_control_count[3],
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[3],
                        stats.nested_if_alternate_block_if_consequent_block_control_count[4],
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[4],
                        stats.nested_if_alternate_block_if_consequent_block_control_count[5],
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-nested-if-block-if-consequent-block-if-phase] consequent_test={}:{}ns consequent_consequent={}:{}ns consequent_alternate={}:{}ns consequent_close={}:{}ns alternate_test={}:{}ns alternate_consequent={}:{}ns alternate_alternate={}:{}ns alternate_close={}:{}ns",
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_count[0],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_ns[0],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_count[1],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_ns[1],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_count[2],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_ns[2],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_count[3],
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_ns[3],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_count[0],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_ns[0],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_count[1],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_ns[1],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_count[2],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_ns[2],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_count[3],
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-nested-stmt-depth2] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns",
                        stats.nested_stmt_depth2_count[0],
                        stats.nested_stmt_depth2_ns[0],
                        stats.nested_stmt_depth2_count[1],
                        stats.nested_stmt_depth2_ns[1],
                        stats.nested_stmt_depth2_count[2],
                        stats.nested_stmt_depth2_ns[2],
                        stats.nested_stmt_depth2_count[3],
                        stats.nested_stmt_depth2_ns[3],
                        stats.nested_stmt_depth2_count[4],
                        stats.nested_stmt_depth2_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-stmt-depth3] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns",
                        stats.nested_stmt_depth3_count[0],
                        stats.nested_stmt_depth3_ns[0],
                        stats.nested_stmt_depth3_count[1],
                        stats.nested_stmt_depth3_ns[1],
                        stats.nested_stmt_depth3_count[2],
                        stats.nested_stmt_depth3_ns[2],
                        stats.nested_stmt_depth3_count[3],
                        stats.nested_stmt_depth3_ns[3],
                        stats.nested_stmt_depth3_count[4],
                        stats.nested_stmt_depth3_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-nested-var-decl] target={}:{}ns no_let={}:{}ns init={}:{}ns finish={}:{}ns",
                        stats.nested_var_target_count,
                        stats.nested_var_target_ns,
                        stats.nested_var_no_let_count,
                        stats.nested_var_no_let_ns,
                        stats.nested_var_init_count,
                        stats.nested_var_init_ns,
                        stats.nested_var_finish_count,
                        stats.nested_var_finish_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-var-decl-depth3] target={}:{}ns no_let={}:{}ns init={}:{}ns finish={}:{}ns",
                        stats.nested_var_depth3_count[0],
                        stats.nested_var_depth3_ns[0],
                        stats.nested_var_depth3_count[1],
                        stats.nested_var_depth3_ns[1],
                        stats.nested_var_depth3_count[2],
                        stats.nested_var_depth3_ns[2],
                        stats.nested_var_depth3_count[3],
                        stats.nested_var_depth3_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-nested-fn-decl] name={}:{}ns params={}:{}ns dups={}:{}ns body={}:{}ns revalidate={}:{}ns finish={}:{}ns",
                        stats.nested_fn_decl_name_count,
                        stats.nested_fn_decl_name_ns,
                        stats.nested_fn_decl_params_count,
                        stats.nested_fn_decl_params_ns,
                        stats.nested_fn_decl_dups_count,
                        stats.nested_fn_decl_dups_ns,
                        stats.nested_fn_decl_body_count,
                        stats.nested_fn_decl_body_ns,
                        stats.nested_fn_decl_revalidate_count,
                        stats.nested_fn_decl_revalidate_ns,
                        stats.nested_fn_decl_finish_count,
                        stats.nested_fn_decl_finish_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-fn-decl-depth2] name={}:{}ns params={}:{}ns dups={}:{}ns body={}:{}ns revalidate={}:{}ns finish={}:{}ns",
                        stats.nested_fn_decl_depth2_count[0],
                        stats.nested_fn_decl_depth2_ns[0],
                        stats.nested_fn_decl_depth2_count[1],
                        stats.nested_fn_decl_depth2_ns[1],
                        stats.nested_fn_decl_depth2_count[2],
                        stats.nested_fn_decl_depth2_ns[2],
                        stats.nested_fn_decl_depth2_count[3],
                        stats.nested_fn_decl_depth2_ns[3],
                        stats.nested_fn_decl_depth2_count[4],
                        stats.nested_fn_decl_depth2_ns[4],
                        stats.nested_fn_decl_depth2_count[5],
                        stats.nested_fn_decl_depth2_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-nested-fn-body-phase] loop={}:{}ns bound_names={}:{}ns close={}:{}ns restore={}:{}ns desugar={}:{}ns",
                        stats.nested_fn_body_loop_count,
                        stats.nested_fn_body_loop_ns,
                        stats.nested_fn_body_bound_names_count,
                        stats.nested_fn_body_bound_names_ns,
                        stats.nested_fn_body_close_count,
                        stats.nested_fn_body_close_ns,
                        stats.nested_fn_body_restore_count,
                        stats.nested_fn_body_restore_ns,
                        stats.nested_fn_body_desugar_count,
                        stats.nested_fn_body_desugar_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-fn-body-loop-depth] depth2={}:{}ns depth3={}:{}ns depth4_plus={}:{}ns",
                        stats.nested_fn_body_loop_depth2_count,
                        stats.nested_fn_body_loop_depth2_ns,
                        stats.nested_fn_body_loop_depth3_count,
                        stats.nested_fn_body_loop_depth3_ns,
                        stats.nested_fn_body_loop_depth4_plus_count,
                        stats.nested_fn_body_loop_depth4_plus_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-assign] async_disambig={}:{}ns arrow_probe={}:{}ns conditional={}:{}ns rhs={}:{}ns cover={}:{}ns",
                        stats.nested_assign_async_count,
                        stats.nested_assign_async_ns,
                        stats.nested_assign_arrow_count,
                        stats.nested_assign_arrow_ns,
                        stats.nested_assign_conditional_count,
                        stats.nested_assign_conditional_ns,
                        stats.nested_assign_rhs_count,
                        stats.nested_assign_rhs_ns,
                        stats.nested_assign_cover_count,
                        stats.nested_assign_cover_ns
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-assign] async_disambig={}:{}ns arrow_probe={}:{}ns arrow_parse={}:{}ns conditional={}:{}ns rhs={}:{}ns cover={}:{}ns",
                        stats.top_var_init_ident_assign_count[0],
                        stats.top_var_init_ident_assign_ns[0],
                        stats.top_var_init_ident_assign_count[1],
                        stats.top_var_init_ident_assign_ns[1],
                        stats.top_var_init_ident_assign_count[2],
                        stats.top_var_init_ident_assign_ns[2],
                        stats.top_var_init_ident_assign_count[3],
                        stats.top_var_init_ident_assign_ns[3],
                        stats.top_var_init_ident_assign_count[4],
                        stats.top_var_init_ident_assign_ns[4],
                        stats.top_var_init_ident_assign_count[5],
                        stats.top_var_init_ident_assign_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow] head={}:{}ns arrow_token={}:{}ns validate={}:{}ns block_body={}:{}ns expr_body={}:{}ns",
                        stats.top_var_init_ident_arrow_count[0],
                        stats.top_var_init_ident_arrow_ns[0],
                        stats.top_var_init_ident_arrow_count[1],
                        stats.top_var_init_ident_arrow_ns[1],
                        stats.top_var_init_ident_arrow_count[2],
                        stats.top_var_init_ident_arrow_ns[2],
                        stats.top_var_init_ident_arrow_count[3],
                        stats.top_var_init_ident_arrow_ns[3],
                        stats.top_var_init_ident_arrow_count[4],
                        stats.top_var_init_ident_arrow_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-assign] async_disambig={}:{}ns arrow_probe={}:{}ns conditional={}:{}ns rhs={}:{}ns cover={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_assign_count[0],
                        stats.top_var_init_ident_arrow_expr_assign_ns[0],
                        stats.top_var_init_ident_arrow_expr_assign_count[1],
                        stats.top_var_init_ident_arrow_expr_assign_ns[1],
                        stats.top_var_init_ident_arrow_expr_assign_count[2],
                        stats.top_var_init_ident_arrow_expr_assign_ns[2],
                        stats.top_var_init_ident_arrow_expr_assign_count[3],
                        stats.top_var_init_ident_arrow_expr_assign_ns[3],
                        stats.top_var_init_ident_arrow_expr_assign_count[4],
                        stats.top_var_init_ident_arrow_expr_assign_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond] test={}:{}ns question={}:{}ns consequent={}:{}ns alternate={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-binary] left_unary={}:{}ns private_check={}:{}ns op_scan={}:{}ns rhs={}:{}ns assemble={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_binary_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_binary_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_binary_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_binary_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_binary_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_binary_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_binary_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_binary_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_binary_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_binary_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-unary] prefix_unary={}:{}ns prefix_update={}:{}ns await_expr={}:{}ns reserved={}:{}ns postfix={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_unary_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_unary_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_unary_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_unary_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_unary_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_unary_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_unary_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_unary_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_unary_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_unary_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-postfix] lhs={}:{}ns update_check={}:{}ns update_emit={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_postfix_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_postfix_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_postfix_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_postfix_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_postfix_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_postfix_ns[2]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-lhs] new_base={}:{}ns primary_base={}:{}ns continuation={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_lhs_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_lhs_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_lhs_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_lhs_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_lhs_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_lhs_ns[2]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-primary] fn_class={}:{}ns paren_template={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_primary_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_primary_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_primary_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_primary_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_primary_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_primary_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_primary_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_primary_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_primary_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_primary_ns[4],
                        stats.top_var_init_ident_arrow_expr_cond_primary_count[5],
                        stats.top_var_init_ident_arrow_expr_cond_primary_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-fnclass] fn_head={}:{}ns fn_params={}:{}ns fn_body={}:{}ns fn_revalidate={}:{}ns class_head={}:{}ns class_super={}:{}ns class_body={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[4],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[5],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[5],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_count[6],
                        stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[6]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-classbody] setup={}:{}ns member_loop={}:{}ns close={}:{}ns validate={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_classbody_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_classbody_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-class-member] static_block={}:{}ns accessor={}:{}ns method={}:{}ns field={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_class_member_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_member_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-class-method] params={}:{}ns body={}:{}ns kind_validate={}:{}ns strict_revalidate={}:{}ns push={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_class_method_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-class-method-body-stmt] var={}:{}ns function={}:{}ns expression={}:{}ns block={}:{}ns control={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-class-method-body-control] if={}:{}ns for={}:{}ns loop={}:{}ns switch_try_with={}:{}ns return_throw={}:{}ns break_continue={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[5],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-class-method-body-if] test={}:{}ns consequent={}:{}ns alternate={}:{}ns close={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_ns[3]
                    );
                    eprintln!(
                        "[parser-profile-fn-body-top-var-init-ident-arrow-expr-cond-class-method-body-if-substmt] consequent_var={}:{}ns consequent_function={}:{}ns consequent_expression={}:{}ns consequent_block={}:{}ns consequent_control={}:{}ns alternate_var={}:{}ns alternate_function={}:{}ns alternate_expression={}:{}ns alternate_block={}:{}ns alternate_control={}:{}ns",
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns[0],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns[1],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns[2],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns[3],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count[4],
                        stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns[4]
                    );
                    eprintln!(
                        "[parser-profile-exclusive] nested_var_stmt={}ns nested_fn_stmt={}ns var_init={}ns fn_decl_body={}ns assign_conditional={}ns",
                        stats.nested_stmt_var_exclusive_ns,
                        stats.nested_stmt_function_exclusive_ns,
                        stats.nested_var_init_exclusive_ns,
                        stats.nested_fn_decl_body_exclusive_ns,
                        stats.nested_assign_conditional_exclusive_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-phase] assign={}:{}ns expr={}:{}ns",
                        stats.nested_var_init_assign_count,
                        stats.nested_var_init_assign_ns,
                        stats.nested_var_init_expr_count,
                        stats.nested_var_init_expr_ns
                    );
                    eprintln!(
                        "[parser-profile-bump-lexer] fetch={}:{}ns goal={}:{}ns trivia={}:{}ns punct={}:{}ns",
                        stats.bump_fetch_count,
                        stats.bump_fetch_ns,
                        stats.bump_goal_count,
                        stats.bump_goal_ns,
                        stats.lexer_trivia_count,
                        stats.lexer_trivia_ns,
                        stats.lexer_punct_count,
                        stats.lexer_punct_ns
                    );
                    eprintln!(
                        "[parser-profile-lexer-token-family] ident={}:{}ns private_ident={}:{}ns numeric={}:{}ns string={}:{}ns template={}:{}ns regex={}:{}ns hashbang={}:{}ns eof={}:{}ns",
                        stats.lexer_ident_count,
                        stats.lexer_ident_ns,
                        stats.lexer_private_ident_count,
                        stats.lexer_private_ident_ns,
                        stats.lexer_numeric_count,
                        stats.lexer_numeric_ns,
                        stats.lexer_string_count,
                        stats.lexer_string_ns,
                        stats.lexer_template_count,
                        stats.lexer_template_ns,
                        stats.lexer_regex_count,
                        stats.lexer_regex_ns,
                        stats.lexer_hashbang_count,
                        stats.lexer_hashbang_ns,
                        stats.lexer_eof_count,
                        stats.lexer_eof_ns
                    );
                    eprintln!(
                        "[parser-profile-lexer-string-phase] scan={}:{}ns convert={}:{}ns token={}:{}ns escape={}:{}ns no_escape={}:{}ns",
                        stats.lexer_string_scan_count,
                        stats.lexer_string_scan_ns,
                        stats.lexer_string_convert_count,
                        stats.lexer_string_convert_ns,
                        stats.lexer_string_token_count,
                        stats.lexer_string_token_ns,
                        stats.lexer_string_escape_count,
                        stats.lexer_string_escape_ns,
                        stats.lexer_string_no_escape_count,
                        stats.lexer_string_no_escape_ns
                    );
                    eprintln!(
                        "[parser-profile-lexer-string-no-escape] ascii={}:{}ns non_ascii={}:{}ns decode={}:{}ns marker={}:{}ns push={}:{}ns advance={}:{}ns",
                        stats.lexer_string_no_escape_ascii_count,
                        stats.lexer_string_no_escape_ascii_ns,
                        stats.lexer_string_no_escape_non_ascii_count,
                        stats.lexer_string_no_escape_non_ascii_ns,
                        stats.lexer_string_no_escape_decode_count,
                        stats.lexer_string_no_escape_decode_ns,
                        stats.lexer_string_no_escape_marker_count,
                        stats.lexer_string_no_escape_marker_ns,
                        stats.lexer_string_no_escape_push_count,
                        stats.lexer_string_no_escape_push_ns,
                        stats.lexer_string_no_escape_advance_count,
                        stats.lexer_string_no_escape_advance_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-family] fn_class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.nested_var_init_fn_class_count,
                        stats.nested_var_init_fn_class_ns,
                        stats.nested_var_init_paren_count,
                        stats.nested_var_init_paren_ns,
                        stats.nested_var_init_object_array_count,
                        stats.nested_var_init_object_array_ns,
                        stats.nested_var_init_ident_count,
                        stats.nested_var_init_ident_ns,
                        stats.nested_var_init_literal_count,
                        stats.nested_var_init_literal_ns,
                        stats.nested_var_init_other_count,
                        stats.nested_var_init_other_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-direct] fn_class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.nested_var_init_direct_fn_class_count,
                        stats.nested_var_init_direct_fn_class_ns,
                        stats.nested_var_init_direct_paren_count,
                        stats.nested_var_init_direct_paren_ns,
                        stats.nested_var_init_direct_object_array_count,
                        stats.nested_var_init_direct_object_array_ns,
                        stats.nested_var_init_direct_ident_count,
                        stats.nested_var_init_direct_ident_ns,
                        stats.nested_var_init_direct_literal_count,
                        stats.nested_var_init_direct_literal_ns,
                        stats.nested_var_init_direct_other_count,
                        stats.nested_var_init_direct_other_ns
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-depth3] fn_class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.nested_var_init_depth3_count[0],
                        stats.nested_var_init_depth3_ns[0],
                        stats.nested_var_init_depth3_count[1],
                        stats.nested_var_init_depth3_ns[1],
                        stats.nested_var_init_depth3_count[2],
                        stats.nested_var_init_depth3_ns[2],
                        stats.nested_var_init_depth3_count[3],
                        stats.nested_var_init_depth3_ns[3],
                        stats.nested_var_init_depth3_count[4],
                        stats.nested_var_init_depth3_ns[4],
                        stats.nested_var_init_depth3_count[5],
                        stats.nested_var_init_depth3_ns[5]
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-depth3-object-array] object={}:{}ns array={}:{}ns",
                        stats.nested_var_init_depth3_object_array_count[0],
                        stats.nested_var_init_depth3_object_array_ns[0],
                        stats.nested_var_init_depth3_object_array_count[1],
                        stats.nested_var_init_depth3_object_array_ns[1]
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-depth3-object-prop] spread={}:{}ns generator_method={}:{}ns async_method={}:{}ns accessor={}:{}ns colon={}:{}ns method={}:{}ns shorthand={}:{}ns",
                        stats.nested_var_init_depth3_object_prop_count[0],
                        stats.nested_var_init_depth3_object_prop_ns[0],
                        stats.nested_var_init_depth3_object_prop_count[1],
                        stats.nested_var_init_depth3_object_prop_ns[1],
                        stats.nested_var_init_depth3_object_prop_count[2],
                        stats.nested_var_init_depth3_object_prop_ns[2],
                        stats.nested_var_init_depth3_object_prop_count[3],
                        stats.nested_var_init_depth3_object_prop_ns[3],
                        stats.nested_var_init_depth3_object_prop_count[4],
                        stats.nested_var_init_depth3_object_prop_ns[4],
                        stats.nested_var_init_depth3_object_prop_count[5],
                        stats.nested_var_init_depth3_object_prop_ns[5],
                        stats.nested_var_init_depth3_object_prop_count[6],
                        stats.nested_var_init_depth3_object_prop_ns[6]
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-depth3-object-colon] key={}:{}ns value={}:{}ns comma={}:{}ns",
                        stats.nested_var_init_depth3_object_colon_count[0],
                        stats.nested_var_init_depth3_object_colon_ns[0],
                        stats.nested_var_init_depth3_object_colon_count[1],
                        stats.nested_var_init_depth3_object_colon_ns[1],
                        stats.nested_var_init_depth3_object_colon_count[2],
                        stats.nested_var_init_depth3_object_colon_ns[2]
                    );
                    eprintln!(
                        "[parser-profile-nested-var-init-depth3-object-colon-value] fn_class={}:{}ns paren={}:{}ns object_array={}:{}ns ident={}:{}ns literal={}:{}ns other={}:{}ns",
                        stats.nested_var_init_depth3_object_colon_value_count[0],
                        stats.nested_var_init_depth3_object_colon_value_ns[0],
                        stats.nested_var_init_depth3_object_colon_value_count[1],
                        stats.nested_var_init_depth3_object_colon_value_ns[1],
                        stats.nested_var_init_depth3_object_colon_value_count[2],
                        stats.nested_var_init_depth3_object_colon_value_ns[2],
                        stats.nested_var_init_depth3_object_colon_value_count[3],
                        stats.nested_var_init_depth3_object_colon_value_ns[3],
                        stats.nested_var_init_depth3_object_colon_value_count[4],
                        stats.nested_var_init_depth3_object_colon_value_ns[4],
                        stats.nested_var_init_depth3_object_colon_value_count[5],
                        stats.nested_var_init_depth3_object_colon_value_ns[5]
                    );
                });
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum Kind {
        Module,
        Statement,
        Expression,
        FunctionBody,
        ClassBody,
        StmtVar,
        StmtFunction,
        StmtExpression,
        StmtBlock,
        StmtControl,
        StmtLabel,
        ExprConditional,
        ExprBinary,
        ExprLhs,
        ExprPrimary,
        FnBodyLoop,
        FnBodyBoundNames,
        FnBodyDesugar,
        LhsBase,
        LhsCont,
        LhsMember,
        LhsComputed,
        LhsCall,
        LhsTemplate,
        PrimaryIdent,
        PrimaryLiteral,
        PrimaryFnClass,
        PrimaryFunction,
        PrimaryClass,
        PrimaryObjectArray,
        PrimaryParenTemplate,
        PrimaryParen,
        PrimaryTemplate,
        PrimaryOther,
        CallArgs,
        ParenInner,
        ParenConditional,
        ParenBinary,
        ParenLhs,
        ParenPrimary,
        FnExprParams,
        FnExprBody,
        FnExprRevalidate,
        ClassExprSuper,
        ClassExprBody,
        FnBodyLoopTop,
        FnBodyLoopNested,
        NestedStmtVar,
        NestedStmtFunction,
        NestedStmtExpression,
        NestedStmtBlock,
        NestedStmtControl,
        NestedControlIf,
        NestedControlFor,
        NestedControlLoop,
        NestedControlSwitchTryWith,
        NestedControlReturnThrow,
        NestedControlBreakContinue,
        NestedIfTest,
        NestedIfConsequent,
        NestedIfAlternate,
        NestedIfClose,
        NestedVarTarget,
        NestedVarNoLet,
        NestedVarInit,
        NestedVarInitAssign,
        NestedVarInitExpr,
        BumpFetch,
        BumpGoal,
        LexerTrivia,
        LexerPunct,
        LexerIdent,
        LexerPrivateIdent,
        LexerNumeric,
        LexerString,
        LexerTemplate,
        LexerRegex,
        LexerHashbang,
        LexerEof,
        LexerStringScan,
        LexerStringConvert,
        LexerStringToken,
        LexerStringEscape,
        LexerStringNoEscape,
        LexerStringNoEscapeAscii,
        LexerStringNoEscapeNonAscii,
        LexerStringNoEscapeDecode,
        LexerStringNoEscapeMarker,
        LexerStringNoEscapePush,
        LexerStringNoEscapeAdvance,
        NestedVarInitFnClass,
        NestedVarInitParen,
        NestedVarInitObjectArray,
        NestedVarInitIdent,
        NestedVarInitLiteral,
        NestedVarInitOther,
        NestedVarFinish,
        NestedFnDeclName,
        NestedFnDeclParams,
        NestedFnDeclDups,
        NestedFnDeclBody,
        NestedFnDeclRevalidate,
        NestedFnDeclFinish,
        NestedFnBodyLoop,
        NestedFnBodyLoopDepth2,
        NestedFnBodyLoopDepth3,
        NestedFnBodyLoopDepth4Plus,
        NestedFnBodyBoundNames,
        NestedFnBodyClose,
        NestedFnBodyRestore,
        NestedFnBodyDesugar,
        NestedAssignAsync,
        NestedAssignArrow,
        NestedAssignConditional,
        NestedAssignRhs,
        NestedAssignCover,
    }

    pub(crate) struct ParenDepthGuard {
        active: bool,
    }

    impl ParenDepthGuard {
        pub(crate) fn new() -> Self {
            if !enabled() {
                return Self { active: false };
            }
            PAREN_DEPTH.with(|depth| depth.set(depth.get() + 1));
            Self { active: true }
        }
    }

    impl Drop for ParenDepthGuard {
        fn drop(&mut self) {
            if self.active {
                PAREN_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
        }
    }

    pub(crate) fn paren_guard(kind: Kind) -> Option<Guard> {
        (enabled() && PAREN_DEPTH.with(|depth| depth.get() > 0)).then(|| Guard::new(kind))
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ParenInnerFamily {
        Function,
        Class,
        Paren,
        ObjectArray,
        Ident,
        Literal,
        Other,
    }

    impl ParenInnerFamily {
        fn index(self) -> usize {
            match self {
                ParenInnerFamily::Function => 0,
                ParenInnerFamily::Class => 1,
                ParenInnerFamily::Paren => 2,
                ParenInnerFamily::ObjectArray => 3,
                ParenInnerFamily::Ident => 4,
                ParenInnerFamily::Literal => 5,
                ParenInnerFamily::Other => 6,
            }
        }
    }

    pub(crate) struct ParenInnerFamilyGuard {
        family: ParenInnerFamily,
        start: Option<Instant>,
    }

    impl ParenInnerFamilyGuard {
        pub(crate) fn new(family: ParenInnerFamily) -> Self {
            Self {
                family,
                start: enabled().then(Instant::now),
            }
        }
    }

    impl Drop for ParenInnerFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.paren_inner_family_count[idx] += 1;
                stats.paren_inner_family_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct FunctionBodyDepthGuard {
        active: bool,
    }

    impl FunctionBodyDepthGuard {
        pub(crate) fn new() -> Self {
            if !enabled() {
                return Self { active: false };
            }
            FUNCTION_BODY_DEPTH.with(|depth| depth.set(depth.get() + 1));
            Self { active: true }
        }
    }

    impl Drop for FunctionBodyDepthGuard {
        fn drop(&mut self) {
            if self.active {
                FUNCTION_BODY_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
        }
    }

    pub(crate) fn function_body_depth() -> u32 {
        FUNCTION_BODY_DEPTH.with(Cell::get)
    }

    pub(crate) fn nested_statement_guard(kind: Kind) -> Option<Guard> {
        (function_body_depth() > 1).then(|| Guard::new(kind))
    }

    #[derive(Clone, Copy)]
    pub(crate) enum StatementFamily {
        Var,
        Function,
        Expression,
        Block,
        Control,
    }

    impl StatementFamily {
        fn index(self) -> usize {
            match self {
                StatementFamily::Var => 0,
                StatementFamily::Function => 1,
                StatementFamily::Expression => 2,
                StatementFamily::Block => 3,
                StatementFamily::Control => 4,
            }
        }
    }

    pub(crate) struct FunctionBodyTopStatementFamilyGuard {
        family: StatementFamily,
        start: Option<Instant>,
    }

    impl FunctionBodyTopStatementFamilyGuard {
        pub(crate) fn new(family: StatementFamily) -> Self {
            Self {
                family,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopStatementFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_stmt_family_count[idx] += 1;
                stats.fn_body_top_stmt_family_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct FunctionBodyTopVariableDeclPhaseGuard {
        phase: VariableDeclPhase,
        start: Option<Instant>,
    }

    impl FunctionBodyTopVariableDeclPhaseGuard {
        pub(crate) fn new(phase: VariableDeclPhase) -> Self {
            Self {
                phase,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopVariableDeclPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_var_decl_phase_count[idx] += 1;
                stats.fn_body_top_var_decl_phase_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct FunctionBodyTopFunctionDeclPhaseGuard {
        phase: FunctionDeclPhase,
        start: Option<Instant>,
    }

    impl FunctionBodyTopFunctionDeclPhaseGuard {
        pub(crate) fn new(phase: FunctionDeclPhase) -> Self {
            Self {
                phase,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopFunctionDeclPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_fn_decl_phase_count[idx] += 1;
                stats.fn_body_top_fn_decl_phase_ns[idx] += ns;
            });
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum FunctionFamilyKind {
        Function,
        AsyncFunction,
        Class,
        DecoratedClass,
    }

    impl FunctionFamilyKind {
        fn index(self) -> usize {
            match self {
                FunctionFamilyKind::Function => 0,
                FunctionFamilyKind::AsyncFunction => 1,
                FunctionFamilyKind::Class => 2,
                FunctionFamilyKind::DecoratedClass => 3,
            }
        }
    }

    pub(crate) struct FunctionBodyTopFunctionFamilyGuard {
        kind: FunctionFamilyKind,
        start: Option<Instant>,
    }

    impl FunctionBodyTopFunctionFamilyGuard {
        pub(crate) fn new(kind: FunctionFamilyKind) -> Self {
            Self {
                kind,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopFunctionFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.kind.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_function_family_count[idx] += 1;
                stats.fn_body_top_function_family_ns[idx] += ns;
            });
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ClassDeclPhase {
        Name,
        Super,
        Body,
        Finish,
    }

    impl ClassDeclPhase {
        fn index(self) -> usize {
            match self {
                ClassDeclPhase::Name => 0,
                ClassDeclPhase::Super => 1,
                ClassDeclPhase::Body => 2,
                ClassDeclPhase::Finish => 3,
            }
        }
    }

    pub(crate) struct FunctionBodyTopClassDeclPhaseGuard {
        phase: ClassDeclPhase,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassDeclPhaseGuard {
        pub(crate) fn new(phase: ClassDeclPhase) -> Self {
            Self {
                phase,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassDeclPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_class_decl_phase_count[idx] += 1;
                stats.fn_body_top_class_decl_phase_ns[idx] += ns;
            });
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum VarInitFamily {
        FnClass,
        Paren,
        ObjectArray,
        Ident,
        Literal,
        Other,
    }

    impl VarInitFamily {
        fn index(self) -> usize {
            match self {
                VarInitFamily::FnClass => 0,
                VarInitFamily::Paren => 1,
                VarInitFamily::ObjectArray => 2,
                VarInitFamily::Ident => 3,
                VarInitFamily::Literal => 4,
                VarInitFamily::Other => 5,
            }
        }
    }

    pub(crate) struct FunctionBodyTopVarInitFamilyGuard {
        family: VarInitFamily,
        start: Option<Instant>,
    }

    impl FunctionBodyTopVarInitFamilyGuard {
        pub(crate) fn new(family: VarInitFamily) -> Self {
            Self {
                family,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopVarInitFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_var_init_family_count[idx] += 1;
                stats.fn_body_top_var_init_family_ns[idx] += ns;
            });
        }
    }

    pub(crate) fn function_body_top_var_init_expr_start() -> Option<Instant> {
        (enabled() && function_body_depth() <= 1).then(Instant::now)
    }

    pub(crate) fn record_function_body_top_var_init_expr(
        family: VarInitFamily,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = family.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.fn_body_top_var_init_expr_family_count[idx] += 1;
            stats.fn_body_top_var_init_expr_family_ns[idx] += ns;
        });
    }

    pub(crate) struct TopVarInitIdentGuard {
        active: bool,
    }

    impl TopVarInitIdentGuard {
        pub(crate) fn new(active: bool) -> Option<Self> {
            if !enabled() || !active || function_body_depth() > 1 {
                return None;
            }
            TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.set(depth.get() + 1));
            Some(Self { active: true })
        }
    }

    pub(crate) struct TopVarInitIdentAssignFrameGuard {
        active: bool,
    }

    impl TopVarInitIdentAssignFrameGuard {
        pub(crate) fn new() -> Option<Self> {
            if !enabled() || !TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0) {
                return None;
            }
            TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.set(depth.get() + 1));
            Some(Self { active: true })
        }
    }

    impl Drop for TopVarInitIdentAssignFrameGuard {
        fn drop(&mut self) {
            if self.active {
                TOP_VAR_INIT_IDENT_ASSIGN_DEPTH
                    .with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
        }
    }

    impl Drop for TopVarInitIdentGuard {
        fn drop(&mut self) {
            if self.active {
                TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentAssignPhase {
        AsyncDisambig,
        ArrowProbe,
        ArrowParse,
        Conditional,
        Rhs,
        Cover,
    }

    impl TopVarInitIdentAssignPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentAssignPhase::AsyncDisambig => 0,
                TopVarInitIdentAssignPhase::ArrowProbe => 1,
                TopVarInitIdentAssignPhase::ArrowParse => 2,
                TopVarInitIdentAssignPhase::Conditional => 3,
                TopVarInitIdentAssignPhase::Rhs => 4,
                TopVarInitIdentAssignPhase::Cover => 5,
            }
        }
    }

    pub(crate) fn top_var_init_ident_assign_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 1))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_assign_phase(
        phase: TopVarInitIdentAssignPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_assign_count[idx] += 1;
            stats.top_var_init_ident_assign_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowPhase {
        Head,
        ArrowToken,
        Validate,
        BlockBody,
        ExprBody,
    }

    impl TopVarInitIdentArrowPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowPhase::Head => 0,
                TopVarInitIdentArrowPhase::ArrowToken => 1,
                TopVarInitIdentArrowPhase::Validate => 2,
                TopVarInitIdentArrowPhase::BlockBody => 3,
                TopVarInitIdentArrowPhase::ExprBody => 4,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 1))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_phase(
        phase: TopVarInitIdentArrowPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_count[idx] += 1;
            stats.top_var_init_ident_arrow_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprAssignPhase {
        AsyncDisambig,
        ArrowProbe,
        Conditional,
        Rhs,
        Cover,
    }

    impl TopVarInitIdentArrowExprAssignPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprAssignPhase::AsyncDisambig => 0,
                TopVarInitIdentArrowExprAssignPhase::ArrowProbe => 1,
                TopVarInitIdentArrowExprAssignPhase::Conditional => 2,
                TopVarInitIdentArrowExprAssignPhase::Rhs => 3,
                TopVarInitIdentArrowExprAssignPhase::Cover => 4,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_assign_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_assign_phase(
        phase: TopVarInitIdentArrowExprAssignPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_assign_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_assign_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondPhase {
        Test,
        Question,
        Consequent,
        Alternate,
    }

    impl TopVarInitIdentArrowExprCondPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondPhase::Test => 0,
                TopVarInitIdentArrowExprCondPhase::Question => 1,
                TopVarInitIdentArrowExprCondPhase::Consequent => 2,
                TopVarInitIdentArrowExprCondPhase::Alternate => 3,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_phase(
        phase: TopVarInitIdentArrowExprCondPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondBinaryPhase {
        LeftUnary,
        PrivateCheck,
        OpScan,
        Rhs,
        Assemble,
    }

    impl TopVarInitIdentArrowExprCondBinaryPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondBinaryPhase::LeftUnary => 0,
                TopVarInitIdentArrowExprCondBinaryPhase::PrivateCheck => 1,
                TopVarInitIdentArrowExprCondBinaryPhase::OpScan => 2,
                TopVarInitIdentArrowExprCondBinaryPhase::Rhs => 3,
                TopVarInitIdentArrowExprCondBinaryPhase::Assemble => 4,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_binary_phase_start(
        min_prec: u8,
    ) -> Option<Instant> {
        (min_prec == 0
            && enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_binary_phase(
        phase: TopVarInitIdentArrowExprCondBinaryPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_binary_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_binary_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondUnaryPhase {
        PrefixUnary,
        PrefixUpdate,
        AwaitExpr,
        Reserved,
        Postfix,
    }

    impl TopVarInitIdentArrowExprCondUnaryPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondUnaryPhase::PrefixUnary => 0,
                TopVarInitIdentArrowExprCondUnaryPhase::PrefixUpdate => 1,
                TopVarInitIdentArrowExprCondUnaryPhase::AwaitExpr => 2,
                TopVarInitIdentArrowExprCondUnaryPhase::Reserved => 3,
                TopVarInitIdentArrowExprCondUnaryPhase::Postfix => 4,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_unary_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_unary_phase(
        phase: TopVarInitIdentArrowExprCondUnaryPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_unary_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_unary_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondPostfixPhase {
        Lhs,
        UpdateCheck,
        UpdateEmit,
    }

    impl TopVarInitIdentArrowExprCondPostfixPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondPostfixPhase::Lhs => 0,
                TopVarInitIdentArrowExprCondPostfixPhase::UpdateCheck => 1,
                TopVarInitIdentArrowExprCondPostfixPhase::UpdateEmit => 2,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_postfix_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_postfix_phase(
        phase: TopVarInitIdentArrowExprCondPostfixPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_postfix_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_postfix_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondLhsPhase {
        NewBase,
        PrimaryBase,
        Continuation,
    }

    impl TopVarInitIdentArrowExprCondLhsPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondLhsPhase::NewBase => 0,
                TopVarInitIdentArrowExprCondLhsPhase::PrimaryBase => 1,
                TopVarInitIdentArrowExprCondLhsPhase::Continuation => 2,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_lhs_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_lhs_phase(
        phase: TopVarInitIdentArrowExprCondLhsPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_lhs_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_lhs_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondPrimaryPhase {
        FnClass,
        ParenTemplate,
        ObjectArray,
        Ident,
        Literal,
        Other,
    }

    impl TopVarInitIdentArrowExprCondPrimaryPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondPrimaryPhase::FnClass => 0,
                TopVarInitIdentArrowExprCondPrimaryPhase::ParenTemplate => 1,
                TopVarInitIdentArrowExprCondPrimaryPhase::ObjectArray => 2,
                TopVarInitIdentArrowExprCondPrimaryPhase::Ident => 3,
                TopVarInitIdentArrowExprCondPrimaryPhase::Literal => 4,
                TopVarInitIdentArrowExprCondPrimaryPhase::Other => 5,
            }
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondPrimaryGuard {
        phase: TopVarInitIdentArrowExprCondPrimaryPhase,
        start: Option<Instant>,
    }

    impl Drop for TopVarInitIdentArrowExprCondPrimaryGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.top_var_init_ident_arrow_expr_cond_primary_count[idx] += 1;
                stats.top_var_init_ident_arrow_expr_cond_primary_ns[idx] += ns;
            });
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_primary_guard(
        phase: TopVarInitIdentArrowExprCondPrimaryPhase,
    ) -> TopVarInitIdentArrowExprCondPrimaryGuard {
        TopVarInitIdentArrowExprCondPrimaryGuard {
            phase,
            start: (enabled()
                && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
            .then(Instant::now),
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondFnClassPhase {
        FnHead,
        FnParams,
        FnBody,
        FnRevalidate,
        ClassHead,
        ClassSuper,
        ClassBody,
    }

    impl TopVarInitIdentArrowExprCondFnClassPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondFnClassPhase::FnHead => 0,
                TopVarInitIdentArrowExprCondFnClassPhase::FnParams => 1,
                TopVarInitIdentArrowExprCondFnClassPhase::FnBody => 2,
                TopVarInitIdentArrowExprCondFnClassPhase::FnRevalidate => 3,
                TopVarInitIdentArrowExprCondFnClassPhase::ClassHead => 4,
                TopVarInitIdentArrowExprCondFnClassPhase::ClassSuper => 5,
                TopVarInitIdentArrowExprCondFnClassPhase::ClassBody => 6,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_fnclass_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_fnclass_phase(
        phase: TopVarInitIdentArrowExprCondFnClassPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_fnclass_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_fnclass_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum TopVarInitIdentArrowExprCondClassBodyPhase {
        Setup,
        MemberLoop,
        Close,
        Validate,
    }

    impl TopVarInitIdentArrowExprCondClassBodyPhase {
        fn index(self) -> usize {
            match self {
                TopVarInitIdentArrowExprCondClassBodyPhase::Setup => 0,
                TopVarInitIdentArrowExprCondClassBodyPhase::MemberLoop => 1,
                TopVarInitIdentArrowExprCondClassBodyPhase::Close => 2,
                TopVarInitIdentArrowExprCondClassBodyPhase::Validate => 3,
            }
        }
    }

    pub(crate) fn top_var_init_ident_arrow_expr_cond_classbody_phase_start() -> Option<Instant> {
        (enabled()
            && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
            && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
        .then(Instant::now)
    }

    pub(crate) fn record_top_var_init_ident_arrow_expr_cond_classbody_phase(
        phase: TopVarInitIdentArrowExprCondClassBodyPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.top_var_init_ident_arrow_expr_cond_classbody_count[idx] += 1;
            stats.top_var_init_ident_arrow_expr_cond_classbody_ns[idx] += ns;
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ClassMemberFamily {
        StaticBlock,
        Accessor,
        Method,
        Field,
    }

    impl ClassMemberFamily {
        fn index(self) -> usize {
            match self {
                ClassMemberFamily::StaticBlock => 0,
                ClassMemberFamily::Accessor => 1,
                ClassMemberFamily::Method => 2,
                ClassMemberFamily::Field => 3,
            }
        }
    }

    pub(crate) struct FunctionBodyTopClassMemberFamilyGuard {
        family: ClassMemberFamily,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassMemberFamilyGuard {
        pub(crate) fn new(family: ClassMemberFamily) -> Self {
            Self {
                family,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassMemberFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_class_member_family_count[idx] += 1;
                stats.fn_body_top_class_member_family_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondClassMemberFamilyGuard {
        family: ClassMemberFamily,
        start: Option<Instant>,
    }

    impl TopVarInitIdentArrowExprCondClassMemberFamilyGuard {
        pub(crate) fn new(family: ClassMemberFamily) -> Self {
            Self {
                family,
                start: (enabled()
                    && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                    && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
                .then(Instant::now),
            }
        }
    }

    impl Drop for TopVarInitIdentArrowExprCondClassMemberFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.top_var_init_ident_arrow_expr_cond_class_member_count[idx] += 1;
                stats.top_var_init_ident_arrow_expr_cond_class_member_ns[idx] += ns;
            });
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ClassMethodPhase {
        Params,
        Body,
        KindValidate,
        StrictRevalidate,
        Push,
    }

    impl ClassMethodPhase {
        fn index(self) -> usize {
            match self {
                ClassMethodPhase::Params => 0,
                ClassMethodPhase::Body => 1,
                ClassMethodPhase::KindValidate => 2,
                ClassMethodPhase::StrictRevalidate => 3,
                ClassMethodPhase::Push => 4,
            }
        }
    }

    pub(crate) struct FunctionBodyTopClassMethodPhaseGuard {
        phase: ClassMethodPhase,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassMethodPhaseGuard {
        pub(crate) fn new(phase: ClassMethodPhase) -> Self {
            Self {
                phase,
                start: (enabled() && function_body_depth() <= 1).then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassMethodPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_class_method_phase_count[idx] += 1;
                stats.fn_body_top_class_method_phase_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondClassMethodPhaseGuard {
        phase: ClassMethodPhase,
        start: Option<Instant>,
    }

    impl TopVarInitIdentArrowExprCondClassMethodPhaseGuard {
        pub(crate) fn new(phase: ClassMethodPhase) -> Self {
            Self {
                phase,
                start: (enabled()
                    && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                    && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2))
                .then(Instant::now),
            }
        }
    }

    impl Drop for TopVarInitIdentArrowExprCondClassMethodPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.top_var_init_ident_arrow_expr_cond_class_method_count[idx] += 1;
                stats.top_var_init_ident_arrow_expr_cond_class_method_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyGuard {
        active: bool,
    }

    impl FunctionBodyTopClassMethodBodyGuard {
        pub(crate) fn new() -> Self {
            if !enabled() || function_body_depth() > 1 {
                return Self { active: false };
            }
            TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.set(depth.get() + 1));
            Self { active: true }
        }
    }

    impl Drop for FunctionBodyTopClassMethodBodyGuard {
        fn drop(&mut self) {
            if self.active {
                TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
        }
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyStatementFamilyGuard {
        family: StatementFamily,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassMethodBodyStatementFamilyGuard {
        pub(crate) fn new(family: StatementFamily) -> Self {
            let active = enabled() && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0);
            Self {
                family,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassMethodBodyStatementFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_class_method_body_stmt_family_count[idx] += 1;
                stats.fn_body_top_class_method_body_stmt_family_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondClassMethodBodyStatementFamilyGuard {
        family: StatementFamily,
        start: Option<Instant>,
    }

    impl TopVarInitIdentArrowExprCondClassMethodBodyStatementFamilyGuard {
        pub(crate) fn new(family: StatementFamily) -> Self {
            let active = enabled()
                && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2);
            Self {
                family,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for TopVarInitIdentArrowExprCondClassMethodBodyStatementFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_count[idx] += 1;
                stats.top_var_init_ident_arrow_expr_cond_class_method_body_stmt_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyControlFamilyGuard {
        family: ControlFamily,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassMethodBodyControlFamilyGuard {
        pub(crate) fn new(family: ControlFamily) -> Self {
            let active = enabled() && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0);
            Self {
                family,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassMethodBodyControlFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_class_method_body_control_family_count[idx] += 1;
                stats.fn_body_top_class_method_body_control_family_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard {
        family: ControlFamily,
        start: Option<Instant>,
    }

    impl TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard {
        pub(crate) fn new(family: ControlFamily) -> Self {
            let active = enabled()
                && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2);
            Self {
                family,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for TopVarInitIdentArrowExprCondClassMethodBodyControlFamilyGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_count[idx] += 1;
                stats.top_var_init_ident_arrow_expr_cond_class_method_body_control_ns[idx] += ns;
            });
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ControlFamily {
        If,
        For,
        Loop,
        SwitchTryWith,
        ReturnThrow,
        BreakContinue,
    }

    impl ControlFamily {
        fn index(self) -> usize {
            match self {
                ControlFamily::If => 0,
                ControlFamily::For => 1,
                ControlFamily::Loop => 2,
                ControlFamily::SwitchTryWith => 3,
                ControlFamily::ReturnThrow => 4,
                ControlFamily::BreakContinue => 5,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum IfPhase {
        Test,
        Consequent,
        Alternate,
        Close,
    }

    impl IfPhase {
        fn index(self) -> usize {
            match self {
                IfPhase::Test => 0,
                IfPhase::Consequent => 1,
                IfPhase::Alternate => 2,
                IfPhase::Close => 3,
            }
        }
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfPhaseGuard {
        phase: IfPhase,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassMethodBodyIfPhaseGuard {
        pub(crate) fn new(phase: IfPhase) -> Self {
            let active = enabled() && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0);
            Self {
                phase,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.fn_body_top_class_method_body_if_phase_count[idx] += 1;
                stats.fn_body_top_class_method_body_if_phase_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard {
        phase: IfPhase,
        start: Option<Instant>,
    }

    impl TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard {
        pub(crate) fn new(phase: IfPhase) -> Self {
            let active = enabled()
                && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2);
            Self {
                phase,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for TopVarInitIdentArrowExprCondClassMethodBodyIfPhaseGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_count[idx] += 1;
                stats.top_var_init_ident_arrow_expr_cond_class_method_body_if_ns[idx] += ns;
            });
        }
    }

    pub(crate) struct TopVarInitIdentArrowExprCondClassMethodBodyIfSubstatementGuard {
        branch: IfBranch,
        family: StatementFamily,
        start: Option<Instant>,
    }

    impl TopVarInitIdentArrowExprCondClassMethodBodyIfSubstatementGuard {
        pub(crate) fn new(branch: IfBranch, family: StatementFamily) -> Self {
            let active = enabled()
                && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_DEPTH.with(|depth| depth.get() > 0)
                && TOP_VAR_INIT_IDENT_ASSIGN_DEPTH.with(|depth| depth.get() == 2);
            Self {
                branch,
                family,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for TopVarInitIdentArrowExprCondClassMethodBodyIfSubstatementGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    IfBranch::Consequent => {
                        stats
                            .top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_count
                            [idx] += 1;
                        stats
                            .top_var_init_ident_arrow_expr_cond_class_method_body_if_consequent_family_ns
                            [idx] += ns;
                    }
                    IfBranch::Alternate => {
                        stats
                            .top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_count
                            [idx] += 1;
                        stats
                            .top_var_init_ident_arrow_expr_cond_class_method_body_if_alternate_family_ns
                            [idx] += ns;
                    }
                }
            });
        }
    }

    pub(crate) struct NestedStatementDepthGuard {
        family: StatementFamily,
        depth: u32,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedStatementDepthGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.depth {
                    2 => {
                        stats.nested_stmt_depth2_count[idx] += 1;
                        stats.nested_stmt_depth2_ns[idx] += ns;
                    }
                    3 => {
                        stats.nested_stmt_depth3_count[idx] += 1;
                        stats.nested_stmt_depth3_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_statement_depth_guard(
        family: StatementFamily,
    ) -> Option<NestedStatementDepthGuard> {
        if !enabled() {
            return None;
        }
        let depth = function_body_depth();
        matches!(depth, 2 | 3).then(|| NestedStatementDepthGuard {
            family,
            depth,
            started: Instant::now(),
            active: true,
        })
    }

    #[derive(Clone, Copy)]
    pub(crate) enum IfBranch {
        Consequent,
        Alternate,
    }

    pub(crate) struct NestedIfSubstatementGuard {
        branch: IfBranch,
        family: StatementFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfSubstatementGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    IfBranch::Consequent => {
                        stats.nested_if_consequent_family_count[idx] += 1;
                        stats.nested_if_consequent_family_ns[idx] += ns;
                    }
                    IfBranch::Alternate => {
                        stats.nested_if_alternate_family_count[idx] += 1;
                        stats.nested_if_alternate_family_ns[idx] += ns;
                    }
                }
            });
        }
    }

    pub(crate) fn nested_if_substatement_guard(
        branch: IfBranch,
        family: StatementFamily,
    ) -> Option<NestedIfSubstatementGuard> {
        (enabled() && function_body_depth() > 1).then(|| NestedIfSubstatementGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfSubstatementGuard {
        branch: IfBranch,
        family: StatementFamily,
        start: Option<Instant>,
    }

    impl FunctionBodyTopClassMethodBodyIfSubstatementGuard {
        pub(crate) fn new(branch: IfBranch, family: StatementFamily) -> Self {
            let active = enabled() && TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0);
            Self {
                branch,
                family,
                start: active.then(Instant::now),
            }
        }
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfSubstatementGuard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    IfBranch::Consequent => {
                        stats.fn_body_top_class_method_body_if_consequent_family_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_consequent_family_ns[idx] += ns;
                    }
                    IfBranch::Alternate => {
                        stats.fn_body_top_class_method_body_if_alternate_family_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_alternate_family_ns[idx] += ns;
                    }
                }
            });
        }
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfBranchBlockGuard {
        prev_branch: u8,
        prev_depth: u32,
        active: bool,
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfBranchBlockGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH
                .with(|branch| branch.set(self.prev_branch));
            TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(|depth| depth.set(self.prev_depth));
        }
    }

    pub(crate) fn function_body_top_class_method_body_if_branch_block_guard(
        branch: IfBranch,
    ) -> Option<FunctionBodyTopClassMethodBodyIfBranchBlockGuard> {
        if !enabled() || !TOP_CLASS_METHOD_BODY_DEPTH.with(|depth| depth.get() > 0) {
            return None;
        }
        let branch_value = match branch {
            IfBranch::Consequent => 1,
            IfBranch::Alternate => 2,
        };
        let prev_branch = TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH.with(|slot| {
            let prev = slot.get();
            slot.set(branch_value);
            prev
        });
        let prev_depth = TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(|depth| {
            let prev = depth.get();
            depth.set(0);
            prev
        });
        Some(FunctionBodyTopClassMethodBodyIfBranchBlockGuard {
            prev_branch,
            prev_depth,
            active: true,
        })
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfBranchBlockDepthGuard {
        active: bool,
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfBranchBlockDepthGuard {
        fn drop(&mut self) {
            if self.active {
                TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(|depth| {
                    depth.set(depth.get().saturating_sub(1));
                });
            }
        }
    }

    pub(crate) fn function_body_top_class_method_body_if_branch_block_depth_guard(
    ) -> Option<FunctionBodyTopClassMethodBodyIfBranchBlockDepthGuard> {
        if !enabled()
            || TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH.with(|branch| branch.get() == 0)
        {
            return None;
        }
        TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(|depth| depth.set(depth.get() + 1));
        Some(FunctionBodyTopClassMethodBodyIfBranchBlockDepthGuard { active: true })
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfBranchBlockStatementGuard {
        branch: u8,
        family: StatementFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfBranchBlockStatementGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_count[idx] +=
                            1;
                        stats.fn_body_top_class_method_body_if_consequent_block_stmt_ns[idx] += ns;
                    }
                    2 => {
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_alternate_block_stmt_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn function_body_top_class_method_body_if_branch_block_statement_guard(
        family: StatementFamily,
    ) -> Option<FunctionBodyTopClassMethodBodyIfBranchBlockStatementGuard> {
        if !enabled() {
            return None;
        }
        let branch = TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(FunctionBodyTopClassMethodBodyIfBranchBlockStatementGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfBranchBlockControlGuard {
        branch: u8,
        family: ControlFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfBranchBlockControlGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.fn_body_top_class_method_body_if_consequent_block_control_count
                            [idx] += 1;
                        stats.fn_body_top_class_method_body_if_consequent_block_control_ns[idx] +=
                            ns;
                    }
                    2 => {
                        stats.fn_body_top_class_method_body_if_alternate_block_control_count
                            [idx] += 1;
                        stats.fn_body_top_class_method_body_if_alternate_block_control_ns[idx] +=
                            ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn function_body_top_class_method_body_if_branch_block_control_guard(
        family: ControlFamily,
    ) -> Option<FunctionBodyTopClassMethodBodyIfBranchBlockControlGuard> {
        if !enabled() {
            return None;
        }
        let branch = TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(FunctionBodyTopClassMethodBodyIfBranchBlockControlGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfBranchBlockIfPhaseGuard {
        branch: u8,
        phase: IfPhase,
        started: Instant,
        active: bool,
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfBranchBlockIfPhaseGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_count
                            [idx] += 1;
                        stats.fn_body_top_class_method_body_if_consequent_block_if_phase_ns[idx] +=
                            ns;
                    }
                    2 => {
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_count
                            [idx] += 1;
                        stats.fn_body_top_class_method_body_if_alternate_block_if_phase_ns[idx] +=
                            ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn function_body_top_class_method_body_if_branch_block_if_phase_guard(
        phase: IfPhase,
    ) -> Option<FunctionBodyTopClassMethodBodyIfBranchBlockIfPhaseGuard> {
        if !enabled() {
            return None;
        }
        let branch = TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(FunctionBodyTopClassMethodBodyIfBranchBlockIfPhaseGuard {
            branch,
            phase,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct FunctionBodyTopClassMethodBodyIfBranchBlockIfSubstatementGuard {
        branch: u8,
        if_branch: IfBranch,
        family: StatementFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for FunctionBodyTopClassMethodBodyIfBranchBlockIfSubstatementGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match (self.branch, self.if_branch) {
                    (1, IfBranch::Consequent) => {
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_consequent_block_if_consequent_family_ns[idx] += ns;
                    }
                    (1, IfBranch::Alternate) => {
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_consequent_block_if_alternate_family_ns[idx] += ns;
                    }
                    (2, IfBranch::Consequent) => {
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_alternate_block_if_consequent_family_ns[idx] += ns;
                    }
                    (2, IfBranch::Alternate) => {
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_count[idx] += 1;
                        stats.fn_body_top_class_method_body_if_alternate_block_if_alternate_family_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn function_body_top_class_method_body_if_branch_block_if_substatement_guard(
        if_branch: IfBranch,
        family: StatementFamily,
    ) -> Option<FunctionBodyTopClassMethodBodyIfBranchBlockIfSubstatementGuard> {
        if !enabled() {
            return None;
        }
        let branch = TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || TOP_CLASS_METHOD_BODY_IF_BRANCH_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(
            FunctionBodyTopClassMethodBodyIfBranchBlockIfSubstatementGuard {
                branch,
                if_branch,
                family,
                started: Instant::now(),
                active: true,
            },
        )
    }

    pub(crate) struct NestedIfBranchBlockGuard {
        prev_branch: u8,
        prev_depth: u32,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            IF_BRANCH_BLOCK_BRANCH.with(|branch| branch.set(self.prev_branch));
            IF_BRANCH_BLOCK_PARSE_DEPTH.with(|depth| depth.set(self.prev_depth));
        }
    }

    pub(crate) fn nested_if_branch_block_guard(
        branch: IfBranch,
    ) -> Option<NestedIfBranchBlockGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch_value = match branch {
            IfBranch::Consequent => 1,
            IfBranch::Alternate => 2,
        };
        let prev_branch = IF_BRANCH_BLOCK_BRANCH.with(|branch| {
            let prev = branch.get();
            branch.set(branch_value);
            prev
        });
        let prev_depth = IF_BRANCH_BLOCK_PARSE_DEPTH.with(|depth| {
            let prev = depth.get();
            depth.set(0);
            prev
        });
        Some(NestedIfBranchBlockGuard {
            prev_branch,
            prev_depth,
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockDepthGuard {
        active: bool,
    }

    impl Drop for NestedIfBranchBlockDepthGuard {
        fn drop(&mut self) {
            if self.active {
                IF_BRANCH_BLOCK_PARSE_DEPTH.with(|depth| {
                    depth.set(depth.get().saturating_sub(1));
                });
            }
        }
    }

    pub(crate) fn nested_if_branch_block_depth_guard() -> Option<NestedIfBranchBlockDepthGuard> {
        if !enabled() || IF_BRANCH_BLOCK_BRANCH.with(|branch| branch.get() == 0) {
            return None;
        }
        IF_BRANCH_BLOCK_PARSE_DEPTH.with(|depth| depth.set(depth.get() + 1));
        Some(NestedIfBranchBlockDepthGuard { active: true })
    }

    pub(crate) struct NestedIfBranchBlockStatementGuard {
        branch: u8,
        family: StatementFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockStatementGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.nested_if_consequent_block_stmt_count[idx] += 1;
                        stats.nested_if_consequent_block_stmt_ns[idx] += ns;
                    }
                    2 => {
                        stats.nested_if_alternate_block_stmt_count[idx] += 1;
                        stats.nested_if_alternate_block_stmt_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_statement_guard(
        family: StatementFamily,
    ) -> Option<NestedIfBranchBlockStatementGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_PARSE_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockStatementGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockControlGuard {
        branch: u8,
        family: ControlFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockControlGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.nested_if_consequent_block_control_count[idx] += 1;
                        stats.nested_if_consequent_block_control_ns[idx] += ns;
                    }
                    2 => {
                        stats.nested_if_alternate_block_control_count[idx] += 1;
                        stats.nested_if_alternate_block_control_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_control_guard(
        family: ControlFamily,
    ) -> Option<NestedIfBranchBlockControlGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_PARSE_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockControlGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockIfPhaseGuard {
        branch: u8,
        phase: IfPhase,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfPhaseGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.nested_if_consequent_block_if_phase_count[idx] += 1;
                        stats.nested_if_consequent_block_if_phase_ns[idx] += ns;
                    }
                    2 => {
                        stats.nested_if_alternate_block_if_phase_count[idx] += 1;
                        stats.nested_if_alternate_block_if_phase_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_if_phase_guard(
        phase: IfPhase,
    ) -> Option<NestedIfBranchBlockIfPhaseGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_PARSE_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockIfPhaseGuard {
            branch,
            phase,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockIfBranchGuard {
        outer_branch: u8,
        inner_branch: IfBranch,
        family: StatementFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfBranchGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match (self.outer_branch, self.inner_branch) {
                    (1, IfBranch::Consequent) => {
                        stats.nested_if_consequent_block_if_consequent_family_count[idx] += 1;
                        stats.nested_if_consequent_block_if_consequent_family_ns[idx] += ns;
                    }
                    (1, IfBranch::Alternate) => {
                        stats.nested_if_consequent_block_if_alternate_family_count[idx] += 1;
                        stats.nested_if_consequent_block_if_alternate_family_ns[idx] += ns;
                    }
                    (2, IfBranch::Consequent) => {
                        stats.nested_if_alternate_block_if_consequent_family_count[idx] += 1;
                        stats.nested_if_alternate_block_if_consequent_family_ns[idx] += ns;
                    }
                    (2, IfBranch::Alternate) => {
                        stats.nested_if_alternate_block_if_alternate_family_count[idx] += 1;
                        stats.nested_if_alternate_block_if_alternate_family_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_if_branch_guard(
        inner_branch: IfBranch,
        family: StatementFamily,
    ) -> Option<NestedIfBranchBlockIfBranchGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let outer_branch = IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if outer_branch == 0 || IF_BRANCH_BLOCK_PARSE_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockIfBranchGuard {
            outer_branch,
            inner_branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockIfConsequentBlockGuard {
        prev_branch: u8,
        prev_depth: u32,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfConsequentBlockGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH.with(|branch| branch.set(self.prev_branch));
            IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(|depth| depth.set(self.prev_depth));
        }
    }

    pub(crate) fn nested_if_branch_block_if_consequent_block_guard(
    ) -> Option<NestedIfBranchBlockIfConsequentBlockGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_PARSE_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        let prev_branch = IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH.with(|slot| {
            let prev = slot.get();
            slot.set(branch);
            prev
        });
        let prev_depth = IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(|depth| {
            let prev = depth.get();
            depth.set(0);
            prev
        });
        Some(NestedIfBranchBlockIfConsequentBlockGuard {
            prev_branch,
            prev_depth,
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockIfConsequentBlockDepthGuard {
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfConsequentBlockDepthGuard {
        fn drop(&mut self) {
            if self.active {
                IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(|depth| {
                    depth.set(depth.get().saturating_sub(1));
                });
            }
        }
    }

    pub(crate) fn nested_if_branch_block_if_consequent_block_depth_guard(
    ) -> Option<NestedIfBranchBlockIfConsequentBlockDepthGuard> {
        if !enabled() || IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH.with(|branch| branch.get() == 0)
        {
            return None;
        }
        IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(|depth| depth.set(depth.get() + 1));
        Some(NestedIfBranchBlockIfConsequentBlockDepthGuard { active: true })
    }

    pub(crate) struct NestedIfBranchBlockIfConsequentBlockStatementGuard {
        branch: u8,
        family: StatementFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfConsequentBlockStatementGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.nested_if_consequent_block_if_consequent_block_stmt_count[idx] += 1;
                        stats.nested_if_consequent_block_if_consequent_block_stmt_ns[idx] += ns;
                    }
                    2 => {
                        stats.nested_if_alternate_block_if_consequent_block_stmt_count[idx] += 1;
                        stats.nested_if_alternate_block_if_consequent_block_stmt_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_if_consequent_block_statement_guard(
        family: StatementFamily,
    ) -> Option<NestedIfBranchBlockIfConsequentBlockStatementGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockIfConsequentBlockStatementGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockIfConsequentBlockControlGuard {
        branch: u8,
        family: ControlFamily,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfConsequentBlockControlGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.family.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.nested_if_consequent_block_if_consequent_block_control_count[idx] +=
                            1;
                        stats.nested_if_consequent_block_if_consequent_block_control_ns[idx] += ns;
                    }
                    2 => {
                        stats.nested_if_alternate_block_if_consequent_block_control_count[idx] += 1;
                        stats.nested_if_alternate_block_if_consequent_block_control_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_if_consequent_block_control_guard(
        family: ControlFamily,
    ) -> Option<NestedIfBranchBlockIfConsequentBlockControlGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockIfConsequentBlockControlGuard {
            branch,
            family,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) struct NestedIfBranchBlockIfConsequentBlockIfPhaseGuard {
        branch: u8,
        phase: IfPhase,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedIfBranchBlockIfConsequentBlockIfPhaseGuard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.branch {
                    1 => {
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_count[idx] +=
                            1;
                        stats.nested_if_consequent_block_if_consequent_block_if_phase_ns[idx] += ns;
                    }
                    2 => {
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_count[idx] +=
                            1;
                        stats.nested_if_alternate_block_if_consequent_block_if_phase_ns[idx] += ns;
                    }
                    _ => {}
                }
            });
        }
    }

    pub(crate) fn nested_if_branch_block_if_consequent_block_if_phase_guard(
        phase: IfPhase,
    ) -> Option<NestedIfBranchBlockIfConsequentBlockIfPhaseGuard> {
        if !enabled() || function_body_depth() <= 1 {
            return None;
        }
        let branch = IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_BRANCH.with(Cell::get);
        if branch == 0 || IF_BRANCH_BLOCK_IF_CONSEQUENT_BLOCK_DEPTH.with(Cell::get) != 1 {
            return None;
        }
        Some(NestedIfBranchBlockIfConsequentBlockIfPhaseGuard {
            branch,
            phase,
            started: Instant::now(),
            active: true,
        })
    }

    #[derive(Clone, Copy)]
    pub(crate) enum FunctionDeclPhase {
        Name,
        Params,
        Dups,
        Body,
        Revalidate,
        Finish,
    }

    impl FunctionDeclPhase {
        fn index(self) -> usize {
            match self {
                FunctionDeclPhase::Name => 0,
                FunctionDeclPhase::Params => 1,
                FunctionDeclPhase::Dups => 2,
                FunctionDeclPhase::Body => 3,
                FunctionDeclPhase::Revalidate => 4,
                FunctionDeclPhase::Finish => 5,
            }
        }
    }

    pub(crate) struct NestedFunctionDeclDepth2Guard {
        phase: FunctionDeclPhase,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedFunctionDeclDepth2Guard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.nested_fn_decl_depth2_count[idx] += 1;
                stats.nested_fn_decl_depth2_ns[idx] += ns;
            });
        }
    }

    pub(crate) fn nested_function_decl_depth2_guard(
        phase: FunctionDeclPhase,
    ) -> Option<NestedFunctionDeclDepth2Guard> {
        if !enabled() || function_body_depth() != 2 {
            return None;
        }
        Some(NestedFunctionDeclDepth2Guard {
            phase,
            started: Instant::now(),
            active: true,
        })
    }

    #[derive(Clone, Copy)]
    pub(crate) enum VariableDeclPhase {
        Target,
        NoLet,
        Init,
        Finish,
    }

    impl VariableDeclPhase {
        fn index(self) -> usize {
            match self {
                VariableDeclPhase::Target => 0,
                VariableDeclPhase::NoLet => 1,
                VariableDeclPhase::Init => 2,
                VariableDeclPhase::Finish => 3,
            }
        }
    }

    pub(crate) struct NestedVariableDepth3Guard {
        phase: VariableDeclPhase,
        started: Instant,
        active: bool,
    }

    impl Drop for NestedVariableDepth3Guard {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            let ns = self.started.elapsed().as_nanos() as u64;
            let idx = self.phase.index();
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                stats.nested_var_depth3_count[idx] += 1;
                stats.nested_var_depth3_ns[idx] += ns;
            });
        }
    }

    pub(crate) fn nested_variable_depth3_guard(
        phase: VariableDeclPhase,
    ) -> Option<NestedVariableDepth3Guard> {
        if !enabled() || function_body_depth() != 3 {
            return None;
        }
        Some(NestedVariableDepth3Guard {
            phase,
            started: Instant::now(),
            active: true,
        })
    }

    pub(crate) fn nested_phase_guard(kind: Kind) -> Option<Guard> {
        (function_body_depth() > 1).then(|| Guard::new(kind))
    }

    pub(crate) fn nested_var_init_direct_start() -> Option<Instant> {
        (enabled() && function_body_depth() > 1).then(Instant::now)
    }

    pub(crate) fn record_nested_var_init_direct(kind: Kind, start: Option<Instant>) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            match kind {
                Kind::NestedVarInitFnClass => {
                    stats.nested_var_init_direct_fn_class_count += 1;
                    stats.nested_var_init_direct_fn_class_ns += ns;
                }
                Kind::NestedVarInitParen => {
                    stats.nested_var_init_direct_paren_count += 1;
                    stats.nested_var_init_direct_paren_ns += ns;
                }
                Kind::NestedVarInitObjectArray => {
                    stats.nested_var_init_direct_object_array_count += 1;
                    stats.nested_var_init_direct_object_array_ns += ns;
                }
                Kind::NestedVarInitIdent => {
                    stats.nested_var_init_direct_ident_count += 1;
                    stats.nested_var_init_direct_ident_ns += ns;
                }
                Kind::NestedVarInitLiteral => {
                    stats.nested_var_init_direct_literal_count += 1;
                    stats.nested_var_init_direct_literal_ns += ns;
                }
                Kind::NestedVarInitOther => {
                    stats.nested_var_init_direct_other_count += 1;
                    stats.nested_var_init_direct_other_ns += ns;
                }
                _ => {}
            }
        });
    }

    pub(crate) fn record_nested_var_init_depth3(kind: Kind, start: Option<Instant>) {
        let Some(start) = start else {
            return;
        };
        if function_body_depth() != 3 {
            return;
        }
        let ns = start.elapsed().as_nanos() as u64;
        let idx = match kind {
            Kind::NestedVarInitFnClass => 0,
            Kind::NestedVarInitParen => 1,
            Kind::NestedVarInitObjectArray => 2,
            Kind::NestedVarInitIdent => 3,
            Kind::NestedVarInitLiteral => 4,
            Kind::NestedVarInitOther => 5,
            _ => return,
        };
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.nested_var_init_depth3_count[idx] += 1;
            stats.nested_var_init_depth3_ns[idx] += ns;
        });
    }

    pub(crate) fn record_nested_var_init_depth3_object_array(
        is_array: bool,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        if function_body_depth() != 3 {
            return;
        }
        let ns = start.elapsed().as_nanos() as u64;
        let idx = usize::from(is_array);
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.nested_var_init_depth3_object_array_count[idx] += 1;
            stats.nested_var_init_depth3_object_array_ns[idx] += ns;
        });
    }

    pub(crate) struct Depth3ObjectInitGuard {
        active: bool,
    }

    impl Drop for Depth3ObjectInitGuard {
        fn drop(&mut self) {
            if self.active {
                DEPTH3_OBJECT_INIT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
        }
    }

    pub(crate) fn depth3_object_init_guard() -> Option<Depth3ObjectInitGuard> {
        if !enabled() || function_body_depth() != 3 {
            return None;
        }
        DEPTH3_OBJECT_INIT_DEPTH.with(|depth| depth.set(depth.get() + 1));
        Some(Depth3ObjectInitGuard { active: true })
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ObjectPropPhase {
        Spread,
        GeneratorMethod,
        AsyncMethod,
        Accessor,
        Colon,
        Method,
        Shorthand,
    }

    impl ObjectPropPhase {
        fn index(self) -> usize {
            match self {
                ObjectPropPhase::Spread => 0,
                ObjectPropPhase::GeneratorMethod => 1,
                ObjectPropPhase::AsyncMethod => 2,
                ObjectPropPhase::Accessor => 3,
                ObjectPropPhase::Colon => 4,
                ObjectPropPhase::Method => 5,
                ObjectPropPhase::Shorthand => 6,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ObjectColonPhase {
        Key,
        Value,
        Comma,
    }

    impl ObjectColonPhase {
        fn index(self) -> usize {
            match self {
                ObjectColonPhase::Key => 0,
                ObjectColonPhase::Value => 1,
                ObjectColonPhase::Comma => 2,
            }
        }
    }

    pub(crate) fn depth3_object_prop_start() -> Option<Instant> {
        (enabled() && DEPTH3_OBJECT_INIT_DEPTH.with(|depth| depth.get() > 0)).then(Instant::now)
    }

    pub(crate) fn record_depth3_object_prop(phase: ObjectPropPhase, start: Option<Instant>) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.nested_var_init_depth3_object_prop_count[idx] += 1;
            stats.nested_var_init_depth3_object_prop_ns[idx] += ns;
        });
    }

    pub(crate) fn record_depth3_object_colon_phase(
        phase: ObjectColonPhase,
        start: Option<Instant>,
    ) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = phase.index();
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.nested_var_init_depth3_object_colon_count[idx] += 1;
            stats.nested_var_init_depth3_object_colon_ns[idx] += ns;
        });
    }

    pub(crate) fn record_depth3_object_colon_value(kind: Kind, start: Option<Instant>) {
        let Some(start) = start else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        let idx = match kind {
            Kind::NestedVarInitFnClass => 0,
            Kind::NestedVarInitParen => 1,
            Kind::NestedVarInitObjectArray => 2,
            Kind::NestedVarInitIdent => 3,
            Kind::NestedVarInitLiteral => 4,
            Kind::NestedVarInitOther => 5,
            _ => return,
        };
        STATS.with(|stats| {
            let mut stats = stats.borrow_mut();
            stats.nested_var_init_depth3_object_colon_value_count[idx] += 1;
            stats.nested_var_init_depth3_object_colon_value_ns[idx] += ns;
        });
    }

    pub(crate) struct Guard {
        kind: Kind,
        start: Option<Instant>,
        exclusive_active: bool,
    }

    impl Guard {
        pub(crate) fn new(kind: Kind) -> Self {
            let active = enabled();
            if active {
                EXCLUSIVE_STACK.with(|stack| stack.borrow_mut().push(0));
            }
            Self {
                kind,
                start: active.then(Instant::now),
                exclusive_active: active,
            }
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            let Some(start) = self.start else {
                return;
            };
            let ns = start.elapsed().as_nanos() as u64;
            let exclusive_ns = if self.exclusive_active {
                EXCLUSIVE_STACK.with(|stack| {
                    let mut stack = stack.borrow_mut();
                    let child_ns = stack.pop().unwrap_or(0);
                    if let Some(parent_child_ns) = stack.last_mut() {
                        *parent_child_ns = parent_child_ns.saturating_add(ns);
                    }
                    ns.saturating_sub(child_ns)
                })
            } else {
                ns
            };
            STATS.with(|stats| {
                let mut stats = stats.borrow_mut();
                match self.kind {
                    Kind::Module => {
                        stats.module_count += 1;
                        stats.module_ns += ns;
                    }
                    Kind::Statement => {
                        stats.statement_count += 1;
                        stats.statement_ns += ns;
                    }
                    Kind::Expression => {
                        stats.expression_count += 1;
                        stats.expression_ns += ns;
                    }
                    Kind::FunctionBody => {
                        stats.function_body_count += 1;
                        stats.function_body_ns += ns;
                    }
                    Kind::ClassBody => {
                        stats.class_body_count += 1;
                        stats.class_body_ns += ns;
                    }
                    Kind::StmtVar => {
                        stats.stmt_var_count += 1;
                        stats.stmt_var_ns += ns;
                    }
                    Kind::StmtFunction => {
                        stats.stmt_function_count += 1;
                        stats.stmt_function_ns += ns;
                    }
                    Kind::StmtExpression => {
                        stats.stmt_expression_count += 1;
                        stats.stmt_expression_ns += ns;
                    }
                    Kind::StmtBlock => {
                        stats.stmt_block_count += 1;
                        stats.stmt_block_ns += ns;
                    }
                    Kind::StmtControl => {
                        stats.stmt_control_count += 1;
                        stats.stmt_control_ns += ns;
                    }
                    Kind::StmtLabel => {
                        stats.stmt_label_count += 1;
                        stats.stmt_label_ns += ns;
                    }
                    Kind::ExprConditional => {
                        stats.expr_conditional_count += 1;
                        stats.expr_conditional_ns += ns;
                    }
                    Kind::ExprBinary => {
                        stats.expr_binary_count += 1;
                        stats.expr_binary_ns += ns;
                    }
                    Kind::ExprLhs => {
                        stats.expr_lhs_count += 1;
                        stats.expr_lhs_ns += ns;
                    }
                    Kind::ExprPrimary => {
                        stats.expr_primary_count += 1;
                        stats.expr_primary_ns += ns;
                    }
                    Kind::FnBodyLoop => {
                        stats.fn_body_loop_count += 1;
                        stats.fn_body_loop_ns += ns;
                    }
                    Kind::FnBodyBoundNames => {
                        stats.fn_body_bound_names_count += 1;
                        stats.fn_body_bound_names_ns += ns;
                    }
                    Kind::FnBodyDesugar => {
                        stats.fn_body_desugar_count += 1;
                        stats.fn_body_desugar_ns += ns;
                    }
                    Kind::LhsBase => {
                        stats.lhs_base_count += 1;
                        stats.lhs_base_ns += ns;
                    }
                    Kind::LhsCont => {
                        stats.lhs_cont_count += 1;
                        stats.lhs_cont_ns += ns;
                    }
                    Kind::LhsMember => {
                        stats.lhs_member_count += 1;
                        stats.lhs_member_ns += ns;
                    }
                    Kind::LhsComputed => {
                        stats.lhs_computed_count += 1;
                        stats.lhs_computed_ns += ns;
                    }
                    Kind::LhsCall => {
                        stats.lhs_call_count += 1;
                        stats.lhs_call_ns += ns;
                    }
                    Kind::LhsTemplate => {
                        stats.lhs_template_count += 1;
                        stats.lhs_template_ns += ns;
                    }
                    Kind::PrimaryIdent => {
                        stats.primary_ident_count += 1;
                        stats.primary_ident_ns += ns;
                    }
                    Kind::PrimaryLiteral => {
                        stats.primary_literal_count += 1;
                        stats.primary_literal_ns += ns;
                    }
                    Kind::PrimaryFnClass => {
                        stats.primary_fn_class_count += 1;
                        stats.primary_fn_class_ns += ns;
                    }
                    Kind::PrimaryFunction => {
                        stats.primary_function_count += 1;
                        stats.primary_function_ns += ns;
                    }
                    Kind::PrimaryClass => {
                        stats.primary_class_count += 1;
                        stats.primary_class_ns += ns;
                    }
                    Kind::PrimaryObjectArray => {
                        stats.primary_object_array_count += 1;
                        stats.primary_object_array_ns += ns;
                    }
                    Kind::PrimaryParenTemplate => {
                        stats.primary_paren_template_count += 1;
                        stats.primary_paren_template_ns += ns;
                    }
                    Kind::PrimaryParen => {
                        stats.primary_paren_count += 1;
                        stats.primary_paren_ns += ns;
                    }
                    Kind::PrimaryTemplate => {
                        stats.primary_template_count += 1;
                        stats.primary_template_ns += ns;
                    }
                    Kind::PrimaryOther => {
                        stats.primary_other_count += 1;
                        stats.primary_other_ns += ns;
                    }
                    Kind::CallArgs => {
                        stats.call_args_count += 1;
                        stats.call_args_ns += ns;
                    }
                    Kind::ParenInner => {
                        stats.paren_inner_count += 1;
                        stats.paren_inner_ns += ns;
                    }
                    Kind::ParenConditional => {
                        stats.paren_conditional_count += 1;
                        stats.paren_conditional_ns += ns;
                    }
                    Kind::ParenBinary => {
                        stats.paren_binary_count += 1;
                        stats.paren_binary_ns += ns;
                    }
                    Kind::ParenLhs => {
                        stats.paren_lhs_count += 1;
                        stats.paren_lhs_ns += ns;
                    }
                    Kind::ParenPrimary => {
                        stats.paren_primary_count += 1;
                        stats.paren_primary_ns += ns;
                    }
                    Kind::FnExprParams => {
                        stats.fn_expr_params_count += 1;
                        stats.fn_expr_params_ns += ns;
                    }
                    Kind::FnExprBody => {
                        stats.fn_expr_body_count += 1;
                        stats.fn_expr_body_ns += ns;
                    }
                    Kind::FnExprRevalidate => {
                        stats.fn_expr_revalidate_count += 1;
                        stats.fn_expr_revalidate_ns += ns;
                    }
                    Kind::ClassExprSuper => {
                        stats.class_expr_super_count += 1;
                        stats.class_expr_super_ns += ns;
                    }
                    Kind::ClassExprBody => {
                        stats.class_expr_body_count += 1;
                        stats.class_expr_body_ns += ns;
                    }
                    Kind::FnBodyLoopTop => {
                        stats.fn_body_loop_top_count += 1;
                        stats.fn_body_loop_top_ns += ns;
                    }
                    Kind::FnBodyLoopNested => {
                        stats.fn_body_loop_nested_count += 1;
                        stats.fn_body_loop_nested_ns += ns;
                    }
                    Kind::NestedStmtVar => {
                        stats.nested_stmt_var_count += 1;
                        stats.nested_stmt_var_ns += ns;
                        stats.nested_stmt_var_exclusive_ns += exclusive_ns;
                    }
                    Kind::NestedStmtFunction => {
                        stats.nested_stmt_function_count += 1;
                        stats.nested_stmt_function_ns += ns;
                        stats.nested_stmt_function_exclusive_ns += exclusive_ns;
                    }
                    Kind::NestedStmtExpression => {
                        stats.nested_stmt_expression_count += 1;
                        stats.nested_stmt_expression_ns += ns;
                    }
                    Kind::NestedStmtBlock => {
                        stats.nested_stmt_block_count += 1;
                        stats.nested_stmt_block_ns += ns;
                    }
                    Kind::NestedStmtControl => {
                        stats.nested_stmt_control_count += 1;
                        stats.nested_stmt_control_ns += ns;
                    }
                    Kind::NestedControlIf => {
                        stats.nested_control_if_count += 1;
                        stats.nested_control_if_ns += ns;
                    }
                    Kind::NestedControlFor => {
                        stats.nested_control_for_count += 1;
                        stats.nested_control_for_ns += ns;
                    }
                    Kind::NestedControlLoop => {
                        stats.nested_control_loop_count += 1;
                        stats.nested_control_loop_ns += ns;
                    }
                    Kind::NestedControlSwitchTryWith => {
                        stats.nested_control_switch_try_with_count += 1;
                        stats.nested_control_switch_try_with_ns += ns;
                    }
                    Kind::NestedControlReturnThrow => {
                        stats.nested_control_return_throw_count += 1;
                        stats.nested_control_return_throw_ns += ns;
                    }
                    Kind::NestedControlBreakContinue => {
                        stats.nested_control_break_continue_count += 1;
                        stats.nested_control_break_continue_ns += ns;
                    }
                    Kind::NestedIfTest => {
                        stats.nested_if_test_count += 1;
                        stats.nested_if_test_ns += ns;
                    }
                    Kind::NestedIfConsequent => {
                        stats.nested_if_consequent_count += 1;
                        stats.nested_if_consequent_ns += ns;
                    }
                    Kind::NestedIfAlternate => {
                        stats.nested_if_alternate_count += 1;
                        stats.nested_if_alternate_ns += ns;
                    }
                    Kind::NestedIfClose => {
                        stats.nested_if_close_count += 1;
                        stats.nested_if_close_ns += ns;
                    }
                    Kind::NestedVarTarget => {
                        stats.nested_var_target_count += 1;
                        stats.nested_var_target_ns += ns;
                    }
                    Kind::NestedVarNoLet => {
                        stats.nested_var_no_let_count += 1;
                        stats.nested_var_no_let_ns += ns;
                    }
                    Kind::NestedVarInit => {
                        stats.nested_var_init_count += 1;
                        stats.nested_var_init_ns += ns;
                        stats.nested_var_init_exclusive_ns += exclusive_ns;
                    }
                    Kind::NestedVarInitAssign => {
                        stats.nested_var_init_assign_count += 1;
                        stats.nested_var_init_assign_ns += ns;
                    }
                    Kind::NestedVarInitExpr => {
                        stats.nested_var_init_expr_count += 1;
                        stats.nested_var_init_expr_ns += ns;
                    }
                    Kind::BumpFetch => {
                        stats.bump_fetch_count += 1;
                        stats.bump_fetch_ns += ns;
                    }
                    Kind::BumpGoal => {
                        stats.bump_goal_count += 1;
                        stats.bump_goal_ns += ns;
                    }
                    Kind::LexerTrivia => {
                        stats.lexer_trivia_count += 1;
                        stats.lexer_trivia_ns += ns;
                    }
                    Kind::LexerPunct => {
                        stats.lexer_punct_count += 1;
                        stats.lexer_punct_ns += ns;
                    }
                    Kind::LexerIdent => {
                        stats.lexer_ident_count += 1;
                        stats.lexer_ident_ns += ns;
                    }
                    Kind::LexerPrivateIdent => {
                        stats.lexer_private_ident_count += 1;
                        stats.lexer_private_ident_ns += ns;
                    }
                    Kind::LexerNumeric => {
                        stats.lexer_numeric_count += 1;
                        stats.lexer_numeric_ns += ns;
                    }
                    Kind::LexerString => {
                        stats.lexer_string_count += 1;
                        stats.lexer_string_ns += ns;
                    }
                    Kind::LexerTemplate => {
                        stats.lexer_template_count += 1;
                        stats.lexer_template_ns += ns;
                    }
                    Kind::LexerRegex => {
                        stats.lexer_regex_count += 1;
                        stats.lexer_regex_ns += ns;
                    }
                    Kind::LexerHashbang => {
                        stats.lexer_hashbang_count += 1;
                        stats.lexer_hashbang_ns += ns;
                    }
                    Kind::LexerEof => {
                        stats.lexer_eof_count += 1;
                        stats.lexer_eof_ns += ns;
                    }
                    Kind::LexerStringScan => {
                        stats.lexer_string_scan_count += 1;
                        stats.lexer_string_scan_ns += ns;
                    }
                    Kind::LexerStringConvert => {
                        stats.lexer_string_convert_count += 1;
                        stats.lexer_string_convert_ns += ns;
                    }
                    Kind::LexerStringToken => {
                        stats.lexer_string_token_count += 1;
                        stats.lexer_string_token_ns += ns;
                    }
                    Kind::LexerStringEscape => {
                        stats.lexer_string_escape_count += 1;
                        stats.lexer_string_escape_ns += ns;
                    }
                    Kind::LexerStringNoEscape => {
                        stats.lexer_string_no_escape_count += 1;
                        stats.lexer_string_no_escape_ns += ns;
                    }
                    Kind::LexerStringNoEscapeAscii => {
                        stats.lexer_string_no_escape_ascii_count += 1;
                        stats.lexer_string_no_escape_ascii_ns += ns;
                    }
                    Kind::LexerStringNoEscapeNonAscii => {
                        stats.lexer_string_no_escape_non_ascii_count += 1;
                        stats.lexer_string_no_escape_non_ascii_ns += ns;
                    }
                    Kind::LexerStringNoEscapeDecode => {
                        stats.lexer_string_no_escape_decode_count += 1;
                        stats.lexer_string_no_escape_decode_ns += ns;
                    }
                    Kind::LexerStringNoEscapeMarker => {
                        stats.lexer_string_no_escape_marker_count += 1;
                        stats.lexer_string_no_escape_marker_ns += ns;
                    }
                    Kind::LexerStringNoEscapePush => {
                        stats.lexer_string_no_escape_push_count += 1;
                        stats.lexer_string_no_escape_push_ns += ns;
                    }
                    Kind::LexerStringNoEscapeAdvance => {
                        stats.lexer_string_no_escape_advance_count += 1;
                        stats.lexer_string_no_escape_advance_ns += ns;
                    }
                    Kind::NestedVarInitFnClass => {
                        stats.nested_var_init_fn_class_count += 1;
                        stats.nested_var_init_fn_class_ns += ns;
                    }
                    Kind::NestedVarInitParen => {
                        stats.nested_var_init_paren_count += 1;
                        stats.nested_var_init_paren_ns += ns;
                    }
                    Kind::NestedVarInitObjectArray => {
                        stats.nested_var_init_object_array_count += 1;
                        stats.nested_var_init_object_array_ns += ns;
                    }
                    Kind::NestedVarInitIdent => {
                        stats.nested_var_init_ident_count += 1;
                        stats.nested_var_init_ident_ns += ns;
                    }
                    Kind::NestedVarInitLiteral => {
                        stats.nested_var_init_literal_count += 1;
                        stats.nested_var_init_literal_ns += ns;
                    }
                    Kind::NestedVarInitOther => {
                        stats.nested_var_init_other_count += 1;
                        stats.nested_var_init_other_ns += ns;
                    }
                    Kind::NestedVarFinish => {
                        stats.nested_var_finish_count += 1;
                        stats.nested_var_finish_ns += ns;
                    }
                    Kind::NestedFnDeclName => {
                        stats.nested_fn_decl_name_count += 1;
                        stats.nested_fn_decl_name_ns += ns;
                    }
                    Kind::NestedFnDeclParams => {
                        stats.nested_fn_decl_params_count += 1;
                        stats.nested_fn_decl_params_ns += ns;
                    }
                    Kind::NestedFnDeclDups => {
                        stats.nested_fn_decl_dups_count += 1;
                        stats.nested_fn_decl_dups_ns += ns;
                    }
                    Kind::NestedFnDeclBody => {
                        stats.nested_fn_decl_body_count += 1;
                        stats.nested_fn_decl_body_ns += ns;
                        stats.nested_fn_decl_body_exclusive_ns += exclusive_ns;
                    }
                    Kind::NestedFnDeclRevalidate => {
                        stats.nested_fn_decl_revalidate_count += 1;
                        stats.nested_fn_decl_revalidate_ns += ns;
                    }
                    Kind::NestedFnDeclFinish => {
                        stats.nested_fn_decl_finish_count += 1;
                        stats.nested_fn_decl_finish_ns += ns;
                    }
                    Kind::NestedFnBodyLoop => {
                        stats.nested_fn_body_loop_count += 1;
                        stats.nested_fn_body_loop_ns += ns;
                    }
                    Kind::NestedFnBodyLoopDepth2 => {
                        stats.nested_fn_body_loop_depth2_count += 1;
                        stats.nested_fn_body_loop_depth2_ns += ns;
                    }
                    Kind::NestedFnBodyLoopDepth3 => {
                        stats.nested_fn_body_loop_depth3_count += 1;
                        stats.nested_fn_body_loop_depth3_ns += ns;
                    }
                    Kind::NestedFnBodyLoopDepth4Plus => {
                        stats.nested_fn_body_loop_depth4_plus_count += 1;
                        stats.nested_fn_body_loop_depth4_plus_ns += ns;
                    }
                    Kind::NestedFnBodyBoundNames => {
                        stats.nested_fn_body_bound_names_count += 1;
                        stats.nested_fn_body_bound_names_ns += ns;
                    }
                    Kind::NestedFnBodyClose => {
                        stats.nested_fn_body_close_count += 1;
                        stats.nested_fn_body_close_ns += ns;
                    }
                    Kind::NestedFnBodyRestore => {
                        stats.nested_fn_body_restore_count += 1;
                        stats.nested_fn_body_restore_ns += ns;
                    }
                    Kind::NestedFnBodyDesugar => {
                        stats.nested_fn_body_desugar_count += 1;
                        stats.nested_fn_body_desugar_ns += ns;
                    }
                    Kind::NestedAssignAsync => {
                        stats.nested_assign_async_count += 1;
                        stats.nested_assign_async_ns += ns;
                    }
                    Kind::NestedAssignArrow => {
                        stats.nested_assign_arrow_count += 1;
                        stats.nested_assign_arrow_ns += ns;
                    }
                    Kind::NestedAssignConditional => {
                        stats.nested_assign_conditional_count += 1;
                        stats.nested_assign_conditional_ns += ns;
                        stats.nested_assign_conditional_exclusive_ns += exclusive_ns;
                    }
                    Kind::NestedAssignRhs => {
                        stats.nested_assign_rhs_count += 1;
                        stats.nested_assign_rhs_ns += ns;
                    }
                    Kind::NestedAssignCover => {
                        stats.nested_assign_cover_count += 1;
                        stats.nested_assign_cover_ns += ns;
                    }
                }
            });
        }
    }
}

pub struct Parser<'src> {
    src: &'src str,
    lx: Lexer<'src>,

    lookahead: Token,

    pub(crate) function_body_depth: u32,

    pub(crate) tagged_template_call_ends: std::collections::HashSet<usize>,

    pub(crate) strict_mode: bool,

    pub(crate) in_generator: bool,

    pub(crate) in_async: bool,

    pub(crate) last_body_became_strict: bool,

    pub(crate) in_function_params: bool,

    pub(crate) current_lex_goal: LexerGoal,

    pub(crate) lookahead_after_dot: bool,

    pub(crate) pending_fn_body_close_regexp: bool,

    pub(crate) in_disallowed: bool,

    pub(crate) allow_cover_initialized_name_in_for_head: bool,

    pub(crate) allow_annex_b_function_in_substatement: bool,

    pub(crate) parse_goal: ParseGoal,

    pub(crate) saw_using_declaration: bool,

    pub(crate) parse_depth: usize,
}

impl<'src> Parser<'src> {
    pub(crate) const MAX_PARSE_DEPTH: usize = 256;

    pub fn new(src: &'src str) -> Result<Self, ParseError> {
        Self::new_with_strict(src, false)
    }

    pub fn new_with_strict(src: &'src str, force_strict: bool) -> Result<Self, ParseError> {

        Self::new_with_strict_and_goal(src, force_strict, ParseGoal::Script)
    }

    pub fn new_with_strict_and_goal(
        src: &'src str,
        force_strict: bool,
        parse_goal: ParseGoal,
    ) -> Result<Self, ParseError> {

        let force_strict = force_strict || matches!(parse_goal, ParseGoal::Module);
        let mut lx = Lexer::new(src);
        if force_strict {
            lx.set_strict(true);
        }

        lx.set_script_goal(matches!(parse_goal, ParseGoal::Script));
        let lookahead = lx.next_token(LexerGoal::RegExp).map_err(lex_to_parse)?;

        let initial_await_is_operator =
            matches!(parse_goal, ParseGoal::Module | ParseGoal::Untagged);
        let current_lex_goal = derive_lex_goal_after_in_context(
            &lookahead.kind,
            force_strict,
            false,
            initial_await_is_operator,
        );
        Ok(Self {
            src,
            lx,
            lookahead,
            function_body_depth: 0,
            tagged_template_call_ends: std::collections::HashSet::new(),
            strict_mode: force_strict,
            in_generator: false,
            in_async: false,
            last_body_became_strict: false,
            in_function_params: false,
            current_lex_goal,
            lookahead_after_dot: false,
            pending_fn_body_close_regexp: false,
            in_disallowed: false,
            allow_cover_initialized_name_in_for_head: false,
            allow_annex_b_function_in_substatement: false,
            parse_goal,
            saw_using_declaration: false,
            parse_depth: 0,
        })
    }

    pub(crate) fn is_module_goal(&self) -> bool {
        matches!(self.parse_goal, ParseGoal::Module)
    }

    pub(crate) fn is_script_goal(&self) -> bool {
        matches!(self.parse_goal, ParseGoal::Script)
    }

    pub(crate) fn goal_allows_top_level_await(&self) -> bool {
        matches!(self.parse_goal, ParseGoal::Module | ParseGoal::Untagged)
    }

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        let _profile_root = parse_profile::RootGuard::new(self.src.len());
        let _profile = parse_profile::Guard::new(parse_profile::Kind::Module);
        let module_start = self.lookahead.span.start;
        let mut body: Vec<ModuleItem> = Vec::new();
        let mut import_entries: Vec<ImportEntry> = Vec::new();
        let mut local_export_entries: Vec<ExportEntry> = Vec::new();
        let mut indirect_export_entries: Vec<ExportEntry> = Vec::new();
        let mut star_export_entries: Vec<ExportEntry> = Vec::new();

        if self.peek_use_strict_directive() {
            self.strict_mode = true;
            self.lx.set_strict(true);

            if self.lx.last_string_had_legacy_escape {
                return Err(self.err_here(
                    "legacy octal/non-octal escape sequence in strict-mode string literal".into(),
                ));
            }
        }

        let reject_module_syntax = self.is_script_goal();

        while !self.at_eof() {

            if matches!(self.lookahead.kind, TokenKind::Hashbang(_)) {
                self.bump_regexp()?;
                continue;
            }
            if self.is_ident("import") && !self.is_dynamic_import_call_after_import() {
                if self.next_token_text_is("typeof") {
                    return Err(self.err_here("Unexpected token 'typeof'".into()));
                }
                if reject_module_syntax {
                    return Err(self.err_here(
                        "import declarations may only appear at the top level of a module".into(),
                    ));
                }
                self.reject_escaped_contextual_keyword("import")?;
                let decl = self.parse_import_declaration()?;
                self.collect_import_entries(&decl, &mut import_entries);
                body.push(ModuleItem::Import(decl));
                continue;
            }
            if self.is_ident("export") {
                if reject_module_syntax {
                    return Err(self.err_here(
                        "export declarations may only appear at the top level of a module".into(),
                    ));
                }
                self.reject_escaped_contextual_keyword("export")?;
                let decl = self.parse_export_declaration()?;
                self.collect_export_entries(
                    &decl,
                    &mut local_export_entries,
                    &mut indirect_export_entries,
                    &mut star_export_entries,
                );
                body.push(ModuleItem::Export(decl));
                continue;
            }
            if reject_module_syntax
                && self.is_ident("using")
                && self.using_starts_declaration(self.lookahead_span().end)
            {
                return Err(self
                    .err_here("`using` declarations are not allowed at Script top level".into()));
            }
            if reject_module_syntax
                && self.is_ident("await")
                && self.await_using_starts_declaration()
            {
                return Err(self.err_here(
                    "`await using` declarations are not allowed at Script top level".into(),
                ));
            }

            let stmt = self.parse_statement()?;
            body.push(ModuleItem::Statement(stmt));
        }

        if self.is_module_goal() {
            self.check_module_bound_names(&body)?;
            self.check_module_import_export_bound_names(
                &body,
                &import_entries,
                &local_export_entries,
            )?;
            if let Some(span) = Self::module_items_new_target_span(&body) {
                return Err(ParseError {
                    span,
                    message: "`new.target` is not allowed at module top level".into(),
                });
            }
            crate::private_names_valid::validate_all_private_names(
                &Module {
                    span: Span::new(module_start, self.lookahead.span.start),
                    body: body.clone(),
                    import_entries: import_entries.clone(),
                    local_export_entries: local_export_entries.clone(),
                    indirect_export_entries: indirect_export_entries.clone(),
                    star_export_entries: star_export_entries.clone(),
                },
                self.src,
            )?;
            if let Some(span) = Self::module_items_duplicate_label_span(&body) {
                return Err(ParseError {
                    span,
                    message: "module body contains duplicate label".into(),
                });
            }
        }

        {
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for entry in local_export_entries
                .iter()
                .chain(indirect_export_entries.iter())
            {
                if let Some(name) = entry.export_name.as_deref() {
                    if !seen.insert(name) {
                        return Err(ParseError {
                            span: Span::new(module_start, self.lookahead.span.start),
                            message: format!("Duplicate export name '{}'", name),
                        });
                    }
                }
            }
        }

        self.check_top_level_bound_names(&body)?;

        let body = if self.saw_using_declaration {
            let first_using = body.iter().position(|it| {
                matches!(it, ModuleItem::Statement(Stmt::Variable(v))
                    if matches!(v.kind, VariableKind::Using | VariableKind::AwaitUsing))
            });

            let tail_is_desugarable = |items: &[ModuleItem]| {
                items.iter().all(|it| {
                    matches!(it, ModuleItem::Statement(_))
                        || matches!(
                            it,
                            ModuleItem::Export(rusty_js_ast::ExportDeclaration::Named {
                                source: None,
                                ..
                            })
                        )
                })
            };
            match first_using {
                Some(idx) if tail_is_desugarable(&body[idx..]) => {
                    let mut out: Vec<ModuleItem> = body[..idx].to_vec();
                    let mut clauses: Vec<ModuleItem> = Vec::new();
                    let mut tail: Vec<Stmt> = Vec::new();
                    for it in &body[idx..] {
                        match it {
                            ModuleItem::Statement(s) => tail.push(s.clone()),
                            other => clauses.push(other.clone()),
                        }
                    }
                    out.extend(clauses);
                    out.extend(
                        crate::stmt::desugar_using_block(tail)
                            .into_iter()
                            .map(ModuleItem::Statement),
                    );
                    out
                }
                _ => body,
            }
        } else {
            body
        };

        Ok(Module {
            span: Span::new(module_start, self.lookahead.span.start),
            body,
            import_entries,
            local_export_entries,
            indirect_export_entries,
            star_export_entries,
        })
    }

    fn check_module_bound_names(&self, body: &[ModuleItem]) -> Result<(), ParseError> {
        use std::collections::{HashMap, HashSet};

        let mut lexical: HashMap<String, Span> = HashMap::new();
        let mut lexical_dups: HashSet<String> = HashSet::new();
        let mut vars: HashMap<String, Span> = HashMap::new();

        fn add_name(
            map: &mut HashMap<String, Span>,
            dups: Option<&mut HashSet<String>>,
            name: &BindingIdentifier,
        ) {
            if map.contains_key(&name.name) {
                if let Some(dups) = dups {
                    dups.insert(name.name.clone());
                }
            } else {
                map.insert(name.name.clone(), name.span);
            }
        }

        fn collect_stmt_names(
            stmt: &Stmt,
            lexical: &mut HashMap<String, Span>,
            lexical_dups: &mut HashSet<String>,
            vars: &mut HashMap<String, Span>,
        ) {
            match stmt {
                Stmt::FunctionDecl { name: Some(n), .. }
                | Stmt::ClassDecl { name: Some(n), .. } => {
                    add_name(lexical, Some(lexical_dups), n);
                }
                Stmt::Variable(v) => {
                    for decl in &v.declarators {
                        for id in decl.target.collect_names() {
                            if matches!(v.kind, VariableKind::Var) {
                                vars.entry(id.name.clone()).or_insert(id.span);
                            } else {
                                add_name(lexical, Some(lexical_dups), &id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        for item in body {
            match item {
                ModuleItem::Import(imp) => {
                    if let Some(n) = &imp.default_binding {
                        add_name(&mut lexical, Some(&mut lexical_dups), n);
                    }
                    if let Some(n) = &imp.namespace_binding {
                        add_name(&mut lexical, Some(&mut lexical_dups), n);
                    }
                    for spec in &imp.named_imports {
                        add_name(&mut lexical, Some(&mut lexical_dups), &spec.local);
                    }
                }
                ModuleItem::Export(ExportDeclaration::Default { body, .. }) => match body {
                    DefaultExportBody::HoistableFunction { name: Some(n), .. }
                    | DefaultExportBody::Class { name: Some(n), .. } => {
                        add_name(&mut lexical, Some(&mut lexical_dups), n);
                    }
                    _ => {}
                },
                ModuleItem::Export(ExportDeclaration::Declaration {
                    decl_stmt: Some(stmt),
                    ..
                }) => {
                    collect_stmt_names(stmt, &mut lexical, &mut lexical_dups, &mut vars);
                }
                ModuleItem::Statement(stmt) => {
                    collect_stmt_names(stmt, &mut lexical, &mut lexical_dups, &mut vars);
                }
                _ => {}
            }
        }

        if let Some(name) = lexical_dups.iter().next() {
            let span = lexical.get(name).copied().unwrap_or(Span::new(0, 0));
            return Err(ParseError {
                span,
                message: format!(
                    "Identifier `{}` has already been declared in this module",
                    name
                ),
            });
        }
        for (name, lex_span) in lexical {
            if vars.contains_key(&name) {
                return Err(ParseError {
                    span: lex_span,
                    message: format!(
                        "Identifier `{}` cannot be redeclared (module lexical/var conflict)",
                        name
                    ),
                });
            }
        }
        Ok(())
    }

    pub fn module_items_new_target_span(body: &[ModuleItem]) -> Option<Span> {
        fn expr_new_target_span(expr: &Expr) -> Option<Span> {
            match expr {
                Expr::MetaProperty {
                    meta,
                    property,
                    span,
                } if meta == "new" && property == "target" => Some(*span),
                Expr::Member {
                    object, property, ..
                } => expr_new_target_span(object).or_else(|| match property.as_ref() {
                    rusty_js_ast::MemberProperty::Computed { expr, .. } => {
                        expr_new_target_span(expr)
                    }
                    _ => None,
                }),
                Expr::Call {
                    callee, arguments, ..
                }
                | Expr::New {
                    callee, arguments, ..
                } => expr_new_target_span(callee).or_else(|| {
                    arguments.iter().find_map(|arg| match arg {
                        rusty_js_ast::Argument::Expr(e)
                        | rusty_js_ast::Argument::Spread { expr: e, .. } => expr_new_target_span(e),
                    })
                }),
                Expr::Parenthesized { expr, .. }
                | Expr::Update { argument: expr, .. }
                | Expr::Unary { argument: expr, .. } => expr_new_target_span(expr),
                Expr::Binary { left, right, .. }
                | Expr::Assign {
                    target: left,
                    value: right,
                    ..
                } => expr_new_target_span(left).or_else(|| expr_new_target_span(right)),
                Expr::Conditional {
                    test,
                    consequent,
                    alternate,
                    ..
                } => expr_new_target_span(test)
                    .or_else(|| expr_new_target_span(consequent))
                    .or_else(|| expr_new_target_span(alternate)),
                Expr::Sequence { expressions, .. } | Expr::TemplateLiteral { expressions, .. } => {
                    expressions.iter().find_map(expr_new_target_span)
                }
                Expr::Array { elements, .. } => elements.iter().find_map(|el| match el {
                    rusty_js_ast::ArrayElement::Expr(e)
                    | rusty_js_ast::ArrayElement::Spread { expr: e, .. } => expr_new_target_span(e),
                    rusty_js_ast::ArrayElement::Elision { .. } => None,
                }),
                Expr::Object { properties, .. } => properties.iter().find_map(|prop| match prop {
                    rusty_js_ast::ObjectProperty::Property { key, value, .. } => {
                        let key_hit = match key {
                            rusty_js_ast::ObjectKey::Computed { expr, .. } => {
                                expr_new_target_span(expr)
                            }
                            _ => None,
                        };
                        key_hit.or_else(|| expr_new_target_span(value))
                    }
                    rusty_js_ast::ObjectProperty::Spread { expr, .. } => expr_new_target_span(expr),
                }),
                Expr::Class {
                    super_class,
                    members,
                    ..
                } => super_class
                    .as_deref()
                    .and_then(expr_new_target_span)
                    .or_else(|| {
                        members.iter().find_map(|member| match member {
                            rusty_js_ast::ClassMember::Field { name, init, .. } => {
                                let name_hit = match name {
                                    rusty_js_ast::ClassMemberName::Computed { expr, .. } => {
                                        expr_new_target_span(expr)
                                    }
                                    _ => None,
                                };
                                name_hit.or_else(|| init.as_ref().and_then(expr_new_target_span))
                            }
                            rusty_js_ast::ClassMember::Method { name, .. } => match name {
                                rusty_js_ast::ClassMemberName::Computed { expr, .. } => {
                                    expr_new_target_span(expr)
                                }
                                _ => None,
                            },
                            rusty_js_ast::ClassMember::StaticBlock { .. } => None,
                        })
                    }),

                Expr::Function { .. } => None,
                Expr::Arrow { body, .. } => match body {
                    rusty_js_ast::ArrowBody::Expression(e) => expr_new_target_span(e),
                    rusty_js_ast::ArrowBody::Block(stmts) => stmts_new_target_span(stmts),
                },
                _ => None,
            }
        }

        fn stmt_new_target_span(stmt: &Stmt) -> Option<Span> {
            match stmt {
                Stmt::Expression { expr, .. } => expr_new_target_span(expr),
                Stmt::Variable(vs) => vs
                    .declarators
                    .iter()
                    .filter_map(|d| d.init.as_ref())
                    .find_map(expr_new_target_span),
                Stmt::Block { body, .. } => stmts_new_target_span(body),
                Stmt::ClassDecl {
                    super_class,
                    members,
                    ..
                } => {
                    let super_hit = super_class.as_ref().and_then(expr_new_target_span);
                    super_hit.or_else(|| {
                        members.iter().find_map(|member| match member {
                            rusty_js_ast::ClassMember::Field { name, init, .. } => {
                                let name_hit = match name {
                                    rusty_js_ast::ClassMemberName::Computed { expr, .. } => {
                                        expr_new_target_span(expr)
                                    }
                                    _ => None,
                                };
                                name_hit.or_else(|| init.as_ref().and_then(expr_new_target_span))
                            }
                            rusty_js_ast::ClassMember::Method { name, .. } => match name {
                                rusty_js_ast::ClassMemberName::Computed { expr, .. } => {
                                    expr_new_target_span(expr)
                                }
                                _ => None,
                            },
                            rusty_js_ast::ClassMember::StaticBlock { .. } => None,
                        })
                    })
                }
                Stmt::If {
                    test,
                    consequent,
                    alternate,
                    ..
                } => expr_new_target_span(test)
                    .or_else(|| stmt_new_target_span(consequent))
                    .or_else(|| alternate.as_deref().and_then(stmt_new_target_span)),
                Stmt::For {
                    init,
                    test,
                    update,
                    body,
                    ..
                } => init
                    .as_ref()
                    .and_then(|init| match init {
                        rusty_js_ast::ForInit::Expression(e) => expr_new_target_span(e),
                        rusty_js_ast::ForInit::Variable(vs) => vs
                            .declarators
                            .iter()
                            .filter_map(|d| d.init.as_ref())
                            .find_map(expr_new_target_span),
                    })
                    .or_else(|| test.as_ref().and_then(expr_new_target_span))
                    .or_else(|| update.as_ref().and_then(expr_new_target_span))
                    .or_else(|| stmt_new_target_span(body)),
                Stmt::ForIn { right, body, .. } | Stmt::ForOf { right, body, .. } => {
                    expr_new_target_span(right).or_else(|| stmt_new_target_span(body))
                }
                Stmt::While { test, body, .. } | Stmt::DoWhile { test, body, .. } => {
                    expr_new_target_span(test).or_else(|| stmt_new_target_span(body))
                }
                Stmt::With { object, body, .. } => {
                    expr_new_target_span(object).or_else(|| stmt_new_target_span(body))
                }
                Stmt::Switch {
                    discriminant,
                    cases,
                    ..
                } => expr_new_target_span(discriminant).or_else(|| {
                    cases.iter().find_map(|case| {
                        case.test
                            .as_ref()
                            .and_then(expr_new_target_span)
                            .or_else(|| stmts_new_target_span(&case.consequent))
                    })
                }),
                Stmt::Try {
                    block,
                    handler,
                    finalizer,
                    ..
                } => stmt_new_target_span(block)
                    .or_else(|| {
                        handler
                            .as_ref()
                            .and_then(|handler| stmt_new_target_span(&handler.body))
                    })
                    .or_else(|| finalizer.as_deref().and_then(stmt_new_target_span)),
                Stmt::Return { argument, .. } => argument.as_ref().and_then(expr_new_target_span),
                Stmt::Throw { argument, .. } => expr_new_target_span(argument),
                Stmt::Labelled { body, .. } => stmt_new_target_span(body),
                Stmt::FunctionDecl { .. } => None,
                _ => None,
            }
        }

        fn stmts_new_target_span(stmts: &[Stmt]) -> Option<Span> {
            stmts.iter().find_map(stmt_new_target_span)
        }

        for item in body {
            let span = match item {
                ModuleItem::Statement(stmt) => stmt_new_target_span(stmt),
                ModuleItem::Export(ExportDeclaration::Declaration {
                    decl_stmt: Some(stmt),
                    ..
                }) => stmt_new_target_span(stmt),
                ModuleItem::Export(ExportDeclaration::Default { body, .. }) => match body {
                    DefaultExportBody::Expression { expr } => expr_new_target_span(expr),
                    DefaultExportBody::Class {
                        super_class,
                        members,
                        ..
                    } => super_class
                        .as_ref()
                        .and_then(expr_new_target_span)
                        .or_else(|| {
                            members.iter().find_map(|member| match member {
                                rusty_js_ast::ClassMember::Field { name, init, .. } => {
                                    let name_hit = match name {
                                        rusty_js_ast::ClassMemberName::Computed {
                                            expr, ..
                                        } => expr_new_target_span(expr),
                                        _ => None,
                                    };
                                    name_hit
                                        .or_else(|| init.as_ref().and_then(expr_new_target_span))
                                }
                                rusty_js_ast::ClassMember::Method { name, .. } => match name {
                                    rusty_js_ast::ClassMemberName::Computed { expr, .. } => {
                                        expr_new_target_span(expr)
                                    }
                                    _ => None,
                                },
                                rusty_js_ast::ClassMember::StaticBlock { .. } => None,
                            })
                        }),
                    DefaultExportBody::HoistableFunction { .. } => None,
                },
                _ => None,
            };
            if span.is_some() {
                return span;
            }
        }
        None
    }

    fn check_module_import_export_bound_names(
        &self,
        body: &[ModuleItem],
        imports: &[ImportEntry],
        local_exports: &[ExportEntry],
    ) -> Result<(), ParseError> {
        use std::collections::HashSet;

        let mut bound = HashSet::new();

        fn collect_stmt_bound_names(stmt: &Stmt, out: &mut HashSet<String>) {
            match stmt {
                Stmt::FunctionDecl { name: Some(n), .. }
                | Stmt::ClassDecl { name: Some(n), .. } => {
                    out.insert(n.name.clone());
                }
                Stmt::Variable(v) => {
                    for decl in &v.declarators {
                        for id in decl.target.collect_names() {
                            out.insert(id.name.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        for item in body {
            match item {
                ModuleItem::Import(imp) => {
                    if let Some(n) = &imp.default_binding {
                        bound.insert(n.name.clone());
                    }
                    if let Some(n) = &imp.namespace_binding {
                        bound.insert(n.name.clone());
                    }
                    for spec in &imp.named_imports {
                        bound.insert(spec.local.name.clone());
                    }
                }
                ModuleItem::Export(ExportDeclaration::Default { body, .. }) => match body {
                    DefaultExportBody::HoistableFunction { name: Some(n), .. }
                    | DefaultExportBody::Class { name: Some(n), .. } => {
                        bound.insert(n.name.clone());
                    }
                    _ => {}
                },
                ModuleItem::Export(ExportDeclaration::Declaration {
                    decl_stmt: Some(stmt),
                    ..
                }) => collect_stmt_bound_names(stmt, &mut bound),
                ModuleItem::Statement(stmt) => collect_stmt_bound_names(stmt, &mut bound),
                _ => {}
            }
        }

        for entry in imports {
            if entry.local_name == "eval" || entry.local_name == "arguments" {
                return Err(ParseError {
                    span: Span::new(0, 0),
                    message: format!(
                        "Import binding `{}` is not allowed in module code",
                        entry.local_name
                    ),
                });
            }
        }

        for entry in local_exports {
            let Some(local_name) = entry.local_name.as_deref() else {
                continue;
            };
            if local_name.starts_with('*') {
                continue;
            }
            if !bound.contains(local_name) {
                return Err(ParseError {
                    span: Span::new(0, 0),
                    message: format!("Exported local binding `{local_name}` is not declared"),
                });
            }
        }

        Ok(())
    }

    fn module_items_duplicate_label_span(body: &[ModuleItem]) -> Option<Span> {
        use std::collections::HashSet;

        fn stmt_duplicate_label_span(stmt: &Stmt, labels: &mut HashSet<String>) -> Option<Span> {
            match stmt {
                Stmt::Labelled { label, body, .. } => {
                    if !labels.insert(label.name.clone()) {
                        return Some(label.span);
                    }
                    stmt_duplicate_label_span(body, labels)
                }
                Stmt::Block { body, .. } => stmts_duplicate_label_span(body, labels),
                Stmt::If {
                    consequent,
                    alternate,
                    ..
                } => stmt_duplicate_label_span(consequent, labels).or_else(|| {
                    alternate
                        .as_deref()
                        .and_then(|stmt| stmt_duplicate_label_span(stmt, labels))
                }),
                _ => None,
            }
        }

        fn stmts_duplicate_label_span(stmts: &[Stmt], labels: &HashSet<String>) -> Option<Span> {

            for stmt in stmts {
                let mut local = labels.clone();
                if let Some(span) = stmt_duplicate_label_span(stmt, &mut local) {
                    return Some(span);
                }
            }
            None
        }

        let labels = HashSet::new();
        for item in body {
            if let ModuleItem::Statement(stmt) = item {
                let mut local = labels.clone();
                if let Some(span) = stmt_duplicate_label_span(stmt, &mut local) {
                    return Some(span);
                }
            }
        }
        None
    }

    fn parse_import_declaration(&mut self) -> Result<ImportDeclaration, ParseError> {
        let start = self.lookahead.span.start;
        self.expect_keyword("import")?;

        let mut source_phase = false;
        let mut import_defer = false;
        let mut default_binding: Option<BindingIdentifier> = None;
        let mut namespace_binding: Option<BindingIdentifier> = None;
        let mut named_imports: Vec<ImportSpecifier> = Vec::new();
        let mut specifier_required = true;

        if matches!(self.lookahead.kind, TokenKind::String(_)) {
            let specifier = self.parse_module_specifier()?;
            let attributes = self.parse_optional_attributes()?;
            self.consume_semicolon()?;
            return Ok(ImportDeclaration {
                span: Span::new(start, self.last_span_end()),
                source_phase: false,
                import_defer: false,
                specifier,
                default_binding: None,
                namespace_binding: None,
                named_imports: vec![],
                attributes,
            });
        }

        if self.is_ident("source")
            && (!self.next_token_text_is("from")
                || self.next_token_text_sequence_is("from", "from"))
        {
            source_phase = true;
            self.bump_regexp()?;
        }

        if self.is_ident("defer") && self.next_token_text_is("*") {
            import_defer = true;
            self.bump_regexp()?;
        }

        if !import_defer {
            if let TokenKind::Ident(name) = &self.lookahead.kind {
                if name != "*" && name != "{" {
                    default_binding = Some(self.parse_binding_identifier()?);
                    if self.is_punct(Punct::Comma) {
                        self.bump_regexp()?;
                    } else {
                        specifier_required = true;
                    }
                }
            }
        }

        if self.is_punct(Punct::Star) {
            self.bump_regexp()?;
            self.reject_escaped_contextual_keyword("as")?;
            self.expect_ident("as")?;
            namespace_binding = Some(self.parse_binding_identifier()?);
        } else if self.is_punct(Punct::LBrace) {
            named_imports = self.parse_named_imports()?;
        }

        let _ = specifier_required;
        self.reject_escaped_contextual_keyword("from")?;
        self.expect_ident("from")?;
        let specifier = self.parse_module_specifier()?;
        let attributes = self.parse_optional_attributes()?;
        self.consume_semicolon()?;

        Ok(ImportDeclaration {
            span: Span::new(start, self.last_span_end()),
            source_phase,
            import_defer,
            specifier,
            default_binding,
            namespace_binding,
            named_imports,
            attributes,
        })
    }

    fn parse_named_imports(&mut self) -> Result<Vec<ImportSpecifier>, ParseError> {
        self.expect_punct(Punct::LBrace)?;
        let mut out = Vec::new();
        while !self.is_punct(Punct::RBrace) {
            let start = self.lookahead.span.start;
            let imported = self.parse_module_export_name()?;
            let local: BindingIdentifier = if self.is_ident("as") {
                self.reject_escaped_contextual_keyword("as")?;
                self.bump_regexp()?;
                self.parse_binding_identifier()?
            } else {

                match &imported {
                    ModuleExportName::Ident(b) => b.clone(),
                    ModuleExportName::String { span, .. } => {
                        return Err(self.err_at(
                            *span,
                            "string-literal imported name requires `as Local`".into(),
                        ));
                    }
                }
            };
            let end = self.last_span_end();
            out.push(ImportSpecifier {
                span: Span::new(start, end),
                imported,
                local,
            });
            if self.is_punct(Punct::Comma) {
                self.bump_regexp()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RBrace)?;
        Ok(out)
    }

    pub(crate) fn is_dynamic_import_call_after_import(&mut self) -> bool {

        let p = Self::skip_ws_and_comments_allow_lt(self.src.as_bytes(), self.lookahead.span.end);
        matches!(self.src.as_bytes().get(p), Some(b'(') | Some(b'.'))
    }

    pub(crate) fn next_non_ws_after_current_is_dot(&self) -> bool {
        let p = Self::skip_ws_and_comments_allow_lt(self.src.as_bytes(), self.lookahead.span.end);
        matches!(self.src.as_bytes().get(p), Some(b'.'))
    }

    pub(crate) fn next_after_current_is_dot_meta(&self) -> bool {
        let bytes = self.src.as_bytes();
        let mut p = Self::skip_ws_and_comments_allow_lt(bytes, self.lookahead.span.end);
        if bytes.get(p) != Some(&b'.') {
            return false;
        }
        p += 1;
        p = Self::skip_ws_and_comments_allow_lt(bytes, p);
        if !self.src[p..].starts_with("meta") {
            return false;
        }

        matches!(
            bytes.get(p + 4),
            None | Some(b'.')
                | Some(b'(')
                | Some(b')')
                | Some(b';')
                | Some(b' ')
                | Some(b'\t')
                | Some(b'\n')
                | Some(b'\r')
                | Some(b'[')
                | Some(b',')
        )
    }

    fn parse_export_declaration(&mut self) -> Result<ExportDeclaration, ParseError> {
        let start = self.lookahead.span.start;
        self.expect_keyword("export")?;

        if self.is_ident("default") {
            self.reject_escaped_contextual_keyword("default")?;
            self.bump_regexp()?;
            let body = self.parse_default_export_body()?;
            return Ok(ExportDeclaration::Default {
                span: Span::new(start, self.last_span_end()),
                body,
            });
        }
        if self.is_ident("defer") {
            return Err(self.err_here("`export defer` is not valid".into()));
        }

        if self.is_punct(Punct::Star) {
            self.bump_regexp()?;
            if self.is_ident("as") {
                self.reject_escaped_contextual_keyword("as")?;
                self.bump_regexp()?;
                let exported = self.parse_module_export_name()?;
                self.reject_escaped_contextual_keyword("from")?;
                self.expect_ident("from")?;
                let source = self.parse_module_specifier()?;
                let attributes = self.parse_optional_attributes()?;
                self.consume_semicolon()?;
                return Ok(ExportDeclaration::StarAsFrom {
                    span: Span::new(start, self.last_span_end()),
                    exported,
                    source,
                    attributes,
                });
            }
            self.reject_escaped_contextual_keyword("from")?;
            self.expect_ident("from")?;
            let source = self.parse_module_specifier()?;
            let attributes = self.parse_optional_attributes()?;
            self.consume_semicolon()?;
            return Ok(ExportDeclaration::StarFrom {
                span: Span::new(start, self.last_span_end()),
                source,
                attributes,
            });
        }

        if self.is_punct(Punct::LBrace) {
            let specifiers = self.parse_named_exports()?;
            let source = if self.is_ident("from") {
                self.reject_escaped_contextual_keyword("from")?;
                self.bump_regexp()?;
                Some(self.parse_module_specifier()?)
            } else {
                None
            };
            if source.is_none() {
                if let Some(spec) = specifiers
                    .iter()
                    .find(|spec| matches!(spec.local, ModuleExportName::String { .. }))
                {
                    return Err(ParseError {
                        span: spec.span,
                        message: "local export binding name cannot be a string literal".into(),
                    });
                }
            }
            let attributes = if source.is_some() {
                self.parse_optional_attributes()?
            } else {
                vec![]
            };
            self.consume_semicolon()?;
            return Ok(ExportDeclaration::Named {
                span: Span::new(start, self.last_span_end()),
                specifiers,
                source,
                attributes,
            });
        }

        if matches!(self.lookahead.kind, TokenKind::String(_)) {
            let spec_start = self.lookahead.span.start;
            let local = self.parse_module_export_name()?;
            self.reject_escaped_contextual_keyword("as")?;
            self.expect_ident("as")?;
            let exported = self.parse_module_export_name()?;
            self.reject_escaped_contextual_keyword("from")?;
            self.expect_ident("from")?;
            let source = self.parse_module_specifier()?;
            let attributes = self.parse_optional_attributes()?;
            self.consume_semicolon()?;
            return Ok(ExportDeclaration::Named {
                span: Span::new(start, self.last_span_end()),
                specifiers: vec![ExportSpecifier {
                    span: Span::new(spec_start, self.last_span_end()),
                    local,
                    exported,
                }],
                source: Some(source),
                attributes,
            });
        }

        let decl_start = self.lookahead.span.start;
        let (decl_span, names, decl_stmt) = self.parse_declaration_for_export()?;
        Ok(ExportDeclaration::Declaration {
            span: Span::new(start, decl_span.end),
            decl_span: Span::new(decl_start, decl_span.end),
            names,
            decl_stmt,
        })
    }

    fn parse_named_exports(&mut self) -> Result<Vec<ExportSpecifier>, ParseError> {
        self.expect_punct(Punct::LBrace)?;
        let mut out = Vec::new();
        while !self.is_punct(Punct::RBrace) {
            let start = self.lookahead.span.start;
            let local = self.parse_module_export_name()?;
            let exported: ModuleExportName = if self.is_ident("as") {
                self.reject_escaped_contextual_keyword("as")?;
                self.bump_regexp()?;
                self.parse_module_export_name()?
            } else {
                local.clone()
            };
            out.push(ExportSpecifier {
                span: Span::new(start, self.last_span_end()),
                local,
                exported,
            });
            if self.is_punct(Punct::Comma) {
                self.bump_regexp()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RBrace)?;
        Ok(out)
    }

    fn parse_default_export_body(&mut self) -> Result<DefaultExportBody, ParseError> {

        if self.is_ident("function") {
            return self.parse_default_function(false);
        }
        if self.is_ident("async") {

            let mut p = self.lookahead.span.end;
            while p < self.src.len() && self.src.as_bytes()[p].is_ascii_whitespace() {
                p += 1;
            }
            if Self::bytes_at_identifier_keyword(self.src.as_bytes(), p, "function") {
                self.reject_escaped_contextual_keyword("async")?;
                self.bump_regexp()?;
                return self.parse_default_function(true);
            }
        }
        if self.is_ident("class") {
            return self.parse_default_class();
        }
        if self.is_ident("const") || self.is_ident("let") || self.is_ident("var") {
            return Err(
                self.err_here("export default cannot be followed by a variable declaration".into())
            );
        }

        let expr = self.parse_assignment_expression()?;
        self.consume_semicolon()?;
        Ok(DefaultExportBody::Expression { expr })
    }

    fn parse_default_function(&mut self, is_async: bool) -> Result<DefaultExportBody, ParseError> {
        self.expect_keyword("function")?;
        let is_generator = if self.is_punct(Punct::Star) {
            self.bump_regexp()?;
            true
        } else {
            false
        };
        let name = if matches!(self.lookahead.kind, TokenKind::Ident(_))
            && !self.is_punct(Punct::LParen)
        {
            Some(self.parse_binding_identifier()?)
        } else {
            None
        };
        let params = self.parse_function_parameters_ga(is_generator, is_async)?;
        let body = self.parse_function_body_gs(
            Some(is_generator),
            Some(is_async),
            Self::is_simple_param_list(&params),
        )?;
        Ok(DefaultExportBody::HoistableFunction {
            name,
            params,
            body,
            is_async,
            is_generator,
        })
    }

    fn parse_default_class(&mut self) -> Result<DefaultExportBody, ParseError> {
        self.expect_keyword("class")?;
        let name = if matches!(self.lookahead.kind, TokenKind::Ident(ref s) if s != "extends" && s != "{")
            && !self.is_punct(Punct::LBrace)
        {
            Some(self.parse_binding_identifier()?)
        } else {
            None
        };
        let super_class = if self.is_ident("extends") {
            self.bump_regexp()?;
            Some(self.parse_left_hand_side_expression()?)
        } else {
            None
        };
        let members = self.parse_class_body()?;
        Ok(DefaultExportBody::Class {
            name,
            super_class,
            members,
        })
    }

    fn parse_module_specifier(&mut self) -> Result<ModuleSpecifier, ParseError> {
        let tok = self.lookahead.clone();
        match &tok.kind {
            TokenKind::String(s) => {
                self.bump_regexp()?;
                Ok(ModuleSpecifier {
                    value: s.clone(),
                    span: tok.span,
                })
            }
            _ => Err(self.err_here("expected module specifier (string literal)".into())),
        }
    }

    fn parse_module_export_name(&mut self) -> Result<ModuleExportName, ParseError> {
        let tok = self.lookahead.clone();
        match &tok.kind {
            TokenKind::Ident(name) => {
                self.bump_regexp()?;
                Ok(ModuleExportName::Ident(BindingIdentifier {
                    name: name.clone(),
                    span: tok.span,
                }))
            }
            TokenKind::String(s) => {
                if !self.string_literal_span_is_well_formed_unicode(tok.span) {
                    return Err(ParseError {
                        span: tok.span,
                        message: "module export string name must be well-formed Unicode".into(),
                    });
                }
                self.bump_regexp()?;
                Ok(ModuleExportName::String {
                    value: s.clone(),
                    span: tok.span,
                })
            }
            _ => Err(self.err_here("expected identifier or string literal".into())),
        }
    }

    fn string_literal_span_is_well_formed_unicode(&self, span: Span) -> bool {
        let raw = self.src.get(span.start..span.end).unwrap_or("");
        let bytes = raw.as_bytes();
        if bytes.len() < 2 {
            return true;
        }
        let mut i = 1;
        let end = bytes.len().saturating_sub(1);
        let mut pending_high = false;
        while i < end {
            if bytes[i] != b'\\' {
                if pending_high {
                    return false;
                }
                if let Some(ch) = raw[i..end].chars().next() {
                    i += ch.len_utf8();
                } else {
                    return false;
                }
                continue;
            }
            i += 1;
            if i >= end {
                return false;
            }
            if bytes[i] != b'u' {
                if pending_high {
                    return false;
                }
                i += 1;
                continue;
            }
            i += 1;
            let Some((cp, next)) = Self::scan_unicode_escape_codepoint(bytes, i, end) else {
                return false;
            };
            i = next;
            if (0xD800..=0xDBFF).contains(&cp) {
                if pending_high {
                    return false;
                }
                pending_high = true;
            } else if (0xDC00..=0xDFFF).contains(&cp) {
                if !pending_high {
                    return false;
                }
                pending_high = false;
            } else if pending_high {
                return false;
            }
        }
        !pending_high
    }

    fn scan_unicode_escape_codepoint(
        bytes: &[u8],
        mut i: usize,
        end: usize,
    ) -> Option<(u32, usize)> {
        if i < end && bytes[i] == b'{' {
            i += 1;
            let mut val = 0_u32;
            let mut count = 0;
            while i < end && bytes[i] != b'}' {
                val = val
                    .checked_mul(16)?
                    .checked_add(Self::hex_digit_value(bytes[i])?)?;
                if val > 0x10FFFF {
                    return None;
                }
                i += 1;
                count += 1;
                if count > 6 {
                    return None;
                }
            }
            if count == 0 || i >= end || bytes[i] != b'}' {
                return None;
            }
            Some((val, i + 1))
        } else {
            let mut val = 0_u32;
            for _ in 0..4 {
                if i >= end {
                    return None;
                }
                val = val
                    .checked_mul(16)?
                    .checked_add(Self::hex_digit_value(bytes[i])?)?;
                i += 1;
            }
            Some((val, i))
        }
    }

    fn hex_digit_value(b: u8) -> Option<u32> {
        match b {
            b'0'..=b'9' => Some((b - b'0') as u32),
            b'a'..=b'f' => Some((b - b'a' + 10) as u32),
            b'A'..=b'F' => Some((b - b'A' + 10) as u32),
            _ => None,
        }
    }

    fn parse_binding_identifier(&mut self) -> Result<BindingIdentifier, ParseError> {
        let tok = self.lookahead.clone();
        if let TokenKind::Ident(name) = &tok.kind {

            if is_unconditional_reserved_word(name) {
                return Err(ParseError {
                    span: tok.span,
                    message: format!(
                        "`{}` is a reserved word and cannot be used as a binding identifier",
                        name
                    ),
                });
            }

            if self.strict_mode && (name == "eval" || name == "arguments") {
                return Err(ParseError {
                    span: tok.span,
                    message: format!(
                        "Binding identifier '{}' is not allowed in strict mode",
                        name
                    ),
                });
            }

            if self.strict_mode && is_strict_reserved_word(name) {
                return Err(ParseError {
                    span: tok.span,
                    message: format!(
                        "`{}` is a reserved word in strict mode and cannot be used as a binding identifier",
                        name
                    ),
                });
            }
            if (self.in_generator || self.strict_mode) && name == "yield" {
                return Err(ParseError {
                    span: tok.span,
                    message: "`yield` is not a valid binding in this context".into(),
                });
            }

            if (self.in_async || self.is_module_goal()) && name == "await" {
                return Err(ParseError {
                    span: tok.span,
                    message: "`await` is not a valid binding in async or module code".into(),
                });
            }
            self.bump_regexp()?;
            Ok(BindingIdentifier {
                name: name.clone(),
                span: tok.span,
            })
        } else {
            Err(self.err_here("expected identifier".into()))
        }
    }

    fn parse_optional_attributes(&mut self) -> Result<Vec<ImportAttribute>, ParseError> {

        if self.lookahead.preceded_by_line_terminator {
            return Ok(vec![]);
        }
        if !(self.is_ident("with") || self.is_ident("assert")) {
            return Ok(vec![]);
        }
        self.bump_regexp()?;
        self.expect_punct(Punct::LBrace)?;
        let mut out = Vec::new();

        let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        while !self.is_punct(Punct::RBrace) {
            let start = self.lookahead.span.start;
            let key = self.parse_module_export_name()?;

            let key_str = match &key {
                ModuleExportName::Ident(b) => b.name.clone(),
                ModuleExportName::String { value, .. } => value.clone(),
            };
            if !seen_keys.insert(key_str.clone()) {
                return Err(self.err_here(format!("duplicate import attribute key `{key_str}`")));
            }
            self.expect_punct(Punct::Colon)?;
            let value = match &self.lookahead.kind {
                TokenKind::String(s) => {
                    let s = s.clone();
                    self.bump_regexp()?;
                    s
                }
                _ => return Err(self.err_here("expected string literal in attribute value".into())),
            };
            out.push(ImportAttribute {
                span: Span::new(start, self.last_span_end()),
                key,
                value,
            });
            if self.is_punct(Punct::Comma) {
                self.bump_regexp()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RBrace)?;
        Ok(out)
    }

    fn skip_statement_or_decl(&mut self) -> Result<Span, ParseError> {
        let start = self.lookahead.span.start;
        let mut depth_brace = 0i32;
        let mut depth_paren = 0i32;
        let mut depth_bracket = 0i32;
        loop {
            if self.at_eof() {
                break;
            }
            match self.lookahead.kind {
                TokenKind::Punct(Punct::LBrace) => depth_brace += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    if depth_brace == 0 {
                        break;
                    }
                    depth_brace -= 1;
                }
                TokenKind::Punct(Punct::LParen) => depth_paren += 1,
                TokenKind::Punct(Punct::RParen) => depth_paren -= 1,
                TokenKind::Punct(Punct::LBracket) => depth_bracket += 1,
                TokenKind::Punct(Punct::RBracket) => depth_bracket -= 1,
                TokenKind::Punct(Punct::Semicolon) => {
                    if depth_brace == 0 && depth_paren == 0 && depth_bracket == 0 {
                        let end = self.lookahead.span.end;
                        self.bump_regexp()?;
                        return Ok(Span::new(start, end));
                    }
                }
                _ => {}
            }

            if depth_brace == 0
                && depth_paren == 0
                && depth_bracket == 0
                && self.lookahead.preceded_by_line_terminator
                && (self.is_ident("import") || self.is_ident("export"))
            {
                break;
            }
            self.bump_regexp()?;
        }
        Ok(Span::new(start, self.last_span_end()))
    }

    fn parse_declaration_for_export(
        &mut self,
    ) -> Result<(Span, Vec<BindingIdentifier>, Option<Box<Stmt>>), ParseError> {
        let start = self.lookahead.span.start;
        let mut names: Vec<BindingIdentifier> = Vec::new();
        let is_func = self.is_ident("function") || self.is_ident("async");
        let is_class = self.is_ident("class");
        let is_let = self.is_ident("let");
        let is_const = self.is_ident("const");
        let is_var = self.is_ident("var");
        let stmt_opt: Option<Box<Stmt>> = if is_func {
            let is_async_kw = self.is_ident("async");
            let async_start = if is_async_kw {
                Some(self.lookahead.span.start)
            } else {
                None
            };
            if is_async_kw {
                self.bump_regexp()?;
            }
            let stmt = self.parse_function_decl_stmt(is_async_kw, async_start)?;
            if let Stmt::FunctionDecl { name: Some(bi), .. } = &stmt {
                names.push(bi.clone());
            }
            Some(Box::new(stmt))
        } else if is_class {
            let stmt = self.parse_class_decl_stmt()?;
            if let Stmt::ClassDecl { name: Some(bi), .. } = &stmt {
                names.push(bi.clone());
            }
            Some(Box::new(stmt))
        } else if is_let || is_const || is_var {

            let vs = self.parse_variable_statement()?;
            for d in &vs.declarators {
                for id in d.target.collect_names() {
                    names.push(id.clone());
                }
            }
            Some(Box::new(Stmt::Variable(vs)))
        } else {
            return Ok((self.skip_statement_or_decl()?, names, None));
        };
        Ok((Span::new(start, self.last_span_end()), names, stmt_opt))
    }

    fn extract_destructure_names_object(
        &mut self,
        out: &mut Vec<BindingIdentifier>,
    ) -> Result<(), ParseError> {
        let mut depth = 1i32;
        while depth > 0 && !self.at_eof() {
            match &self.lookahead.kind {
                TokenKind::Punct(Punct::LBrace) => {
                    depth += 1;
                    self.bump_regexp()?;
                }
                TokenKind::Punct(Punct::RBrace) => {
                    depth -= 1;
                    self.bump_regexp()?;
                }
                TokenKind::Ident(n) => {
                    if depth == 1 {

                        let name = n.clone();
                        let span = self.lookahead.span;
                        self.bump_regexp()?;
                        if self.is_punct(Punct::Colon) {
                            self.bump_regexp()?;

                            if let TokenKind::Ident(nn) = &self.lookahead.kind {
                                out.push(BindingIdentifier {
                                    name: nn.clone(),
                                    span: self.lookahead.span,
                                });
                                self.bump_regexp()?;
                            }
                        } else {
                            out.push(BindingIdentifier { name, span });
                        }
                    } else {
                        self.bump_regexp()?;
                    }
                }
                _ => {
                    self.bump_regexp()?;
                }
            }
        }
        Ok(())
    }

    fn extract_destructure_names_array(
        &mut self,
        out: &mut Vec<BindingIdentifier>,
    ) -> Result<(), ParseError> {
        let mut depth = 1i32;
        while depth > 0 && !self.at_eof() {
            match &self.lookahead.kind {
                TokenKind::Punct(Punct::LBracket) => {
                    depth += 1;
                    self.bump_regexp()?;
                }
                TokenKind::Punct(Punct::RBracket) => {
                    depth -= 1;
                    self.bump_regexp()?;
                }
                TokenKind::Ident(n) => {
                    if depth == 1 {
                        out.push(BindingIdentifier {
                            name: n.clone(),
                            span: self.lookahead.span,
                        });
                    }
                    self.bump_regexp()?;
                }
                _ => {
                    self.bump_regexp()?;
                }
            }
        }
        Ok(())
    }

    fn skip_balanced(&mut self, open: Punct, close: Punct) -> Result<(), ParseError> {
        if !self.is_punct(open) {
            return Err(self.err_here(format!("expected `{:?}`", open)));
        }
        self.bump_regexp()?;
        let mut depth = 1i32;
        while depth > 0 {
            if self.at_eof() {
                return Err(self.err_here(format!("unterminated `{:?}`", open)));
            }
            match self.lookahead.kind {
                TokenKind::Punct(p) if p == open => depth += 1,
                TokenKind::Punct(p) if p == close => depth -= 1,
                _ => {}
            }
            self.bump_regexp()?;
        }
        Ok(())
    }

    fn collect_import_entries(&self, decl: &ImportDeclaration, out: &mut Vec<ImportEntry>) {
        let mr = decl.specifier.value.clone();
        if let Some(b) = &decl.default_binding {
            out.push(ImportEntry {
                module_request: mr.clone(),
                import_name: if decl.source_phase {
                    ImportName::Source
                } else {
                    ImportName::Default
                },
                local_name: b.name.clone(),
                import_defer: decl.import_defer,
            });
        }
        if let Some(b) = &decl.namespace_binding {
            out.push(ImportEntry {
                module_request: mr.clone(),
                import_name: ImportName::Namespace,
                local_name: b.name.clone(),
                import_defer: decl.import_defer,
            });
        }
        for spec in &decl.named_imports {
            let imported = match &spec.imported {
                ModuleExportName::Ident(b) => b.name.clone(),
                ModuleExportName::String { value, .. } => value.clone(),
            };
            out.push(ImportEntry {
                module_request: mr.clone(),
                import_name: ImportName::Single(imported),
                local_name: spec.local.name.clone(),
                import_defer: decl.import_defer,
            });
        }
    }

    fn collect_export_entries(
        &self,
        decl: &ExportDeclaration,
        local: &mut Vec<ExportEntry>,
        indirect: &mut Vec<ExportEntry>,
        star: &mut Vec<ExportEntry>,
    ) {
        match decl {
            ExportDeclaration::Declaration { names, .. } => {
                for n in names {
                    local.push(ExportEntry {
                        export_name: Some(n.name.clone()),
                        module_request: None,
                        import_name: None,
                        local_name: Some(n.name.clone()),
                    });
                }
            }
            ExportDeclaration::Named {
                specifiers, source, ..
            } => {
                let mr = source.as_ref().map(|s| s.value.clone());
                for spec in specifiers {
                    let exported_name = match &spec.exported {
                        ModuleExportName::Ident(b) => b.name.clone(),
                        ModuleExportName::String { value, .. } => value.clone(),
                    };
                    let local_name = match &spec.local {
                        ModuleExportName::Ident(b) => Some(b.name.clone()),
                        ModuleExportName::String { value, .. } => Some(value.clone()),
                    };
                    let entry = ExportEntry {
                        export_name: Some(exported_name),
                        module_request: mr.clone(),
                        import_name: if mr.is_some() {
                            local_name.clone().map(ExportImportName::Single)
                        } else {
                            None
                        },
                        local_name: if mr.is_none() { local_name } else { None },
                    };
                    if mr.is_some() {
                        indirect.push(entry);
                    } else {
                        local.push(entry);
                    }
                }
            }
            ExportDeclaration::StarFrom { source, .. } => {
                star.push(ExportEntry {
                    export_name: None,
                    module_request: Some(source.value.clone()),
                    import_name: Some(ExportImportName::All),
                    local_name: None,
                });
            }
            ExportDeclaration::StarAsFrom {
                exported, source, ..
            } => {
                let name = match exported {
                    ModuleExportName::Ident(b) => b.name.clone(),
                    ModuleExportName::String { value, .. } => value.clone(),
                };
                indirect.push(ExportEntry {
                    export_name: Some(name),
                    module_request: Some(source.value.clone()),
                    import_name: Some(ExportImportName::All),
                    local_name: None,
                });
            }
            ExportDeclaration::Default { body, .. } => {
                let local_name = match body {
                    DefaultExportBody::HoistableFunction { name, .. } => {
                        name.as_ref().map(|b| b.name.clone())
                    }
                    DefaultExportBody::Class { name, .. } => name.as_ref().map(|b| b.name.clone()),
                    DefaultExportBody::Expression { .. } => None,
                };
                local.push(ExportEntry {
                    export_name: Some("default".into()),
                    module_request: None,
                    import_name: None,
                    local_name: local_name.or_else(|| Some("*default*".into())),
                });
            }
        }
    }

    fn bump_regexp(&mut self) -> Result<Token, ParseError> {

        let next = {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::BumpFetch);
            self.lx
                .next_token(self.current_lex_goal)
                .map_err(lex_to_parse)?
        };
        let cur = std::mem::replace(&mut self.lookahead, next);
        {
            let _profile = parse_profile::Guard::new(parse_profile::Kind::BumpGoal);
            self.lookahead_after_dot = matches!(
                cur.kind,
                TokenKind::Punct(Punct::Dot) | TokenKind::Punct(Punct::OptionalChain)
            );
            self.current_lex_goal = self.derive_lex_goal_after_current();
        }
        Ok(cur)
    }

    fn at_eof(&self) -> bool {
        matches!(self.lookahead.kind, TokenKind::Eof)
    }

    pub(crate) fn last_span_end(&self) -> usize {

        self.lookahead.span.start
    }

    pub(crate) fn is_punct(&self, p: Punct) -> bool {
        matches!(self.lookahead.kind, TokenKind::Punct(q) if q == p)
    }

    pub(crate) fn is_ident(&self, name: &str) -> bool {
        matches!(&self.lookahead.kind, TokenKind::Ident(n) if n == name)
    }

    pub(crate) fn bytes_at_identifier_keyword(bytes: &[u8], start: usize, name: &str) -> bool {
        let name_bytes = name.as_bytes();
        let end = start + name_bytes.len();
        if end > bytes.len() || &bytes[start..end] != name_bytes {
            return false;
        }
        !bytes
            .get(end)
            .copied()
            .map(|b| b == b'_' || b == b'$' || b.is_ascii_alphanumeric())
            .unwrap_or(false)
    }

    pub(crate) fn next_token_text_is(&self, name: &str) -> bool {
        let bytes = self.src.as_bytes();
        let mut i = self.lookahead.span.end;
        while i < bytes.len()
            && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
        {
            i += 1;
        }
        let name_bytes = name.as_bytes();
        let end = i + name_bytes.len();
        if end > bytes.len() || &bytes[i..end] != name_bytes {
            return false;
        }
        !bytes
            .get(end)
            .copied()
            .map(|b| b == b'_' || b == b'$' || b.is_ascii_alphanumeric())
            .unwrap_or(false)
    }

    fn next_token_text_sequence_is(&self, first: &str, second: &str) -> bool {
        let bytes = self.src.as_bytes();
        let mut i = self.lookahead.span.end;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let first_bytes = first.as_bytes();
        let first_end = i + first_bytes.len();
        if first_end > bytes.len() || &bytes[i..first_end] != first_bytes {
            return false;
        }
        if bytes
            .get(first_end)
            .copied()
            .map(|b| b == b'_' || b == b'$' || b.is_ascii_alphanumeric())
            .unwrap_or(false)
        {
            return false;
        }
        i = first_end;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let second_bytes = second.as_bytes();
        let second_end = i + second_bytes.len();
        if second_end > bytes.len() || &bytes[i..second_end] != second_bytes {
            return false;
        }
        !bytes
            .get(second_end)
            .copied()
            .map(|b| b == b'_' || b == b'$' || b.is_ascii_alphanumeric())
            .unwrap_or(false)
    }

    pub(crate) fn peek_use_strict_directive(&self) -> bool {
        fn skip_trivia(bytes: &[u8], mut i: usize) -> (usize, bool) {
            let mut saw_lt = false;
            loop {
                while i < bytes.len() {
                    match bytes[i] {
                        b' ' | b'\t' | 0x0b | 0x0c => i += 1,
                        b'\n' => {
                            saw_lt = true;
                            i += 1;
                        }
                        b'\r' => {
                            saw_lt = true;
                            i += 1;
                            if bytes.get(i) == Some(&b'\n') {
                                i += 1;
                            }
                        }
                        _ => break,
                    }
                }
                if bytes.get(i) == Some(&b'/') && bytes.get(i + 1) == Some(&b'/') {
                    i += 2;
                    while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                        i += 1;
                    }
                    continue;
                }
                if bytes.get(i) == Some(&b'/') && bytes.get(i + 1) == Some(&b'*') {
                    i += 2;
                    while i + 1 < bytes.len() {
                        if bytes[i] == b'\n' || bytes[i] == b'\r' {
                            saw_lt = true;
                        }
                        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                break;
            }
            (i, saw_lt)
        }

        let bytes = self.src.as_bytes();
        let mut i = self.lookahead.span.start;
        if i >= bytes.len() {
            return false;
        }
        loop {
            i = skip_trivia(bytes, i).0;
            let quote = match bytes.get(i).copied() {
                Some(b @ (b'\'' | b'"')) => b,
                _ => return false,
            };
            let literal_start = i + 1;
            i += 1;
            let mut escaped = false;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => {
                        escaped = true;
                        i += 2;
                    }
                    b if b == quote => break,
                    b'\n' | b'\r' => return false,
                    _ => i += 1,
                }
            }
            if i >= bytes.len() {
                return false;
            }
            let literal_end = i;
            i += 1;
            if !escaped && &bytes[literal_start..literal_end] == b"use strict" {
                return true;
            }
            let (j, saw_lt) = skip_trivia(bytes, i);
            match bytes.get(j).copied() {
                Some(b';') => i = j + 1,
                Some(b'}') | None if saw_lt => i = j,
                Some(_) if saw_lt => i = j,
                None => return false,
                _ => return false,
            }
        }
    }

    pub(crate) fn is_contextual_keyword(&self, name: &str) -> bool {
        if !matches!(&self.lookahead.kind, TokenKind::Ident(n) if n == name) {
            return false;
        }
        let sp = self.lookahead.span;
        let raw = &self.source()[sp.start..sp.end];
        raw == name
    }

    pub(crate) fn expect_punct(&mut self, p: Punct) -> Result<(), ParseError> {
        if self.is_punct(p) {
            self.bump_regexp()?;
            Ok(())
        } else {
            Err(self.err_here(format!("expected `{:?}`", p)))
        }
    }

    pub(crate) fn expect_keyword(&mut self, kw: &str) -> Result<(), ParseError> {
        if self.is_ident(kw) {
            self.bump_regexp()?;
            Ok(())
        } else {
            Err(self.err_here(format!("expected `{}`", kw)))
        }
    }

    pub(crate) fn expect_ident(&mut self, name: &str) -> Result<(), ParseError> {
        if self.is_ident(name) {
            self.bump_regexp()?;
            Ok(())
        } else {
            Err(self.err_here(format!("expected `{}`", name)))
        }
    }

    fn consume_semicolon(&mut self) -> Result<(), ParseError> {

        if self.is_punct(Punct::Semicolon) {
            self.bump_regexp()?;
            return Ok(());
        }
        if self.at_eof()
            || self.is_punct(Punct::RBrace)
            || self.lookahead.preceded_by_line_terminator
        {
            return Ok(());
        }
        Err(self.err_here("expected semicolon or line terminator".into()))
    }

    pub(crate) fn consume_statement_rbrace(&mut self) -> Result<(), ParseError> {
        if !self.is_punct(Punct::RBrace) {
            return Err(self.err_here("expected `}`".into()));
        }
        let _cur = std::mem::replace(
            &mut self.lookahead,
            self.lx
                .next_token(LexerGoal::RegExp)
                .map_err(lex_to_parse)?,
        );
        self.current_lex_goal = self.derive_lex_goal_after_current();
        Ok(())
    }

    pub(crate) fn consume_cond_rparen(&mut self) -> Result<(), ParseError> {
        if !self.is_punct(Punct::RParen) {
            return Err(self.err_here("expected `)`".into()));
        }
        let _cur = std::mem::replace(
            &mut self.lookahead,
            self.lx
                .next_token(LexerGoal::RegExp)
                .map_err(lex_to_parse)?,
        );
        self.current_lex_goal = self.derive_lex_goal_after_current();
        Ok(())
    }

    pub(crate) fn current_kind(&self) -> &TokenKind {
        &self.lookahead.kind
    }
    pub(crate) fn lookahead_span(&self) -> Span {
        self.lookahead.span
    }
    pub(crate) fn lookahead_preceded_by_lt(&self) -> bool {
        self.lookahead.preceded_by_line_terminator
    }

    pub(crate) fn set_lexer_strict(&mut self, strict: bool) {
        self.lx.set_strict(strict);
    }

    pub(crate) fn last_string_had_legacy_escape(&self) -> bool {
        self.lx.last_string_had_legacy_escape
    }
    pub(crate) fn source(&self) -> &str {
        self.src
    }

    pub(crate) fn ident_source_has_escape(&self, span: Span) -> bool {
        self.src.as_bytes()[span.start..span.end]
            .iter()
            .any(|&b| b == b'\\')
    }

    pub(crate) fn reject_escaped_contextual_keyword(&self, kw: &str) -> Result<(), ParseError> {
        if self.ident_source_has_escape(self.lookahead_span()) {
            return Err(ParseError {
                span: self.lookahead_span(),
                message: format!(
                    "the `{}` contextual keyword must not contain a unicode escape",
                    kw
                ),
            });
        }
        Ok(())
    }
    pub(crate) fn at_eof_internal(&self) -> bool {
        self.at_eof()
    }
    pub(crate) fn bump(&mut self) -> Result<Token, ParseError> {
        self.bump_regexp()
    }
    pub(crate) fn skip_balanced_public(
        &mut self,
        open: Punct,
        close: Punct,
    ) -> Result<(), ParseError> {
        self.skip_balanced(open, close)
    }

    pub(crate) fn enter_template_tail(&mut self) -> Result<(), ParseError> {
        let pos = self.lookahead.span.start;
        self.lx.set_pos(pos);
        self.lookahead = self
            .lx
            .next_token(LexerGoal::TemplateTail)
            .map_err(lex_to_parse)?;
        self.current_lex_goal = self.derive_lex_goal_after_current();
        Ok(())
    }

    fn derive_lex_goal_after_current(&self) -> LexerGoal {

        if self.lookahead_after_dot && matches!(self.lookahead.kind, TokenKind::Ident(_)) {
            return LexerGoal::Div;
        }
        let await_is_operator =
            self.in_async || (self.function_body_depth == 0 && self.goal_allows_top_level_await());
        derive_lex_goal_after_in_context(
            &self.lookahead.kind,
            self.strict_mode,
            self.in_generator,
            await_is_operator,
        )
    }

    pub(crate) fn consume_semicolon_pub(&mut self) -> Result<(), ParseError> {
        self.consume_semicolon()
    }

    pub(crate) fn parse_object_binding_pattern_body(
        &mut self,
        open_start: usize,
    ) -> Result<rusty_js_ast::ObjectPattern, ParseError> {
        use rusty_js_ast::{
            BindingElement, BindingIdentifier, BindingPattern, ObjectPattern,
            ObjectPatternProperty, PropertyKey,
        };
        let mut properties: Vec<ObjectPatternProperty> = Vec::new();
        let mut rest: Option<Box<BindingIdentifier>> = None;
        loop {
            if matches!(self.current_kind(), TokenKind::Punct(Punct::RBrace)) {
                break;
            }

            if matches!(self.current_kind(), TokenKind::Punct(Punct::Spread)) {
                self.bump()?;
                let n_span = self.lookahead_span();
                if let TokenKind::Ident(n) = self.current_kind().clone() {
                    self.bump()?;
                    rest = Some(Box::new(BindingIdentifier {
                        name: n,
                        span: n_span,
                    }));
                } else {
                    return Err(self.err_here("object rest must be a plain identifier".into()));
                }

                break;
            }

            let prop_start = self.lookahead_span().start;
            let (key, shorthand_ident): (PropertyKey, Option<BindingIdentifier>) =
                match self.current_kind().clone() {
                    TokenKind::Ident(name) => {
                        let span = self.lookahead_span();
                        self.bump()?;
                        let id = BindingIdentifier {
                            name: name.clone(),
                            span,
                        };
                        (PropertyKey::Identifier(id.clone()), Some(id))
                    }
                    TokenKind::String(value) => {
                        self.bump()?;
                        (PropertyKey::String(std::rc::Rc::new(value)), None)
                    }
                    TokenKind::Number(value, _) => {
                        self.bump()?;
                        (PropertyKey::Number(value), None)
                    }
                    TokenKind::BigInt(digits, kind) => {
                        self.bump()?;
                        (
                            PropertyKey::String(std::rc::Rc::new(
                                Self::bigint_literal_property_name(&digits, kind),
                            )),
                            None,
                        )
                    }
                    TokenKind::Punct(Punct::LBracket) => {
                        self.bump()?;
                        let expr = self.parse_assignment_expression()?;
                        self.expect_punct(Punct::RBracket)?;
                        (PropertyKey::Computed(expr), None)
                    }
                    _ => {
                        return Err(self
                            .err_here("expected property name in object binding pattern".into()))
                    }
                };
            let (value, shorthand) =
                if matches!(self.current_kind(), TokenKind::Punct(Punct::Colon)) {
                    self.bump()?;
                    let elem_start = self.lookahead_span().start;
                    let target = self.parse_binding_target()?;
                    let default = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign))
                    {
                        self.bump()?;
                        Some(self.parse_assignment_expression()?)
                    } else {
                        None
                    };
                    let elem_end = self.last_span_end();
                    (
                        BindingElement {
                            target,
                            default,
                            span: Span::new(elem_start, elem_end),
                        },
                        false,
                    )
                } else {

                    let id = shorthand_ident.ok_or_else(|| {
                        self.err_here("non-identifier key requires `: value`".into())
                    })?;

                    if is_unconditional_reserved_word(&id.name)
                        || (self.strict_mode && is_strict_reserved_word(&id.name))
                    {
                        return Err(self.err_at(
                            id.span,
                            format!(
                            "`{}` is a reserved word and cannot be used as a binding identifier",
                            id.name
                        ),
                        ));
                    }
                    let elem_start = id.span.start;
                    let target = BindingPattern::Identifier(id);
                    let default = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign))
                    {
                        self.bump()?;
                        Some(self.parse_assignment_expression()?)
                    } else {
                        None
                    };
                    let elem_end = self.last_span_end();
                    (
                        BindingElement {
                            target,
                            default,
                            span: Span::new(elem_start, elem_end),
                        },
                        true,
                    )
                };
            let prop_end = self.last_span_end();
            properties.push(ObjectPatternProperty {
                key,
                value,
                shorthand,
                span: Span::new(prop_start, prop_end),
            });
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                self.bump()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RBrace)?;
        let end = self.last_span_end();
        Ok(ObjectPattern {
            properties,
            rest,
            span: Span::new(open_start, end),
        })
    }

    pub(crate) fn parse_array_binding_pattern_body(
        &mut self,
        open_start: usize,
    ) -> Result<rusty_js_ast::ArrayPattern, ParseError> {
        use rusty_js_ast::{ArrayPattern, BindingElement, BindingPattern};
        let mut elements: Vec<Option<BindingElement>> = Vec::new();
        let mut rest: Option<Box<BindingPattern>> = None;
        loop {
            if matches!(self.current_kind(), TokenKind::Punct(Punct::RBracket)) {
                break;
            }

            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                elements.push(None);
                self.bump()?;
                continue;
            }

            if matches!(self.current_kind(), TokenKind::Punct(Punct::Spread)) {
                self.bump()?;
                let target = self.parse_binding_target()?;
                rest = Some(Box::new(target));
                break;
            }
            let elem_start = self.lookahead_span().start;
            let target = self.parse_binding_target()?;
            let default = if matches!(self.current_kind(), TokenKind::Punct(Punct::Assign)) {
                self.bump()?;
                Some(self.parse_assignment_expression()?)
            } else {
                None
            };
            let elem_end = self.last_span_end();
            elements.push(Some(BindingElement {
                target,
                default,
                span: Span::new(elem_start, elem_end),
            }));
            if matches!(self.current_kind(), TokenKind::Punct(Punct::Comma)) {
                self.bump()?;
            } else {
                break;
            }
        }
        self.expect_punct(Punct::RBracket)?;
        let end = self.last_span_end();
        Ok(ArrayPattern {
            elements,
            rest,
            span: Span::new(open_start, end),
        })
    }

    pub(crate) fn parse_binding_target(
        &mut self,
    ) -> Result<rusty_js_ast::BindingPattern, ParseError> {
        use rusty_js_ast::{BindingIdentifier, BindingPattern};
        match self.current_kind().clone() {
            TokenKind::Ident(n) => {
                let span = self.lookahead_span();

                if is_unconditional_reserved_word(&n) {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "`{}` is a reserved word and cannot be used as a binding identifier",
                            n
                        ),
                    });
                }

                if self.strict_mode && (n == "eval" || n == "arguments") {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "Binding identifier '{}' is not allowed in strict mode",
                            n
                        ),
                    });
                }

                if self.strict_mode && is_strict_reserved_word(&n) {
                    return Err(ParseError {
                        span,
                        message: format!(
                            "`{}` is a reserved word in strict mode and cannot be used as a binding identifier",
                            n
                        ),
                    });
                }
                if (self.in_generator || self.strict_mode) && n == "yield" {
                    return Err(ParseError {
                        span,
                        message: "`yield` is not a valid binding in this context".into(),
                    });
                }
                if (self.in_async || self.is_module_goal()) && n == "await" {
                    return Err(ParseError {
                        span,
                        message: "`await` is not a valid binding in async or module code".into(),
                    });
                }
                self.bump()?;
                Ok(BindingPattern::Identifier(BindingIdentifier {
                    name: n,
                    span,
                }))
            }
            TokenKind::Punct(Punct::LBrace) => {
                let open_start = self.lookahead_span().start;
                self.bump()?;
                Ok(BindingPattern::Object(
                    self.parse_object_binding_pattern_body(open_start)?,
                ))
            }
            TokenKind::Punct(Punct::LBracket) => {
                let open_start = self.lookahead_span().start;
                self.bump()?;
                Ok(BindingPattern::Array(
                    self.parse_array_binding_pattern_body(open_start)?,
                ))
            }
            _ => Err(self.err_here("expected binding identifier or pattern".into())),
        }
    }

    pub(crate) fn err_here(&self, message: String) -> ParseError {
        ParseError {
            span: self.lookahead.span,
            message,
        }
    }

    pub(crate) fn enter_parse_depth(&mut self) -> Result<(), ParseError> {
        if self.parse_depth >= Self::MAX_PARSE_DEPTH {
            return Err(self.err_here("parser nesting depth exceeded".into()));
        }
        self.parse_depth += 1;
        Ok(())
    }

    pub(crate) fn leave_parse_depth(&mut self) {
        self.parse_depth = self.parse_depth.saturating_sub(1);
    }

    pub(crate) fn err_at(&self, span: Span, message: String) -> ParseError {
        ParseError { span, message }
    }
}

pub(crate) fn is_reserved_word(name: &str) -> bool {
    matches!(
        name,

        "await" | "break" | "case" | "catch" | "class" | "const" | "continue"
        | "debugger" | "default" | "delete" | "do" | "else" | "enum" | "export"
        | "extends" | "false" | "finally" | "for" | "function" | "if" | "import"
        | "in" | "instanceof" | "new" | "null" | "return" | "super" | "switch"
        | "this" | "throw" | "true" | "try" | "typeof" | "var" | "void" | "while"
        | "with" | "yield"

        | "implements" | "interface" | "let" | "package" | "private"
        | "protected" | "public" | "static"
    )
}

pub(crate) fn is_unconditional_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
    )
}

pub(crate) fn is_strict_reserved_word(name: &str) -> bool {
    matches!(
        name,
        "implements"
            | "interface"
            | "let"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
    )
}

fn derive_lex_goal_after(prev_kind: &TokenKind) -> LexerGoal {
    if token_completes_expression(prev_kind) {
        LexerGoal::Div
    } else {
        LexerGoal::RegExp
    }
}

fn derive_lex_goal_after_in_context(
    prev_kind: &TokenKind,
    strict_mode: bool,
    in_generator: bool,
    await_is_operator: bool,
) -> LexerGoal {
    if matches!(prev_kind, TokenKind::Ident(s) if s == "yield") && !strict_mode && !in_generator {
        return LexerGoal::Div;
    }
    if matches!(prev_kind, TokenKind::Ident(s) if s == "await") && !await_is_operator {
        return LexerGoal::Div;
    }
    derive_lex_goal_after(prev_kind)
}

fn token_completes_expression(t: &TokenKind) -> bool {
    match t {
        TokenKind::Ident(s) => {
            matches!(s.as_str(), "this" | "super" | "null" | "true" | "false")
                || !matches!(
                    s.as_str(),
                    "return"
                        | "throw"
                        | "new"
                        | "delete"
                        | "typeof"
                        | "void"
                        | "await"
                        | "yield"
                        | "if"
                        | "else"
                        | "for"
                        | "while"
                        | "do"
                        | "switch"
                        | "case"
                        | "default"
                        | "break"
                        | "continue"
                        | "try"
                        | "catch"
                        | "finally"
                        | "class"
                        | "function"
                        | "var"
                        | "const"
                        | "in"
                        | "instanceof"
                        | "import"
                        | "export"
                        | "extends"
                        | "with"
                        | "debugger"
                )

        }
        TokenKind::Number(..) | TokenKind::String(..) | TokenKind::BigInt(..) => true,
        TokenKind::Template { part, .. } => {
            matches!(part, TemplatePart::NoSubstitution | TemplatePart::Tail)
        }
        TokenKind::Regex { .. } => true,
        TokenKind::PrivateIdent(_) => true,
        TokenKind::Punct(p) => matches!(
            p,
            Punct::RParen | Punct::RBracket | Punct::RBrace | Punct::Inc | Punct::Dec
        ),
        _ => false,
    }
}

fn lex_to_parse(e: LexError) -> ParseError {
    ParseError {
        span: e.span,
        message: format!("lex error: {} ({:?})", e.message, e.kind),
    }
}
