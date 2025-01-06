use bevy_macro_utils::{derive_label, BevyManifest};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use quote::quote;

pub fn derive_node_label_inner(input: TokenStream) -> syn::Result<TokenStream2> {
    let input: syn::DeriveInput = syn::parse(input)?;

    let bevy_ecs = BevyManifest::default().get_path("bevy_ecs");
    let label_path = syn::parse2(quote! { ::bevy_seedling::node_label::NodeLabel }).unwrap();

    let ident = &input.ident;
    let component_derive = quote! {
        impl #bevy_ecs::component::Component for #ident {
            const STORAGE_TYPE: #bevy_ecs::component::StorageType = #bevy_ecs::component::StorageType::Table;

            #[allow(unused_variables)]
            fn register_component_hooks(hooks: &mut #bevy_ecs::component::ComponentHooks) {
                hooks.on_insert(|mut world: #bevy_ecs::world::DeferredWorld, entity: #bevy_ecs::entity::Entity, _| {
                    let value = world.get::<Self>(entity).unwrap();
                    let interned = <Self as #label_path>::intern(value);

                    world
                        .commands()
                        .entity(entity)
                        .entry::<::bevy_seedling::node_label::NodeLabels>()
                        .or_insert(::core::default::Default::default())
                        .and_modify(move |mut labels| {
                            labels.insert(interned);
                        });
                });
            }
        }
    };

    let mut dyn_eq_path = bevy_ecs.clone();
    dyn_eq_path.segments.push(format_ident!("schedule").into());
    dyn_eq_path.segments.push(format_ident!("DynEq").into());

    let label_derive: TokenStream2 =
        derive_label(input, "NodeLabel", &label_path, &dyn_eq_path).into();

    Ok(quote! {
        #component_derive
        #label_derive
    })
}

pub fn derive_pool_label_inner(input: TokenStream) -> syn::Result<TokenStream2> {
    let input: syn::DeriveInput = syn::parse(input)?;

    let bevy_ecs = BevyManifest::default().get_path("bevy_ecs");
    let label_path = syn::parse2(quote! { ::bevy_seedling::sample::label::PoolLabel }).unwrap();

    let ident = &input.ident;
    let component_derive = quote! {
        impl #bevy_ecs::component::Component for #ident {
            const STORAGE_TYPE: #bevy_ecs::component::StorageType = #bevy_ecs::component::StorageType::Table;

            #[allow(unused_variables)]
            fn register_component_hooks(hooks: &mut #bevy_ecs::component::ComponentHooks) {
                hooks.on_insert(|mut world: #bevy_ecs::world::DeferredWorld, entity: #bevy_ecs::entity::Entity, _| {
                    let value = world.get::<Self>(entity).unwrap();
                    let container = ::bevy_seedling::sample::label::PoolLabelContainer::new(value);

                    world
                        .commands()
                        .entity(entity)
                        .insert(container);
                });
            }
        }
    };

    let mut dyn_eq_path = bevy_ecs.clone();
    dyn_eq_path.segments.push(format_ident!("schedule").into());
    dyn_eq_path.segments.push(format_ident!("DynEq").into());

    let label_derive: TokenStream2 =
        derive_label(input, "PoolLabel", &label_path, &dyn_eq_path).into();

    Ok(quote! {
        #component_derive
        #label_derive
    })
}
