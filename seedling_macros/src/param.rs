extern crate proc_macro;

use bevy_macro_utils::fq_std::{FQOption, FQResult};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::spanned::Spanned;

pub fn derive_param_inner(
    input: TokenStream,
    firewheel_path: TokenStream2,
) -> syn::Result<TokenStream2> {
    let input: syn::DeriveInput = syn::parse(input)?;
    let identifier = &input.ident;

    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.span(),
            "`AudioParam` can only be derived on structs",
        ));
    };

    // NOTE: a trivial optimization would be to automatically
    // flatten structs with only a single field so their
    // paths can be one index shorter.
    let fields: Vec<_> = match &data.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| (f.ident.as_ref().unwrap().to_token_stream(), &f.ty))
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let accessor: syn::Index = i.into();
                (accessor.to_token_stream(), &f.ty)
            })
            .collect(),
        syn::Fields::Unit => Vec::new(),
    };

    let messages = fields.iter().enumerate().map(|(i, (identifier, _))| {
        let index = i as u32;
        quote! {
            self.#identifier.diff(&cmp.#identifier, &mut writer, path.with(#index));
        }
    });

    let patches = fields.iter().enumerate().map(|(i, (identifier, _))| {
        let index = i as u32;
        quote! {
            #FQOption::Some(#index) => self.#identifier.patch(data, &path[1..])
        }
    });

    let ticks = fields.iter().map(|(identifier, _)| {
        quote! {
            self.#identifier.tick(time);
        }
    });

    let (impl_generics, ty_generics, where_generics) = input.generics.split_for_impl();

    let mut where_generics = where_generics.cloned().unwrap_or_else(|| syn::WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });

    let param_path = quote! { #firewheel_path::param };

    for (_, ty) in &fields {
        where_generics
            .predicates
            .push(syn::parse2(quote! { #ty: #param_path::AudioParam }).unwrap());
    }

    Ok(quote! {
        impl #impl_generics #param_path::AudioParam for #identifier #ty_generics #where_generics {
            fn diff(&self, cmp: &Self, mut writer: impl FnMut(#param_path::ParamEvent), path: #param_path::ParamPath) {
                #(#messages)*
            }

            fn patch(&mut self, data: &#param_path::ParamData, path: &[u32]) -> #FQResult<(), #param_path::PatchError> {
                match path.first() {
                    #(#patches,)*
                    _ => #FQResult::Err(#param_path::PatchError::InvalidPath),
                }
            }

            fn tick(&mut self, time: #firewheel_path::clock::ClockSeconds) {
                #(#ticks)*
            }
        }
    })
}
