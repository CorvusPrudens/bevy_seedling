use bevy_macro_utils::{derive_label, BevyManifest};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub fn derive_node_label_inner(input: TokenStream) -> syn::Result<TokenStream2> {
    let input: syn::DeriveInput = syn::parse(input)?;

    let bevy_ecs = BevyManifest::shared(|manifest| manifest.get_path("bevy_ecs"));
    let label_path = syn::parse2(quote! { ::bevy_seedling::prelude::NodeLabel }).unwrap();

    let ident = &input.ident;
    let component_derive = quote! {
        impl #bevy_ecs::component::Component for #ident {
            const STORAGE_TYPE: #bevy_ecs::component::StorageType = #bevy_ecs::component::StorageType::Table;
            type Mutability = #bevy_ecs::component::Immutable;

            #[allow(unused_variables)]
            fn on_insert() -> Option<#bevy_ecs::lifecycle::ComponentHook> {
                Some(::bevy_seedling::node::label::insert_node_label::<Self>)
            }
        }
    };

    let label_derive: TokenStream2 = derive_label(input, "NodeLabel", &label_path).into();

    Ok(quote! {
        #component_derive
        #label_derive
    })
}

pub fn derive_pool_label_inner(input: TokenStream) -> syn::Result<TokenStream2> {
    let input: syn::DeriveInput = syn::parse(input)?;

    let bevy_ecs = BevyManifest::shared(|manifest| manifest.get_path("bevy_ecs"));
    let label_path = syn::parse2(quote! { ::bevy_seedling::prelude::PoolLabel }).unwrap();

    let ident = &input.ident;
    let component_derive = quote! {
        impl #bevy_ecs::component::Component for #ident {
            const STORAGE_TYPE: #bevy_ecs::component::StorageType = #bevy_ecs::component::StorageType::Table;
            type Mutability = #bevy_ecs::component::Immutable;

            fn on_insert() -> Option<#bevy_ecs::lifecycle::ComponentHook> {
                Some(::bevy_seedling::pool::label::insert_pool_label::<Self>)
            }

            fn on_remove() -> Option<#bevy_ecs::lifecycle::ComponentHook> {
                Some(::bevy_seedling::pool::label::remove_pool_label::<Self>)
            }
        }
    };

    let label_derive: TokenStream2 = derive_label(input, "PoolLabel", &label_path).into();

    Ok(quote! {
        #component_derive
        #label_derive
    })
}
