#![deny(unused_must_use)]

use crate::diagnostics::diagnostic_builder::{DiagnosticDeriveBuilder, DiagnosticDeriveKind};
use crate::diagnostics::error::{span_err, DiagnosticDeriveError};
use crate::diagnostics::utils::SetOnce;
use proc_macro2::TokenStream;
use quote::quote;
use synstructure::Structure;

/// The central struct for constructing the `into_diagnostic` method from an annotated struct.
pub(crate) struct DiagnosticDerive<'a> {
    structure: Structure<'a>,
    handler: syn::Ident,
    builder: DiagnosticDeriveBuilder,
}

impl<'a> DiagnosticDerive<'a> {
    pub(crate) fn new(diag: syn::Ident, handler: syn::Ident, structure: Structure<'a>) -> Self {
        Self {
            builder: DiagnosticDeriveBuilder { diag, kind: DiagnosticDeriveKind::Diagnostic },
            handler,
            structure,
        }
    }

    pub(crate) fn into_tokens(self) -> TokenStream {
        let DiagnosticDerive { mut structure, handler, mut builder } = self;

        let implementation = builder.each_variant(&mut structure, |mut builder, variant| {
            let preamble = builder.preamble(&variant);
            let body = builder.body(&variant);

            let diag = &builder.parent.diag;
            let init = match builder.slug.value_ref() {
                None => {
                    span_err(builder.span, "diagnostic slug not specified")
                        .help(&format!(
                            "specify the slug as the first argument to the `#[diag(...)]` \
                            attribute, such as `#[diag(hir_analysis::example_error)]`",
                        ))
                        .emit();
                    return DiagnosticDeriveError::ErrorHandled.to_compile_error();
                }
                Some(slug) => {
                    quote! {
                        let mut #diag = #handler.struct_diagnostic(rustc_errors::fluent::#slug);
                    }
                }
            };

            quote! {
                #init
                #preamble
                #body
                #diag
            }
        });

        structure.gen_impl(quote! {
            gen impl<'__diagnostic_handler_sess, G>
                    rustc_errors::IntoDiagnostic<'__diagnostic_handler_sess, G>
                    for @Self
                where G: rustc_errors::EmissionGuarantee
            {
                fn into_diagnostic(
                    self,
                    #handler: &'__diagnostic_handler_sess rustc_errors::Handler
                ) -> rustc_errors::DiagnosticBuilder<'__diagnostic_handler_sess, G> {
                    use rustc_errors::IntoDiagnosticArg;
                    #implementation
                }
            }
        })
    }
}

/// The central struct for constructing the `decorate_lint` method from an annotated struct.
pub(crate) struct LintDiagnosticDerive<'a> {
    structure: Structure<'a>,
    builder: DiagnosticDeriveBuilder,
}

impl<'a> LintDiagnosticDerive<'a> {
    pub(crate) fn new(diag: syn::Ident, structure: Structure<'a>) -> Self {
        Self {
            builder: DiagnosticDeriveBuilder { diag, kind: DiagnosticDeriveKind::LintDiagnostic },
            structure,
        }
    }

    pub(crate) fn into_tokens(self) -> TokenStream {
        let LintDiagnosticDerive { mut structure, mut builder } = self;

        let implementation = builder.each_variant(&mut structure, |mut builder, variant| {
            let preamble = builder.preamble(&variant);
            let body = builder.body(&variant);

            let diag = &builder.parent.diag;

            quote! {
                #preamble
                #body
                #diag
            }
        });

        let msg = builder.each_variant(&mut structure, |mut builder, variant| {
            // HACK(wafflelapkin): initialize slug (???)
            let _preamble = builder.preamble(&variant);

            match builder.slug.value_ref() {
                None => {
                    span_err(builder.span, "diagnostic slug not specified")
                        .help(&format!(
                            "specify the slug as the first argument to the attribute, such as \
                            `#[diag(compiletest::example)]`",
                        ))
                        .emit();
                    return DiagnosticDeriveError::ErrorHandled.to_compile_error();
                }
                Some(slug) => quote! { rustc_errors::fluent::#slug.into() },
            }
        });

        let diag = &builder.diag;
        structure.gen_impl(quote! {
            gen impl<'__a> rustc_errors::DecorateLint<'__a, ()> for @Self {
                fn decorate_lint<'__b>(self, #diag: &'__b mut rustc_errors::DiagnosticBuilder<'__a, ()>) -> &'__b mut rustc_errors::DiagnosticBuilder<'__a, ()> {
                    use rustc_errors::IntoDiagnosticArg;
                    #implementation
                }

                fn msg(&self) -> rustc_errors::DiagnosticMessage {
                    #msg
                }
            }
        })
    }
}
