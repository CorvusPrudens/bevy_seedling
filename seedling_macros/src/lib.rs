extern crate proc_macro;

use proc_macro::TokenStream;

mod label;
mod param;

#[proc_macro_derive(NodeLabel)]
pub fn derive_node_label(input: TokenStream) -> TokenStream {
    label::derive_node_label_inner(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(PoolLabel)]
pub fn derive_pool_label(input: TokenStream) -> TokenStream {
    label::derive_pool_label_inner(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(AudioParam)]
pub fn derive_audio_param(input: TokenStream) -> TokenStream {
    param::derive_param_inner(input, quote::quote! { ::bevy_seedling::firewheel })
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
