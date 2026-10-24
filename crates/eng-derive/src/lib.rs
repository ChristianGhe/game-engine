//! `#[derive(Schema)]` for plain-data structs and fieldless enums.
//!
//! ```ignore
//! #[derive(Schema)]
//! #[schema(name = "eng.core.PlayerCommand", version = 1)]
//! pub struct PlayerCommand { /* ... */ }
//! ```
//!
//! The stable name is always written by hand: Rust paths change with refactors,
//! stable names must not. Fields are encoded in declaration order; enum
//! variants by declaration index.
#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitInt, LitStr, parse_macro_input};

#[proc_macro_derive(Schema, attributes(schema))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

struct Attrs {
    name: LitStr,
    version: u32,
}

fn parse_attrs(input: &DeriveInput) -> syn::Result<Attrs> {
    let mut name: Option<LitStr> = None;
    let mut version: Option<u32> = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("schema") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                name = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            } else if meta.path.is_ident("version") {
                version = Some(meta.value()?.parse::<LitInt>()?.base10_parse::<u32>()?);
                Ok(())
            } else {
                Err(meta.error("expected `name = \"...\"` or `version = N`"))
            }
        })?;
    }
    let missing = |what: &str| {
        syn::Error::new_spanned(
            &input.ident,
            format!(
                "Schema derive needs #[schema({what})], e.g. #[schema(name = \"eng.core.Tick\", version = 1)]"
            ),
        )
    };
    let name = name.ok_or_else(|| missing("name = \"...\""))?;
    let version = version.ok_or_else(|| missing("version = N"))?;
    let value = name.value();
    let valid_chars = value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
    if !valid_chars || !value.contains('.') || value.starts_with('.') || value.ends_with('.') {
        return Err(syn::Error::new_spanned(
            &name,
            "schema name must be dotted ASCII, e.g. \"eng.core.Tick\"",
        ));
    }
    Ok(Attrs { name, version })
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "Schema derive does not support generic types",
        ));
    }
    let attrs = parse_attrs(input)?;
    let ident = &input.ident;
    let name = &attrs.name;
    let version = attrs.version;
    let body = match &input.data {
        Data::Struct(data) => expand_struct(name, version, &data.fields),
        Data::Enum(data) => expand_enum(name, version, data)?,
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "Schema derive does not support unions",
            ));
        }
    };
    Ok(quote! {
        impl ::eng_schema_core::Schema for #ident {
            const NAME: &'static str = #name;
            const VERSION: u32 = #version;
            #body
        }
    })
}

fn expand_struct(name: &LitStr, version: u32, fields: &Fields) -> TokenStream2 {
    let mut descs = Vec::new();
    let mut visits = Vec::new();
    let mut decodes = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let ty = &field.ty;
        let (label, member) = match &field.ident {
            Some(ident) => (ident.to_string(), quote!(#ident)),
            None => {
                let idx = syn::Index::from(index);
                (index.to_string(), quote!(#idx))
            }
        };
        descs.push(quote! {
            (#label, <#ty as ::eng_schema_core::Schema>::describe())
        });
        visits.push(quote! {
            v.field(#label);
            ::eng_schema_core::Schema::visit(&self.#member, v);
        });
        decodes.push(quote! {
            #member: <#ty as ::eng_schema_core::Schema>::decode(r)?
        });
    }
    let construct = match fields {
        Fields::Unit => quote!(Self),
        _ => quote!(Self { #(#decodes),* }),
    };
    quote! {
        fn describe() -> ::eng_schema_core::TypeDesc {
            ::eng_schema_core::TypeDesc::Struct {
                name: #name,
                version: #version,
                fields: ::eng_schema_core::__private::vec![#(#descs),*],
            }
        }

        fn visit(&self, v: &mut dyn ::eng_schema_core::Visitor) {
            v.begin_struct(#name);
            #(#visits)*
            v.end_struct();
        }

        fn decode(
            r: &mut ::eng_schema_core::Reader<'_>,
        ) -> ::core::result::Result<Self, ::eng_schema_core::DecodeError> {
            ::core::result::Result::Ok(#construct)
        }
    }
}

fn expand_enum(name: &LitStr, version: u32, data: &syn::DataEnum) -> syn::Result<TokenStream2> {
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "Schema derive needs at least one enum variant",
        ));
    }
    let mut labels = Vec::new();
    let mut visit_arms = Vec::new();
    let mut decode_arms = Vec::new();
    for (index, variant) in data.variants.iter().enumerate() {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "Schema derive supports only fieldless enum variants",
            ));
        }
        if variant.discriminant.is_some() {
            return Err(syn::Error::new_spanned(
                variant,
                "Schema encodes variants by declaration order; remove the explicit discriminant",
            ));
        }
        let ident = &variant.ident;
        let label = ident.to_string();
        let index = u32::try_from(index).expect("fewer than 2^32 variants");
        labels.push(label.clone());
        visit_arms.push(quote!(Self::#ident => (#index, #label)));
        decode_arms.push(quote!(#index => ::core::result::Result::Ok(Self::#ident)));
    }
    Ok(quote! {
        fn describe() -> ::eng_schema_core::TypeDesc {
            ::eng_schema_core::TypeDesc::Enum {
                name: #name,
                version: #version,
                variants: ::eng_schema_core::__private::vec![#(#labels),*],
            }
        }

        fn visit(&self, v: &mut dyn ::eng_schema_core::Visitor) {
            let (index, label): (u32, &'static str) = match self {
                #(#visit_arms),*
            };
            v.variant(#name, index, label);
        }

        fn decode(
            r: &mut ::eng_schema_core::Reader<'_>,
        ) -> ::core::result::Result<Self, ::eng_schema_core::DecodeError> {
            match r.read_u32()? {
                #(#decode_arms,)*
                other => ::core::result::Result::Err(
                    ::eng_schema_core::DecodeError::InvalidVariant { ty: #name, index: other },
                ),
            }
        }
    })
}
